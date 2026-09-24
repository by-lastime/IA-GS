// IA'GS modifications: native photo preparation, review and camera metadata.
use crate::{
    error::{Result, SplatError},
    presets::Quality,
    process::{ProcessManager, ProcessSpec, ProcessUpdate},
    project::manager::atomic_write_json,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashSet},
    path::{Path, PathBuf},
    sync::Arc,
};
use tauri::{Emitter, Manager, State};
use tokio::sync::Mutex;

#[derive(Default)]
pub struct PhotoController {
    active: Mutex<Option<ProcessManager>>,
    roots: Mutex<HashSet<PathBuf>>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Photo {
    pub id: String,
    pub original_name: String,
    pub width: u32,
    pub height: u32,
    pub original_width: u32,
    pub original_height: u32,
    pub camera_key: String,
    pub focal35: Option<f64>,
    pub digital_zoom: Option<f64>,
    pub instances: u32,
    pub coverage: f64,
    pub warnings: Vec<String>,
    pub enabled: bool,
    pub reviewed: bool,
    pub sha256: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preparation {
    pub version: u32,
    pub app: String,
    pub algorithm: String,
    pub max_dimension: u32,
    pub name: String,
    pub photos: Vec<Photo>,
    pub complete: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhotoSession {
    pub root: PathBuf,
    pub manifest: Preparation,
}
#[derive(Deserialize, Serialize)]
pub struct Stroke {
    pub erase: bool,
    pub radius: f64,
    pub points: Vec<[f64; 2]>,
}
#[derive(Deserialize, Serialize)]
pub struct PhotoEdit {
    pub id: String,
    pub strokes: Vec<Stroke>,
    pub select: Option<[f64; 2]>,
    pub enabled: bool,
    pub reviewed: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraGroup {
    pub images: Vec<String>,
    pub width: u32,
    pub height: u32,
    pub focal_pixels: Option<f64>,
}

pub fn validate_input(input: &Path, quality: Quality) -> Result<()> {
    if !input.is_dir() {
        return Err(SplatError::Process(
            "IA'GS 仅接受照片文件夹，暂不支持视频".into(),
        ));
    }
    if quality == Quality::High {
        return Err(SplatError::Process("请选择 Plus 或 Pro 模式".into()));
    }
    Ok(())
}
pub fn read_preparation(root: &Path) -> Result<Preparation> {
    let manifest: Preparation =
        serde_json::from_slice(&std::fs::read(root.join("processing.json"))?)?;
    if manifest.app != "IA'GS" || manifest.version != 1 || manifest.photos.len() > 10000 {
        return Err(SplatError::Process("不是支持的 IA'GS 照片项目".into()));
    }
    let mut names = HashSet::new();
    for p in &manifest.photos {
        if p.id.len() != 12
            || !p.id.starts_with("frame_")
            || !p.id[6..].bytes().all(|b| b.is_ascii_digit())
            || !names.insert(p.id.clone())
            || p.width == 0
            || p.height == 0
            || p.width > 4096
            || p.height > 4096
        {
            return Err(SplatError::Process("照片清单包含无效条目".into()));
        }
    }
    Ok(manifest)
}
fn helper(app: &tauri::AppHandle) -> PathBuf {
    let resource = app
        .path()
        .resource_dir()
        .ok()
        .map(|p| p.join("engines/macos/arm64/bin/iags-photo"));
    resource.filter(|p| p.is_file()).unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../engines/macos/arm64/bin/iags-photo")
    })
}
async fn authorize(
    app: &tauri::AppHandle,
    state: &PhotoController,
    root: PathBuf,
) -> Result<PhotoSession> {
    let root = tokio::fs::canonicalize(root).await?;
    let manifest = read_preparation(&root)?;
    // Only generated display assets are accessible through the webview protocol.
    for sub in ["output", "previews", "masks"] {
        app.asset_protocol_scope()
            .allow_directory(root.join(sub), false)
            .map_err(|e| SplatError::Process(e.to_string()))?;
    }
    state.roots.lock().await.insert(root.clone());
    Ok(PhotoSession { root, manifest })
}
async fn checked_root(state: &PhotoController, root: &str) -> Result<PathBuf> {
    let root = tokio::fs::canonicalize(root).await?;
    if !state.roots.lock().await.contains(&root) {
        return Err(SplatError::Process("请先打开这个照片项目".into()));
    }
    Ok(root)
}
async fn acquire(state: &PhotoController) -> Result<ProcessManager> {
    let mut active = state.active.lock().await;
    if active.is_some() {
        return Err(SplatError::Process("正在处理照片，请稍后再试".into()));
    }
    let manager = ProcessManager::new();
    *active = Some(manager.clone());
    Ok(manager)
}
pub async fn run_preparation(
    executable: &Path,
    input: &Path,
    root: &Path,
    manager: &ProcessManager,
    observer: Option<crate::process::ProcessObserver>,
) -> Result<()> {
    let result = manager
        .run(ProcessSpec {
            executable: executable.into(),
            args: vec!["prepare".into(), input.into(), root.into(), "2400".into()],
            working_directory: None,
            log_path: Some(root.join("logs/preparation.log")),
            observer,
        })
        .await?;
    if !result.success {
        return Err(SplatError::Process(result.failure_detail()));
    }
    Ok(())
}
#[tauri::command]
pub async fn prepare_photos(
    app: tauri::AppHandle,
    state: State<'_, PhotoController>,
    input: String,
    projects_root: String,
    task_name: Option<String>,
) -> Result<PhotoSession> {
    validate_input(Path::new(&input), Quality::Balanced)?;
    crate::project::ProjectManager::validate_root(Path::new(&projects_root)).await?;
    let manager = acquire(&state).await?;
    let name = crate::project::manager::sanitize_project_name(
        task_name
            .as_deref()
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| {
                Path::new(&input)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Photos")
            }),
    );
    let root = Path::new(&projects_root).join(format!(
        "{}_{}_{}",
        chrono::Local::now().format("%Y%m%d-%H%M%S"),
        name,
        &uuid::Uuid::new_v4().to_string()[..8]
    ));
    let emitter = app.clone();
    let observer = Arc::new(move |update| {
        if let ProcessUpdate::Line { line, .. } = update {
            if let Ok(event) = serde_json::from_str::<serde_json::Value>(&line) {
                let _ = emitter.emit("photo-progress", event);
            }
        }
    });
    let result = run_preparation(
        &helper(&app),
        Path::new(&input),
        &root,
        &manager,
        Some(observer),
    )
    .await;
    *state.active.lock().await = None;
    result?;
    if let Some(name) = task_name {
        let mut manifest = read_preparation(&root)?;
        manifest.name = name.trim().chars().take(80).collect();
        atomic_write_json(&root.join("processing.json"), &manifest).await?;
    }
    authorize(&app, &state, root).await
}
#[tauri::command]
pub async fn open_preparation(
    app: tauri::AppHandle,
    state: State<'_, PhotoController>,
    root: String,
) -> Result<PhotoSession> {
    authorize(&app, &state, PathBuf::from(root)).await
}
#[tauri::command]
pub async fn cancel_preparation(state: State<'_, PhotoController>) -> Result<()> {
    if let Some(manager) = state.active.lock().await.as_ref() {
        manager.cancel();
    }
    Ok(())
}
#[tauri::command]
pub async fn edit_photo(
    app: tauri::AppHandle,
    state: State<'_, PhotoController>,
    root: String,
    edit: PhotoEdit,
) -> Result<PhotoSession> {
    let root = checked_root(&state, &root).await?;
    let manifest = read_preparation(&root)?;
    if !manifest.photos.iter().any(|p| p.id == edit.id) {
        return Err(SplatError::Process("找不到照片".into()));
    }
    if edit.strokes.len() > 500
        || edit.strokes.iter().any(|s| {
            !s.radius.is_finite()
                || s.radius <= 0.0
                || s.radius > 0.25
                || s.points.len() > 20000
                || s.points
                    .iter()
                    .flatten()
                    .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
        })
        || edit
            .select
            .is_some_and(|p| p.iter().any(|v| !v.is_finite() || !(0.0..=1.0).contains(v)))
    {
        return Err(SplatError::Process("无效的遮罩笔画".into()));
    }
    let manager = acquire(&state).await?;
    let result = async {
        let edit_path = root.join(".photo-edit.json");
        atomic_write_json(&edit_path, &edit).await?;
        let result = manager
            .run(ProcessSpec {
                executable: helper(&app),
                args: vec!["edit".into(), root.clone().into(), edit_path.clone().into()],
                working_directory: None,
                log_path: Some(root.join("logs/preparation.log")),
                observer: None,
            })
            .await?;
        let _ = tokio::fs::remove_file(&edit_path).await;
        if !result.success {
            return Err(SplatError::Process(result.failure_detail()));
        }
        // A materialized input is an immutable snapshot; discard it after edits.
        if root.join(".training").is_dir() {
            tokio::fs::remove_dir_all(root.join(".training")).await?;
        }
        authorize(&app, &state, root).await
    }
    .await;
    *state.active.lock().await = None;
    result
}

pub fn camera_groups(photos: &[Photo]) -> Vec<CameraGroup> {
    let mut groups: BTreeMap<String, CameraGroup> = BTreeMap::new();
    for (i, p) in photos.iter().enumerate() {
        let focal = p
            .focal35
            .filter(|f| f.is_finite() && *f > 0.0)
            .map(|f| f * ((p.width as f64).hypot(p.height as f64)) / (36_f64.hypot(24.0)));
        let key = format!("{}|{}x{}", p.camera_key, p.width, p.height);
        let group = groups.entry(key).or_insert(CameraGroup {
            images: vec![],
            width: p.width,
            height: p.height,
            focal_pixels: focal,
        });
        group.images.push(format!("frame_{:06}.png", i + 1));
    }
    groups.into_values().collect()
}

pub fn fingerprint(directory: &Path) -> Result<String> {
    use std::io::Read;
    let mut hash = Sha256::new();
    hash.update(b"iags-photo-v1");
    let mut files = crate::video::list_images(directory)?;
    files.push(directory.join("camera-groups.json"));
    for file in files {
        hash.update(file.file_name().unwrap().to_string_lossy().as_bytes());
        let mut input = std::fs::File::open(file)?;
        let mut buffer = [0u8; 65536];
        loop {
            let n = input.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            hash.update(&buffer[..n]);
        }
    }
    Ok(format!("{:x}", hash.finalize()))
}
pub fn copy_tree(source: &Path, destination: &Path) -> Result<()> {
    std::fs::create_dir_all(destination)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            return Err(SplatError::Process("重建缓存不应包含符号链接".into()));
        }
        if kind.is_dir() {
            copy_tree(&entry.path(), &destination.join(entry.file_name()))?;
        } else if kind.is_file() {
            std::fs::copy(entry.path(), destination.join(entry.file_name()))?;
        }
    }
    Ok(())
}
pub async fn materialize(root: &Path, approve_all: bool) -> Result<PathBuf> {
    let mut manifest = read_preparation(root)?;
    if !manifest.complete {
        return Err(SplatError::Process(
            "照片处理未完成，请重新导入原照片".into(),
        ));
    }
    let photos: Vec<Photo> = manifest
        .photos
        .iter()
        .filter(|p| p.enabled)
        .cloned()
        .collect();
    if photos.len() < 2 {
        return Err(SplatError::Process(
            "至少保留两张照片；建议 30 张以上、连续重叠视角".into(),
        ));
    }
    if !approve_all && photos.iter().any(|p| !p.reviewed) {
        return Err(SplatError::Process("请先检查照片或确认已检查全部".into()));
    }
    let staging = root.join(format!(".training-{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir(&staging).await?;
    let result = async {
        for (i, p) in photos.iter().enumerate() {
            tokio::fs::copy(
                root.join(format!("output/{}.png", p.id)),
                staging.join(format!("frame_{:06}.png", i + 1)),
            )
            .await?;
        }
        atomic_write_json(&staging.join("camera-groups.json"), &camera_groups(&photos)).await?;
        let fingerprint = tokio::task::spawn_blocking({
            let staging = staging.clone();
            move || fingerprint(&staging)
        })
        .await
        .map_err(|e| SplatError::Process(e.to_string()))??;
        tokio::fs::write(staging.join("fingerprint.txt"), fingerprint).await?;
        let dest = root.join(".training");
        if dest.exists() {
            tokio::fs::remove_dir_all(&dest).await?;
        }
        tokio::fs::rename(&staging, &dest).await?;
        if approve_all {
            for p in &mut manifest.photos {
                if p.enabled {
                    p.reviewed = true;
                }
            }
            atomic_write_json(&root.join("processing.json"), &manifest).await?;
        }
        Ok(dest)
    }
    .await;
    if result.is_err() {
        let _ = tokio::fs::remove_dir_all(staging).await;
    }
    result
}
#[tauri::command]
pub async fn finalize_photos(
    state: State<'_, PhotoController>,
    root: String,
    approve_all: bool,
) -> Result<PathBuf> {
    let root = checked_root(&state, &root).await?;
    let _manager = acquire(&state).await?;
    let result = materialize(&root, approve_all).await;
    *state.active.lock().await = None;
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fingerprint_changes_with_alpha_or_camera_settings() {
        let dir = tempfile::tempdir().unwrap();
        let mut image = image::RgbaImage::from_pixel(4, 4, image::Rgba([100, 110, 120, 255]));
        image.save(dir.path().join("frame_000001.png")).unwrap();
        std::fs::write(dir.path().join("camera-groups.json"), "[]").unwrap();
        let a = fingerprint(dir.path()).unwrap();
        image.put_pixel(1, 1, image::Rgba([100, 110, 120, 0]));
        image.save(dir.path().join("frame_000001.png")).unwrap();
        let b = fingerprint(dir.path()).unwrap();
        assert_ne!(a, b);
        std::fs::write(
            dir.path().join("camera-groups.json"),
            "[{\"focalPixels\":100}]",
        )
        .unwrap();
        assert_ne!(b, fingerprint(dir.path()).unwrap());
    }
    #[test]
    fn rejects_video_and_high() {
        let d = tempfile::tempdir().unwrap();
        assert!(validate_input(Path::new("a.mp4"), Quality::Fast).is_err());
        assert!(validate_input(d.path(), Quality::High).is_err());
        assert!(validate_input(d.path(), Quality::Balanced).is_ok());
    }
    fn photo(id: &str, key: &str) -> Photo {
        Photo {
            id: id.into(),
            original_name: id.into(),
            width: 2400,
            height: 1800,
            original_width: 4000,
            original_height: 3000,
            camera_key: key.into(),
            focal35: Some(50.0),
            digital_zoom: Some(1.0),
            instances: 1,
            coverage: 0.4,
            warnings: vec![],
            enabled: true,
            reviewed: false,
            sha256: "test".into(),
        }
    }
    #[test]
    fn groups_lenses_and_keeps_scaled_intrinsics() {
        let p = vec![
            photo("frame_000001", "wide"),
            photo("frame_000002", "tele"),
            photo("frame_000003", "wide"),
        ];
        let g = camera_groups(&p);
        assert_eq!(g.len(), 2);
        assert_eq!(g.iter().map(|g| g.images.len()).sum::<usize>(), 3);
        assert!((g[0].focal_pixels.unwrap() - 3466.876).abs() < 0.01);
    }
    #[tokio::test]
    async fn review_gate_and_exclusions() {
        let d = tempfile::tempdir().unwrap();
        let mut m = Preparation {
            version: 1,
            app: "IA'GS".into(),
            algorithm: "test".into(),
            max_dimension: 2400,
            name: "test".into(),
            photos: vec![photo("frame_000001", "a"), photo("frame_000002", "b")],
            complete: true,
        };
        atomic_write_json(&d.path().join("processing.json"), &m)
            .await
            .unwrap();
        assert!(materialize(d.path(), false).await.is_err());
        m.photos[0].enabled = false;
        atomic_write_json(&d.path().join("processing.json"), &m)
            .await
            .unwrap();
        assert!(materialize(d.path(), true).await.is_err());
    }
}
