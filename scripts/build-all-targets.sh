#!/usr/bin/env bash
set -euo pipefail
export PATH="$HOME/.cargo/bin:$PATH"
OUT="release"
mkdir -p "$OUT"

echo "==> macOS ARM64 (native)"
cargo build --release -p kova
cp target/aarch64-apple-darwin/release/kova "$OUT/kova-macos-arm64"

echo "==> macOS x86_64 (cross)"
if rustup target list --installed | grep -q x86_64-apple-darwin; then
  cargo build --release -p kova --target x86_64-apple-darwin
  cp target/x86_64-apple-darwin/release/kova "$OUT/kova-macos-x86_64"
else
  echo "  SKIP: x86_64-apple-darwin not installed"
fi

echo "==> Android ARM64"
if [ -n "${ANDROID_NDK_HOME:-}" ]; then
  NDK_TOOLCHAIN="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/darwin-x86_64"
  export CC_aarch64_linux_android="$NDK_TOOLCHAIN/bin/aarch64-linux-android28-clang"
  export CXX_aarch64_linux_android="$NDK_TOOLCHAIN/bin/aarch64-linux-android28-clang++"
  export AR_aarch64_linux_android="$NDK_TOOLCHAIN/bin/llvm-ar"
  export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$CC_aarch64_linux_android"
  cd android && cargo build --release --target aarch64-linux-android && cd ..
  cp android/target/aarch64-linux-android/release/libkova_android.so "$OUT/libkova_android.so"
else
  echo "  SKIP: ANDROID_NDK_HOME not set"
fi

echo "==> WASM"
if rustup target list --installed | grep -q wasm32-unknown-unknown; then
  cargo build --release -p kova --target wasm32-unknown-unknown --no-default-features 2>/dev/null || echo "  SKIP: WASM build failed (expected — needs wasm features)"
fi

echo "==> Linux x86_64 (build on remote node)"
echo "  Run on st/gd: cargo build --release -p kova"
echo "  Then: scp st:~/kova/target/release/kova $OUT/kova-linux-x86_64"

echo ""
echo "=== Built artifacts ==="
ls -lh "$OUT"/
