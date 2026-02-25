---
name: kibel-agentic-search
description: Use this skill for high-precision Kibela note retrieval via ambiguity-planner-first workflows.
allowed-tools: Bash(kibel:auth status),Bash(kibel:auth login),Bash(kibel:search note),Bash(kibel:search user),Bash(kibel:note get),Bash(kibel:note get-many),Bash(kibel:note get-from-path),Bash(rg:*),Bash(python3:*)
---

# kibel Agentic Search

## When to use this

- User asks to find relevant Kibela notes with high precision.
- User needs recent notes from self or specific users.
- User needs ambiguity-tolerant retrieval with reproducible CLI commands.

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
if ! command -v python3 >/dev/null 2>&1; then
  echo "python3 not found in PATH" >&2
  exit 127
fi

AUTH_JSON="$("${KBIN}" auth status 2>/dev/null)" || {
  echo "auth status command failed" >&2
  exit 3
}
printf '%s' "${AUTH_JSON}" | python3 -c 'import json,sys; d=json.load(sys.stdin); sys.exit(0 if d.get("ok") is True else 1)' || {
  echo "auth is not ready; run auth login first" >&2
  exit 3
}
printf '%s' "${AUTH_JSON}" | python3 -c 'import json,sys; d=json.load(sys.stdin); sys.exit(0 if d.get("data", {}).get("logged_in") is True else 1)' || {
  echo "auth is not ready; run auth login first" >&2
  exit 3
}

SMOKE_JSON="$("${KBIN}" search note --query "test" --first 1 2>/dev/null)" || {
  echo "search note smoke failed" >&2
  exit 3
}
printf '%s' "${SMOKE_JSON}" | python3 -c 'import json,sys; d=json.load(sys.stdin); sys.exit(0 if d.get("ok") is True else 1)' || {
  echo "search note smoke returned not ok" >&2
  exit 3
}
printf '%s' "${SMOKE_JSON}" | python3 -c 'import json,sys; d=json.load(sys.stdin); sys.exit(0 if isinstance(d.get("data", {}).get("results"), list) else 1)' || {
  echo "search note output shape mismatch: .data.results[] expected" >&2
  exit 3
}
```

Proceed only if `ok: true`.

If auth is not ready, recover with one of these before continuing:

```bash
# interactive (recommended for local)
"${KBIN}" auth login --origin "https://<tenant>.kibe.la" --team "<tenant>"

# non-interactive token pipe (CI/temporary, `--with-token` reads stdin)
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

- `search note` items: `.data.results[]`
- `search note` page cursor: `.data.page_info.endCursor`
- `search user` items: `.data.users[]`
- `auth status`: `.data.logged_in`, `.data.team`, `.data.origin`

## Retrieval strategy

Default mode is ambiguity-planner-first retrieval.
Single-query search is a fast-path fallback.

### 1. Mine latest

```bash
"${KBIN}" search note --mine --first 10
```

Use this when user intent is "my latest docs", "what I wrote recently", or "my notes list".

### 2. Ambiguity planner (default)

Before retrieval, decompose the user question:

- `intent`: what answer is needed
- `target`: team/project/system/person
- `artifact`: guide/spec/runbook/postmortem/policy
- `time`: latest/current/specific period
- `scope`: org-wide or local team scope

Generate candidate queries from these facets (not single fixed keywords).

Candidate budget:

- `fast`: up to 2 candidates
- `balanced`: up to 4 candidates
- `deep`: up to 7 candidates

Corrective thresholds by profile:

| profile | min_top5_relevance | min_must_have_evidence_hits |
|---|---:|---:|
| fast | 0.60 | 1 |
| balanced | 0.75 | 2 |
| deep | 0.85 | 2 |

### 3. Candidate recall (default)

```bash
declare -a CANDIDATES=(
  "<anchor_query>"
  "<artifact_query>"
  "<scope_or_time_query>"
)
for q in "${CANDIDATES[@]}"; do
  "${KBIN}" search note --query "${q}" --first 16
done
```

Cursor pagination per candidate:

```bash
"${KBIN}" search note --query "<candidate_query>" --after "<cursor>" --first 16
```

Optional reusable preset:

```bash
"${KBIN}" search note --query "<query>" --save-preset "<name>"
"${KBIN}" search note --preset "<name>"
```

### 4. Precision narrowing

```bash
"${KBIN}" search note \
  --query "<candidate_query>" \
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
"${KBIN}" search user --query "<query>" --group-id "<GROUP_ID>" --folder-id "<FOLDER_ID>" --first 10
```

### 5. Corrective loop (required when evidence is weak)

Trigger when:

- `top5_relevance < min_top5_relevance(profile)`
- `must_have_evidence_hits < min_must_have_evidence_hits(profile)`
- key facets (`artifact` / `target`) are missing
- evidence conflicts

Actions:

1. rewrite candidate queries using missing facets
2. re-run candidate recall within budget
3. narrow by `group-id` / `folder-id` / optional `user-id`

### Fast-path single query (fallback)

Use only when query is explicit and low-latency is priority.

```bash
"${KBIN}" search note --query "<exact_query>" --first 16
```

## Verification step (required for high-precision output)

Use result `id` or `path` to validate top hits:

```bash
"${KBIN}" note get --id "<NOTE_ID>"
"${KBIN}" note get-many --id "<NOTE_ID_1>" --id "<NOTE_ID_2>"
"${KBIN}" note get-from-path --path "/notes/<number>"
```

High-precision rule:

- If you claim high precision, every returned claim must have at least one `note get/get-many/get-from-path` evidence source.
- If evidence cannot be fetched for a claim, move it to `Unknowns` and do not present it as confirmed.

## Output contract (agent response)

Always return:

1. candidate queries and filters used
2. top matches with title + URL + reason
3. unknowns/gaps

Example format:

```text
Search basis:
- candidates: ["オンボーディング資料", "受け入れマニュアル", "入社手順"]
- filters: user-id=U1, group-id=G1

Top matches:
1. <title> (<url>) - <why relevant>
2. <title> (<url>) - <why relevant>

Unknowns:
- <missing context or ambiguous terms>
```

## References and templates

- `references/commands.md`: compact command cookbook.
- `templates/recall_precision_loop.sh`: recall runner (single query / candidate list).
