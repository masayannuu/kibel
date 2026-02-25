# Command Reference (kibel-agentic-search)

## Required

```bash
kibel --json auth status
```

## Core retrieval

Mine latest:

```bash
kibel --json search note --mine --first 10
```

Broad recall:

```bash
kibel --json search note --query "<query>" --first 16
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

## Verification

```bash
kibel --json note get --id "<NOTE_ID>"
kibel --json note get-from-path --path "/notes/<number>"
```
