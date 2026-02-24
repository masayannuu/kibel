# Maintenance Guide

## Add a new resource

1. endpoint introspection snapshot を更新し、ローカル契約へ反映する。
   - `cargo run -p kibel-tools -- resource-contract refresh-endpoint --origin "$KIBELA_ORIGIN" --token "$KIBELA_ACCESS_TOKEN"`
2. `kibel-client` に入力構造体・実行メソッドを追加。
3. `kibel` に CLI サブコマンドを追加。
4. `--json` の出力形を固定化。
5. unit test + stub E2E を追加。
6. all-resource contract snapshot/codegen を同期。
   - `cargo run -p kibel-tools -- resource-contract write`

## Regression checklist

- 既存 error code マッピングを壊していないか。
- auth precedence を壊していないか。
- 非互換な引数名変更をしていないか。
- token や機微情報が出力されていないか。

## Release checklist

1. `CHANGELOG.md` の Unreleased を更新する。
2. 全チェックコマンドを実行して成功させる。
3. package 検証を実行する。
   - `cargo package --locked -p kibel-client`
4. `README.md` と `docs/` を実装に合わせて更新する。
5. `v*.*.*` タグを push して release workflow の成果物（tar.gz + sha256）を確認する。
6. crates.io 公開する場合は `kibel-client` を先に publish し、index反映後に `kibel` を publish する。
7. `kibel` publish 前に dry-run を行う。
   - `cargo publish --dry-run -p kibel`
