# kibel

Rust CLI + client library for Kibela GraphQL.

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

### 2. Fallback install from source (Cargo)

```bash
# install from source checkout
cargo install --path crates/kibel

# or build locally
cargo build --release -p kibel
./target/release/kibel --help
```

### 3. Set environment

```bash
export KIBELA_ORIGIN="https://my-team.kibe.la"
export KIBELA_TEAM="my-team"
export KIBELA_ACCESS_TOKEN="<your-token>"
```

### 4. Run commands

```bash
kibel --json auth status
kibel --json search note --query onboarding --first 16
kibel --json note get --id N1
kibel --json graphql run --query 'query Q($id: ID!) { note(id: $id) { id title } }' --variables '{"id":"N1"}'
```

`graphql run` の mutation は `--allow-mutation` が必要で、さらに trusted resource contract で許可された root field のみ実行できます（delete/member/org-setting 系は既定で拒否）。

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
