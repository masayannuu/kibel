# Command Map (kibel-cli-operator)

## Auth/config

- `kibel --json auth status`
- `kibel --json auth login --team <team> --with-token`
- `kibel --json config profiles`
- `kibel --json config set team <team>`

## Search

- `kibel --json search note --query "<query>" --first 16`
- `kibel --json search note --mine --first 10`
- `kibel --json search folder --query "<query>" --first 16`

## Note

- `kibel --json note get --id <NOTE_ID>`
- `kibel --json note get-from-path --path /notes/<number>`
- `kibel --json note create --title "<t>" --content "<c>" --group-id <GROUP_ID> --draft`
- `kibel --json note update --id <NOTE_ID> --base-content "<old>" --new-content "<new>"`
- `kibel --json note move-to-folder --id <NOTE_ID> --from-folder <GROUP_ID:NAME> --to-folder <GROUP_ID:NAME>`
- `kibel --json note attach-to-folder --id <NOTE_ID> --folder <GROUP_ID:NAME>`

## Folder/group/feed

- `kibel --json group list --first 16`
- `kibel --json folder list --first 16`
- `kibel --json folder get --id <FOLDER_ID> --first 16`
- `kibel --json folder get-from-path --path /folders/<number> --first 16`
- `kibel --json folder notes --folder-id <FOLDER_ID> --first 16`
- `kibel --json folder create --group-id <GROUP_ID> --full-name "<name>"`
- `kibel --json feed sections --kind NOTE --group-id <GROUP_ID> --first 16`

## Comment

- `kibel --json comment create --note-id <NOTE_ID> --content "<text>"`
- `kibel --json comment reply --comment-id <COMMENT_ID> --content "<text>"`

## GraphQL

Query:

```bash
kibel --json graphql run --query 'query Q { groups(first: 1) { edges { node { id name } } } }'
```

Query + variables:

```bash
kibel --json graphql run \
  --query 'query Q($id: ID!) { note(id: $id) { id title } }' \
  --variables '{"id":"<NOTE_ID>"}'
```

Use `--allow-mutation` only when explicitly required and allowed.
