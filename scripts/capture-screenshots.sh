#!/usr/bin/env bash
# Unlicense — cochranblock.org
# Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

# Capture GUI screenshots for Proof of Artifacts.
# Requires: kova built with --features serve, screencapture (macOS)
# Usage: ./scripts/capture-screenshots.sh

set -euo pipefail
SHOTS="$(dirname "$0")/../screenshots"
mkdir -p "$SHOTS"

echo "[capture] Starting kova serve in background..."
KOVA_SKIP_WASM=1 cargo run --bin kova -- s -d &
KOVA_PID=$!
sleep 3

echo "[capture] Taking serve screenshots via exopack..."
# Use exopack screenshot for HTML pages
cargo run -p exopack --features screenshot -- \
  --url "http://127.0.0.1:3002" \
  --out "$SHOTS/serve-index.png" 2>/dev/null || true

echo "[capture] Stopping kova serve..."
kill $KOVA_PID 2>/dev/null || true

echo "[capture] Taking TUI screenshot..."
# TUI: use script + ANSI-to-image if available
# Fallback: document that TUI screenshots need manual capture
echo "TUI screenshots require manual capture (run 'kova' and screenshot)."

echo "[capture] Taking token validator output..."
KOVA_SKIP_WASM=1 cargo run --bin kova -- tokens > "$SHOTS/tokens-output.txt" 2>/dev/null || true

echo "[capture] Done. Screenshots in $SHOTS/"
ls -la "$SHOTS/"