// Modified for IA'GS (2026-09-24); see docs/CHANGES_FROM_UPSTREAM.md.
use std::path::{Path, PathBuf};

use chrono::{Local, Utc};
use uuid::Uuid;

use crate::{
    error::{Result, SplatError},
    presets::Quality,
    project::{
        catalog, PipelineStateFile, ProjectInputType, ProjectMetadata, ProjectStatus,
        PROJECT_APP_ID,
    },
};

#[derive(Debug, Clone)]
pub struct ProjectPaths {
    pub id: Uuid,
    pub project: PathBuf,
    pub metadata: PathBuf,
    pub source: PathBuf,
    pub output: PathBuf,
    pub work: PathBuf,
    pub frames: PathBuf,
    pub masks: PathBuf,
    pub colmap: PathBuf,
    pub brush: PathBuf,
    pub logs: PathBuf,
    pub state: PathBuf,
}

impl ProjectPaths {
    pub fn existing(id: Uuid, project: PathBuf) -> Self {
        let source = project.join("source");
        let work = project.join("work");
        Self {
            id,
            metadata: project.join("project.json"),
            output: project.clone(),
            frames: work.join("frames"),
            masks: work.join("masks"),
            colmap: work.join("colmap"),
            brush: work.join("brush"),
            logs: project.join("logs"),
            state: project.join("state.json"),
            project,
            source,
            work,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProjectManager {
    projects_root: PathBuf,
    register_in_catalog: bool,
    task_name: Option<String>,
}

impl ProjectManager {
    pub fn system_default() -> Result<Self> {
        Ok(Self {
            projects_root: catalog::default_projects_root()?,
            register_in_catalog: true,
            task_name: None,
        })
    }
    pub fn with_root(projects_root: PathBuf) -> Self {
        Self {
            projects_root,
            register_in_catalog: true,
            task_name: None,
        }
    }
    pub fn for_diagnostics(projects_root: PathBuf) -> Self {
        Self {
            projects_root,
            register_in_catalog: false,
            task_name: None,
        }
    }

    pub fn with_task_name(mut self, name: &str) -> Result<Self> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 80 || name.chars().any(char::is_control) {
            return Err(SplatError::Process(
                "任务名称需为 1–80 个字符，且不能包含控制字符".into(),
            ));
        }
        self.task_name = Some(name.to_owned());
        Ok(self)
    }

    pub async fn validate_root(root: &Path) -> Result<()> {
        tokio::fs::create_dir_all(root).await?;
        if !root.is_dir() {
            return Err(SplatError::InvalidPath(root.to_path_buf()));
        }
        let probe = root.join(format!(".ooosplat-write-{}.tmp", Uuid::new_v4()));
        tokio::fs::write(&probe, b"IA'GS").await.map_err(|error| {
            SplatError::Process(format!("项目根目录不可写：{}（{error}）", root.display()))
        })?;
        tokio::fs::remove_file(probe).await?;
        Ok(())
    }

    pub async fn create(
        &self,
        input: &Path,
        quality: Quality,
    ) -> Result<(ProjectPaths, ProjectMetadata)> {
        crate::photos::validate_input(input, quality)?;
        let input_type = if input.is_dir() {
            crate::video::analyze_image_sequence(input)?;
            ProjectInputType::Images
        } else {
            validate_video_path(input)?;
            ProjectInputType::Video
        };
        Self::validate_root(&self.projects_root).await?;
        let id = Uuid::new_v4();
        let preparation = input.parent().filter(|root| {
            input.file_name().and_then(|n| n.to_str()) == Some(".training")
                && crate::photos::read_preparation(root).is_ok()
        });
        let preparation_name = preparation
            .and_then(|p| crate::photos::read_preparation(p).ok())
            .map(|m| m.name);
        let stem = if let Some(name) = self.task_name.as_deref().or(preparation_name.as_deref()) {
            name
        } else if input_type == ProjectInputType::Images {
            input
                .file_name()
                .and_then(|v| v.to_str())
                .unwrap_or("images")
        } else {
            input
                .file_stem()
                .and_then(|v| v.to_str())
                .unwrap_or("project")
        };
        let base = format!(
            "{}_{}_{}",
            Local::now().format("%Y%m%d-%H%M%S"),
            sanitize_project_name(stem),
            if quality == Quality::Fast {
                "Plus"
            } else {
                "Pro"
            }
        );
        let project = preparation
            .filter(|root| !root.join("project.json").exists())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| unique_project_path(&self.projects_root, &base));
        if let Some(prep) = preparation {
            if prep != project {
                for folder in ["originals", "output", "masks", "previews", "base-masks"] {
                    let dest = project.join(folder);
                    tokio::fs::create_dir_all(&dest).await?;
                    let mut entries = tokio::fs::read_dir(prep.join(folder)).await?;
                    while let Some(entry) = entries.next_entry().await? {
                        if entry.file_type().await?.is_file() {
                            tokio::fs::copy(entry.path(), dest.join(entry.file_name())).await?;
                        }
                    }
                }
                tokio::fs::copy(
                    prep.join("processing.json"),
                    project.join("processing.json"),
                )
                .await?;
            }
        }
        let source = project.join("source");
        let work = project.join("work");
        let frames = work.join("frames");
        let masks = work.join("masks");
        let colmap = work.join("colmap");
        let brush = work.join("brush");
        let logs = project.join("logs");
        for directory in [&source, &frames, &colmap, &brush, &logs] {
            tokio::fs::create_dir_all(directory).await?;
        }
        let stored_source = if input_type == ProjectInputType::Images {
            let images_dir = source.join("images");
            tokio::fs::create_dir_all(&images_dir).await?;
            for (index, path) in crate::video::list_images(input)?.iter().enumerate() {
                let dest = images_dir.join(crate::video::normalized_image_name(index, path)?);
                tokio::fs::copy(&path, &dest).await?;
            }
            if input.join("camera-groups.json").is_file() {
                tokio::fs::copy(
                    input.join("camera-groups.json"),
                    images_dir.join("camera-groups.json"),
                )
                .await?;
            }
            if input.join("fingerprint.txt").is_file() {
                tokio::fs::copy(
                    input.join("fingerprint.txt"),
                    source.join("fingerprint.txt"),
                )
                .await?;
            }
            images_dir
        } else {
            let extension = input
                .extension()
                .and_then(|v| v.to_str())
                .unwrap_or("mp4")
                .to_ascii_lowercase();
            let stored = source.join(format!("input.{extension}"));
            tokio::fs::copy(input, &stored).await?;
            stored
        };
        let now = Utc::now();
        let metadata = ProjectMetadata {
            schema_version: crate::project::metadata::schema_version(),
            app_id: PROJECT_APP_ID.into(),
            id,
            name: self.task_name.clone().unwrap_or_else(|| stem.to_owned()),
            created_at: now,
            started_at: Some(now),
            completed_at: None,
            duration_ms: None,
            status: ProjectStatus::Running,
            source_path: stored_source,
            input_type,
            quality,
            project_path: project.clone(),
            output_path: None,
            output: None,
            failure_message: None,
            model: "final.ply".into(),
            transform: Default::default(),
            editing: Default::default(),
        };
        let metadata_path = project.join("project.json");
        atomic_write_json(&metadata_path, &metadata).await?;
        let state = project.join("state.json");
        let mut initial_state = PipelineStateFile::created_for(quality, input_type);
        if let Some(prep) = preparation.filter(|p| *p != project) {
            let previous = tokio::fs::read(prep.join("state.json"))
                .await
                .ok()
                .and_then(|b| serde_json::from_slice::<PipelineStateFile>(&b).ok());
            let old_fingerprint = tokio::fs::read(prep.join("source/fingerprint.txt"))
                .await
                .ok();
            let new_fingerprint = tokio::fs::read(source.join("fingerprint.txt")).await.ok();
            if old_fingerprint.is_some() && old_fingerprint == new_fingerprint {
                if let Some(mut cached) = previous.filter(|s| s.reconstruction_complete) {
                    let src = prep.join("work");
                    let dst = work.clone();
                    tokio::task::spawn_blocking(move || -> Result<()> {
                        for name in ["frames", "masks", "colmap"] {
                            let input = src.join(name);
                            if input.is_dir() {
                                crate::photos::copy_tree(&input, &dst.join(name))?;
                            }
                        }
                        Ok(())
                    })
                    .await
                    .map_err(|e| SplatError::Process(e.to_string()))??;
                    cached.preset = quality;
                    cached.brush_complete = false;
                    cached.stage = crate::pipeline::PipelineStage::ValidatingReconstruction;
                    initial_state = cached;
                }
            }
        }
        atomic_write_json(&state, &initial_state).await?;
        if self.register_in_catalog {
            catalog::register_project(id, &project).await?;
        }
        Ok((
            ProjectPaths {
                id,
                project: project.clone(),
                metadata: metadata_path,
                source,
                output: project,
                work,
                frames,
                masks,
                colmap,
                brush,
                logs,
                state,
            },
            metadata,
        ))
    }

    pub async fn write_state(&self, path: &Path, state: &PipelineStateFile) -> Result<()> {
        atomic_write_json(path, state).await
    }
    pub async fn read_state(&self, path: &Path) -> Result<PipelineStateFile> {
        Ok(serde_json::from_slice(&tokio::fs::read(path).await?)?)
    }
    pub async fn write_metadata(&self, path: &Path, metadata: &ProjectMetadata) -> Result<()> {
        atomic_write_json(path, metadata).await
    }
}

pub fn sanitize_project_name(value: &str) -> String {
    let mut result = value
        .chars()
        .map(|ch| {
            if matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || ch.is_control()
            {
                '_'
            } else {
                ch
            }
        })
        .collect::<String>();
    result = result.trim().trim_end_matches(['.', ' ']).to_string();
    if result.is_empty() {
        result = "project".into();
    }
    result.chars().take(64).collect()
}

fn unique_project_path(root: &Path, base: &str) -> PathBuf {
    let initial = root.join(base);
    if !initial.exists() {
        return initial;
    }
    for suffix in 2..10_000 {
        let candidate = root.join(format!("{base}-{suffix}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    root.join(format!("{base}-{}", Uuid::new_v4()))
}

fn validate_video_path(path: &Path) -> Result<()> {
    if !path.is_file() {
        return Err(SplatError::InvalidPath(path.to_path_buf()));
    }
    match path
        .extension()
        .and_then(|v| v.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("mp4" | "mov") => Ok(()),
        _ => Err(SplatError::InvalidVideo("仅支持 MP4 或 MOV 文件".into())),
    }
}

pub async fn atomic_write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<()> {
    let temporary = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(value)?;
    let mut file = tokio::fs::File::create(&temporary).await?;
    use tokio::io::AsyncWriteExt;
    file.write_all(&bytes).await?;
    file.sync_all().await?;
    drop(file);
    atomic_replace(&temporary, path)?;
    Ok(())
}

pub(crate) async fn atomic_replace_file(source: &Path, destination: &Path) -> Result<()> {
    let source = source.to_path_buf();
    let destination = destination.to_path_buf();
    tokio::task::spawn_blocking(move || atomic_replace(&source, &destination))
        .await
        .map_err(|error| SplatError::Process(format!("原子发布任务失败：{error}")))?
}

#[cfg(windows)]
fn atomic_replace(source: &Path, destination: &Path) -> Result<()> {
    use std::{iter, os::windows::ffi::OsStrExt};
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };
    let source_wide = source
        .as_os_str()
        .encode_wide()
        .chain(iter::once(0))
        .collect::<Vec<_>>();
    let destination_wide = destination
        .as_os_str()
        .encode_wide()
        .chain(iter::once(0))
        .collect::<Vec<_>>();
    // SAFETY: Both paths are owned, NUL-terminated UTF-16 buffers that remain live for the call.
    let result = unsafe {
        MoveFileExW(
            source_wide.as_ptr(),
            destination_wide.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result != 0 {
        return Ok(());
    }
    let error = std::io::Error::last_os_error();
    if error.kind() == std::io::ErrorKind::CrossesDevices {
        copy_replace_for_encrypted_directory(source, destination)
    } else {
        Err(error.into())
    }
}

#[cfg(windows)]
fn copy_replace_for_encrypted_directory(source: &Path, destination: &Path) -> Result<()> {
    let backup_extension = destination
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| format!("{extension}.bak"))
        .unwrap_or_else(|| "bak".into());
    let backup = destination.with_extension(backup_extension);

    if backup.exists() {
        std::fs::remove_file(&backup)?;
    }
    if destination.exists() {
        std::fs::copy(destination, &backup)?;
        std::fs::OpenOptions::new()
            .write(true)
            .open(&backup)?
            .sync_all()?;
    }

    let replacement = (|| -> std::io::Result<()> {
        std::fs::copy(source, destination)?;
        std::fs::OpenOptions::new()
            .write(true)
            .open(destination)?
            .sync_all()?;
        std::fs::remove_file(source)?;
        Ok(())
    })();

    if let Err(error) = replacement {
        if backup.is_file() {
            let _ = std::fs::copy(&backup, destination);
        }
        return Err(error.into());
    }
    if backup.is_file() {
        std::fs::remove_file(backup)?;
    }
    Ok(())
}

#[cfg(not(windows))]
fn atomic_replace(source: &Path, destination: &Path) -> Result<()> {
    std::fs::rename(source, destination)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn named_task_persists_display_name_without_using_it_as_a_path() {
        let temporary = tempfile::tempdir().unwrap();
        let input = temporary.path().join("photos");
        std::fs::create_dir(&input).unwrap();
        for name in ["a.png", "b.png"] {
            image::RgbaImage::from_pixel(40, 40, image::Rgba([120, 90, 50, 255]))
                .save(input.join(name))
                .unwrap();
        }
        let root = temporary.path().join("projects");
        let manager = ProjectManager::for_diagnostics(root.clone())
            .with_task_name("  玉饰 / 第一组  ")
            .unwrap();
        let (paths, metadata) = manager.create(&input, Quality::Fast).await.unwrap();
        assert_eq!(metadata.name, "玉饰 / 第一组");
        assert_eq!(paths.project.parent(), Some(root.as_path()));
        let persisted: ProjectMetadata =
            serde_json::from_slice(&std::fs::read(paths.metadata).unwrap()).unwrap();
        assert_eq!(persisted.name, "玉饰 / 第一组");
    }

    #[test]
    fn rejects_empty_overlong_and_control_character_task_names() {
        for name in ["  ".to_string(), "a".repeat(81), "line\nbreak".to_string()] {
            assert!(ProjectManager::for_diagnostics(PathBuf::from("unused"))
                .with_task_name(&name)
                .is_err());
        }
    }

    #[test]
    fn sanitizes_windows_names() {
        assert_eq!(sanitize_project_name("房子:轨迹?.mp4"), "房子_轨迹_.mp4");
    }

    #[test]
    fn limits_names_and_avoids_collisions() {
        let temporary = tempfile::tempdir().unwrap();
        let long = "a".repeat(100);
        assert_eq!(sanitize_project_name(&long).chars().count(), 64);
        let first = temporary.path().join("20260101-120000_demo");
        std::fs::create_dir(&first).unwrap();
        assert_eq!(
            unique_project_path(temporary.path(), "20260101-120000_demo"),
            temporary.path().join("20260101-120000_demo-2")
        );
    }

    #[tokio::test]
    async fn atomically_replaces_existing_json() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("settings.json");
        atomic_write_json(&path, &serde_json::json!({"value": 1}))
            .await
            .unwrap();
        atomic_write_json(&path, &serde_json::json!({"value": 2}))
            .await
            .unwrap();
        let value: serde_json::Value =
            serde_json::from_slice(&tokio::fs::read(path).await.unwrap()).unwrap();
        assert_eq!(value["value"], 2);
    }

    #[cfg(windows)]
    #[test]
    fn copy_replacement_keeps_latest_data_and_cleans_temporary_files() {
        let temporary = tempfile::tempdir().unwrap();
        let source = temporary.path().join("settings.json.tmp");
        let destination = temporary.path().join("settings.json");
        let backup = temporary.path().join("settings.json.bak");
        std::fs::write(&source, br#"{"value":2}"#).unwrap();
        std::fs::write(&destination, br#"{"value":1}"#).unwrap();

        copy_replace_for_encrypted_directory(&source, &destination).unwrap();

        assert_eq!(std::fs::read(&destination).unwrap(), br#"{"value":2}"#);
        assert!(!source.exists());
        assert!(!backup.exists());
    }

    #[tokio::test]
    async fn rejects_video_projects_even_with_unicode_names() {
        let temporary = tempfile::tempdir().unwrap();
        let input = temporary.path().join("鞋子 scan.mp4");
        std::fs::write(&input, b"test").unwrap();
        assert!(
            ProjectManager::for_diagnostics(temporary.path().join("项目 Root"))
                .create(&input, Quality::Balanced)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn copies_image_sequences_with_stable_names() {
        let temporary = tempfile::tempdir().unwrap();
        let input = temporary.path().join("图片序列");
        std::fs::create_dir(&input).unwrap();
        image::RgbImage::new(2, 2)
            .save(input.join("image10.jpg"))
            .unwrap();
        image::RgbImage::new(2, 2)
            .save(input.join("image2.png"))
            .unwrap();
        let (paths, metadata) = ProjectManager::for_diagnostics(temporary.path().join("projects"))
            .create(&input, Quality::Balanced)
            .await
            .unwrap();
        assert_eq!(metadata.input_type, ProjectInputType::Images);
        assert!(metadata.source_path.is_dir());
        assert!(metadata.source_path.join("frame_000001.png").is_file());
        assert!(metadata.source_path.join("frame_000002.jpg").is_file());
        let state: PipelineStateFile =
            serde_json::from_slice(&tokio::fs::read(paths.state).await.unwrap()).unwrap();
        assert_eq!(state.input_type, ProjectInputType::Images);
    }
}
