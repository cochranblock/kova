# Kova Scripts

## sync-prompts-to-cursor.sh

Syncs `kova/assets/prompts/*.mdc` → `~/.cursor/rules/`.

**Source of truth:** `kova/assets/prompts/`. Cursor and Kova both use these. Run after editing prompts:

```bash
./scripts/sync-prompts-to-cursor.sh
```
