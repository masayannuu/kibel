#!/usr/bin/env bash
set -euo pipefail

QUERY="${1:-}"
FIRST="16"
PROFILE="${KIBEL_SEARCH_PROFILE:-balanced}"

if [[ -z "${QUERY}" ]]; then
  echo "usage: $0 <query> [first] [candidate_query ...]" >&2
  exit 2
fi

shift
if [[ "${1:-}" =~ ^[0-9]+$ ]]; then
  FIRST="${1}"
  shift
fi

KBIN="${KIBEL_BIN:-kibel}"
if [[ "${KBIN}" == */* ]]; then
  if [[ ! -x "${KBIN}" ]]; then
    echo "kibel binary not executable: ${KBIN}" >&2
    exit 127
  fi
elif ! command -v "${KBIN}" >/dev/null 2>&1; then
  echo "kibel binary not found in PATH (or set KIBEL_BIN)" >&2
  exit 127
fi

if ! command -v python3 >/dev/null 2>&1; then
  echo "python3 is required for this template" >&2
  exit 127
fi

python3 - "${KBIN}" "${PROFILE}" "${FIRST}" "${QUERY}" "$@" <<'PY'
import json
import subprocess
import sys

kbin = sys.argv[1]
profile = sys.argv[2]
first = int(sys.argv[3])
query = sys.argv[4]
extra_candidates = sys.argv[5:]

profiles = {
    "fast": {"max_candidates": 2, "max_cli_calls": 8},
    "balanced": {"max_candidates": 4, "max_cli_calls": 16},
    "deep": {"max_candidates": 7, "max_cli_calls": 28},
}
if profile not in profiles:
    profile = "balanced"
limits = profiles[profile]

candidates = [query, *extra_candidates]
if len(candidates) > limits["max_candidates"]:
    print(
        f"candidate budget exceeded for profile={profile}: {len(candidates)} > {limits['max_candidates']}",
        file=sys.stderr,
    )
    raise SystemExit(4)


def run_kibel(args):
    proc = subprocess.run([kbin, *args], capture_output=True, text=True)
    if proc.returncode != 0:
        if proc.stderr:
            sys.stderr.write(proc.stderr)
        elif proc.stdout:
            sys.stderr.write(proc.stdout)
        raise SystemExit(proc.returncode)
    try:
        return json.loads(proc.stdout)
    except json.JSONDecodeError as exc:
        print(f"invalid JSON output from kibel: {exc}", file=sys.stderr)
        raise SystemExit(10)


auth = run_kibel(["auth", "status"])
if not (auth.get("ok") is True and auth.get("data", {}).get("logged_in") is True):
    print("auth is not ready; run auth login first", file=sys.stderr)
    raise SystemExit(3)

items = []
latencies = []

print(f"candidate_count={len(candidates)}")
print(f"profile={profile}")
print(f"max_cli_calls={limits['max_cli_calls']}")

for idx, cand in enumerate(candidates, start=1):
    if idx > limits["max_cli_calls"]:
        print(
            f"cli call budget exceeded for profile={profile}: {idx} > {limits['max_cli_calls']}",
            file=sys.stderr,
        )
        raise SystemExit(4)

    payload = run_kibel(["search", "note", "--query", cand, "--first", str(first)])
    results = payload.get("data", {}).get("results")
    if not (payload.get("ok") is True and isinstance(results, list)):
        print("search note output shape mismatch: .data.results[] expected", file=sys.stderr)
        raise SystemExit(3)

    page_info = payload.get("data", {}).get("page_info") or {}
    cursor = page_info.get("endCursor") or ""
    elapsed = (payload.get("meta") or {}).get("elapsed_ms") or 0

    print(f"candidate={cand}")
    print(f"result_count={len(results)}")
    print(f"elapsed_ms={elapsed}")
    if cursor:
        print(f"end_cursor={cursor}")

    for entry in results:
        items.append(
            {
                "candidate": cand,
                "title": entry.get("title"),
                "path": entry.get("path"),
                "url": entry.get("url"),
            }
        )
    latencies.append(int(elapsed))

seen = set()
merged = []
for item in items:
    path = item.get("path")
    if not path or path in seen:
        continue
    seen.add(path)
    merged.append(item)

latencies_sorted = sorted(latencies)
if latencies_sorted:
    p95_idx = max(0, int(len(latencies_sorted) * 0.95) - 1)
    p95_latency = latencies_sorted[p95_idx]
    avg_latency = sum(latencies_sorted) / len(latencies_sorted)
else:
    p95_latency = 0
    avg_latency = 0

metrics = {
    "profile": profile,
    "cli_calls": len(candidates),
    "rounds": 1,
    "note_fetch_count": 0,
    "avg_latency_ms": avg_latency,
    "p95_latency_ms": p95_latency,
}

print("---")
print("merged_top10")
print(json.dumps(merged[:10], ensure_ascii=False))
print("---")
print("metrics")
print(json.dumps(metrics, ensure_ascii=False))
PY
