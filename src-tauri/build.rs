// Modified for IA'GS (2026-09-24); see docs/CHANGES_FROM_UPSTREAM.md.
// IA'GS modification: telemetry is disabled; compile only on supported Macs.
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos")
        || std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() != Ok("aarch64")
    {
        panic!("IA'GS currently supports Apple Silicon macOS only");
    }
    tauri_build::build()
}
