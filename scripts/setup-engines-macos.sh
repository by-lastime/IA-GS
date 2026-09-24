#!/usr/bin/env bash
# Modified for IA'GS (2026-09-24); see docs/CHANGES_FROM_UPSTREAM.md.
set -euo pipefail

if [[ "$(uname -s)" != "Darwin" || "$(uname -m)" != "arm64" ]]; then
  echo "macOS engine setup requires an Apple Silicon Mac." >&2
  exit 1
fi

workspace="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
manifest="$workspace/engines/manifest.macos.json"
cache="$workspace/.cache/engines/macos"
destination="$workspace/engines/macos/arm64"
read_manifest() {
  node -e 'const m=require(process.argv[1]); let v=m; for (const key of process.argv[2].split(".")) v=v[key]; process.stdout.write(String(v));' "$manifest" "$1"
}
mkdir -p "$cache" "$(dirname "$destination")"
format="$(read_manifest distribution.format)"
if [[ -n "${OOOSPLAT_MACOS_ENGINE_ARCHIVE:-}" ]]; then
  # Explicit local override for maintainers rebuilding the upstream engine bundle.
  archive="$OOOSPLAT_MACOS_ENGINE_ARCHIVE"
  [[ -f "$archive" ]] || { echo "Missing local engine archive: $archive" >&2; exit 1; }
  case "$archive" in
    *.tar.xz) format=tar.xz ;;
    *.dmg) format=dmg ;;
    *) echo "Local runtime must be a .dmg or .tar.xz file." >&2; exit 1 ;;
  esac
  if [[ -f "$archive.sha256" ]]; then
    expected="$(awk 'NF { print tolower($1); exit }' "$archive.sha256")"
  else
    expected="$(shasum -a 256 "$archive" | awk '{ print tolower($1) }')"
  fi
else
  archive="$cache/$(read_manifest distribution.archiveName)"
  expected="$(read_manifest distribution.archiveSha256)"
  # Only reuse a fully downloaded file whose digest matches the source manifest.
  if [[ ! -f "$archive" ]] || [[ "$(shasum -a 256 "$archive" | awk '{ print tolower($1) }')" != "$expected" ]]; then
    curl --fail --location --retry 3 "$(read_manifest distribution.sourceUrl)" --output "$archive.download"
    mv "$archive.download" "$archive"
  fi
fi
[[ "$expected" =~ ^[0-9a-f]{64}$ ]] || { echo "Invalid archive checksum." >&2; exit 1; }
actual="$(shasum -a 256 "$archive" | awk '{ print tolower($1) }')"
[[ "$actual" == "$expected" ]] || { echo "macOS engine archive SHA-256 mismatch." >&2; exit 1; }

temporary="$(mktemp -d)"
mount="$temporary/mount"
mounted=0
cleanup() {
  if [[ "$mounted" == 1 ]]; then hdiutil detach "$mount" >/dev/null 2>&1 || true; fi
  rm -rf -- "$temporary"
}
trap cleanup EXIT
staged="$temporary/runtime"
case "$format" in
  dmg)
    mkdir -p "$mount"
    # The official upstream image carries the preserved Apache-2.0 notice.
    # Read-only mounting extracts dependencies; it does not install OOOSplat.
    if ! printf 'Y\n' | hdiutil attach "$archive" -readonly -nobrowse -mountpoint "$mount" >"$temporary/mount.log" 2>&1; then
      tail -n 8 "$temporary/mount.log" >&2
      exit 1
    fi
    mounted=1
    runtime_root="$mount/$(read_manifest distribution.runtimePath)"
    [[ -d "$runtime_root/bin" && -f "$runtime_root/SHA256SUMS" ]] || { echo "Official installer is missing the expected runtime." >&2; exit 1; }
    ditto "$runtime_root" "$staged"
    hdiutil detach "$mount" >/dev/null
    mounted=0
    ;;
  tar.xz)
    if tar -tJf "$archive" | grep -Eq '(^/|(^|/)\.\.(/|$))'; then
      echo "Unsafe path found in macOS engine archive." >&2; exit 1
    fi
    tar -xJf "$archive" -C "$temporary"
    runtime_root="$temporary/ooosplat-engines-macos-arm64"
    [[ -d "$runtime_root" ]] || { echo "Unexpected macOS engine archive layout." >&2; exit 1; }
    mv "$runtime_root" "$staged"
    ;;
  *) echo "Unsupported runtime format: $format" >&2; exit 1 ;;
esac
# Verify content before replacing a runtime that may already be working.
(cd "$staged" && shasum -a 256 -c SHA256SUMS)
if [[ -f "$destination/README.md" ]]; then cp "$destination/README.md" "$staged/README.md"; fi
rm -rf -- "$destination"
mv "$staged" "$destination"
"$workspace/scripts/verify-engines-macos.sh"
