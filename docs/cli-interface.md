# CLI Interface Specification

## Purpose

This document defines the official interfaces of `kibel` as an automation-grade CLI.
The goal is to keep agent/tool integrations stable while allowing additive evolution.

## Interface Layers

`kibel` has four official interface layers.

1. Command interface: command groups, subcommands, and options.
2. Machine interface: `--json` envelope, `error.code`, and process exit code.
3. Config/auth interface: token/origin/team resolution behavior.
4. Safety interface: operation allowlist and explicit unsupported scope.

## Stability Policy

- Stable:
  - Existing command paths documented in this file.
  - Existing `error.code` values and exit code mapping.
  - Existing auth precedence.
- Additive-compatible:
  - New commands, new optional flags, and new response fields.
- Breaking (major-only):
  - Command path rename/removal.
  - `error.code` rename/removal.
  - Exit code mapping change.
  - Auth precedence change.

## Official Command Surface

Query/read:

- `search note`
- `search folder`
- `search user`
- `group list`
- `folder list`
- `folder get`
- `folder get-from-path`
- `folder notes`
- `feed sections`
- `note get`
- `note get-many`
- `note get-from-path`
- `auth status`
- `config profiles`

Write/update (non-destructive operational commands):

- `note create`
- `note update`
- `note move-to-folder`
- `note attach-to-folder`
- `comment create`
- `comment reply`
- `folder create`
- `auth login`
- `config set team`

Ad-hoc lane:

- `graphql run` (guardrailed)

## Search Interface Contract

### `search note`

- `--query` is optional.
- If `--resource` is omitted, default resource is `NOTE`.
- `--after` can be used for forward cursor pagination.
- `--resource` supports:
  - `NOTE`
  - `COMMENT`
  - `ATTACHMENT`
- `--user-id` is repeatable and maps to GraphQL `userIds`.
- `--preset <name>` loads saved search defaults from config.
- `--save-preset <name>` stores the effective search filters to config.
- `--mine` is a dedicated mode for latest notes by current user.
  - `--mine` cannot be combined with other search filters.
  - returns the current user's latest notes ordered by recency.

### `search user`

- `search note` 結果の `author` を集約して user discovery を行う補助コマンド。
- `id`, `account`, `real_name`, `match_count` を返す。
- `--group-id` / `--folder-id` で探索範囲を絞り込める。

### `search folder`

- `--query` is required.

## JSON Envelope Contract

All command groups support `--json` and return:

Success:

```json
{
  "ok": true,
  "data": {},
  "error": null,
  "meta": {
    "request_id": "req-xxxxxxxx",
    "elapsed_ms": 123
  }
}
```

Failure:

```json
{
  "ok": false,
  "data": null,
  "error": {
    "code": "INPUT_INVALID",
    "message": "human-readable message",
    "retryable": false,
    "details": {}
  },
  "meta": {
    "request_id": "req-xxxxxxxx",
    "elapsed_ms": 123
  }
}
```

## Error Code and Exit Code Contract

| `error.code` | exit code | retryable |
| --- | --- | --- |
| `INPUT_INVALID` | 2 | false |
| `AUTH_FAILED` | 3 | false |
| `NOT_FOUND` | 4 | false |
| `PRECONDITION_FAILED` | 5 | false |
| `IDEMPOTENCY_CONFLICT` | 5 | false |
| `THROTTLED_RETRYABLE` | 6 | true |
| `TRANSPORT_ERROR` | 6 | true |
| `THROTTLED_REWRITE_REQUIRED` | 7 | false |
| `UNKNOWN_ERROR` | 10 | false |

## Config/Auth Contract

Token resolution order is fixed:

1. stdin token (`--with-token`)
2. env token (`KIBELA_ACCESS_TOKEN` or `--token-env`)
3. OS credential store
4. config fallback (`~/.config/kibel/config.toml`)

`auth login` persistence contract:

- preferred: OS credential store (tenant-origin subject)
- also persisted to config profile for server/non-keychain environments
- keychain backend failure does not block config persistence
- `search note --save-preset` stores preset filters in config (`search_note_presets`).

Origin/team resolution:

1. Team: `--team` (alias: `--tenant`) / `KIBELA_TEAM` (alias: `KIBELA_TENANT`) then config default team.
2. Origin: `--origin` / `KIBELA_ORIGIN` (alias: `KIBELA_TENANT_ORIGIN`) then team profile origin.

`auth login` interactive fallback (TTY only):

- prompts for missing `origin`, `team`, `token`
- reports Kibela token settings URL (`<origin>/settings/access_tokens`)

## Safety Contract

### Explicitly unsupported in official command surface

- Delete operations
- Member add/remove operations
- Organization/group policy rewrite operations
- Permission model rewrite operations

### `graphql run` boundary

- mutation requires `--allow-mutation`.
- mutation root must be in trusted allowlist.
- no dangerous bypass flag.
- untrusted lane remains POST-only.

Internal bootstrap lane (not public API):

- `search note --mine` uses an internal read-only `currentUser.latestNotes` query path.
- internal lane rejects mutation and root-field mismatch before transport execution.

## Non-goals (not part of official interface)

- internal snapshot file layout details.
- internal generated Rust module shape.
- undocumented ad-hoc behavior in non-JSON mode.
