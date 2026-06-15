use anyhow::{Context, Result};

/// Manage Vox OCI images: build the current project as an OCI image and run it.
///
/// The previous `container init` action — which scanned `@py.import` declarations
/// and generated a Python pyproject.toml + Dockerfile — has been removed alongside
/// the retired Python glue lane (see AGENTS.md §VoxScript-First Glue Code).
pub async fn run(action: ContainerAction) -> Result<()> {
    match action {
        ContainerAction::Build { tag, runtime } => {
            let tag = tag.unwrap_or_else(|| "vox-app:latest".to_string());
            println!("📦 Building container image: {}", tag);

            let pref = runtime
                .unwrap_or_else(|| "auto".to_string())
                .parse::<vox_container::detect::RuntimePreference>()
                .map_err(|e| anyhow::anyhow!("{e}"))?;

            let cwd = std::env::current_dir()?;
            let opts = vox_container::BuildOpts {
                context_dir: cwd,
                dockerfile: None,
                tag,
                build_args: vec![],
            };

            tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
                let rt = vox_container::detect_runtime(pref).context(
                    "No container runtime available. Install Docker or Podman.",
                )?;
                let _ = rt.build(&opts).context("Container build failed")?;
                Ok(())
            })
            .await
            .map_err(|e| anyhow::anyhow!("container build task: {e}"))??;

            println!("✓ Image built successfully.");
        }
        ContainerAction::Run { image, port, runtime } => {
            let image = image.unwrap_or_else(|| "vox-app:latest".to_string());
            println!("🚀 Running container image: {}", image);

            let pref = runtime
                .unwrap_or_else(|| "auto".to_string())
                .parse::<vox_container::detect::RuntimePreference>()
                .map_err(|e| anyhow::anyhow!("{e}"))?;

            let opts = vox_container::RunOpts {
                image,
                ports: port.map(|p| vec![(p, p)]).unwrap_or_default(),
                env: vec![],
                volumes: vec![],
                detach: false,
                name: None,
                rm: true,
                ..Default::default()
            };

            tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
                let rt = vox_container::detect_runtime(pref).context(
                    "No container runtime available. Install Docker or Podman.",
                )?;
                rt.run(&opts).context("Container run failed")?;
                Ok(())
            })
            .await
            .map_err(|e| anyhow::anyhow!("container run task: {e}"))??;
        }
    }
    Ok(())
}

#[derive(Debug, clap::Subcommand, Clone)]
pub enum ContainerAction {
    /// Build an OCI container image from the current directory
    Build {
        /// Image tag (default: vox-app:latest)
        #[arg(short, long)]
        tag: Option<String>,
        /// Container runtime: auto, docker, podman (default: auto)
        #[arg(long, default_value = "auto")]
        runtime: Option<String>,
    },
    /// Run a container image locally
    Run {
        /// Image to run (default: vox-app:latest)
        #[arg(short, long)]
        image: Option<String>,
        /// Host port to expose (mapped to the same container port)
        #[arg(short, long)]
        port: Option<u16>,
        /// Container runtime: auto, docker, podman (default: auto)
        #[arg(long, default_value = "auto")]
        runtime: Option<String>,
    },
}
