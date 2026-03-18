#!/usr/bin/env bash
# Unlicense — cochranblock.org
# Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

# Build kova for Android (Pixel 9 XL Pro — aarch64-linux-android, API 34)
# Prereqs:
#   rustup target add aarch64-linux-android
#   cargo install cargo-ndk
#   Android NDK r26+ installed (brew install --cask android-commandlinetools && sdkmanager "ndk;26.1.10909125")
#   export ANDROID_NDK_HOME=$HOME/Library/Android/sdk/ndk/26.1.10909125

set -euo pipefail
cd "$(dirname "$0")"

echo "[android] Building kova for aarch64-linux-android..."
cargo ndk -t arm64-v8a -o app/src/main/jniLibs build --release

echo "[android] Shared library built:"
ls -lh app/src/main/jniLibs/arm64-v8a/libkova_android.so

echo ""
echo "[android] To package APK:"
echo "  Option A (Gradle): cd android && ./gradlew assembleDebug"
echo "  Option B (manual): use aapt2 + apksigner"
echo ""
echo "[android] To install on Pixel 9 XL Pro:"
echo "  adb install -r app/build/outputs/apk/debug/app-debug.apk"
echo "  adb shell am start -n org.cochranblock.kova/android.app.NativeActivity"