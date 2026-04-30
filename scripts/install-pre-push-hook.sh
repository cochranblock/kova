#!/bin/bash
# Unlicense — cochranblock.org
# Install pre-push test gate. Idempotent. Run from kova repo root.

set -e
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HOOK="$ROOT/.git/hooks/pre-push"

cat > "$HOOK" <<'EOF'
#!/bin/bash
# Pre-push test gate. Blocks push if `cargo test --lib --no-default-features` fails.
# Bypass with `git push --no-verify` only if you know what you're doing.
set -e
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"
echo "[pre-push] cargo test --lib --no-default-features..."
if ! cargo test --lib --no-default-features --quiet 2>&1 | tail -20; then
    echo "[pre-push] tests failed — push blocked."
    echo "[pre-push] fix the test or use --no-verify if intentional."
    exit 1
fi
echo "[pre-push] OK"
EOF

chmod +x "$HOOK"
echo "OK: $HOOK"
