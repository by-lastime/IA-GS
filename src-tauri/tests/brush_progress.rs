//! Opt-in Metal smoke test against a local, already reconstructed dataset.
use iags::{
    engines::brush,
    presets::Quality,
    process::{ProcessManager, ProcessObserver, ProcessUpdate},
};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

#[tokio::test]
#[ignore = "Requires Metal and IAGS_TEST_BRUSH_DATASET pointing to a local COLMAP dataset"]
async fn real_brush_reports_live_iterations_and_exports_a_model() {
    let dataset = PathBuf::from(std::env::var_os("IAGS_TEST_BRUSH_DATASET").expect("dataset path"));
    let engine =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../engines/macos/arm64/bin/brush_app");
    let output = tempfile::tempdir().unwrap();
    let lines = Arc::new(Mutex::new(Vec::new()));
    let captured = lines.clone();
    let observer: ProcessObserver = Arc::new(move |update| {
        if let ProcessUpdate::Line { line, .. } = update {
            if line.contains(" Steps") {
                captured.lock().unwrap().push(line);
            }
        }
    });
    let mut preset = Quality::Fast.preset();
    preset.brush_iterations = 300;
    preset.brush_max_resolution = 256;
    let model = brush::train(
        &engine,
        &dataset,
        output.path(),
        preset,
        output.path().join("brush.log"),
        &ProcessManager::new(),
        Some(observer),
    )
    .await
    .unwrap();
    let lines = lines.lock().unwrap();
    assert!(lines.len() >= 3, "expected live counters, got {lines:?}");
    assert!(lines.iter().any(|line| {
        let Some((left, right)) = line
            .split_once(" Steps")
            .and_then(|(prefix, _)| prefix.rsplit_once('/'))
        else {
            return false;
        };
        let current = left
            .split_whitespace()
            .last()
            .and_then(|v| v.parse::<u64>().ok());
        right.trim() == "300" && current.is_some_and(|v| v > 0 && v < 300)
    }));
    assert!(std::fs::metadata(model).unwrap().len() > 1024);
    println!(
        "{} real progress updates; first: {}; last: {}",
        lines.len(),
        lines.first().unwrap(),
        lines.last().unwrap()
    );
}
