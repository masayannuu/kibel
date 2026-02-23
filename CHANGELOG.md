# Changelog

All notable changes to this project are documented in this file.

## [Unreleased]

### Added

- OSS metadata docs (`LICENSE`, `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`).
- CLI help-surface regression test (`crates/kibel/tests/help_surface.rs`).

### Changed

- Documentation and plans were consolidated to endpoint-first Rust operations.

### Removed

- Legacy upstream/parity research assets not used by current workflows.

## [0.1.0] - 2026-02-23

### Added

- Rust workspace: `kibel`, `kibel-client`, `kibel-tools`.
- 17 Kibela resource command/query support.
- Deterministic schema contract checks in CI.
- Stub E2E and unit tests for auth/error/contract behavior.
