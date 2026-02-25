#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"

readonly_skills=(
  "${ROOT}/skills/kibel-agentic-search/SKILL.md"
  "${ROOT}/skills/kibel-agentic-rag/SKILL.md"
)

fail=0

for file in "${readonly_skills[@]}"; do
  if [[ ! -f "${file}" ]]; then
    echo "missing skill file: ${file}" >&2
    fail=1
    continue
  fi

  allowed_line="$(grep -n '^allowed-tools:' "${file}" | cut -d: -f2- || true)"
  if [[ -z "${allowed_line}" ]]; then
    echo "missing allowed-tools in ${file}" >&2
    fail=1
  fi

  if grep -Fq 'Bash(kibel:*)' "${file}"; then
    echo "forbidden wildcard Bash(kibel:*) in ${file}" >&2
    fail=1
  fi

  if grep -Fq 'Bash(jq:*)' "${file}"; then
    echo "forbidden jq dependency in read-only skill: ${file}" >&2
    fail=1
  fi

  # Read-only skill must not advertise known write/mutation entrypoints.
  if grep -Eq 'kibel:[^)]*(note create|note update|note move-to-folder|note attach-to-folder|comment create|comment reply|folder create|graphql run --allow-mutation)' "${file}"; then
    echo "forbidden write/mutation command in allowed-tools: ${file}" >&2
    fail=1
  fi
done

if [[ "${fail}" -ne 0 ]]; then
  exit 1
fi

echo "skill safety check: ok"
