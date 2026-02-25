# Schema Lifecycle

## Versioning model

- create-note contract snapshot: `schema/contracts/create_note_contract.snapshot.json`
- endpoint introspection snapshot: `schema/introspection/resource_contracts.endpoint.snapshot.json`
- all-resource contract snapshot: `schema/contracts/resource_contracts.snapshot.json`
- generated modules:
  - `crates/kibel-client/src/generated_create_note_contract.rs`
  - `crates/kibel-client/src/generated_resource_contracts.rs`
    - includes trusted operation documents (`document`) used by `kibel-client`

## Update procedure

1. endpoint introspection snapshot を更新する（live source から自動取得）。
2. endpoint snapshot から `createNote` 契約 snapshot を同期する（単一キャプチャ源）。
3. all-resource 契約の snapshot/codegen を同期する。
   - trusted operation document も同時に更新される（endpoint snapshot起点）。
   - 通常運用は strict mode（document fallback 無効）で実行する。
4. unit/E2E を実行する。
5. CI 通過を確認してマージする。

## Scheduled refresh

- GitHub Actions `schema-refresh` workflow が定期実行される。
- 実行内容:
  - live endpoint から endpoint introspection refresh
  - endpoint snapshot から `create-note` snapshot refresh
  - contract codegen/write
  - resource contract compatibility diff (blocking)
  - contract checks + workspace checks
  - 差分があれば PR 作成

## Command checklist

```bash
# refresh endpoint snapshot from live GraphQL
cargo run -p kibel-tools -- resource-contract refresh-endpoint \
  --origin "$KIBELA_ORIGIN" \
  --document-fallback-mode strict

# refresh create-note snapshot from endpoint snapshot
cargo run -p kibel-tools -- create-note-contract refresh-from-endpoint

# refresh contract snapshot/module from committed endpoint snapshot
cargo run -p kibel-tools -- resource-contract write --document-fallback-mode strict

# compatibility diff (blocking in CI)
cargo run -p kibel-tools -- resource-contract diff \
  --base /tmp/base-resource-contracts.snapshot.json \
  --target schema/contracts/resource_contracts.snapshot.json \
  --fail-on-breaking

# compatibility diff (machine-readable for automation)
cargo run -p kibel-tools -- resource-contract diff \
  --format json \
  --base /tmp/base-resource-contracts.snapshot.json \
  --target schema/contracts/resource_contracts.snapshot.json

# deterministic checks
cargo run -p kibel-tools -- create-note-contract check
cargo run -p kibel-tools -- resource-contract check --document-fallback-mode strict
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test -p kibel-client --doc
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

Breakglass only:

```bash
# temporary fallback for historical snapshots without `document`
cargo run -p kibel-tools -- resource-contract check --document-fallback-mode breakglass
```

## Drift policy

- 生成物 stale は即失敗（手動修正禁止、必ず generator を使う）。
- endpoint snapshot 更新差分はレビュー対象にする。
- schema 互換性が崩れる場合は:
  - 互換パスを先に実装
  - 破壊変更は CLI 影響を明示して段階移行
