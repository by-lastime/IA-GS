// Modified for IA'GS (2026-09-24); see docs/CHANGES_FROM_UPSTREAM.md.
// IA'GS CLI: photo-only preparation and reconstruction.
use clap::{Parser, Subcommand, ValueEnum};
use iags::{
    error::{Result, SplatError},
    pipeline::runner::{default_engine_paths, PipelineRunner},
    presets::Quality,
    process::{ProcessManager, ProcessUpdate},
};
use std::path::PathBuf;
#[derive(Clone, Copy, ValueEnum)]
enum Mode {
    Plus,
    Pro,
}
impl From<Mode> for Quality {
    fn from(mode: Mode) -> Self {
        match mode {
            Mode::Plus => Quality::Fast,
            Mode::Pro => Quality::Balanced,
        }
    }
}
#[derive(Parser)]
#[command(
    name = "iags-cli",
    version,
    about = "IA'GS · local photos → transparent PNG → Gaussian PLY (macOS)"
)]
struct Cli {
    #[arg(long, global = true)]
    engine_dir: Option<PathBuf>,
    #[command(subcommand)]
    command: Commands,
}
#[derive(Subcommand)]
enum Commands {
    /// Check the local COLMAP, Brush and native photo helper.
    Health,
    /// Read image count and dimensions (photos only).
    Probe { input: PathBuf },
    /// Copy originals, segment, normalize orientation/color/size; review before Generate.
    Prepare {
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Accept reviewed masks and build a versioned reconstruction input snapshot.
    Finalize {
        project: PathBuf,
        #[arg(long)]
        approve_all: bool,
    },
    /// Train already prepared/reviewed photos. No videos or High mode.
    Generate {
        project: PathBuf,
        #[arg(long, value_enum, default_value = "pro")]
        quality: Mode,
        #[arg(long)]
        projects_root: Option<PathBuf>,
        #[arg(long)]
        diagnostics: bool,
    },
}
#[tokio::main]
async fn main() {
    if let Err(error) = execute(Cli::parse()).await {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
async fn execute(cli: Cli) -> Result<()> {
    let engines = default_engine_paths(cli.engine_dir);
    match cli.command {
        Commands::Health => {
            println!(
                "{}",
                serde_json::to_string_pretty(&engines.check_all().await)?
            );
            let helper = engines.root.join("bin/iags-photo");
            if !helper.is_file() {
                return Err(SplatError::EngineMissing(helper.display().to_string()));
            }
            println!("Native photo helper ready");
        }
        Commands::Probe { input } => {
            iags::photos::validate_input(&input, Quality::Balanced)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&iags::video::analyze_image_sequence(&input)?)?
            );
        }
        Commands::Prepare { input, output } => {
            iags::photos::validate_input(&input, Quality::Balanced)?;
            if output.exists() {
                return Err(SplatError::Process(
                    "目标目录已存在，请指定一个新目录，避免覆盖照片项目".into(),
                ));
            }
            let manager = ProcessManager::new();
            iags::photos::run_preparation(
                &engines.root.join("bin/iags-photo"),
                &input,
                &output,
                &manager,
                Some(std::sync::Arc::new(|event| {
                    if let ProcessUpdate::Line { line, .. } = event {
                        eprintln!("{line}")
                    }
                })),
            )
            .await?;
            println!("{}", output.display());
        }
        Commands::Finalize {
            project,
            approve_all,
        } => {
            let output = iags::photos::materialize(&project, approve_all).await?;
            println!("{}", output.display());
        }
        Commands::Generate {
            project,
            quality,
            projects_root,
            diagnostics,
        } => {
            let quality = Quality::from(quality);
            iags::photos::validate_input(&project, quality)?;
            let input = if project.join("processing.json").is_file() {
                iags::photos::materialize(&project, false).await?
            } else {
                return Err(SplatError::Process(
                    "请先运行 prepare，再检查遮罩并运行 finalize".into(),
                ));
            };
            let input = std::fs::canonicalize(input)?;
            let root = projects_root.unwrap_or_else(|| {
                project
                    .parent()
                    .unwrap_or(std::path::Path::new("."))
                    .to_path_buf()
            });
            let root = std::fs::canonicalize(root)?;
            let runner = PipelineRunner::new(engines, |e| {
                if e.indeterminate {
                    eprintln!("{:?}: {}", e.stage, e.message);
                } else {
                    eprintln!("{:.1}% {:?}: {}", e.progress, e.stage, e.message);
                }
            });
            let result = if diagnostics {
                runner
                    .generate_for_diagnostics(&input, quality, &root)
                    .await?
            } else {
                runner.generate(&input, quality, &root).await?
            };
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
    }
    Ok(())
}
