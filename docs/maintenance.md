# Maintenance Guide

## Add a new resource

1. endpoint introspection snapshot を更新し、ローカル契約へ反映する。
   - `cargo run -p kibel-tools -- resource-contract refresh-endpoint --origin "$KIBELA_ORIGIN" --document-fallback-mode strict`
   - `cargo run -p kibel-tools -- create-note-contract refresh-from-endpoint`
2. `kibel-client` に入力構造体・実行メソッドを追加。
3. `kibel` に CLI サブコマンドを追加。
4. `--json` の出力形を固定化。
5. unit test + stub E2E を追加。
6. all-resource contract snapshot/codegen を同期。
   - `cargo run -p kibel-tools -- resource-contract write --document-fallback-mode strict`

## Regression checklist

- 既存 error code マッピングを壊していないか。
- auth precedence を壊していないか。
- 非互換な引数名変更をしていないか。
- token や機微情報が出力されていないか。

## Scheduled schema refresh

- Workflow: `.github/workflows/schema-refresh.yml`
- Required GitHub Secrets:
  - `KIBELA_ORIGIN`
  - `KIBELA_ACCESS_TOKEN` (read-only を推奨)
- Flow:
  - live introspection refresh (endpoint)
  - create-note snapshot refresh (from endpoint snapshot)
  - compatibility diff (blocking)
  - contract write/check
  - quality checks
  - diff があれば PR 自動作成

## Release checklist

1. `CHANGELOG.md` の Unreleased を更新する。
2. 全チェックコマンドを実行して成功させる。
   - `cargo fmt --all --check`
   - `cargo clippy --workspace --all-targets --all-features -- -D warnings`
   - `cargo test --workspace --all-features`
   - `cargo test -p kibel-client --doc`
   - `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`
   - `cargo run -p kibel-tools -- create-note-contract check`
   - `cargo run -p kibel-tools -- resource-contract check --document-fallback-mode strict`
3. package 検証を実行する。
   - `cargo package --locked -p kibel-client`
4. `README.md` と `docs/` を実装に合わせて更新する。
5. `v*.*.*` タグを push して release workflow を実行する。
6. GitHub Release に以下成果物が揃っていることを確認する。
   - `kibel-${VERSION}-linux-x86_64.tar.gz`
   - `kibel-${VERSION}-linux-aarch64.tar.gz`
   - `kibel-${VERSION}-darwin-x86_64.tar.gz`
   - `kibel-${VERSION}-darwin-aarch64.tar.gz`
   - 各 `*.sha256`
   - `kibel-${VERSION}-checksums.txt`
7. ダウンロード後の検証を実施する。
   - `sha256sum -c kibel-${VERSION}-linux-x86_64.tar.gz.sha256`
   - `sha256sum -c kibel-${VERSION}-linux-aarch64.tar.gz.sha256`
8. release workflow の provenance attestation ステップが成功していることを確認する。
9. Homebrew tap sync を使う場合、`homebrew-tap` job が成功していることを確認する。
10. crates.io 公開する場合は `kibel-client` を先に publish し、index反映後に `kibel` を publish する。
11. `kibel` publish 前に dry-run を行う。
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
