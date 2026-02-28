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

1. endpoint introspection snapshot を更新する（live endpoint から自動取得）。
2. endpoint snapshot から `createNote` 契約 snapshot を同期する（単一のソース）。
3. all-resource 契約の snapshot / codegen を同期する。
   - trusted operation document も同時に更新される（endpoint snapshot 起点）。
4. unit / E2E テストを実行する。
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
  --origin "$KIBELA_ORIGIN"

# refresh create-note snapshot from endpoint snapshot
cargo run -p kibel-tools -- create-note-contract refresh-from-endpoint

# refresh contract snapshot/module from committed endpoint snapshot
cargo run -p kibel-tools -- resource-contract write

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
cargo run -p kibel-tools -- resource-contract check
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test -p kibel-client --doc
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

## Drift policy

- 生成物が古くなっていれば即座に失敗させる（手動修正は禁止、必ず generator 経由で更新する）。
- endpoint snapshot の更新差分はレビュー対象とする。
- schema の互換性が崩れた場合:
  - 互換パスは設けず、差分を明示したうえで本流を直接更新する
