# Workflow Reference (kibel-agentic-rag)

## 1. Recall

Pre-check auth. If not ready:

```bash
kibel --json auth login --origin "https://<tenant>.kibe.la" --team "<tenant>"
```

Run broad search 2-3 times with different query terms:

```bash
kibel --json search note --query "<topic>" --first 16
```

Optional personal context:

```bash
kibel --json search note --mine --first 10
```

## 2. Precision

Narrow candidates:

```bash
kibel --json search note \
  --query "<topic>" \
  --user-id "<USER_ID>" \
  --group-id "<GROUP_ID>" \
  --folder-id "<FOLDER_ID>" \
  --first 16
```

If `<USER_ID>` is unknown, omit it and keep narrowing with group/folder filters.

## 3. Verification

Get full source before final answer:

```bash
kibel --json note get --id "<NOTE_ID>"
```

or

```bash
kibel --json note get-from-path --path "/notes/<number>"
```

## 4. Synthesis

Return:

- answer
- evidence list with URLs
- unknowns/assumptions
