# Architecture

IA'GS keeps OOOSplat's Tauri 2 / React 19 / Rust application and COLMAP + Brush execution pipeline. It adds a Swift command-line helper with a narrow Objective-C bridge to Apple Vision. The helper is compiled locally; end users do not need Python, PyTorch or a cloud API.

## Preparation and review

`prepare_photos` creates a unique folder, then runs `iags-photo prepare`. It copies originals, decodes orientation with ImageIO, uses an sRGB bitmap, limits the long side to 2400 px without cropping or centering, generates a foreground alpha mask and saves an incremental `processing.json`. Native masks keep disconnected foreground pieces and holes. An existing transparent input is not segmented again. Empty decoded images fail explicitly. Vision failures and unusual foreground area are surfaced for review; no image is silently excluded.

The frontend displays generated assets through Tauri's scoped asset protocol. Only directories in an explicitly opened IA'GS photo project are allowed. `edit_photo` validates the photo identifier and normalized brush coordinates. Original RGB is loaded from the prepared preview; selection or brush edits replace only alpha. The native helper writes PNG files atomically. An invalid background click returns an error before replacing a saved image. Pending strokes must be saved or discarded before navigation.

Images are processed sequentially in autorelease pools. `ProcessManager` owns cancellation and kills the native child. Partial preparation folders are retained for inspection but cannot be used as complete datasets. Reimport after a preparation failure uses a fresh folder. A completed preparation is reopenable without rerunning segmentation.

## Cameras

The original single-camera assumption has been removed. Camera keys include device/lens, physical focal length, EXIF 35 mm equivalent focal length, digital zoom and final dimensions. EXIF-unknown images receive separate cameras. A 35 mm equivalent focal length gives an initial focal length in pixels using the sensor-diagonal equivalence; it is a prior, not calibration. Digital zoom is used for grouping and is not multiplied into an already equivalent focal length a second time.

Each group is passed separately to COLMAP `feature_extractor` using an image list, SIMPLE_RADIAL camera model, explicit initial focal length where available and shared intrinsics only within that group. COLMAP then performs exhaustive matching and camera reconstruction. The group JSON is preserved alongside normalized source images and propagated into prepared frames. Different image dimensions are valid.

Training sees the same RGBA images and COLMAP coordinate frame as feature extraction. There is no per-image object bounding-box crop or arbitrary foreground scaling. The older approximate center-scaling EXIF workaround is deliberately not used.

## Snapshots and cache

`finalize_photos` selects enabled photos, requires review, materializes independent PNG copies and camera groups, and computes a SHA-256 fingerprint of ordered filenames, image bytes, group JSON and the preparation schema version. It does not use hard links for editable masks or originals. A training project stores its own input snapshot and fingerprint.

A first run adopts a fresh preparation folder. A later run creates a new sibling project, retaining the earlier output. Only when the current fingerprint matches an earlier reconstructed project are frame/mask/COLMAP checkpoints copied; Brush training is always reset when starting a new run. Alpha, image or camera changes invalidate the fingerprint. Source and mask edits never mutate an already running training snapshot. Existing upstream checkpoint validation still runs after reuse. The cache is for app-managed files; do not manually modify `work/`.

## Presets, progress and files

Plus maps to the original Fast parameters (8,000 steps, 1,200 px). Pro maps to Balanced (15,000 steps, 1,600 px). Both use the complete accepted image list and the same preprocessing. Legacy `high` metadata can still be read, but new High/video tasks are rejected in the CLI, frontend and pipeline.

Brush 0.3.0 runs with a private stderr pseudo-terminal so its official indicatif counter is emitted. The reader handles carriage-return updates and ANSI controls, and parses only the `current / total Steps` counter matching the selected preset. Heartbeats and diagnostic lines retain the latest measured percentage; initialization stays indeterminate until the first counter arrives. At 100% training the UI reports model export, and the overall task completes only after PLY validation/publication. Direct-child process groups and cancellation remain intact, with no shell wrapper. The pre-run time estimate is a rough range, not a completion guarantee.

The validated original `final.ply` is also copied to `results/final.ply`. The retained viewer supports transforms, crop/edit and explicit export; exported edits do not silently overwrite the original training result.

## Platform and privacy

Apple Silicon macOS 15+ only. New app identity `app.iags.desktop`; local state and projects under `IAGS`, independent of OOOSplat. The fork removes the telemetry endpoint and HTTP client dependency; installed processing is local. Upstream engine downloads retain their checksums and license files. FFmpeg files may exist in the upstream runtime archive but video input is disabled and FFmpeg is not a required processing engine.

Build with a complete Apple Command Line Tools SDK. This machine has a few renamed SDK headers; `scripts/sdk-overlay.mjs` creates read-only compiler aliases in ignored `.native-cache` without editing system files. The small Objective-C bridge avoids importing unrelated Swift Vision overlays. These aliases are only a developer environment workaround, not an app runtime dependency.

GitHub CI uses an arm64 macOS runner, checks source/build/tests and does not distribute applications. Runner architecture reference: [GitHub-hosted runners](https://docs.github.com/en/actions/reference/runners/github-hosted-runners).
