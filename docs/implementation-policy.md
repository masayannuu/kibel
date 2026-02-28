# Implementation Policy

## Goals

- Kibela GraphQL を安全に扱える実運用レベルの CLI を維持する。
- `kibel-client` を再利用可能な Rust ライブラリとして保つ。
- API の仕様変更を CI で早期に検知し、互換レイヤーを残さずに対応する。

## Non-negotiable invariants

- 認証優先順位は固定: stdin (`--with-token`) > env (`KIBELA_ACCESS_TOKEN`) > keychain > config。
- すべての失敗はデフォルト JSON 出力で機械判読可能な `error.code` に正規化する（`--text` は人間向け表示専用）。
- 公式 CLI I/F 仕様は `docs/cli-interface.md` を一次ソースとして維持する。
- all-resource 契約は endpoint introspection snapshot を一次ソースとする。
- 新規リソース追加時は unit テストと stub E2E を同時に追加する。
- all-resource E2E は契約 snapshot 起点の動的 GraphQL stub server 検証を維持する。
- free-query 系コマンドを導入する場合も JSON 失敗時の `error.code` 正規化を必須とする。
- free-query 系コマンドは timeout / response size / depth-cost などの実行境界を必ず持つ。
- trusted query transport は persisted-hash GET + POST フォールバックを採用し、untrusted lane は POST のみで運用する。
- CLI の機能スコープは明示的に制限し、破壊的・管理者系操作（delete、member add/remove、organization/group setting 変更）は現時点で提供しない。
- 破壊的・管理者系操作用の `--dangerous` などのバイパスフラグは現リリースで提供しない。

## 2026 lifecycle policy

- Schema evolution を前提に運用し、固定バージョン API には依存しない。
- 変更検知は CI で fail-fast: contract 検証と生成物の stale 検証を必須とする。
- 変更検知の主経路は CI: endpoint snapshot 起点で契約の再現性を保証する。
- scheduled schema refresh workflow を維持し、差分は PR レビューで管理する。
- trusted operation の実行モデルを優先し、ad-hoc 実行経路は明示的に分離する。
- プレリリースフェーズでは互換 alias や legacy fallback を設けない。
- 破壊的変更は即時に本流へ反映し、契約差分の検知で回帰を防ぐ。

## Security posture

- トークンは stdout に出力しない。
- テスト専用環境変数による transport override は実行モードで明確に分離する。
- 書き込み系操作 (`create*`, `move*`, `attach*`, `update*`) では、endpoint が提供する idempotency / precondition 入力を優先して公開し、request shape テストで維持する。
- `graphql run` の mutation は `--allow-mutation` かつ trusted contract の許可リストを満たすものだけ許可し、リスト外は通信前に拒否する。
- runtime introspection はデフォルト OFF とし、必要時のみ環境変数で明示的に有効化する。
