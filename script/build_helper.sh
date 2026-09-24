#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
[[ "$(uname -s)" == Darwin && "$(uname -m)" == arm64 ]] || { echo 'IA GS requires Apple Silicon macOS'; exit 1; }
mkdir -p engines/macos/arm64/bin .native-cache
sdk="${SDKROOT:-$(xcrun --show-sdk-path)}"
# A few locally renamed SDK headers are repaired with read-only aliases, never system edits.
if [[ ! -f "$sdk/System/Library/Frameworks/Foundation.framework/Headers/NSTextCheckingResult.h" && -d /Library/Developer/CommandLineTools/SDKs/MacOSX15.5.sdk ]]; then sdk=/Library/Developer/CommandLineTools/SDKs/MacOSX15.5.sdk; fi
node scripts/sdk-overlay.mjs "$sdk" "$PWD/.native-cache/sdk-overlay.json"
extra_swift=(-vfsoverlay "$PWD/.native-cache/sdk-overlay.json" -Xcc -ivfsoverlay -Xcc "$PWD/.native-cache/sdk-overlay.json")
extra_clang=(-ivfsoverlay "$PWD/.native-cache/sdk-overlay.json")
xcrun clang -fobjc-arc -isysroot "$sdk" "${extra_clang[@]}" -mmacosx-version-min=15.0 -c native/VisionBridge.m -o .native-cache/VisionBridge.o
xcrun swiftc -sdk "$sdk" "${extra_swift[@]}" -O -target arm64-apple-macosx15.0 -module-cache-path "$PWD/.native-cache/fixed-modules" -import-objc-header native/VisionBridge.h native/PhotoPrepare.swift .native-cache/VisionBridge.o -framework Vision -framework CoreVideo -o engines/macos/arm64/bin/iags-photo
