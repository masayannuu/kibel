# Command Map (kibel-cli-operator)

## Auth/config

- `kibel auth status`
- `kibel auth login --origin https://<team>.kibe.la --team <team> --with-token`
- `kibel config profiles`
- `kibel config set team <team>`

Canonical selectors:

- auth status: `.data.logged_in`, `.data.team`, `.data.origin`
- search note: `.data.results[]`, `.data.page_info.endCursor`
- search user: `.data.users[]`

## Search

- `kibel search note --query "<query>" --first 16`
- `kibel search note --query "<query>" --after "<cursor>" --first 16`
- `kibel search note --query "<query>" --save-preset "<name>"`
- `kibel search note --preset "<name>"`
- `kibel search note --mine --first 10`
- `kibel search folder --query "<query>" --first 16`
- `kibel search user --query "<query>" --first 16`

## Note

- `kibel note get --id <NOTE_ID>`
- `kibel note get-many --id <NOTE_ID> --id <NOTE_ID>`
- `kibel note get-from-path --path /notes/<number>`
- `kibel note create --title "<t>" --content "<c>" --group-id <GROUP_ID> --draft`
- `kibel note update --id <NOTE_ID> --base-content "<old>" --new-content "<new>"`
- `kibel note move-to-folder --id <NOTE_ID> --from-folder <GROUP_ID:NAME> --to-folder <GROUP_ID:NAME>`
- `kibel note attach-to-folder --id <NOTE_ID> --folder <GROUP_ID:NAME>`

Folder reference format:

- `<GROUP_ID:NAME>` (例: `1:Engineering`)
- `GROUP_ID` は `group list` で確認する。

## Folder/group/feed

- `kibel group list --first 16`
- `kibel folder list --first 16`
- `kibel folder get --id <FOLDER_ID> --first 16`
- `kibel folder get-from-path --path /folders/<number> --first 16`
- `kibel folder notes --folder-id <FOLDER_ID> --first 16`
- `kibel folder create --group-id <GROUP_ID> --full-name "<name>"`
- `kibel feed sections --kind NOTE --group-id <GROUP_ID> --first 16`

## Comment

- `kibel comment create --note-id <NOTE_ID> --content "<text>"`
- `kibel comment reply --comment-id <COMMENT_ID> --content "<text>"`

## GraphQL

Query:

```bash
kibel graphql run --query 'query Q { groups(first: 1) { edges { node { id name } } } }'
```

Query + variables:

```bash
kibel graphql run \
  --query 'query Q($id: ID!) { note(id: $id) { id title } }' \
  --variables '{"id":"<NOTE_ID>"}'
```

Use `--allow-mutation` only when explicitly required and allowed.
