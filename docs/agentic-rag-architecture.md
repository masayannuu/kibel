# Agentic RAG Architecture

## Goal

Provide a deterministic, evidence-first retrieval workflow for Kibela that coding agents can run with the official `kibel` CLI.

## Design

1. `route_select`: classify the request (`direct` / `multi_hop` / `global`).
2. `seed_recall`: run initial `search note` with base query variants.
3. `frontier_expand`: derive follow-up queries from top candidates.
4. `evidence_pull`: fetch note bodies with `note get` / `note get-from-path` only when required.
5. `corrective_loop`: re-query when evidence support is insufficient.
6. `verification`: claim-by-claim validation before final answer.
7. `finalize`: answer with citations and explicit unknowns.

## Runtime Profiles

- `fast`: low latency and low API/CLI call budget.
- `balanced`: default profile for general QA and troubleshooting.
- `deep`: higher recall with larger call budget.

Skill templates must enforce profile budgets and fail closed when budgets are exceeded.

## Safety Boundary

- Read-only operations only for search/RAG skills.
- Do not run write/mutation commands unless explicitly requested.
- Prefer machine-readable JSON output (`kibel` default); use `--text` only for human-only flows.

## Evaluation KPIs

- `answer_supported_rate`
- `citation_precision`
- `unknowns_rate`
- `avg_cli_calls`
- `p95_latency_ms`

## Acceptance Criteria

- Every answer cites concrete Kibela note URLs/paths.
- Unknowns are explicit when evidence is insufficient.
- Profile budgets are respected and observable in output metrics.
