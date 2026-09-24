# Contributing to IA'GS

Target macOS 15+ and Apple Silicon. Use Node 24, Rust stable and Xcode Command Line Tools. Follow README.md for the development workflow. Run frontend tests/build and Rust tests/clippy before a change is submitted.

Do not commit photos, generated PLY files, local paths from personal datasets, credentials, logs or native engine binaries. Use small synthetic fixtures for tests. Preserve input files and reversible mask editing, and test camera grouping when image preparation changes. Do not silently fill holes, reconstruct missing pixels, reuse stale camera results or report estimated training completion as measured progress.

IA'GS derives from OOOSplat; preserve Apache-2.0 attribution, NOTICE and third-party licenses. Use IA'GS branding for modified software and identify its upstream origins honestly. The current installer is an ad-hoc signed test build. Source-check CI does not publish binaries; installer releases require separate verification.
