#!/bin/bash
# Scrape Rust training data from GitHub for subatomic models.
# All data from the Rust ecosystem. No other languages.
# Output: assets/training_data/*.jsonl
set -euo pipefail

OUT="assets/training_data"
mkdir -p "$OUT"

echo "=== Scraping Rust training data from GitHub ==="

# ── 1. Rust Code Kinds (for lang-detector: lib/bin/test/macro/build) ──

echo ""
echo "--- rust_kinds.jsonl: library vs binary vs test vs macro vs build ---"

# Library code: lib.rs files, pub fn, impl blocks
echo "  scraping library code..."
gh api -X GET "search/code" -f "q=filename:lib.rs language:rust pub fn" -f "per_page=100" --jq '.items[].text_matches[]?.fragment' 2>/dev/null | head -500 | while IFS= read -r line; do
    text=$(echo "$line" | head -c 200)
    [ ${#text} -gt 15 ] && printf '{"text":"%s","label":"library"}\n' "$(echo "$text" | sed 's/"/\\"/g' | tr '\n' ' ')"
done > "$OUT/rust_kinds_raw.jsonl"

# Search by characteristic patterns instead
echo "  scraping via pattern search..."

# Library: pub fn, pub struct, pub trait, impl
for pattern in "pub fn" "pub struct" "pub trait" "impl Display for"; do
    gh api -X GET "search/code" -f "q=$pattern language:rust" -f "per_page=30" --jq '.items[].url' 2>/dev/null | head -20 | while read -r url; do
        # Get raw content via the API
        raw=$(gh api "$url" --jq '.content' 2>/dev/null | base64 -d 2>/dev/null | head -c 200 || true)
        [ ${#raw} -gt 20 ] && printf '{"text":"%s","label":"library"}\n' "$(echo "$raw" | sed 's/"/\\"/g' | tr '\n' ' ' | head -c 200)"
    done >> "$OUT/rust_kinds_raw.jsonl"
    sleep 2  # rate limit
done

# Binary: fn main(), clap, std::process
for pattern in "fn main()" "use clap" "std::process::exit"; do
    gh api -X GET "search/code" -f "q=$pattern filename:main.rs language:rust" -f "per_page=30" --jq '.items[].url' 2>/dev/null | head -20 | while read -r url; do
        raw=$(gh api "$url" --jq '.content' 2>/dev/null | base64 -d 2>/dev/null | head -c 200 || true)
        [ ${#raw} -gt 20 ] && printf '{"text":"%s","label":"binary"}\n' "$(echo "$raw" | sed 's/"/\\"/g' | tr '\n' ' ' | head -c 200)"
    done >> "$OUT/rust_kinds_raw.jsonl"
    sleep 2
done

# Test code: #[test], #[cfg(test)], assert_eq
for pattern in "#[test]" "#[cfg(test)]" "assert_eq!"; do
    gh api -X GET "search/code" -f "q=$pattern language:rust" -f "per_page=30" --jq '.items[].url' 2>/dev/null | head -20 | while read -r url; do
        raw=$(gh api "$url" --jq '.content' 2>/dev/null | base64 -d 2>/dev/null | head -c 200 || true)
        [ ${#raw} -gt 20 ] && printf '{"text":"%s","label":"test"}\n' "$(echo "$raw" | sed 's/"/\\"/g' | tr '\n' ' ' | head -c 200)"
    done >> "$OUT/rust_kinds_raw.jsonl"
    sleep 2
done

# Macro code: macro_rules!, proc_macro, #[derive]
for pattern in "macro_rules!" "proc_macro" "TokenStream"; do
    gh api -X GET "search/code" -f "q=$pattern language:rust" -f "per_page=30" --jq '.items[].url' 2>/dev/null | head -20 | while read -r url; do
        raw=$(gh api "$url" --jq '.content' 2>/dev/null | base64 -d 2>/dev/null | head -c 200 || true)
        [ ${#raw} -gt 20 ] && printf '{"text":"%s","label":"macro"}\n' "$(echo "$raw" | sed 's/"/\\"/g' | tr '\n' ' ' | head -c 200)"
    done >> "$OUT/rust_kinds_raw.jsonl"
    sleep 2
done

# Build scripts: build.rs
for pattern in "println!" "cargo:rustc-link" "cargo:rerun-if-changed"; do
    gh api -X GET "search/code" -f "q=$pattern filename:build.rs language:rust" -f "per_page=30" --jq '.items[].url' 2>/dev/null | head -20 | while read -r url; do
        raw=$(gh api "$url" --jq '.content' 2>/dev/null | base64 -d 2>/dev/null | head -c 200 || true)
        [ ${#raw} -gt 20 ] && printf '{"text":"%s","label":"build"}\n' "$(echo "$raw" | sed 's/"/\\"/g' | tr '\n' ' ' | head -c 200)"
    done >> "$OUT/rust_kinds_raw.jsonl"
    sleep 2
done

echo "  rust_kinds: $(wc -l < "$OUT/rust_kinds_raw.jsonl") examples"

# ── 2. Slop Detector (Rust READMEs and doc comments) ──

echo ""
echo "--- slop_detector.jsonl: AI-generated vs human Rust docs ---"

# Slop: READMEs with AI slop patterns
for word in "utilize" "leverage" "comprehensive" "robust" "seamlessly" "streamline" "empower" "cutting-edge"; do
    gh api -X GET "search/code" -f "q=$word filename:README.md language:markdown" -f "per_page=10" --jq '.items[].url' 2>/dev/null | head -8 | while read -r url; do
        raw=$(gh api "$url" --jq '.content' 2>/dev/null | base64 -d 2>/dev/null | head -c 200 || true)
        [ ${#raw} -gt 20 ] && printf '{"text":"%s","label":"slop"}\n' "$(echo "$raw" | sed 's/"/\\"/g' | tr '\n' ' ' | head -c 200)"
    done >> "$OUT/slop_raw.jsonl"
    sleep 2
done

# Clean: Real Rust project READMEs (well-known crates)
for repo in "tokio-rs/tokio" "serde-rs/serde" "dtolnay/anyhow" "BurntSushi/ripgrep" "sharkdp/bat" "sharkdp/fd" "starship/starship" "alacritty/alacritty" "rust-lang/rustfmt" "rust-lang/cargo"; do
    raw=$(gh api "repos/$repo/readme" --jq '.content' 2>/dev/null | base64 -d 2>/dev/null | head -c 200 || true)
    [ ${#raw} -gt 20 ] && printf '{"text":"%s","label":"clean"}\n' "$(echo "$raw" | sed 's/"/\\"/g' | tr '\n' ' ' | head -c 200)"
    sleep 1
done >> "$OUT/slop_raw.jsonl"

# Clean: Rust doc comments from popular crates
for pattern in "//! " "/// " "//! #"; do
    gh api -X GET "search/code" -f "q=$pattern language:rust repo:tokio-rs/tokio" -f "per_page=20" --jq '.items[].url' 2>/dev/null | head -10 | while read -r url; do
        raw=$(gh api "$url" --jq '.content' 2>/dev/null | base64 -d 2>/dev/null | grep -E "^///|^//!" | head -5 | head -c 200 || true)
        [ ${#raw} -gt 20 ] && printf '{"text":"%s","label":"clean"}\n' "$(echo "$raw" | sed 's/"/\\"/g' | tr '\n' ' ' | head -c 200)"
    done >> "$OUT/slop_raw.jsonl"
    sleep 2
done

echo "  slop_detector: $(wc -l < "$OUT/slop_raw.jsonl") examples"

# ── 3. Code vs English (Rust source vs Rust docs/comments) ──

echo ""
echo "--- code_vs_english.jsonl: Rust source vs Rust docs ---"

# Code: raw Rust from popular repos
for repo in "tokio-rs/tokio" "serde-rs/serde" "dtolnay/anyhow" "BurntSushi/ripgrep" "hyperium/hyper" "rust-lang/cargo"; do
    gh api "repos/$repo/git/trees/HEAD?recursive=1" --jq '.tree[] | select(.path | endswith(".rs")) | .url' 2>/dev/null | head -15 | while read -r url; do
        raw=$(gh api "$url" --jq '.content' 2>/dev/null | base64 -d 2>/dev/null | grep -v "^$" | grep -v "^//" | head -c 200 || true)
        [ ${#raw} -gt 20 ] && printf '{"text":"%s","label":"code"}\n' "$(echo "$raw" | sed 's/"/\\"/g' | tr '\n' ' ' | head -c 200)"
    done >> "$OUT/code_vs_english_raw.jsonl"
    sleep 2
done

# English: Rust doc comments and README content
for repo in "tokio-rs/tokio" "serde-rs/serde" "BurntSushi/ripgrep" "sharkdp/bat" "rust-lang/cargo" "rust-lang/rust"; do
    # README
    raw=$(gh api "repos/$repo/readme" --jq '.content' 2>/dev/null | base64 -d 2>/dev/null | grep -v "^#" | grep -v "^\`\`\`" | grep -v "^|" | grep -v "^-" | head -c 200 || true)
    [ ${#raw} -gt 20 ] && printf '{"text":"%s","label":"english"}\n' "$(echo "$raw" | sed 's/"/\\"/g' | tr '\n' ' ' | head -c 200)"
    sleep 1
done >> "$OUT/code_vs_english_raw.jsonl"

# Also scrape CHANGELOG/CONTRIBUTING for english
for repo in "tokio-rs/tokio" "serde-rs/serde" "BurntSushi/ripgrep"; do
    for file in "CHANGELOG.md" "CONTRIBUTING.md"; do
        raw=$(gh api "repos/$repo/contents/$file" --jq '.content' 2>/dev/null | base64 -d 2>/dev/null | grep -v "^#" | grep -v "^\`\`\`" | head -c 200 || true)
        [ ${#raw} -gt 20 ] && printf '{"text":"%s","label":"english"}\n' "$(echo "$raw" | sed 's/"/\\"/g' | tr '\n' ' ' | head -c 200)"
    done
    sleep 1
done >> "$OUT/code_vs_english_raw.jsonl"

echo "  code_vs_english: $(wc -l < "$OUT/code_vs_english_raw.jsonl") examples"

# ── Also scrape from LOCAL repos for more data ──

echo ""
echo "--- Augmenting from local repos ---"

# Local Rust code
for dir in /Users/mcochran/kova/src /Users/mcochran/dev/any-gpu/src /Users/mcochran/dev/pixel-forge/src 2>/dev/null; do
    [ -d "$dir" ] || continue
    find "$dir" -name "*.rs" -type f 2>/dev/null | head -50 | while read -r f; do
        # Code snippets: non-comment, non-empty lines
        head -c 400 "$f" | grep -v "^//" | grep -v "^$" | head -c 200 | while IFS= read -r line; do
            [ ${#line} -gt 15 ] && printf '{"text":"%s","label":"code"}\n' "$(echo "$line" | sed 's/"/\\"/g' | tr '\n' ' ')"
        done

        # Classify for rust_kinds
        name=$(basename "$f")
        if echo "$name" | grep -q "^lib.rs$\|^mod.rs$"; then
            head -c 200 "$f" | sed 's/"/\\"/g' | tr '\n' ' ' | xargs -I{} printf '{"text":"{}","label":"library"}\n'
        elif echo "$name" | grep -q "^main.rs$"; then
            head -c 200 "$f" | sed 's/"/\\"/g' | tr '\n' ' ' | xargs -I{} printf '{"text":"{}","label":"binary"}\n'
        elif echo "$name" | grep -q "^build.rs$"; then
            head -c 200 "$f" | sed 's/"/\\"/g' | tr '\n' ' ' | xargs -I{} printf '{"text":"{}","label":"build"}\n'
        fi
    done >> "$OUT/local_augment.jsonl"
done

# Local doc comments and READMEs for english
for dir in /Users/mcochran/kova /Users/mcochran/dev/any-gpu /Users/mcochran/dev/pixel-forge 2>/dev/null; do
    [ -d "$dir" ] || continue
    find "$dir" -maxdepth 2 -name "*.md" -type f 2>/dev/null | head -20 | while read -r f; do
        head -c 200 "$f" | grep -v "^#" | grep -v "^\`\`\`" | grep -v "^|" | sed 's/"/\\"/g' | tr '\n' ' ' | head -c 200 | xargs -I{} printf '{"text":"{}","label":"english"}\n'
    done >> "$OUT/local_augment.jsonl"
done

# Test code from local repos
find /Users/mcochran/kova/src -name "*.rs" -type f 2>/dev/null | xargs grep -l "#\[test\]" 2>/dev/null | head -20 | while read -r f; do
    # Extract test functions
    grep -A5 "#\[test\]" "$f" | head -c 200 | sed 's/"/\\"/g' | tr '\n' ' ' | xargs -I{} printf '{"text":"{}","label":"test"}\n'
done >> "$OUT/local_augment.jsonl"

echo "  local_augment: $(wc -l < "$OUT/local_augment.jsonl" 2>/dev/null || echo 0) examples"

echo ""
echo "=== Scraping complete ==="
echo "Files in $OUT:"
wc -l "$OUT"/*.jsonl 2>/dev/null
