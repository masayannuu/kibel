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

if ! command -v jq >/dev/null 2>&1 || ! jq --version >/dev/null 2>&1; then
  echo "jq is required for this template" >&2
  exit 127
fi

AUTH_JSON="$("${KBIN}" --json auth status)"
printf '%s' "${AUTH_JSON}" | jq -e '.ok == true and .data.logged_in == true' >/dev/null

tmp_items="$(mktemp)"
tmp_latency="$(mktemp)"
trap 'rm -f "${tmp_items}" "${tmp_latency}"' EXIT

case "${PROFILE}" in
  fast) MAX_CANDIDATES=2; MAX_CLI_CALLS=8 ;;
  deep) MAX_CANDIDATES=7; MAX_CLI_CALLS=28 ;;
  *) PROFILE="balanced"; MAX_CANDIDATES=4; MAX_CLI_CALLS=16 ;;
esac

# Anchor query is always included first.
CANDIDATES=("${QUERY}")
for q in "$@"; do
  CANDIDATES+=("${q}")
done

if [[ "${#CANDIDATES[@]}" -gt "${MAX_CANDIDATES}" ]]; then
  echo "candidate budget exceeded for profile=${PROFILE}: ${#CANDIDATES[@]} > ${MAX_CANDIDATES}" >&2
  exit 4
fi

cli_calls=0
printf 'candidate_count=%s\n' "${#CANDIDATES[@]}"
printf 'profile=%s\n' "${PROFILE}"
printf 'max_cli_calls=%s\n' "${MAX_CLI_CALLS}"

for cand in "${CANDIDATES[@]}"; do
  cli_calls=$((cli_calls + 1))
  if [[ "${cli_calls}" -gt "${MAX_CLI_CALLS}" ]]; then
    echo "cli call budget exceeded for profile=${PROFILE}: ${cli_calls} > ${MAX_CLI_CALLS}" >&2
    exit 4
  fi

  RESULT_JSON="$("${KBIN}" --json search note --query "${cand}" --first "${FIRST}")"
  printf '%s' "${RESULT_JSON}" | jq -e '.ok == true and (.data.results | type) == "array"' >/dev/null

  COUNT="$(printf '%s' "${RESULT_JSON}" | jq '.data.results | length')"
  CURSOR="$(printf '%s' "${RESULT_JSON}" | jq -r '.data.page_info.endCursor // empty')"
  ELAPSED_MS="$(printf '%s' "${RESULT_JSON}" | jq '.meta.elapsed_ms // 0')"

  printf 'candidate=%s\n' "${cand}"
  printf 'result_count=%s\n' "${COUNT}"
  printf 'elapsed_ms=%s\n' "${ELAPSED_MS}"
  if [[ -n "${CURSOR}" ]]; then
    printf 'end_cursor=%s\n' "${CURSOR}"
  fi

  printf '%s' "${RESULT_JSON}" \
    | jq -c --arg q "${cand}" '.data.results[]? | {candidate:$q,title,path,url}' \
    >> "${tmp_items}"
  printf '%s\n' "${ELAPSED_MS}" >> "${tmp_latency}"

done

printf '%s\n' '---' 'merged_top10'
jq -s 'reduce .[] as $i ([]; if ([.[].path] | index($i.path)) then . else . + [$i] end) | .[:10]' "${tmp_items}"
printf '%s\n' '---' 'metrics'
jq -Rcs --arg profile "${PROFILE}" --argjson calls "${cli_calls}" '
  [split("\n")[] | select(length>0) | tonumber] as $arr
  | {
      profile: $profile,
      cli_calls: $calls,
      rounds: 1,
      note_fetch_count: 0,
      avg_latency_ms: (if ($arr|length)==0 then 0 else (($arr|add) / ($arr|length)) end),
      p95_latency_ms: (if ($arr|length)==0 then 0 else (($arr|sort)[((($arr|length)*0.95|floor)-1)|if . < 0 then 0 else . end]) end)
    }' "${tmp_latency}"
