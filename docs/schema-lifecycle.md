# Schema Lifecycle

## Versioning model

- create-note contract snapshot: `research/schema/create_note_contract.snapshot.json`
- endpoint introspection snapshot: `research/schema/resource_contracts.endpoint.snapshot.json`
- all-resource contract snapshot: `research/schema/resource_contracts.snapshot.json`
- generated modules:
  - `crates/kibel-client/src/generated_create_note_contract.rs`
  - `crates/kibel-client/src/generated_resource_contracts.rs`
    - includes trusted operation documents (`document`) used by `kibel-client`

## Update procedure

1. endpoint introspection snapshot を更新する（live source から自動取得）。
2. `createNote` 契約の変化がある場合は `create-note-contract` の snapshot/codegen を同期する。
3. all-resource 契約の snapshot/codegen を同期する。
   - trusted operation document も同時に更新される（endpoint snapshot起点）。
4. unit/E2E を実行する。
5. CI 通過を確認してマージする。

## Scheduled refresh

- GitHub Actions `schema-refresh` workflow が定期実行される。
- 実行内容:
  - live endpoint から `create-note` snapshot refresh
  - live endpoint から endpoint introspection refresh
  - contract codegen/write
  - contract checks + workspace checks
  - 差分があれば PR 作成

## Command checklist

```bash
# refresh create-note snapshot from live GraphQL
cargo run -p kibel-tools -- create-note-contract refresh-snapshot \
  --origin "$KIBELA_ORIGIN"

# refresh endpoint snapshot from live GraphQL
cargo run -p kibel-tools -- resource-contract refresh-endpoint \
  --origin "$KIBELA_ORIGIN"

# refresh contract snapshot/module from committed endpoint snapshot
cargo run -p kibel-tools -- resource-contract write

# deterministic checks
cargo run -p kibel-tools -- create-note-contract check
cargo run -p kibel-tools -- resource-contract check
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test -p kibel-client --doc
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

## Drift policy

- 生成物 stale は即失敗（手動修正禁止、必ず generator を使う）。
- endpoint snapshot 更新差分はレビュー対象にする。
- schema 互換性が崩れる場合は:
  - 互換パスを先に実装
  - 破壊変更は CLI 影響を明示して段階移行
