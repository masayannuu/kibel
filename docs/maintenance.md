# Maintenance Guide

## Add a new resource

1. endpoint introspection snapshot を更新し、ローカル契約に反映する。
   - `cargo run -p kibel-tools -- resource-contract refresh-endpoint --origin "$KIBELA_ORIGIN"`
   - `cargo run -p kibel-tools -- create-note-contract refresh-from-endpoint`
2. `kibel-client` に入力構造体と実行メソッドを追加する。
3. `kibel` に CLI サブコマンドを追加する。
4. デフォルト JSON 出力の envelope 形式を固定する（`--text` は人間向け表示専用）。
5. unit テストと stub E2E を追加する。
6. all-resource contract の snapshot / codegen を同期する。
   - `cargo run -p kibel-tools -- resource-contract write`

## Regression checklist

- 既存の error code マッピングを壊していないか。
- auth の優先順序を壊していないか。
- token や機微情報が出力に含まれていないか。

## Scheduled schema refresh

- Workflow: `.github/workflows/schema-refresh.yml`
- Required GitHub Secrets:
  - `KIBELA_ORIGIN`
  - `KIBELA_ACCESS_TOKEN` (read-only を推奨)
- Flow:
  - live introspection refresh (endpoint)
  - create-note snapshot refresh (from endpoint snapshot)
  - contract diff (blocking)
  - contract write/check
  - quality checks
  - diff があれば PR 自動作成

## Release checklist

1. `CHANGELOG.md` の Unreleased セクションを更新する。
2. 全チェックコマンドを実行し、すべて成功することを確認する。
   - `cargo fmt --all --check`
   - `cargo clippy --workspace --all-targets --all-features -- -D warnings`
   - `cargo test --workspace --all-features`
   - `cargo test -p kibel-client --doc`
   - `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`
   - `cargo run -p kibel-tools -- create-note-contract check`
   - `cargo run -p kibel-tools -- resource-contract check`
3. パッケージの検証を実行する。
   - `cargo package --locked -p kibel-client`
4. `README.md` と `docs/` を実装に合わせて更新する。
5. `v*.*.*` タグを push して release workflow を実行する。
6. GitHub Release に以下の成果物が揃っていることを確認する。
   - `kibel-${VERSION}-linux-x86_64.tar.gz`
   - `kibel-${VERSION}-linux-aarch64.tar.gz`
   - `kibel-${VERSION}-darwin-x86_64.tar.gz`
   - `kibel-${VERSION}-darwin-aarch64.tar.gz`
   - 各 `*.sha256`
   - `kibel-${VERSION}-checksums.txt`
7. ダウンロードしたアセットを検証する。
   - `sha256sum -c kibel-${VERSION}-linux-x86_64.tar.gz.sha256`
   - `sha256sum -c kibel-${VERSION}-linux-aarch64.tar.gz.sha256`
8. release workflow の provenance attestation ステップが成功していることを確認する。
9. Homebrew tap sync を使う場合は `homebrew-tap` job が成功していることを確認する。
10. crates.io に公開する場合は `kibel-client` を先に publish し、index に反映されてから `kibel` を publish する。
11. `kibel` の publish 前に dry-run を行う。
   - `cargo publish --dry-run -p kibel`

## Homebrew tap sync configuration

- Workflow: `.github/workflows/release.yml` (`homebrew-tap` job)
- Required GitHub Secrets:
  - `HOMEBREW_TAP_REPO` (example: `masayannuu/homebrew-tap`)
  - `HOMEBREW_TAP_TOKEN` (contents:write on tap repo)
- Formula renderer:
  - `scripts/render_homebrew_formula.sh`

## Quality observation workflow

- Workflow: `.github/workflows/quality-observe.yml`
- Purpose:
  - `cargo-nextest`, `cargo-deny`, `cargo-semver-checks` の観測を non-blocking で収集する。
  - merge gate を壊さずに品質シグナルを継続監視する。
