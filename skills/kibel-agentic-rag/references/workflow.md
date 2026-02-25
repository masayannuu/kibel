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
kibel auth login --origin "https://<tenant>.kibe.la" --team "<tenant>"
```

Classify question: `procedure / direct / multi_hop / global`.

- `procedure`: 手順・方法・設定系 (`手順`, `方法`, `how-to`, `login/setup/export` など)
- `multi_hop`: 比較・依存・原因分析
- `global`: 全社/横断要約
- `direct`: 単一事実確認

## 3. seed_recall

Run broad search for each planner candidate:

```bash
kibel search note --query "<candidate_query>" --first "${FIRST}"
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
  kibel search note --query "${q}" --first "${FIRST}"
done
```

Count:

```bash
kibel search note --query "<candidate_query>" --first "${FIRST}" | python3 -c 'import json,sys; print(len(json.load(sys.stdin).get("data", {}).get("results", [])))'
```

If results are many, paginate forward:

```bash
kibel search note --query "<candidate_query>" --after "<cursor>" --first "${FIRST}"
```

Cursor:

```bash
kibel search note --query "<candidate_query>" --first "${FIRST}" | python3 -c 'import json,sys; d=json.load(sys.stdin); print((d.get("data", {}).get("page_info", {}) or {}).get("endCursor", ""))'
```

Optional personal context:

```bash
kibel search note --mine --first "${FIRST}"
```

## 4. frontier_expand + precision

Narrow candidates:

```bash
kibel search note \
  --query "<topic>" \
  --user-id "<USER_ID>" \
  --group-id "<GROUP_ID>" \
  --folder-id "<FOLDER_ID>" \
  --first "${FIRST}"
```

If `<USER_ID>` is unknown, omit it and keep narrowing with group/folder filters.
Or discover candidates first:

```bash
kibel search user --query "<topic>" --first 10
```

## 5. evidence_pull + verification

Get full source before final answer:

```bash
kibel note get --id "<NOTE_ID>"
kibel note get-many --id "<NOTE_ID_1>" --id "<NOTE_ID_2>"
```

or

```bash
kibel note get-from-path --path "/notes/<number>"
```

Signal coverage rule:

- Build signal terms from planner/corrective queries and required evidence keywords.
- Prefer notes with at least one strong signal-set match (2+ meaningful terms).
- Avoid promoting notes that only match one weak generic term.
- For auth/login procedures, require compound intent terms (for example auth+login) before accepting evidence.

Procedure verification rule:

- For `procedure` route, final evidence should satisfy both:
  1. strong signal-set match
  2. procedural marker in note body (ordered list, command flags, code block)
- If one is missing, keep the claim unresolved and trigger corrective search.

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
