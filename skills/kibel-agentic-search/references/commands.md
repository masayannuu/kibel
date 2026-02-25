# Command Reference (kibel-agentic-search)

## Required

```bash
kibel auth status
```

Canonical auth selectors:

```bash
kibel auth status | python3 -c 'import json,sys; d=json.load(sys.stdin); print(json.dumps({"logged_in": d.get("data", {}).get("logged_in"), "team": d.get("data", {}).get("team"), "origin": d.get("data", {}).get("origin")}, ensure_ascii=False))'
```

If auth is not ready:

```bash
kibel auth login --origin "https://<tenant>.kibe.la" --team "<tenant>"
```

## Default retrieval (ambiguity-planner-first)

### 1) Build candidate query set from facets

Facet checklist:

- intent
- target
- artifact
- time
- scope

Candidate example:

```bash
declare -a CANDIDATES=(
  "<anchor_query>"
  "<artifact_query>"
  "<scope_or_time_query>"
)
```

### 2) Candidate recall loop

```bash
for q in "${CANDIDATES[@]}"; do
  kibel search note --query "${q}" --first 16
done
```

Count per candidate:

```bash
kibel search note --query "<candidate_query>" --first 16 | python3 -c 'import json,sys; print(len(json.load(sys.stdin).get("data", {}).get("results", [])))'
```

Cursor next page:

```bash
kibel search note --query "<candidate_query>" --after "<cursor>" --first 16
```

Cursor:

```bash
kibel search note --query "<candidate_query>" --first 16 | python3 -c 'import json,sys; d=json.load(sys.stdin); print((d.get("data", {}).get("page_info", {}) or {}).get("endCursor", ""))'
```

### 3) Precision narrowing

```bash
kibel search note \
  --query "<candidate_query>" \
  --user-id "<USER_ID>" \
  --group-id "<GROUP_ID>" \
  --folder-id "<FOLDER_ID>" \
  --first 16
```

`--user-id` is optional. If unknown, narrow by group/folder first and verify candidates with `note get`.

User discovery:

```bash
kibel search user --query "<query>" --first 10
```

User count:

```bash
kibel search user --query "<query>" --first 10 | python3 -c 'import json,sys; print(len(json.load(sys.stdin).get("data", {}).get("users", [])))'
```

### 4) Corrective loop trigger

Re-run with rewritten candidates when:

- `top5_relevance < 0.60/0.75/0.85` (`fast`/`balanced`/`deep`)
- `must_have_evidence_hits < 1/2/2` (`fast`/`balanced`/`deep`)
- key facets are missing
- evidence conflicts

## Fast-path fallback (single query)

Use only for explicit low-latency retrieval:

```bash
kibel search note --query "<exact_query>" --first 16
```

## Mine latest

```bash
kibel search note --mine --first 10
```

`--mine` is exclusive. Do not combine with `--query`, `--user-id`, `--group-id`, `--folder-id`.

## Verification

```bash
kibel note get --id "<NOTE_ID>"
kibel note get-many --id "<NOTE_ID_1>" --id "<NOTE_ID_2>"
kibel note get-from-path --path "/notes/<number>"
```

When returning high-precision results, claims without note-level verification must be placed in `Unknowns`.
