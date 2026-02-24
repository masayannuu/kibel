<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/logo-dark.svg">
    <img src="assets/logo.svg" width="420" height="100" alt="kibel logo">
  </picture>
</p>

# kibel

Rust CLI + client library for Kibela GraphQL.
Primary audience: coding agents and automation workflows that need deterministic, script-friendly Kibela access.

## What This Repo Provides

This repository contains three Rust packages:

- `kibel`: CLI for Kibela operations (search/read/create/update) with machine-readable JSON output.
- `kibel-client`: reusable Rust client library that powers the CLI and can be embedded in other apps.
- `kibel-tools`: schema contract maintenance utilities (snapshot/check/write).

## Why It Exists

Kibela operations are often needed in scripts, CI, and internal tooling.  
`kibel` focuses on:

- predictable automation behavior (stable JSON envelope + error code mapping),
- shared behavior between CLI and library,
- deterministic schema drift detection with committed snapshots.

## Quick Start (CLI)

### 1. Install

```bash
# install from source checkout
cargo install --path crates/kibel

# or build locally
cargo build --release -p kibel
./target/release/kibel --help
```

### 2. Set environment

```bash
export KIBELA_ORIGIN="https://my-team.kibe.la"
export KIBELA_TEAM="my-team"
export KIBELA_ACCESS_TOKEN="<your-token>"
```

### 3. Run commands

```bash
kibel --json auth status
kibel --json search note --query onboarding --first 16
kibel --json note get --id N1
```

## Auth And Config Behavior

Token resolution order is fixed:

1. `--with-token` (stdin)
2. `KIBELA_ACCESS_TOKEN` (or `--token-env`)
3. OS credential store
4. config file (`~/.config/kibel/config.toml`)

Origin and team resolution:

1. Team: `--team` / `KIBELA_TEAM` -> `config.default_team`
2. Origin: `--origin` / `KIBELA_ORIGIN` -> team profile origin

If origin cannot be resolved, commands fail with `INPUT_INVALID`.

## CLI Scope

Current command groups:

- `auth`, `config`
- `search`, `group`, `folder`, `feed`, `comment`, `note`
- `completion`, `version`

Use `kibel --help` and `kibel <group> --help` for full options.

## Library Usage (`kibel-client`)

```rust
use kibel_client::{KibelClient, SearchNoteInput};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = KibelClient::new(
        "https://my-team.kibe.la",
        std::env::var("KIBELA_ACCESS_TOKEN")?,
    )?;

    let note = client.get_note("N1")?;
    println!("note id: {}", note.id);

    let results = client.search_note(&SearchNoteInput {
        query: "onboarding".to_string(),
        resources: vec![],
        coediting: None,
        updated: None,
        group_ids: vec![],
        folder_ids: vec![],
        liker_ids: vec![],
        is_archived: None,
        sort_by: None,
        first: Some(16),
    })?;
    println!("results: {}", results);
    Ok(())
}
```

## Schema Lifecycle

Create-note contract:

- snapshot: `research/schema/create_note_contract.snapshot.json`
- check: `cargo run -p kibel-tools -- create-note-contract check`
- update generated module: `cargo run -p kibel-tools -- create-note-contract write`

All-resource contract:

- endpoint snapshot source: `research/schema/resource_contracts.endpoint.snapshot.json`
- normalized snapshot: `research/schema/resource_contracts.snapshot.json`
- check: `cargo run -p kibel-tools -- resource-contract check`
- update generated module: `cargo run -p kibel-tools -- resource-contract write`

## Development Quality Gates

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

## Project Docs

- `docs/implementation-policy.md`
- `docs/architecture.md`
- `docs/schema-lifecycle.md`
- `docs/maintenance.md`

## OSS Metadata

- `LICENSE`
- `CONTRIBUTING.md`
- `CODE_OF_CONDUCT.md`
- `SECURITY.md`
- `CHANGELOG.md`
