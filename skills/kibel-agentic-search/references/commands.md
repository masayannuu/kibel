# Command Reference (kibel-agentic-search)

## Required

```bash
kibel --json auth status
```

If auth is not ready:

```bash
kibel --json auth login --origin "https://<tenant>.kibe.la" --team "<tenant>"
```

## Core retrieval

Mine latest:

```bash
kibel --json search note --mine --first 10
```

`--mine` is exclusive. Do not combine with `--query`, `--user-id`, `--group-id`, `--folder-id`.

Broad recall:

```bash
kibel --json search note --query "<query>" --first 16
```

Cursor next page:

```bash
kibel --json search note --query "<query>" --after "<cursor>" --first 16
```

User discovery:

```bash
kibel --json search user --query "<query>" --first 10
```

Precision narrowing:

```bash
kibel --json search note \
  --query "<query>" \
  --user-id "<USER_ID>" \
  --group-id "<GROUP_ID>" \
  --folder-id "<FOLDER_ID>" \
  --first 16
```

`--user-id` is optional. If unknown, run precision narrowing without it and verify candidates with `note get`.

## Verification

```bash
kibel --json note get --id "<NOTE_ID>"
kibel --json note get-many --id "<NOTE_ID_1>" --id "<NOTE_ID_2>"
kibel --json note get-from-path --path "/notes/<number>"
```
