# Implementation Policy

## Goals

- Kibela GraphQL を安全に扱える実運用 CLI を維持する。
- `kibel-client` を再利用可能な Rust ライブラリとして保つ。
- 仕様変更時の破壊を CI で早期検知する。

## Non-negotiable invariants

- 認証優先順位は固定: stdin (`--with-token`) > env (`KIBELA_ACCESS_TOKEN`) > keychain > config。
- すべての失敗は `--json` モードで機械判読可能な `error.code` に正規化する。
- all-resource 契約は endpoint introspection snapshot を一次ソースにする。
- 新規リソース追加時は `unit + stub E2E` を同時に追加する。
- all-resource E2E は契約snapshot起点の動的GraphQL stub server検証を維持する。
- free-query 系コマンドを導入する場合も `--json` 失敗時の `error.code` 正規化を必須とする。
- free-query 系コマンドは timeout/response size/depth-cost などの実行境界を必須で持つ。
- trusted query transport は persisted-hash GET + safe POST fallback を採用し、untrusted lane は POST維持で運用する。
- CLI 機能スコープは明示的に制限し、破壊的/管理者系操作（delete、member add/remove、organization/group setting 変更）は現時点で提供しない。
- 破壊的/管理者系操作向けの `--dangerous` などのバイパスフラグは現リリースで提供しない。

## 2026 lifecycle policy

- Schema evolution 前提で運用し、固定バージョン API 前提にはしない。
- 変更検知は CI で fail-fast: contract 検証、生成物の stale 検証を必須化する。
- 変更検知の主経路は CI: endpoint snapshot 起点の契約再現性を保証する。
- scheduled schema refresh workflow を維持し、差分はPRレビューで管理する。
- trusted operation 実行モデルを優先し、ad-hoc 実行経路は明示的に分離する。
- 互換性方針:
  - CLI 破壊的変更は避ける。
  - 互換 alias は明示管理し、移行完了後に削除計画を立てる。

## Security posture

- トークンは stdout に出力しない。
- テスト専用環境変数による transport override は実行モードで明確に分離する。
- 破壊的操作系 (`create*`, `move*`, `attach*`, `update*`) は、endpoint が提供する idempotency/precondition 入力を優先して公開し、request shape テストで維持する。
- `graphql run` の mutation は `--allow-mutation` かつ trusted contract allowlist を満たすものだけ許可し、allowlist 外は通信前に拒否する。
- runtime introspection は default OFF とし、必要時のみ明示envで有効化する。
