<!-- Unlicense — cochranblock.org -->
<!-- Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3 -->

# Kova Scripts

## sync-prompts-to-cursor.sh

Syncs `kova/assets/prompts/*.mdc` → `~/.cursor/rules/`.

**Source of truth:** `kova/assets/prompts/`. Cursor and Kova both use these. Run after editing prompts:

```bash
./scripts/sync-prompts-to-cursor.sh
```