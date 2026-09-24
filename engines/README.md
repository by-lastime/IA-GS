# Native engine runtime

IA'GS supports **macOS 15+ / Apple Silicon**. Engine executables and libraries are not stored in Git. The active runtime is specified in `manifest.macos.json`, restored to `engines/macos/arm64/`, and verified before packaging.

```sh
npm run setup:engines:macos
npm run build:helper
npm run verify:engines:macos
```

The pinned runtime is an unchanged snapshot of the engines bundled in OOOSplat 0.4.1 for macOS, redistributed as a separate IA'GS release asset because the old upstream standalone-engine URL is unavailable. It contains CPU-only COLMAP, the official arm64 Brush build, required shared libraries, upstream checksums and component notices. IA'GS separately builds `iags-photo` from `native/` using Apple system frameworks.

Installed applications contain the runtime and do not need Homebrew or a Python environment. Development setup downloads are cached under `.cache/engines/`; cache and binaries are Git-ignored. Source builds of the upstream runtime remain available through `npm run setup:build-deps:macos` and `npm run build:engines:macos`.

Some Windows/Linux manifests, notices and source modules are retained from upstream for provenance. They are not supported IA'GS targets. Video input is disabled even though FFmpeg/FFprobe may be present in the upstream runtime archive. See `licenses/THIRD_PARTY_NOTICES.txt` and the bundled `BUNDLED-COMPONENTS.json` for dependency attribution.
