<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/logo-dark.svg">
    <img src="assets/logo.svg" width="420" height="100" alt="kibel logo">
  </picture>
</p>

# kibel

Language: 日本語 | [English](README.en.md)

[![CI](https://github.com/masayannuu/kibel/actions/workflows/ci.yml/badge.svg)](https://github.com/masayannuu/kibel/actions/workflows/ci.yml)
[![Release](https://github.com/masayannuu/kibel/actions/workflows/release.yml/badge.svg)](https://github.com/masayannuu/kibel/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

> Kibela Web API 向けの**非公式** CLI / クライアントライブラリです。
> 個人が開発・メンテナンスしており、Bit Journey, Inc. の公式プロダクトではありません。

Kibela GraphQL 用の Rust CLI + クライアントライブラリ。
コーディングエージェントや自動化ワークフローから、安定した出力でスクリプトしやすい Kibela アクセスを提供します。

## このリポジトリが提供するもの

本リポジトリは 3 つの Rust パッケージで構成されています。

- `kibel`: 検索/参照/作成/更新を行う CLI（機械可読 JSON 出力）
- `kibel-client`: CLI の中核となる再利用可能な Rust クライアントライブラリ
- `kibel-tools`: スキーマ契約メンテナンス用のユーティリティ（snapshot/check/write）

## モチベーション

Kibela の操作はスクリプトや CI、社内ツールで必要になることが多く、`kibel` は以下にフォーカスしています。

- 安定した自動化挙動（JSON エンベロープとエラーコードの一貫したマッピング）
- CLI とライブラリで実装を共有
- コミット済みスナップショットによるスキーマ変更の確実な検知

## 公式インタフェース

自動化で連携する際は、以下を安定した仕様としてお使いください。

- `docs/cli-interface.md`: 公式 CLI/API 契約（デフォルト JSON、エラー、終了コード、セーフティ境界）
- `docs/agent-skills.md`: 高精度検索/RAG の公式エージェントワークフロー

## クイックスタート（CLI）

### 1. インストール（推奨: GitHub Release バイナリ）

Linux (`x86_64` / `aarch64`) 例:

```bash
VERSION="vX.Y.Z"
ARCH_RAW="$(uname -m)"
case "${ARCH_RAW}" in
  x86_64) ARCH="x86_64" ;;
  aarch64|arm64) ARCH="aarch64" ;;
  *) echo "unsupported arch: ${ARCH_RAW}" >&2; exit 1 ;;
esac

ASSET="kibel-${VERSION}-linux-${ARCH}.tar.gz"
BASE_URL="https://github.com/masayannuu/kibel/releases/download/${VERSION}"

curl -fL -o "${ASSET}" "${BASE_URL}/${ASSET}"
curl -fL -o "${ASSET}.sha256" "${BASE_URL}/${ASSET}.sha256"
sha256sum -c "${ASSET}.sha256"
tar -xzf "${ASSET}"
sudo install -m 0755 kibel /usr/local/bin/kibel
kibel --version
```

### 2. Homebrew でインストール

```bash
brew install masayannuu/tap/kibel
```

補足:
- Homebrew 配布は `masayannuu/homebrew-tap` 経由で提供しています。
- リリースアセットの取得には public リポジトリが必要です。

### 3. ソースからのフォールバックインストール（Cargo）

```bash
# ソースチェックアウトからインストール
cargo install --path crates/kibel

# もしくはローカルビルド
cargo build --release -p kibel
./target/release/kibel --help
```

### 4. 環境変数の設定

```bash
export KIBELA_ORIGIN="https://my-team.kibe.la"
export KIBELA_TEAM="my-team"
# optional aliases:
export KIBELA_TENANT="my-team"
export KIBELA_TENANT_ORIGIN="https://my-team.kibe.la"
export KIBELA_ACCESS_TOKEN="<your-token>"
```

### 5. コマンド実行

```bash
kibel auth status
kibel search note --query onboarding --first 16
kibel search note --query onboarding --after <cursor> --first 16
kibel search user --query onboarding --first 10
kibel search note --query onboarding --save-preset onboarding
kibel search note --preset onboarding
kibel search note --mine --first 10
kibel note get --id N1
kibel note get-many --id N1 --id N2
kibel graphql run --query 'query Q($id: ID!) { note(id: $id) { id title } }' --variables '{"id":"N1"}'
```

`search note --mine` は自分の最新ノートを取得する専用コマンドです（他の検索フィルタとは併用できません）。
`search note --preset` / `--save-preset` で検索条件をローカル config に保存・再利用できます。

`graphql run` で mutation を実行するには `--allow-mutation` が必要です。実行できるのは trusted resource contract で許可された root field のみで、delete/member/org-setting 系はデフォルトでブロックされます。

## 公式 Agent Skills

本リポジトリは `skills/` 配下に公式スキルを同梱しています。

- `skills/kibel-agentic-search`
- `skills/kibel-agentic-rag`
- `skills/kibel-cli-operator`

インストール（Codex）:

```bash
./scripts/install_kibel_skills.sh
```

再現性重視の場合:

```bash
./scripts/install_kibel_skills.sh --ref v0.2.6
```

その後 Codex を再起動してください。

手動インストール（skill-installer のフォールバック）:

```bash
python "${CODEX_HOME:-$HOME/.codex}/skills/.system/skill-installer/scripts/install-skill-from-github.py" \
  --repo masayannuu/kibel \
  --path \
  skills/kibel-agentic-search \
  skills/kibel-agentic-rag \
  skills/kibel-cli-operator
```

Claude Code では同じ `SKILL.md` を実行プレイブックとして利用できます。
スキルはリリース済みバイナリの利用を前提としており、`kibel` が `PATH` に入っていることを想定しています（必要に応じて `KIBEL_BIN` で上書き可能）。

## 認証と設定の挙動

トークンの解決順序は以下の通りです（固定）。

1. `--with-token`（stdin）
2. `KIBELA_ACCESS_TOKEN`（または `--token-env`）
3. OS クレデンシャルストア
4. config file（`~/.config/kibel/config.toml`）

origin / team の解決順序:

1. Team: `--team`（alias: `--tenant`） / `KIBELA_TEAM`（alias: `KIBELA_TENANT`） -> `config.default_team`
2. Origin: `--origin` / `KIBELA_ORIGIN`（alias: `KIBELA_TENANT_ORIGIN`） -> team profile origin

`auth login` の補足:

- TTY 環境では、未入力のフィールド（origin/team/token）を対話的に入力できます。
- トークンは keychain にテナント・origin ごとに保存されます（`origin::<origin>::team::<team>`）。
- config profile にも token/origin を保存するため、サーバー環境でも keychain なしで実行できます。
- ログイン結果にトークン発行ページの URL を表示します: `<origin>/settings/access_tokens`（例: `https://example.kibe.la/settings/access_tokens`）

origin を解決できない場合は `INPUT_INVALID` エラーになります。

## CLI のスコープ

利用できるコマンドグループ:

- `auth`, `config`
- `search`, `group`, `folder`, `feed`, `comment`, `note`
- `graphql`（ガードレール付きの ad-hoc 実行）
- `completion`, `version`

詳細は `kibel --help` と `kibel <group> --help` を参照してください。

## できること・できないこと（現時点）

できること:

- ノート・コメント・フォルダ・フィードの検索・参照・作成・更新
- `graphql run` による ad-hoc クエリと、安全に制限された mutation

意図的に対象外としているもの:

- delete 系、メンバー追加/削除、組織/グループ設定の変更、権限ポリシー変更などの破壊的・管理者操作
- 上記の制限を回避する隠しオプション

`graphql run` のセーフティ境界:

- mutation には明示的な `--allow-mutation` が必要
- mutation root field は trusted resource-contract の許可リストに含まれている必要がある
- trusted query は persisted-hash GET を試行し、未対応時は POST にフォールバック
- `graphql run`（untrusted lane）は URL への payload 漏洩を避けるため POST のみ使用
- 現行リリースに `--dangerous` のようなオーバーライドは存在しない

create-note の runtime introspection:

- デフォルトは OFF
- 必要な場合のみ明示的に有効化: `KIBEL_ENABLE_RUNTIME_INTROSPECTION=1`

## ライブラリ利用（`kibel-client`）

```rust
use kibel_client::{KibelClient, SearchNoteInput};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = KibelClient::new(
        "https://my-team.kibe.la",
        std::env::var("KIBELA_ACCESS_TOKEN")?,
    )?;

    let note = client.get_note("N1")?;
    println!("note id: {}", note.id);

    let results = client.search_note(&SearchNoteInput {
        query: "onboarding".to_string(),
        resources: vec![],
        coediting: None,
        updated: None,
        group_ids: vec![],
        user_ids: vec![],
        folder_ids: vec![],
        liker_ids: vec![],
        is_archived: None,
        sort_by: None,
        first: Some(16),
        after: None,
    })?;
    println!("results: {}", results);
    Ok(())
}
```

## スキーマライフサイクル

create-note 契約:

- snapshot: `schema/contracts/create_note_contract.snapshot.json`
- endpoint snapshot からの refresh: `cargo run -p kibel-tools -- create-note-contract refresh-from-endpoint`
- check: `cargo run -p kibel-tools -- create-note-contract check`
- 生成モジュールの更新: `cargo run -p kibel-tools -- create-note-contract write`

全リソース契約:

- endpoint snapshot ソース: `schema/introspection/resource_contracts.endpoint.snapshot.json`
- 正規化 snapshot: `schema/contracts/resource_contracts.snapshot.json`
- endpoint snapshot の refresh: `cargo run -p kibel-tools -- resource-contract refresh-endpoint --origin "$KIBELA_ORIGIN"`
- check: `cargo run -p kibel-tools -- resource-contract check`
- 生成モジュールの更新: `cargo run -p kibel-tools -- resource-contract write`
- contract diff（blocking）: `cargo run -p kibel-tools -- resource-contract diff --base <old> --target schema/contracts/resource_contracts.snapshot.json --fail-on-breaking`
- contract diff（machine-readable）: `cargo run -p kibel-tools -- resource-contract diff --format json --base <old> --target schema/contracts/resource_contracts.snapshot.json`

補足:
- trusted operation の `document` は endpoint introspection snapshot から自動生成されます。

## 開発品質ゲート

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test -p kibel-client --doc
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

## プロジェクトドキュメント

- `docs/implementation-policy.md`
- `docs/cli-interface.md`
- `docs/agent-skills.md`
- `docs/agentic-rag-architecture.md`
- `docs/architecture.md`
- `docs/schema-lifecycle.md`
- `docs/maintenance.md`
- `skills/README.md`

## OSS メタデータ

- `LICENSE`
- `CONTRIBUTING.md`
- `CODE_OF_CONDUCT.md`
- `SECURITY.md`
- `CHANGELOG.md`
