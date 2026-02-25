---
name: kibel-agentic-rag
description: Use this skill for evidence-first RAG over Kibela using kibel CLI (retrieve -> verify -> cite).
allowed-tools: Bash(kibel:*),Bash(rg:*),Bash(jq:*)
---

# kibel Agentic RAG

## Goal

Produce high-quality answers grounded in Kibela notes with explicit citations.

## Scope

- Read/search workflows only.
- No note/comment mutations.

## Preflight

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
echo "${AUTH_JSON}" | jq -e '.data.logged_in == true' >/dev/null || {
  echo "auth is not ready; run auth login first" >&2
  exit 3
}

SMOKE_JSON="$("${KBIN}" --json search note --query "test" --first 1 2>/dev/null)" || {
  echo "search note smoke failed" >&2
  exit 3
}
echo "${SMOKE_JSON}" | jq -e '.ok == true' >/dev/null || {
  echo "search note smoke returned not ok" >&2
  exit 3
}
echo "${SMOKE_JSON}" | jq -e '(.data.results | type) == "array"' >/dev/null || {
  echo "search note output shape mismatch: .data.results[] expected" >&2
  exit 3
}
```

If auth is not ready, recover before retrieval:

```bash
# interactive
"${KBIN}" --json auth login --origin "https://<tenant>.kibe.la" --team "<tenant>"

# non-interactive (CI/temporary, `--with-token` reads stdin)
printf '%s' "${KIBELA_ACCESS_TOKEN}" | \
  "${KBIN}" --json auth login --origin "https://<tenant>.kibe.la" --team "<tenant>" --with-token
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

## Execution profile

Use `KIBEL_RAG_PROFILE` (default: `balanced`):

- `fast`: `first=8`, `max_rounds=1`, `max_note_fetch=4`, `max_cli_calls=8`
- `balanced`: `first=16`, `max_rounds=2`, `max_note_fetch=8`, `max_cli_calls=16`
- `deep`: `first=24`, `max_rounds=3`, `max_note_fetch=16`, `max_cli_calls=28`

## Retrieval pipeline (Agentic RAG v2)

1. `route_select`: classify question as `direct / multi_hop / global`.
2. `seed_recall`: run broad query with profile-specific `first`.
3. `frontier_expand`: generate 1-2 follow-up queries from top hits.
4. `evidence_pull`: fetch full notes only for selected candidates.
5. `corrective_loop`: if evidence is weak, re-search with rewritten query.
6. `verification`: run CoVe-style claim checks before final answer.
7. `finalize`: answer + evidence + unknowns.

### Pass 1: Recall

Run 2-3 broad queries (synonyms allowed):

```bash
"${KBIN}" --json search note --query "<topic>" --first "${FIRST:-16}"
```

When result volume is high, paginate forward with cursor:

```bash
"${KBIN}" --json search note --query "<topic>" --after "<cursor>" --first "${FIRST:-16}"
```

Optional reusable preset:

```bash
"${KBIN}" --json search note --query "<topic>" --save-preset "<name>"
"${KBIN}" --json search note --preset "<name>"
```

Optionally include latest self context:

```bash
"${KBIN}" --json search note --mine --first 10
```

### Pass 2: Precision

Narrow with filters where known:

```bash
"${KBIN}" --json search note \
  --query "<topic>" \
  --user-id "<USER_ID>" \
  --group-id "<GROUP_ID>" \
  --folder-id "<FOLDER_ID>" \
  --first "${FIRST:-16}"
```

Rules:

- `--user-id` is optional; if unknown, continue with group/folder filters first.
- When author precision is required, discover author candidates first:

```bash
"${KBIN}" --json search user --query "<topic>" --group-id "<GROUP_ID>" --folder-id "<FOLDER_ID>" --first 10
```

Then inspect returned note metadata (`note get`) to pin the correct author ID.
- `--mine` is for self-latest only; do not combine it with other search filters.

### Pass 3: Verification

Fetch full note bodies for top candidates:

```bash
"${KBIN}" --json note get --id "<NOTE_ID>"
"${KBIN}" --json note get-many --id "<NOTE_ID_1>" --id "<NOTE_ID_2>"
```

or:

```bash
"${KBIN}" --json note get-from-path --path "/notes/<number>"
```

CoVe-style minimum rule:

- 主張ごとに「裏取り質問」を1つ作る。
- 裏取り質問ごとに最低1件 `note get` で本文確認する。
- 裏取りできない主張は `Unknowns` に落とす。

## Ranking rubric

Prefer notes with:

1. direct term overlap with question
2. recent `updatedAt` when recency matters
3. stronger author/owner relevance
4. concrete implementation details over generic summaries

## Answer contract

Return three sections in order.

1. Answer
2. Evidence (title + URL + supporting point)
3. Unknowns / assumptions

Example:

```text
Answer:
<final answer>

Evidence:
1. <title> (<url>) - <supporting point>
2. <title> (<url>) - <supporting point>

Unknowns:
- <what could not be validated from retrieved notes>
```

## Failure handling

- If retrieval has zero relevant hits, report no-evidence explicitly.
- If evidence conflicts, show both sources and mark unresolved.
- Never fabricate citations or note content.

## References and templates

- `references/workflow.md`: compact step-by-step workflow.
- `templates/evidence_answer_template.md`: final response template.
- `templates/profile_scorecard.md`: profile A/B evaluation sheet.
- `docs/agentic-rag-architecture.md`: architecture and KPI-based evaluation.
