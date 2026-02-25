---
name: kibel-cli-operator
description: Use this skill when an agent needs broad operational coverage of the official kibel CLI, including safe GraphQL query execution.
allowed-tools: Bash(kibel:*),Bash(rg:*),Bash(python3:*),Bash(cat:*),Bash(bash:*)
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
if ! command -v python3 >/dev/null 2>&1; then
  echo "python3 not found in PATH" >&2
  exit 127
fi

AUTH_JSON="$("${KBIN}" auth status 2>/dev/null)" || {
  echo "auth status command failed" >&2
  exit 3
}
python3 -c 'import json,sys; d=json.load(sys.stdin); sys.exit(0 if d.get("ok") is True else 1)' <<<"${AUTH_JSON}" || {
  echo "auth is not ready; run auth login first" >&2
  exit 3
}
python3 -c 'import json,sys; d=json.load(sys.stdin); sys.exit(0 if d.get("data", {}).get("logged_in") is True else 1)' <<<"${AUTH_JSON}" || {
  echo "auth is not ready; run auth login first" >&2
  exit 3
}

SMOKE_JSON="$("${KBIN}" search note --query "test" --first 1 2>/dev/null)" || {
  echo "search note smoke failed" >&2
  exit 3
}
python3 -c 'import json,sys; d=json.load(sys.stdin); sys.exit(0 if d.get("ok") is True else 1)' <<<"${SMOKE_JSON}" || {
  echo "search note smoke returned not ok" >&2
  exit 3
}
python3 -c 'import json,sys; d=json.load(sys.stdin); sys.exit(0 if isinstance(d.get("data", {}).get("results"), list) else 1)' <<<"${SMOKE_JSON}" || {
  echo "search note output shape mismatch: .data.results[] expected" >&2
  exit 3
}
"${KBIN}" --help
```

Proceed only when auth status returns `ok: true`.

If auth is not ready:

```bash
# interactive
"${KBIN}" auth login --origin "https://<tenant>.kibe.la" --team "<tenant>"

# non-interactive (CI/temporary, `--with-token` reads stdin)
printf '%s' "${KIBELA_ACCESS_TOKEN}" | \
  "${KBIN}" auth login --origin "https://<tenant>.kibe.la" --team "<tenant>" --with-token
```

Token issue page:

```text
https://<tenant>.kibe.la/settings/access_tokens
```

Tenant placeholder rule:

- Kibela origin `https://<tenant>.kibe.la` の `<tenant>` を使う。
- 例: `https://example.kibe.la` -> `team=example`

Security note:

- ローカル運用は interactive login を優先（keychain/config に保存）。
- `KIBELA_ACCESS_TOKEN` / `--with-token` は CI・一時実行向け。常用しない。

## Canonical JSON selectors

- `search note`: `.data.results[]`
- `search note` cursor: `.data.page_info.endCursor`
- `search user`: `.data.users[]`
- `auth status`: `.data.logged_in`, `.data.team`, `.data.origin`

## Command family guidance

- Discovery/read:
  - `search note`, `search folder`, `search user`
  - `group list`
  - `folder list/get/get-from-path/notes`
  - `feed sections`
  - `note get/get-many/get-from-path`
- Update/write (explicit user intent required):
  - `note create/update/move-to-folder/attach-to-folder`
  - `comment create/reply`
  - `folder create`
- Ad-hoc:
  - `graphql run` for query lane.

## GraphQL run policy

- Query by default (`--allow-mutation` not used).
- Default output is JSON. Use `--text` only when human-readable output is required.
- Prefer query files for non-trivial queries.
- For mutation requests, only execute when user explicitly requests and root field is allowlisted.

## Operator playbook

1. Confirm objective and constraints (read-only / write-allowed).
2. Use structured CLI commands first.
3. For search loops, use cursor pagination (`search note --after`) and optional presets (`--save-preset` / `--preset`) before falling back to ad-hoc GraphQL.
4. Use `graphql run` only when structured command is missing.
5. Return output with:
   - command(s) executed
   - key results
   - unknowns and next action

## References and templates

- `references/command-map.md`
- `templates/graphql_query.sh`
