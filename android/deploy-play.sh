#!/usr/bin/env bash
set -euo pipefail

TRACK="${1:-internal}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# NDK setup
export ANDROID_NDK_HOME=/opt/homebrew/share/android-commandlinetools/ndk/26.1.10909125
NDK_TOOLCHAIN="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/darwin-x86_64"

export CC_aarch64_linux_android="$NDK_TOOLCHAIN/bin/aarch64-linux-android28-clang"
export CXX_aarch64_linux_android="$NDK_TOOLCHAIN/bin/aarch64-linux-android28-clang++"
export AR_aarch64_linux_android="$NDK_TOOLCHAIN/bin/llvm-ar"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$NDK_TOOLCHAIN/bin/aarch64-linux-android28-clang"

# Signing env vars
export KEYSTORE_PASSWORD="${KEYSTORE_PASSWORD:?Set KEYSTORE_PASSWORD}"
export KEY_PASSWORD="${KEY_PASSWORD:?Set KEY_PASSWORD}"

echo "==> Building native library for aarch64-linux-android"
cd "$SCRIPT_DIR"
export PATH="$HOME/.cargo/bin:$PATH"
export ANDROID_HOME="${ANDROID_HOME:-/opt/homebrew/share/android-commandlinetools}"
cargo build --release --target aarch64-linux-android

echo "==> Copying .so to jniLibs"
mkdir -p "$SCRIPT_DIR/app/src/main/jniLibs/arm64-v8a"
cp "$SCRIPT_DIR/target/aarch64-linux-android/release/libkova_android.so" \
   "$SCRIPT_DIR/app/src/main/jniLibs/arm64-v8a/libkova_android.so"

echo "==> Building release AAB"
cd "$SCRIPT_DIR"
./gradlew bundleRelease

AAB="$SCRIPT_DIR/app/build/outputs/bundle/release/app-release.aab"
if [ ! -f "$AAB" ]; then
    echo "ERROR: AAB not found at $AAB"
    exit 1
fi
echo "==> AAB built: $(du -h "$AAB" | cut -f1) — $AAB"

echo "==> Uploading to Google Play (track: $TRACK)"
fastlane supply \
    --aab "$AAB" \
    --package_name org.cochranblock.kova \
    --json_key "$SCRIPT_DIR/play-service-account.json" \
    --track "$TRACK"

echo "==> Done. Deployed to $TRACK track."
