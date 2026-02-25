---
name: kibel-agentic-rag
description: Use this skill for evidence-first RAG over Kibela using kibel CLI (retrieve -> verify -> cite).
allowed-tools: Bash(kibel:--json auth status),Bash(kibel:--json auth login),Bash(kibel:--json search note),Bash(kibel:--json search user),Bash(kibel:--json note get),Bash(kibel:--json note get-many),Bash(kibel:--json note get-from-path),Bash(rg:*),Bash(jq:*)
---

# kibel Agentic RAG

## Goal

Produce high-quality answers grounded in Kibela notes with explicit citations.
Primary operating mode is Japanese-first retrieval for Japanese teams.

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
printf '%s' "${AUTH_JSON}" | jq -e '.ok == true' >/dev/null || {
  echo "auth is not ready; run auth login first" >&2
  exit 3
}
printf '%s' "${AUTH_JSON}" | jq -e '.data.logged_in == true' >/dev/null || {
  echo "auth is not ready; run auth login first" >&2
  exit 3
}

SMOKE_JSON="$("${KBIN}" --json search note --query "test" --first 1 2>/dev/null)" || {
  echo "search note smoke failed" >&2
  exit 3
}
printf '%s' "${SMOKE_JSON}" | jq -e '.ok == true' >/dev/null || {
  echo "search note smoke returned not ok" >&2
  exit 3
}
printf '%s' "${SMOKE_JSON}" | jq -e '(.data.results | type) == "array"' >/dev/null || {
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

Corrective thresholds by profile:

| profile | min_top5_relevance | min_must_have_evidence_hits |
|---|---:|---:|
| fast | 0.60 | 1 |
| balanced | 0.75 | 2 |
| deep | 0.85 | 2 |

## Ambiguity planner (Japanese-first)

Use this policy before retrieval.

1. Detect query language: `ja / en / mixed`.
2. Normalize query:
   - trim extra spaces
   - normalize full/half-width where possible
   - keep original entity strings (project names, product names)
3. Decompose ambiguity into facets:
   - `intent` (what answer is needed)
   - `target` (team/project/system/person)
   - `artifact` (guide/spec/postmortem/runbook/policy)
   - `time` (latest/current/specific period)
   - `scope` (all-org vs team-local)
4. Generate candidate queries from facets, not from fixed dictionaries.

Candidate classes:

- `anchor`: normalized original query
- `artifact-focused`: intent + artifact
- `scope-focused`: target + artifact or target + intent
- `time-focused`: artifact + recency constraint words
- `verification-focused`: claim-check style query for weak claims

Candidate budget by profile:

- `fast`: up to 2 candidates
- `balanced`: up to 4 candidates
- `deep`: up to 7 candidates

Ranking priority:

- optimize `top3-5` relevance first
- allow some noise in `top10` only when it improves coverage

## Retrieval pipeline (Agentic RAG v2)

1. `ambiguity_planner`: normalize + decompose + generate candidate queries.
2. `route_select`: classify question as `direct / multi_hop / global`.
3. `seed_recall`: run broad query with profile-specific `first`.
4. `frontier_expand`: generate 1-2 follow-up queries from top hits.
5. `evidence_pull`: fetch full notes only for selected candidates.
6. `corrective_loop`: if evidence is weak, re-search with rewritten query.
7. `verification`: run CoVe-style claim checks before final answer.
8. `finalize`: answer + evidence + unknowns.

### Pass 1: Recall

Run broad queries using planner candidates:

```bash
"${KBIN}" --json search note --query "<topic>" --first "${FIRST:-16}"
```

Candidate loop example:

```bash
declare -a CANDIDATES=(
  "<anchor_query>"
  "<artifact_focused_query>"
  "<scope_or_time_focused_query>"
)
for q in "${CANDIDATES[@]}"; do
  "${KBIN}" --json search note --query "${q}" --first "${FIRST:-16}"
done
```

Language fallback rule:

- `ja`: keep all candidates Japanese-first
- `en`: use mixed candidates (`ja + en`) when team docs are Japanese-heavy
- `mixed`: prioritize candidates matching target team terminology

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

Japanese-first verification rule:

- Prefer evidence whose title/body matches Japanese domain terminology used by the team.
- If only English hits support a claim, keep the claim but mark terminology gap in `Unknowns`.

Corrective trigger rule:

- trigger corrective loop when any of the following holds:
  - `top5_relevance < min_top5_relevance(profile)`
  - `must_have_evidence_hits < min_must_have_evidence_hits(profile)`
  - key facet (`artifact` or `target`) has no strong evidence
  - claims conflict across sources
- first corrective action: rewrite candidates using missing facet terms
- second corrective action: narrow with filters (`group-id`, `folder-id`, `user-id`) when available

## Ranking rubric

Prefer notes with:

1. facet-aware overlap with candidate set (`intent/target/artifact/time`)
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
- `templates/ambiguity_planner_card.md`: ambiguity decomposition worksheet.
- `eval/dataset_v1.json`: reproducible question set (`>=20` cases).
- `eval/run_eval_v1.js`: baseline/planner/planner+corrective evaluation runner.
- `docs/agentic-rag-architecture.md`: architecture and KPI-based evaluation.
- `docs/agentic-rag-evaluation-protocol.md`: evaluation contract and gates.
