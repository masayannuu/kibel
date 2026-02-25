# AGENTS Guide for kibel

## Purpose

Build and maintain a production-grade Kibela CLI + reusable Rust client library.

## Source of truth

- README.md
- docs/implementation-policy.md
- docs/cli-interface.md
- docs/agent-skills.md
- docs/architecture.md
- docs/schema-lifecycle.md
- docs/maintenance.md

Schema contracts:

- research/schema/create_note_contract.snapshot.json
- research/schema/resource_contracts.endpoint.snapshot.json
- research/schema/resource_contracts.snapshot.json

Endpoint snapshot refresh:

- `cargo run -p kibel-tools -- resource-contract refresh-endpoint --origin "$KIBELA_ORIGIN" --token "$KIBELA_ACCESS_TOKEN"`

Primary schema source:

- `research/schema/resource_contracts.endpoint.snapshot.json` (derived from Kibela GraphQL endpoint introspection via `kibel-tools`)

Secondary external reference for behavior consistency:

- https://github.com/kibela/kibela-mcp-server/blob/main/src/lib/kibela.ts
- https://github.com/kibela/kibela-mcp-server/blob/main/src/lib/schemas.ts

## Execution loop

1. Open / update an exec plan in `.agent/execplans/` before non-trivial edits.
2. Keep diffs small and test continuously.
3. Prefer deterministic checks over manual claims.
4. Record unknowns and next action at the end of each work cycle.

## Repository targets

- Rust workspace split:
  - `crates/kibel-client` (library)
  - `crates/kibel` (CLI binary)
  - `crates/kibel-tools` (contract maintenance CLI)
- Contract source:
  - create-note: `research/schema/create_note_contract.snapshot.json`
  - all-resource endpoint snapshot: `research/schema/resource_contracts.endpoint.snapshot.json`
  - all-resource derived snapshot: `research/schema/resource_contracts.snapshot.json`
- Auth precedence must remain:
  1) stdin token (`--with-token`)
  2) env (`KIBELA_ACCESS_TOKEN`)
  3) OS credential store
  4) local config fallback

## Quality gates

- Unit tests for client and CLI argument parsing
- Contract tests against pinned API assumptions
- Clear machine-readable error codes (`--json` mode)
- Completion + version command coverage
- Mandatory verification commands:
  - `cargo run -p kibel-tools -- create-note-contract check`
  - `cargo run -p kibel-tools -- resource-contract check`
  - `cargo fmt --all --check`
  - `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  - `cargo test --workspace --all-features`
  - `cargo test -p kibel-client --doc`
  - `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`
  - `cargo package --locked -p kibel-client`
