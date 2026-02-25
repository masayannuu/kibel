# Workflow Reference (kibel-agentic-rag)

## Profile setup

Set profile budget before retrieval:

```bash
PROFILE="${KIBEL_RAG_PROFILE:-balanced}"
case "${PROFILE}" in
  fast) FIRST=8; MAX_ROUNDS=1; MAX_NOTE_FETCH=4; MAX_CLI_CALLS=8; MIN_TOP5=0.60; MIN_MUST_HAVE=1 ;;
  deep) FIRST=24; MAX_ROUNDS=3; MAX_NOTE_FETCH=16; MAX_CLI_CALLS=28; MIN_TOP5=0.85; MIN_MUST_HAVE=2 ;;
  *) FIRST=16; MAX_ROUNDS=2; MAX_NOTE_FETCH=8; MAX_CLI_CALLS=16; MIN_TOP5=0.75; MIN_MUST_HAVE=2 ;;
esac
```

## 1. ambiguity_planner (Japanese-first)

Decompose ambiguity before retrieval.

- normalize original query
- extract facets: `intent`, `target`, `artifact`, `time`, `scope`
- generate candidate queries from facet combinations

Candidate budget:

- `fast`: up to 2 candidates
- `balanced`: up to 4 candidates
- `deep`: up to 7 candidates

Candidate classes:

- anchor candidate (normalized original query)
- artifact-focused candidate (`artifact + intent`)
- scope-focused candidate (`target + artifact`)
- time-focused candidate (`artifact + time constraint`)
- verification candidate (`claim check` style)

Use planner worksheet:

```bash
cat templates/ambiguity_planner_card.md
```

## 2. route_select

Pre-check auth. If not ready:

```bash
kibel --json auth login --origin "https://<tenant>.kibe.la" --team "<tenant>"
```

Classify question: `direct / multi_hop / global`.

## 3. seed_recall

Run broad search for each planner candidate:

```bash
kibel --json search note --query "<candidate_query>" --first "${FIRST}"
```

Loop example:

```bash
declare -a CANDIDATES=(
  "<anchor_query>"
  "<artifact_query>"
  "<scope_or_time_query>"
)
for q in "${CANDIDATES[@]}"; do
  # budget guard (example)
  # [[ "${CLI_CALLS}" -lt "${MAX_CLI_CALLS}" ]] || { echo "budget exceeded: cli_calls"; exit 4; }
  kibel --json search note --query "${q}" --first "${FIRST}"
done
```

Count:

```bash
kibel --json search note --query "<candidate_query>" --first "${FIRST}" | jq '.data.results | length'
```

If results are many, paginate forward:

```bash
kibel --json search note --query "<candidate_query>" --after "<cursor>" --first "${FIRST}"
```

Cursor:

```bash
kibel --json search note --query "<candidate_query>" --first "${FIRST}" | jq -r '.data.page_info.endCursor // empty'
```

Optional personal context:

```bash
kibel --json search note --mine --first "${FIRST}"
```

## 4. frontier_expand + precision

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

## 5. evidence_pull + verification

Get full source before final answer:

```bash
kibel --json note get --id "<NOTE_ID>"
kibel --json note get-many --id "<NOTE_ID_1>" --id "<NOTE_ID_2>"
```

or

```bash
kibel --json note get-from-path --path "/notes/<number>"
```

## 6. corrective_loop

If evidence coverage is still weak:

- rewrite candidates using missing facets (`artifact`/`target`/`time`)
- for Japanese teams, prefer Japanese terminology aligned to team docs
- re-run `seed_recall` with remaining budget
- stop when no new evidence is found twice

Numeric trigger:

- run corrective when `top5_relevance < MIN_TOP5` or `must_have_evidence_hits < MIN_MUST_HAVE`

## 7. Synthesis

Return:

- answer
- evidence list with URLs
- unknowns/assumptions

Evaluation note:

- track `top5_relevance` first (primary quality gate)
- keep `top10_relevance` as secondary recall signal
- track `corrective_trigger_rate` and `ambiguity_resolution_rate`
