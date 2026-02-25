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

"${KBIN}" --json auth status
```

## Retrieval pipeline

### Pass 1: Recall

Run 2-3 broad queries (synonyms allowed):

```bash
"${KBIN}" --json search note --query "<topic>" --first 16
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

### Pass 3: Verification

Fetch full note bodies for top candidates:

```bash
"${KBIN}" --json note get --id "<NOTE_ID>"
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
