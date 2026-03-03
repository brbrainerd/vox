use anyhow::{Context, Result};
use std::path::PathBuf;
use vox_container::{run_py_setup, PySetupOpts};
use vox_ast::decl::Decl;

/// Manage Vox container environments, specifically Python-aware OCI images.
pub async fn run(action: ContainerAction) -> Result<()> {
    match action {
        ContainerAction::Init { file, out_dir, dockerfile, project_name } => {
            println!("🛠️ Initializing Python container environment...");

            // 1. Parse Vox file to extract @py.import declarations
            let source = std::fs::read_to_string(&file)
                .with_context(|| format!("Failed to read source file: {}", file.display()))?;
            let tokens = vox_lexer::cursor::lex(&source);
            let module = vox_parser::parser::parse(tokens)
                .map_err(|e| anyhow::anyhow!("Parse errors in {}: {:?}", file.display(), e))?;

            let py_imports: Vec<String> = module.declarations.iter()
                .filter_map(|d| match d {
                    Decl::PyImport(p) => Some(p.module.clone()),
                    _ => None,
                })
                .collect();

            if py_imports.is_empty() {
                println!("ℹ️ No @py.import declarations found in {}. Nothing to do.", file.display());
                return Ok(());
            }

            // 2. Determine project name
            let name = project_name.unwrap_or_else(|| {
                file.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("vox-app")
                    .to_string()
            });

            // 3. Run setup
            let opts = PySetupOpts {
                project_name: name,
                py_imports,
                generate_dockerfile: dockerfile,
                out_dir: out_dir.unwrap_or_else(|| PathBuf::from(".")),
            };

            // run_py_setup is sync, so wrap it if needed, but here we can just call it
            run_py_setup(&opts).context("Python container setup failed")?;
        }
        ContainerAction::Build { tag, runtime } => {
            let tag = tag.unwrap_or_else(|| "vox-app:latest".to_string());
            println!("📦 Building container image: {}", tag);

            let pref = runtime.unwrap_or_else(|| "auto".to_string())
                .parse::<vox_container::detect::RuntimePreference>()
                .map_err(|e| anyhow::anyhow!("{e}"))?;

            let rt = vox_container::detect_runtime(pref).context(
                "No container runtime available. Install Docker or Podman.",
            )?;

            let opts = vox_container::BuildOpts {
                context_dir: std::env::current_dir()?,
                dockerfile: None,
                tag,
                build_args: vec![],
            };
            rt.build(&opts).context("Container build failed")?;
            println!("✓ Image built successfully.");
        }
        ContainerAction::Run { image, port, runtime } => {
            let image = image.unwrap_or_else(|| "vox-app:latest".to_string());
            println!("🚀 Running container image: {}", image);

            let pref = runtime.unwrap_or_else(|| "auto".to_string())
                .parse::<vox_container::detect::RuntimePreference>()
                .map_err(|e| anyhow::anyhow!("{e}"))?;

            let rt = vox_container::detect_runtime(pref).context(
                "No container runtime available. Install Docker or Podman.",
            )?;

            let opts = vox_container::RunOpts {
                image,
                ports: port.map(|p| vec![(p, p)]).unwrap_or_default(),
                env: vec![],
                volumes: vec![],
                detach: false,
                name: None,
                rm: true,
            };
            rt.run(&opts).context("Container run failed")?;
        }
    }
    Ok(())
}

#[derive(Debug, clap::Subcommand, Clone)]
pub enum ContainerAction {
    /// Detect Python/uv/CUDA and generate pyproject.toml; run `uv sync`
    Init {
        /// Path to the .vox source file to parse for @py.import declarations
        #[arg(short, long, required = true)]
        file: std::path::PathBuf,
        /// Output directory for pyproject.toml (and optional Dockerfile)
        #[arg(short, long)]
        out_dir: Option<std::path::PathBuf>,
        /// Also generate a CUDA-aware Dockerfile in the output directory
        #[arg(long)]
        dockerfile: bool,
        /// Override the project name used in pyproject.toml
        #[arg(long)]
        project_name: Option<String>,
    },
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
