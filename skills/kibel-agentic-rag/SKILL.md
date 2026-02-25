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
- 例: `https://spikestudio.kibe.la` -> `team=spikestudio`

Security note:

- ローカル運用は interactive login を優先（keychain/config に保存）。
- `KIBELA_ACCESS_TOKEN` / `--with-token` は CI・一時実行向け。常用しない。

## Retrieval pipeline

### Pass 1: Recall

Run 2-3 broad queries (synonyms allowed):

```bash
"${KBIN}" --json search note --query "<topic>" --first 16
```

When result volume is high, paginate forward with cursor:

```bash
"${KBIN}" --json search note --query "<topic>" --after "<cursor>" --first 16
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
  --first 16
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
