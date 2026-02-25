---
name: kibel-agentic-search
description: Use this skill for fast and precise Kibela note retrieval via the official kibel CLI interfaces.
allowed-tools: Bash(kibel:*),Bash(rg:*),Bash(jq:*)
---

# kibel Agentic Search

## When to use this

- User asks to find relevant Kibela notes quickly.
- User needs recent notes from self or specific users.
- User needs query + filter narrowing with reproducible CLI commands.

## Do not use this for

- Creating/updating/moving notes.
- Member/admin/destructive operations.

## Preflight (required)

```bash
KBIN="${KIBEL_BIN:-kibel}"
if [[ "${KBIN}" == */* ]]; then
  [[ -x "${KBIN}" ]] || { echo "kibel binary not executable: ${KBIN}" >&2; exit 127; }
elif ! command -v "${KBIN}" >/dev/null 2>&1; then
  echo "kibel not found in PATH (or set KIBEL_BIN)" >&2
  exit 127
fi

AUTH_JSON="$("${KBIN}" --json auth status 2>/dev/null)" || {
  echo "auth status command failed" >&2
  exit 3
}
echo "${AUTH_JSON}" | jq -e '.ok == true' >/dev/null || {
  echo "auth is not ready; run auth login first" >&2
  exit 3
}
```

Proceed only if `ok: true`.

If auth is not ready, recover with one of these before continuing:

```bash
# interactive (recommended for local)
"${KBIN}" --json auth login --origin "https://<tenant>.kibe.la" --team "<tenant>"

# non-interactive token pipe (CI/temporary, `--with-token` reads stdin)
printf '%s' "${KIBELA_ACCESS_TOKEN}" | \
  "${KBIN}" --json auth login --origin "https://<tenant>.kibe.la" --team "<tenant>" --with-token
```

Token issue page:

```text
https://<tenant>.kibe.la/settings/access_tokens
```

Tenant placeholder rule:

- Kibela origin `https://<tenant>.kibe.la` の `<tenant>` を使う。
- 例: `https://spikestudio.kibe.la` -> `team=spikestudio`

Security note:

- ローカル運用は interactive login を優先（keychain/config に保存）。
- `KIBELA_ACCESS_TOKEN` / `--with-token` は CI・一時実行向け。常用しない。

## Core retrieval flows

### 1. Mine latest (high precision, low latency)

```bash
"${KBIN}" --json search note --mine --first 10
```

Use this when user intent is "my latest docs", "what I wrote recently", or "my notes list".

### 2. Broad recall

```bash
"${KBIN}" --json search note --query "<query>" --first 16
```

Cursor pagination:

```bash
"${KBIN}" --json search note --query "<query>" --after "<cursor>" --first 16
```

Optional reusable preset:

```bash
"${KBIN}" --json search note --query "<query>" --save-preset "<name>"
"${KBIN}" --json search note --preset "<name>"
```

### 3. Precision narrowing

```bash
"${KBIN}" --json search note \
  --query "<query>" \
  --user-id "<USER_ID>" \
  --group-id "<GROUP_ID>" \
  --folder-id "<FOLDER_ID>" \
  --first 16
```

Notes:

- `--query` can be empty for filter-first retrieval.
- if `--resource` is omitted, `NOTE` is used by default.
- `--mine` is exclusive and cannot be combined with other search filters (`INPUT_INVALID`).
- `--user-id` is optional. Do not block on unknown user IDs.
- If user ID is unknown, discover candidates first:

```bash
"${KBIN}" --json search user --query "<query>" --group-id "<GROUP_ID>" --folder-id "<FOLDER_ID>" --first 10
```

Then verify candidate notes.

## Verification step (optional but recommended)

Use result `id` or `path` to validate top hits:

```bash
"${KBIN}" --json note get --id "<NOTE_ID>"
"${KBIN}" --json note get-many --id "<NOTE_ID_1>" --id "<NOTE_ID_2>"
"${KBIN}" --json note get-from-path --path "/notes/<number>"
```

## Output contract (agent response)

Always return:

1. query/filters used
2. top matches with title + URL + reason
3. unknowns/gaps

Example format:

```text
Search basis:
- query: onboarding
- filters: user-id=U1, group-id=G1

Top matches:
1. <title> (<url>) - <why relevant>
2. <title> (<url>) - <why relevant>

Unknowns:
- <missing context or ambiguous terms>
```

## References and templates

- `references/commands.md`: compact command cookbook.
- `templates/recall_precision_loop.sh`: one-shot query recall runner.
