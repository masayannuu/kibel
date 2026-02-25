# Agent Skills

## Goal

Provide official agent workflows so Codex/Claude Code can use `kibel` immediately for high-quality search and RAG.

## Skill Pack

- `skills/kibel-agentic-search`
  - Ambiguity-planner-first retrieval workflow for Kibela notes.
- `skills/kibel-agentic-rag`
  - Ambiguity-planner + corrective-loop + verification を含む Agentic RAG workflow.
- `skills/kibel-cli-operator`
  - Broad CLI operation skill including safe `graphql run` query workflows.

## Design Principles

- Read-first default: no mutation commands.
- Deterministic steps: preflight, recall, precision, verification, report.
- Profile-driven exploration: `fast` / `balanced` / `deep`.
- Evidence-first output: every claim should be traceable to Kibela note URL/path.
- Stable interface usage: only official command surface and JSON envelope.
- Distribution-first runtime: skills assume a released `kibel` binary is installed in `PATH`.
- Optional override: set `KIBEL_BIN=/absolute/path/to/kibel` when the binary location is custom.

## Codex Installation (GitHub path)

Recommended:

```bash
./scripts/install_kibel_skills.sh
```

Pinned install for reproducibility:

```bash
./scripts/install_kibel_skills.sh --ref v0.2.6
```

Fallback (direct skill-installer):

```bash
python "${CODEX_HOME:-$HOME/.codex}/skills/.system/skill-installer/scripts/install-skill-from-github.py" \
  --repo masayannuu/kibel \
  --path \
  skills/kibel-agentic-search \
  skills/kibel-agentic-rag \
  skills/kibel-cli-operator
```

Then restart Codex.

## Claude Code Usage

Claude Code can use the same workflow docs directly:

- `skills/kibel-agentic-search/SKILL.md`
- `skills/kibel-agentic-rag/SKILL.md`
- `skills/kibel-cli-operator/SKILL.md`

Treat each `SKILL.md` as an execution playbook and keep commands unchanged.

## Agentic RAG Architecture

詳細設計と評価指標は以下を参照:

- `docs/agentic-rag-architecture.md`
- `docs/agentic-rag-evaluation-2026-02-25.md`
- `docs/agentic-rag-evaluation-protocol.md`
- `docs/agentic-rag-evaluation-v1-balanced-2026-02-25.md`

## Guardrails

- Do not run write commands in search/RAG flows unless explicitly requested.
- Read-only skill metadata is checked by `scripts/check_skill_safety.sh` in CI.
- Keep `graphql run` in query-only mode for these skills.
- Preserve machine-readable outputs (`--json`) for reproducibility.
- If `auth status` is not ready, recover with `auth login --origin ... --team ...` before any retrieval flow.
- Parse `auth status` JSON and fail closed (`exit 3`) when `.ok != true`.
- Use tenant from origin consistently (`https://<tenant>.kibe.la` -> `--team <tenant>`).
- Prefer interactive login for local use; keep env/stdin token injection for CI or temporary runs.
- Treat `--user-id` as optional in retrieval loops; if unknown, narrow by group/folder first and verify via `note get`.
