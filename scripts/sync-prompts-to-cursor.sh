#!/bin/bash
# Sync kova/assets/prompts/*.mdc → ~/.cursor/rules/
# Source of truth: kova/assets/prompts. Cursor and Kova both use these.
set -e
KOVA_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CURSOR_RULES="$HOME/.cursor/rules"
mkdir -p "$CURSOR_RULES"
for f in "$KOVA_ROOT/assets/prompts"/*.mdc; do
  [ -f "$f" ] || continue
  name=$(basename "$f")
  cp "$f" "$CURSOR_RULES/$name"
  echo "Synced: $name"
done
echo "Done. Cursor rules updated from kova/assets/prompts/"
