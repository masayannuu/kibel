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
"${KBIN}" --json search note --query "${QUERY}" --first "${FIRST}"
