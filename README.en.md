<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/logo-dark.svg">
    <img src="assets/logo.svg" width="420" height="100" alt="kibel logo">
  </picture>
</p>

# kibel

Language: [Japanese](README.md) | English

[![CI](https://github.com/masayannuu/kibel/actions/workflows/ci.yml/badge.svg)](https://github.com/masayannuu/kibel/actions/workflows/ci.yml)
[![Release](https://github.com/masayannuu/kibel/actions/workflows/release.yml/badge.svg)](https://github.com/masayannuu/kibel/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

> Community-maintained **unofficial** CLI/client for Kibela Web API.
> Not affiliated with or endorsed by Bit Journey, Inc.

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

## Official Interfaces

Treat the following as canonical for automation integrations:

- `docs/cli-interface.md`: official CLI/API contract (default JSON, errors, exit codes, safety boundary).
- `docs/agent-skills.md`: official agent workflows for high-precision retrieval and RAG.

## Quick Start (CLI)

### 1. Install (recommended: GitHub Release binary)

Linux (`x86_64` / `aarch64`) example:

```bash
VERSION="vX.Y.Z"
ARCH_RAW="$(uname -m)"
case "${ARCH_RAW}" in
  x86_64) ARCH="x86_64" ;;
  aarch64|arm64) ARCH="aarch64" ;;
  *) echo "unsupported arch: ${ARCH_RAW}" >&2; exit 1 ;;
esac

ASSET="kibel-${VERSION}-linux-${ARCH}.tar.gz"
BASE_URL="https://github.com/masayannuu/kibel/releases/download/${VERSION}"

curl -fL -o "${ASSET}" "${BASE_URL}/${ASSET}"
curl -fL -o "${ASSET}.sha256" "${BASE_URL}/${ASSET}.sha256"
sha256sum -c "${ASSET}.sha256"
tar -xzf "${ASSET}"
sudo install -m 0755 kibel /usr/local/bin/kibel
kibel --version
```

### 2. Install via Homebrew

```bash
brew install masayannuu/tap/kibel
```

Note:
- Homebrew distribution is provided via `masayannuu/homebrew-tap`.
- Public repo visibility is required for unauthenticated users to fetch release assets.

### 3. Fallback install from source (Cargo)

```bash
# install from source checkout
cargo install --path crates/kibel

# or build locally
cargo build --release -p kibel
./target/release/kibel --help
```

### 4. Set environment

```bash
export KIBELA_ORIGIN="https://my-team.kibe.la"
export KIBELA_TEAM="my-team"
# optional aliases:
export KIBELA_TENANT="my-team"
export KIBELA_TENANT_ORIGIN="https://my-team.kibe.la"
export KIBELA_ACCESS_TOKEN="<your-token>"
```

### 5. Run commands

```bash
kibel auth status
kibel search note --query onboarding --first 16
kibel search note --query onboarding --after <cursor> --first 16
kibel search user --query onboarding --first 10
kibel search note --query onboarding --save-preset onboarding
kibel search note --preset onboarding
kibel search note --mine --first 10
kibel note get --id N1
kibel note get-many --id N1 --id N2
kibel graphql run --query 'query Q($id: ID!) { note(id: $id) { id title } }' --variables '{"id":"N1"}'
```

`search note --mine` is dedicated to the current user's latest notes only (cannot be combined with other search filters).
`search note --preset` / `--save-preset` stores and reuses search settings in local config.

`graphql run` mutations require `--allow-mutation`, and only trusted resource-contract allowlisted root fields are permitted (delete/member/org-setting roots are blocked by default).

## Official Agent Skills

This repo ships official skills under `skills/`:

- `skills/kibel-agentic-search`
- `skills/kibel-agentic-rag`
- `skills/kibel-cli-operator`

Install (Codex):

```bash
./scripts/install_kibel_skills.sh
```

Recommended for reproducibility:

```bash
./scripts/install_kibel_skills.sh --ref v0.2.6
```

Then restart Codex.

Fallback (manual skill-installer):

```bash
python "${CODEX_HOME:-$HOME/.codex}/skills/.system/skill-installer/scripts/install-skill-from-github.py" \
  --repo masayannuu/kibel \
  --path \
  skills/kibel-agentic-search \
  skills/kibel-agentic-rag \
  skills/kibel-cli-operator
```

For Claude Code, use the same `SKILL.md` files directly as execution playbooks.
Skills are distribution-first: they assume `kibel` is installed in `PATH` (or override with `KIBEL_BIN`).

## Auth And Config Behavior

Token resolution order is fixed:

1. `--with-token` (stdin)
2. `KIBELA_ACCESS_TOKEN` (or `--token-env`)
3. OS credential store
4. config file (`~/.config/kibel/config.toml`)

Origin and team resolution:

1. Team: `--team` (alias: `--tenant`) / `KIBELA_TEAM` (alias: `KIBELA_TENANT`) -> `config.default_team`
2. Origin: `--origin` / `KIBELA_ORIGIN` (alias: `KIBELA_TENANT_ORIGIN`) -> team profile origin

`auth login` notes:

- Missing fields are prompted interactively on TTY (origin/team/token).
- Token storage is tenant-origin aware in keychain (`origin::<origin>::team::<team>` subject).
- Config profile also stores token/origin so server environments can run without keychain.
- Token settings URL shown in login result: `<origin>/settings/access_tokens` (example: `https://example.kibe.la/settings/access_tokens`)

If origin cannot be resolved, commands fail with `INPUT_INVALID`.

## CLI Scope

Current command groups:

- `auth`, `config`
- `search`, `group`, `folder`, `feed`, `comment`, `note`
- `graphql` (ad-hoc execution with guardrails)
- `completion`, `version`

Use `kibel --help` and `kibel <group> --help` for full options.

## Supported vs Not Supported (Current)

Supported:

- automation-friendly operations: search/read/create/update around notes, comments, folders, and feeds.
- `graphql run` for ad-hoc queries and bounded/safe mutations.

Not supported (intentional):

- destructive/admin operations such as delete flows, member add/remove, organization/group setting rewrites, and permission policy rewrites.
- any hidden bypass path for these operations.

`graphql run` safety boundary:

- mutation requires explicit `--allow-mutation`.
- mutation root field must be in trusted resource-contract allowlist.
- trusted query commands use GET + persisted-hash negotiation with safe POST fallback.
- `graphql run` (untrusted lane) stays POST-only to avoid URL leakage of ad-hoc payloads.
- no `--dangerous` override exists in current release.

Create-note runtime introspection policy:

- default is OFF in production path.
- enable explicitly only when needed: `KIBEL_ENABLE_RUNTIME_INTROSPECTION=1`.

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
        user_ids: vec![],
        folder_ids: vec![],
        liker_ids: vec![],
        is_archived: None,
        sort_by: None,
        first: Some(16),
        after: None,
    })?;
    println!("results: {}", results);
    Ok(())
}
```

## Schema Lifecycle

Create-note contract:

- snapshot: `schema/contracts/create_note_contract.snapshot.json`
- refresh from endpoint snapshot: `cargo run -p kibel-tools -- create-note-contract refresh-from-endpoint`
- check: `cargo run -p kibel-tools -- create-note-contract check`
- update generated module: `cargo run -p kibel-tools -- create-note-contract write`

All-resource contract:

- endpoint snapshot source: `schema/introspection/resource_contracts.endpoint.snapshot.json`
- normalized snapshot: `schema/contracts/resource_contracts.snapshot.json`
- refresh endpoint snapshot: `cargo run -p kibel-tools -- resource-contract refresh-endpoint --origin "$KIBELA_ORIGIN"`
- check: `cargo run -p kibel-tools -- resource-contract check`
- update generated module: `cargo run -p kibel-tools -- resource-contract write`
- contract diff (blocking): `cargo run -p kibel-tools -- resource-contract diff --base <old> --target schema/contracts/resource_contracts.snapshot.json --fail-on-breaking`
- contract diff (machine-readable): `cargo run -p kibel-tools -- resource-contract diff --format json --base <old> --target schema/contracts/resource_contracts.snapshot.json`

Notes:
- trusted operation `document` entries are auto-generated from the endpoint introspection snapshot.

## Development Quality Gates

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test -p kibel-client --doc
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

## Project Docs

- `docs/implementation-policy.md`
- `docs/cli-interface.md`
- `docs/agent-skills.md`
- `docs/agentic-rag-architecture.md`
- `docs/architecture.md`
- `docs/schema-lifecycle.md`
- `docs/maintenance.md`
- `skills/README.md`

## OSS Metadata

- `LICENSE`
- `CONTRIBUTING.md`
- `CODE_OF_CONDUCT.md`
- `SECURITY.md`
- `CHANGELOG.md`
