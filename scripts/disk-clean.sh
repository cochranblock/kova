#!/bin/bash
# Unlicense — cochranblock.org
# Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

# disk-clean.sh — free disk space by purging regenerable caches.
# Run: ./scripts/disk-clean.sh [--aggressive]
# Safe: only deletes build artifacts and temp files, never source code.

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
GRAY='\033[0;90m'
NC='\033[0m'

AGGRESSIVE=false
[ "$1" = "--aggressive" ] && AGGRESSIVE=true

before=$(df -h / | tail -1 | awk '{print $4}')
echo -e "${CYAN}disk-clean${NC} — freeing regenerable caches"
echo -e "${GRAY}available before: ${before}${NC}"
echo ""

freed=0

clean() {
    local label="$1"
    local path="$2"
    if [ -e "$path" ]; then
        local size
        size=$(du -sh "$path" 2>/dev/null | cut -f1)
        rm -rf "$path"
        echo -e "  ${GREEN}+${NC} ${label} ${GRAY}(${size})${NC}"
        freed=1
    fi
}

# 1. Rust incremental build cache (largest, fully regenerable)
clean "incremental (debug)" "$HOME/target/aarch64-apple-darwin/debug/incremental"
clean "incremental (release)" "$HOME/target/aarch64-apple-darwin/release/incremental"

# 2. Claude Code task output files
TASK_DIR="/private/tmp/claude-501/-Users-mcochran/tasks"
if [ -d "$TASK_DIR" ]; then
    size=$(du -sh "$TASK_DIR" 2>/dev/null | cut -f1)
    rm -rf "$TASK_DIR"
    mkdir -p "$TASK_DIR"
    echo -e "  ${GREEN}+${NC} claude task outputs ${GRAY}(${size})${NC}"
    freed=1
fi

# 3. Doc output
clean "doc (debug)" "$HOME/target/aarch64-apple-darwin/debug/doc"
clean "doc (release)" "$HOME/target/aarch64-apple-darwin/release/doc"

# 4. Project-local target dirs (workspace uses ~/target/)
for d in "$HOME/kova/target" "$HOME/kova/kova-web/target"; do
    clean "local target: $(basename $(dirname $d))" "$d"
done

# -- aggressive mode: also clears cargo registry + git caches --
if $AGGRESSIVE; then
    echo ""
    echo -e "${CYAN}aggressive mode${NC}"
    clean "cargo registry cache" "$HOME/.cargo/registry/cache"
    clean "cargo git db" "$HOME/.cargo/git/db"
    clean "cargo git checkouts" "$HOME/.cargo/git/checkouts"
    clean "debug deps" "$HOME/target/aarch64-apple-darwin/debug/deps"
    clean "Xcode DerivedData" "$HOME/Library/Developer/Xcode/DerivedData"
fi

echo ""
after=$(df -h / | tail -1 | awk '{print $4}')
echo -e "${CYAN}available after: ${after}${NC} ${GRAY}(was ${before})${NC}"

if [ "$freed" -eq 0 ]; then
    echo -e "${GRAY}nothing to clean${NC}"
fi