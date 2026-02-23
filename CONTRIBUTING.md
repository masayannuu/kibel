# Contributing

## Scope

`kibel` is a production-focused Kibela CLI and reusable Rust client library.
Contributions should prioritize deterministic behavior, stable JSON output, and schema-safe changes.

## Development setup

```bash
cargo build --workspace
cargo test --workspace
```

## Required checks before PR

```bash
cargo run -p kibel-tools -- create-note-contract check
cargo run -p kibel-tools -- resource-contract check
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build -p kibel
cargo package --locked -p kibel-client
```

## Schema-related changes

If Kibela schema changes affect `createNote` or resource contracts:

```bash
cargo run -p kibel-tools -- create-note-contract write
cargo run -p kibel-tools -- resource-contract write
```

Then commit updated snapshot + generated Rust files together.

## Pull request expectations

- Include behavior summary and affected command paths.
- Include tests for behavior changes (`unit` and/or `stub E2E`).
- Do not include secrets or real access tokens in any file or log.
