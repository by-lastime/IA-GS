# Native engine runtime

IA'GS supports **macOS 15+ / Apple Silicon**. Engine executables and libraries are not stored in Git. The active runtime is specified in `manifest.macos.json`, restored to `engines/macos/arm64/`, and verified before packaging.

```sh
npm run setup:engines:macos
npm run build:helper
npm run verify:engines:macos
```

The pinned runtime is extracted from the **official OOOSplat 0.4.1 macOS installer**. Its DMG SHA-256 is pinned in the source manifest. Setup mounts it read-only, copies only `engines/macos/arm64/`, unmounts it and verifies the runtime's own SHA256SUMS. It does not install or launch OOOSplat.

This avoids the unavailable upstream standalone-engine URL. IA'GS publishes its source code and app installer, but does not host a separate engine archive. The copied runtime contains CPU-only COLMAP, arm64 Brush, shared libraries and component notices. IA'GS builds `iags-photo` separately from `native/` using Apple frameworks.

Installed applications contain the runtime and do not need Homebrew or a Python environment. Development setup downloads are cached under `.cache/engines/`; cache and binaries are Git-ignored. Source builds of the upstream runtime remain available through `npm run setup:build-deps:macos` and `npm run build:engines:macos`.

Some Windows/Linux manifests, notices and source modules are retained from upstream for provenance. They are not supported IA'GS targets. Video input is disabled even though FFmpeg/FFprobe may be present in the upstream runtime archive. See `licenses/THIRD_PARTY_NOTICES.txt` and the bundled `BUNDLED-COMPONENTS.json` for dependency attribution.

Maintainers can use `OOOSPLAT_MACOS_ENGINE_ARCHIVE=/path/to/runtime.tar.xz` (or a `.dmg`) to explicitly select a locally rebuilt archive. A matching `.sha256` file is checked when present. This override intentionally trusts a maintainer-selected local archive; the normal network path always checks the pinned official installer digest.
