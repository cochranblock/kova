#!/usr/bin/env bash
set -euo pipefail
export PATH="$HOME/.cargo/bin:$PATH"
cargo build --target aarch64-apple-ios --release
echo "Static lib at: target/aarch64-apple-ios/release/libkova_ios.a"
echo "Size: $(du -h target/aarch64-apple-ios/release/libkova_ios.a | cut -f1)"
