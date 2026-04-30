#!/bin/bash
# Unlicense — cochranblock.org
# Binary size gate. Enforces the "<10MB binary" claim on cochranblock.org.
# Runs after `cargo build --release --features serve`. Exits non-zero if too big.

set -e

LIMIT_BYTES=${KOVA_SIZE_LIMIT_BYTES:-10485760}  # 10 MiB default
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="$ROOT/target/release/kova"

if [ ! -f "$BIN" ]; then
    echo "[size-gate] binary not built: $BIN"
    echo "[size-gate] run: cargo build --release --features serve -p kova-engine --bin kova"
    exit 2
fi

# stat -c on Linux, stat -f%z on macOS
SIZE=$(stat -c%s "$BIN" 2>/dev/null || stat -f%z "$BIN")
HUMAN=$(numfmt --to=iec --suffix=B "$SIZE" 2>/dev/null || echo "${SIZE}B")
LIMIT_HUMAN=$(numfmt --to=iec --suffix=B "$LIMIT_BYTES" 2>/dev/null || echo "${LIMIT_BYTES}B")

echo "[size-gate] $BIN: $HUMAN (limit: $LIMIT_HUMAN)"

if [ "$SIZE" -gt "$LIMIT_BYTES" ]; then
    echo "[size-gate] FAIL: $HUMAN exceeds $LIMIT_HUMAN"
    echo "[size-gate] cochranblock.org claims '<10MB binary' — fix the build or update the claim."
    exit 1
fi

# Append to binary-sizes.log for historical tracking
LOG="$ROOT/binary-sizes.log"
NODE="${HOSTNAME:-$(hostname -s 2>/dev/null || echo unknown)}"
PROFILE="${KOVA_BUILD_PROFILE:-release}"
TS=$(date -u +%Y-%m-%dT%H:%M:%SZ)
printf "%s  %-16s %-16s %10d  size-gate pass\n" "$TS" "$NODE" "kova-engine($PROFILE)" "$SIZE" >> "$LOG"

echo "[size-gate] OK"
