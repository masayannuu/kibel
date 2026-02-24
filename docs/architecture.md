# Architecture

## Workspace split

- `crates/kibel-client`
  - Kibela GraphQL クライアント
  - 認証トークン解決・設定ロード・エラー正規化の基盤
- `crates/kibel`
  - CLI surface
  - 引数解釈、実行分岐、JSON envelope の生成
- `crates/kibel-tools`
  - schema/contract snapshot と generated module の保守用 CLI

## Resource model

本CLIは Kibela GraphQL endpoint から得た17リソース契約に対応する。

- Query resources
  - `searchNote`, `searchFolder`
  - `getGroups`, `getFolders`, `getNotes`, `getNote`, `getNoteFromPath`, `getFolder`, `getFolderFromPath`, `getFeedSections`
- Command resources
  - `createNote`, `createComment`, `createCommentReply`, `createFolder`, `moveNoteToAnotherFolder`, `attachNoteToFolder`, `updateNoteContent`

## Execution flow

1. `kibel` が CLI 引数を parse。
2. 認証・origin/team 解決。
3. `kibel-client` が GraphQL query/mutation を送信。
4. GraphQL error/extensions.code を CLI error code へ正規化。
5. JSON envelope (`ok/data/error/meta`) で返却。

## Design principles

- クエリ定義は Kibela GraphQL endpoint 契約に寄せる（引数名・フィールド名を揃える）。
- デフォルト件数 (`first`) は既存CLI互換の既定値を使用する。
- モデルは「必要十分」: CLIで利用するフィールドだけを厳選し、過剰な静的型を避ける。
- 契約管理は endpoint snapshot 起点の単一路線:
  - CI基準: endpoint introspection snapshot から作る snapshot/codegen
  - refresh: `kibel-tools resource-contract refresh-endpoint`
- built-in 操作は generated trusted operation registry を経由して実行前検証する。
- `graphql run` は trusted registry外の明示経路として、guardrail 適用後に実行する。
- 仕様差異がある場合は runtime introspection か fallback contract で吸収する（現在 createNote）。
