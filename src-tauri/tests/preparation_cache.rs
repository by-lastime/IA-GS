// IA'GS integration regression, opt-in because it uses an existing local reconstruction.
use iags::{
    photos,
    presets::Quality,
    project::{PipelineStateFile, ProjectManager},
};
#[tokio::test]
#[ignore = "Set IAGS_TEST_PREPARATION to a completed, disposable IA'GS photo project"]
async fn reuses_cameras_between_modes_but_invalidates_changed_alpha() {
    let source = std::path::PathBuf::from(std::env::var("IAGS_TEST_PREPARATION").unwrap());
    let temporary = tempfile::tempdir().unwrap();
    let input = photos::materialize(&source, false).await.unwrap();
    let manager = ProjectManager::for_diagnostics(temporary.path().to_path_buf());
    let (cached, metadata) = manager.create(&input, Quality::Balanced).await.unwrap();
    let state: PipelineStateFile =
        serde_json::from_slice(&std::fs::read(&cached.state).unwrap()).unwrap();
    assert_eq!(metadata.quality, Quality::Balanced);
    assert!(state.features_complete && state.matching_complete && state.reconstruction_complete);
    assert!(!state.brush_complete);
    assert!(cached.colmap.join("database.db").is_file());
    // Editing a result in the NEW disposable copy must invalidate cached poses.
    let manifest = photos::read_preparation(&cached.project).unwrap();
    let first = cached
        .project
        .join(format!("output/{}.png", manifest.photos[0].id));
    let mut rgba = image::open(&first).unwrap().to_rgba8();
    let alpha = rgba.get_pixel(0, 0)[3];
    rgba.get_pixel_mut(0, 0)[3] = 255 - alpha;
    rgba.save(&first).unwrap();
    let changed = photos::materialize(&cached.project, false).await.unwrap();
    let (fresh, _) = manager.create(&changed, Quality::Balanced).await.unwrap();
    let state: PipelineStateFile =
        serde_json::from_slice(&std::fs::read(&fresh.state).unwrap()).unwrap();
    assert!(
        !state.features_complete
            && !state.matching_complete
            && !state.reconstruction_complete
            && !state.brush_complete
    );
    assert!(!fresh.colmap.join("database.db").exists());
}
