# kibel Official Skills

This directory contains official agent skills for `kibel`.

## Available skills

- `kibel-agentic-search`
- `kibel-agentic-rag`
- `kibel-cli-operator`

## Quick install (Codex)

```bash
./scripts/install_kibel_skills.sh
```

Pinned install for reproducibility:

```bash
./scripts/install_kibel_skills.sh --ref v0.2.6
```

Fallback (manual skill-installer):

```bash
python "${CODEX_HOME:-$HOME/.codex}/skills/.system/skill-installer/scripts/install-skill-from-github.py" \
  --repo masayannuu/kibel \
  --path \
  skills/kibel-agentic-search \
  skills/kibel-agentic-rag \
  skills/kibel-cli-operator
```

Restart Codex after installation.

## Manual use (Codex / Claude Code)

Open each `SKILL.md` and execute the workflow as-is:

- `skills/kibel-agentic-search/SKILL.md`
- `skills/kibel-agentic-rag/SKILL.md`
- `skills/kibel-cli-operator/SKILL.md`

## Runtime expectation

- Official skills target distributed binaries: `kibel` must be available in `PATH`.
- If your environment uses a custom install location, set `KIBEL_BIN=/absolute/path/to/kibel`.
- Python runtime (`python3`) is required for skill-side JSON checks.
- `jq` is not required by official read-only skills.

## References

- `docs/agentic-rag-architecture.md`
