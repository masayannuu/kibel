#!/usr/bin/env bash
set -euo pipefail

SCRIPT_NAME="$(basename "$0")"

usage() {
  cat <<USAGE
Usage: ${SCRIPT_NAME} [options]

Install official kibel skills for Codex via local skill-installer.

Options:
  --repo OWNER/REPO   Source repository (default: masayannuu/kibel)
  --ref REF           Git ref/tag/sha to pin (recommended for reproducibility)
  --dest DIR          Destination skills directory (optional)
  --method MODE       Installer fetch method: auto|download|git (default: auto)
  --python BIN        Python executable override
  --dry-run           Print command only, do not execute
  -h, --help          Show this help

Environment overrides:
  CODEX_HOME          Base Codex home (default: ~/.codex)
  KIBEL_SKILLS_REPO   Default repo override
  KIBEL_SKILLS_REF    Default ref override
USAGE
}

repo="${KIBEL_SKILLS_REPO:-masayannuu/kibel}"
ref="${KIBEL_SKILLS_REF:-}"
dest=""
method="auto"
python_bin=""
dry_run=0

while [ "$#" -gt 0 ]; do
  case "$1" in
    --repo)
      repo="${2:-}"
      shift 2
      ;;
    --ref)
      ref="${2:-}"
      shift 2
      ;;
    --dest)
      dest="${2:-}"
      shift 2
      ;;
    --method)
      method="${2:-}"
      shift 2
      ;;
    --python)
      python_bin="${2:-}"
      shift 2
      ;;
    --dry-run)
      dry_run=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "error: unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [ -z "${repo}" ]; then
  echo "error: repo must not be empty" >&2
  exit 2
fi

if [ -z "${python_bin}" ]; then
  if command -v python3 >/dev/null 2>&1; then
    python_bin="python3"
  elif command -v python >/dev/null 2>&1; then
    python_bin="python"
  else
    echo "error: python3/python not found" >&2
    exit 1
  fi
fi

codex_home="${CODEX_HOME:-$HOME/.codex}"
installer="${codex_home}/skills/.system/skill-installer/scripts/install-skill-from-github.py"
if [ ! -f "${installer}" ]; then
  echo "error: skill installer not found: ${installer}" >&2
  echo "hint: install Codex skill-installer first, then rerun ${SCRIPT_NAME}" >&2
  exit 1
fi

case "${method}" in
  auto|download|git) ;;
  *)
    echo "error: --method must be one of: auto, download, git" >&2
    exit 2
    ;;
esac

cmd=(
  "${python_bin}" "${installer}"
  --repo "${repo}"
  --path
  skills/kibel-agentic-search
  skills/kibel-agentic-rag
  skills/kibel-cli-operator
  --method "${method}"
)

if [ -n "${ref}" ]; then
  cmd+=(--ref "${ref}")
fi

if [ -n "${dest}" ]; then
  cmd+=(--dest "${dest}")
fi

echo "[kibel-skills] repo=${repo} ref=${ref:-<default>} method=${method}"
if [ "${dry_run}" -eq 1 ]; then
  echo "[kibel-skills] dry-run command:"
  printf '  %q' "${cmd[@]}"
  echo
  exit 0
fi

"${cmd[@]}"
echo "[kibel-skills] install complete. restart Codex if already running."
