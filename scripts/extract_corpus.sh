#!/bin/bash
# Extract .rs files from downloaded .crate tarballs into corpus.
# Run on bt after get-all-crates completes.
# Output: /mnt/data/corpus/{crate_name}/*.rs (first 200 chars per file for trigram training)
set -euo pipefail

CRATES="/mnt/data/crates"
CORPUS="/mnt/data/corpus"
STATS="/mnt/data/corpus_stats.txt"

echo "=== Extracting Rust corpus from .crate files ==="

total=0
extracted=0
errors=0

# Find all .crate files recursively.
find "$CRATES" -name "*.crate" -type f | while read -r crate_file; do
    total=$((total + 1))
    crate_name=$(basename "$crate_file" .crate)

    # Extract only .rs files, strip the top-level directory.
    tmpdir=$(mktemp -d)
    if tar xzf "$crate_file" --include='*.rs' -C "$tmpdir" 2>/dev/null; then
        # Find extracted .rs files.
        rs_files=$(find "$tmpdir" -name "*.rs" -type f 2>/dev/null)
        if [ -n "$rs_files" ]; then
            out_dir="$CORPUS/$crate_name"
            mkdir -p "$out_dir"
            echo "$rs_files" | while read -r rs; do
                base=$(basename "$rs")
                cp "$rs" "$out_dir/$base" 2>/dev/null || true
            done
            extracted=$((extracted + 1))
        fi
    else
        errors=$((errors + 1))
    fi
    rm -rf "$tmpdir"

    # Progress every 1000 crates.
    if [ $((total % 1000)) -eq 0 ]; then
        echo "  processed: $total crates, $extracted with .rs files, $errors errors"
        du -sh "$CORPUS" 2>/dev/null
    fi
done

echo ""
echo "=== Extraction complete ==="
echo "Total .crate files: $total"
echo "Crates with .rs: $extracted"
echo "Errors: $errors"
du -sh "$CORPUS"

# Count .rs files and total lines.
echo ""
echo "--- Corpus stats ---"
rs_count=$(find "$CORPUS" -name "*.rs" -type f | wc -l)
echo "Total .rs files: $rs_count"
find "$CORPUS" -name "*.rs" -type f -exec cat {} + | wc -l | xargs echo "Total lines:"
