#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
bash script/build_helper.sh
cargo test --manifest-path src-tauri/Cargo.toml --test native_photos -- --ignored
