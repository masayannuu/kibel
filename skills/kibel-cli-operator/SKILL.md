---
name: kibel-cli-operator
description: Use this skill when an agent needs broad operational coverage of the official kibel CLI, including safe GraphQL query execution.
allowed-tools: Bash(kibel:*),Bash(rg:*),Bash(jq:*),Bash(cat:*),Bash(bash:*)
---

# kibel CLI Operator

## When to use this

- User asks for end-to-end CLI usage.
- Task spans multiple command groups (`search`, `note`, `folder`, `group`, `feed`, `graphql`).
- You need safe ad-hoc GraphQL queries in addition to structured commands.

## Required preflight

```bash
KBIN="${KIBEL_BIN:-kibel}"
if [[ "${KBIN}" == */* ]]; then
  [[ -x "${KBIN}" ]] || { echo "kibel binary not executable: ${KBIN}" >&2; exit 127; }
elif ! command -v "${KBIN}" >/dev/null 2>&1; then
  echo "kibel not found in PATH (or set KIBEL_BIN)" >&2
  exit 127
fi

"${KBIN}" --json auth status
"${KBIN}" --help
```

Proceed only when auth status returns `ok: true`.

## Command family guidance

- Discovery/read:
  - `search note`, `search folder`
  - `group list`
  - `folder list/get/get-from-path/notes`
  - `feed sections`
  - `note get/get-from-path`
- Update/write (explicit user intent required):
  - `note create/update/move-to-folder/attach-to-folder`
  - `comment create/reply`
  - `folder create`
- Ad-hoc:
  - `graphql run` for query lane.

## GraphQL run policy

- Query by default (`--allow-mutation` not used).
- Keep `--json` enabled.
- Prefer query files for non-trivial queries.
- For mutation requests, only execute when user explicitly requests and root field is allowlisted.

## Operator playbook

1. Confirm objective and constraints (read-only / write-allowed).
2. Use structured CLI commands first.
3. Use `graphql run` only when structured command is missing.
4. Return output with:
   - command(s) executed
   - key results
   - unknowns and next action

## References and templates

- `references/command-map.md`
- `templates/graphql_query.sh`
