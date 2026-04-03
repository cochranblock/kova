#!/bin/bash
# Build JSONL training data from extracted Rust corpus.
# Run on bt after extract_corpus.sh completes.
# Output: /mnt/data/training/*.jsonl
set -euo pipefail

CORPUS="/mnt/data/corpus"
OUT="/mnt/data/training"
mkdir -p "$OUT"

echo "=== Building training data from Rust corpus ==="

# ── 1. Rust Kinds: library / binary / test / macro / build ──

echo ""
echo "--- rust_kinds.jsonl ---"
> "$OUT/rust_kinds.jsonl"

find "$CORPUS" -name "lib.rs" -type f | shuf | head -2000 | while read -r f; do
    text=$(head -c 200 "$f" | tr '\n\r' '  ' | sed 's/"/\\"/g' | head -c 200)
    [ ${#text} -gt 15 ] && printf '{"text":"%s","label":"library"}\n' "$text"
done >> "$OUT/rust_kinds.jsonl"

find "$CORPUS" -name "main.rs" -type f | shuf | head -2000 | while read -r f; do
    text=$(head -c 200 "$f" | tr '\n\r' '  ' | sed 's/"/\\"/g' | head -c 200)
    [ ${#text} -gt 15 ] && printf '{"text":"%s","label":"binary"}\n' "$text"
done >> "$OUT/rust_kinds.jsonl"

# Test code: files containing #[test]
find "$CORPUS" -name "*.rs" -type f -exec grep -l '#\[test\]' {} + 2>/dev/null | shuf | head -2000 | while read -r f; do
    # Extract around a #[test] block.
    text=$(grep -A5 '#\[test\]' "$f" | head -c 200 | tr '\n\r' '  ' | sed 's/"/\\"/g')
    [ ${#text} -gt 15 ] && printf '{"text":"%s","label":"test"}\n' "$text"
done >> "$OUT/rust_kinds.jsonl"

# Macro code: files containing macro_rules! or proc_macro
find "$CORPUS" -name "*.rs" -type f -exec grep -l 'macro_rules!' {} + 2>/dev/null | shuf | head -2000 | while read -r f; do
    text=$(grep -A5 'macro_rules!' "$f" | head -c 200 | tr '\n\r' '  ' | sed 's/"/\\"/g')
    [ ${#text} -gt 15 ] && printf '{"text":"%s","label":"macro"}\n' "$text"
done >> "$OUT/rust_kinds.jsonl"

# Build scripts
find "$CORPUS" -name "build.rs" -type f | shuf | head -2000 | while read -r f; do
    text=$(head -c 200 "$f" | tr '\n\r' '  ' | sed 's/"/\\"/g' | head -c 200)
    [ ${#text} -gt 15 ] && printf '{"text":"%s","label":"build"}\n' "$text"
done >> "$OUT/rust_kinds.jsonl"

echo "  rust_kinds: $(wc -l < "$OUT/rust_kinds.jsonl") examples"
# Show class distribution.
for cls in library binary test macro build; do
    echo "    $cls: $(grep -c "\"$cls\"" "$OUT/rust_kinds.jsonl")"
done

# ── 2. Slop Detector: AI doc comments vs human doc comments ──

echo ""
echo "--- slop_detector.jsonl ---"
> "$OUT/slop_detector.jsonl"

# Slop: lines containing banned words from any .rs doc comment.
SLOP_PATTERN='utilize|leverage|comprehensive|robust|seamlessly|scalable|paradigm|synergy|cutting-edge|streamline|empower|delve|foster|harness|groundbreaking|innovative|revolutionize|unprecedented'
find "$CORPUS" -name "*.rs" -type f -exec grep -iE "$SLOP_PATTERN" {} + 2>/dev/null | shuf | head -3000 | while IFS= read -r line; do
    text=$(echo "$line" | cut -d: -f2- | head -c 200 | tr '\n\r' '  ' | sed 's/"/\\"/g')
    [ ${#text} -gt 15 ] && printf '{"text":"%s","label":"slop"}\n' "$text"
done >> "$OUT/slop_detector.jsonl"

# Clean: doc comments (//! and ///) from random .rs files, excluding slop.
find "$CORPUS" -name "*.rs" -type f | shuf | head -5000 | while read -r f; do
    grep -E '^[[:space:]]*(///|//!)' "$f" 2>/dev/null | grep -ivE "$SLOP_PATTERN" | head -2 | while IFS= read -r line; do
        text=$(echo "$line" | head -c 200 | tr '\n\r' '  ' | sed 's/"/\\"/g')
        [ ${#text} -gt 15 ] && printf '{"text":"%s","label":"clean"}\n' "$text"
    done
done >> "$OUT/slop_detector.jsonl"

# Also clean: plain code lines (no slop possible in fn signatures).
find "$CORPUS" -name "*.rs" -type f | shuf | head -2000 | while read -r f; do
    grep -E '^[[:space:]]*(pub )?(fn |struct |enum |trait |impl |use |mod )' "$f" 2>/dev/null | head -1 | while IFS= read -r line; do
        text=$(echo "$line" | head -c 200 | tr '\n\r' '  ' | sed 's/"/\\"/g')
        [ ${#text} -gt 15 ] && printf '{"text":"%s","label":"clean"}\n' "$text"
    done
done >> "$OUT/slop_detector.jsonl"

echo "  slop_detector: $(wc -l < "$OUT/slop_detector.jsonl") examples"
for cls in clean slop; do
    echo "    $cls: $(grep -c "\"$cls\"" "$OUT/slop_detector.jsonl")"
done

# ── 3. Code vs English: Rust source lines vs doc/comment lines ──

echo ""
echo "--- code_vs_english.jsonl ---"
> "$OUT/code_vs_english.jsonl"

# Code: non-comment, non-empty Rust lines.
find "$CORPUS" -name "*.rs" -type f | shuf | head -3000 | while read -r f; do
    grep -v '^[[:space:]]*$' "$f" 2>/dev/null | grep -v '^[[:space:]]*//' | head -2 | while IFS= read -r line; do
        text=$(echo "$line" | head -c 200 | tr '\n\r' '  ' | sed 's/"/\\"/g')
        [ ${#text} -gt 15 ] && printf '{"text":"%s","label":"code"}\n' "$text"
    done
done >> "$OUT/code_vs_english.jsonl"

# English: doc comments and module-level docs.
find "$CORPUS" -name "*.rs" -type f | shuf | head -3000 | while read -r f; do
    grep -E '^[[:space:]]*(///|//!)' "$f" 2>/dev/null | sed 's|^[[:space:]]*///[[:space:]]*||; s|^[[:space:]]*//![[:space:]]*||' | head -2 | while IFS= read -r line; do
        text=$(echo "$line" | head -c 200 | tr '\n\r' '  ' | sed 's/"/\\"/g')
        [ ${#text} -gt 15 ] && printf '{"text":"%s","label":"english"}\n' "$text"
    done
done >> "$OUT/code_vs_english.jsonl"

echo "  code_vs_english: $(wc -l < "$OUT/code_vs_english.jsonl") examples"
for cls in code english; do
    echo "    $cls: $(grep -c "\"$cls\"" "$OUT/code_vs_english.jsonl")"
done

echo ""
echo "=== Training data complete ==="
ls -lh "$OUT"/*.jsonl
