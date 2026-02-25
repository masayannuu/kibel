#!/usr/bin/env bash
set -euo pipefail

QUERY="${1:-}"
FIRST="${2:-16}"

if [[ -z "${QUERY}" ]]; then
  echo "usage: $0 <query> [first]" >&2
  exit 2
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

"${KBIN}" --json auth status
RESULT_JSON="$("${KBIN}" --json search note --query "${QUERY}" --first "${FIRST}")"
echo "${RESULT_JSON}" | jq -e '.ok == true' >/dev/null
echo "${RESULT_JSON}" | jq -e '(.data.results | type) == "array"' >/dev/null
COUNT="$(echo "${RESULT_JSON}" | jq '.data.results | length')"
CURSOR="$(echo "${RESULT_JSON}" | jq -r '.data.page_info.endCursor // empty')"
echo "result_count=${COUNT}"
if [[ -n "${CURSOR}" ]]; then
  echo "end_cursor=${CURSOR}"
fi
echo "${RESULT_JSON}"
