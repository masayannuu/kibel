# Workflow Reference (kibel-agentic-rag)

## Profile setup

Set profile budget before retrieval:

```bash
PROFILE="${KIBEL_RAG_PROFILE:-balanced}"
case "${PROFILE}" in
  fast) FIRST=8; MAX_ROUNDS=1; MAX_NOTE_FETCH=4 ;;
  deep) FIRST=24; MAX_ROUNDS=3; MAX_NOTE_FETCH=16 ;;
  *) FIRST=16; MAX_ROUNDS=2; MAX_NOTE_FETCH=8 ;;
esac
```

## 1. route_select

Pre-check auth. If not ready:

```bash
kibel --json auth login --origin "https://<tenant>.kibe.la" --team "<tenant>"
```

Classify question: `direct / multi_hop / global`.

## 2. seed_recall

Run broad search 1-3 times with different query terms:

```bash
kibel --json search note --query "<topic>" --first "${FIRST}"
```

Count:

```bash
kibel --json search note --query "<topic>" --first "${FIRST}" | jq '.data.results | length'
```

If results are many, paginate forward:

```bash
kibel --json search note --query "<topic>" --after "<cursor>" --first "${FIRST}"
```

Cursor:

```bash
kibel --json search note --query "<topic>" --first "${FIRST}" | jq -r '.data.page_info.endCursor // empty'
```

Optional personal context:

```bash
kibel --json search note --mine --first "${FIRST}"
```

## 3. frontier_expand + precision

Narrow candidates:

```bash
kibel --json search note \
  --query "<topic>" \
  --user-id "<USER_ID>" \
  --group-id "<GROUP_ID>" \
  --folder-id "<FOLDER_ID>" \
  --first "${FIRST}"
```

If `<USER_ID>` is unknown, omit it and keep narrowing with group/folder filters.
Or discover candidates first:

```bash
kibel --json search user --query "<topic>" --first 10
```

## 4. evidence_pull + verification

Get full source before final answer:

```bash
kibel --json note get --id "<NOTE_ID>"
kibel --json note get-many --id "<NOTE_ID_1>" --id "<NOTE_ID_2>"
```

or

```bash
kibel --json note get-from-path --path "/notes/<number>"
```

## 5. corrective_loop

If evidence coverage is still weak:

- rewrite query using missing entities/constraints
- re-run `seed_recall` with remaining budget
- stop when no new evidence is found twice

## 6. Synthesis

Return:

- answer
- evidence list with URLs
- unknowns/assumptions
