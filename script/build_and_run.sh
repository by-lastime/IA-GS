#!/usr/bin/env bash
# Run the development app only. No .app / DMG is packaged.
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ ! -d node_modules ]]; then npm ci; fi
pkill -x iags >/dev/null 2>&1 || true
bash script/build_helper.sh
exec npm run tauri -- dev
