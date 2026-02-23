mod commands;
pub mod templates;
pub mod v0;

use clap::Parser;

#[derive(Parser)]
#[command(name = "vox", about = "The Vox AI-native language compiler", version)]
enum Cli {
    /// Build a Vox source file, producing TypeScript output
    Build {
        /// Path to the .vox source file
        #[arg(required = true)]
        file: std::path::PathBuf,
        /// Output directory for generated files
        #[arg(short, long, default_value = "dist")]
        out_dir: std::path::PathBuf,
    },
    /// Type-check a Vox source file without producing output
    Check {
        /// Path to the .vox source file
        #[arg(required = true)]
        file: std::path::PathBuf,
    },
    /// Run tests for the Vox program
    Test {
        /// Path to the .vox source file
        #[arg(required = true)]
        file: std::path::PathBuf,
    },
    /// Run a Vox source file using the backend
    Run {
        /// Path to the .vox source file
        #[arg(required = true)]
        file: std::path::PathBuf,
        /// Arguments to pass to the application
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Bundle a Vox source file into a complete web application
    Bundle {
        /// Path to the .vox source file
        #[arg(required = true)]
        file: std::path::PathBuf,
        /// Output directory for generated files
        #[arg(short, long, default_value = "dist")]
        out_dir: std::path::PathBuf,
        /// Compile for a specific target triple (e.g. x86_64-unknown-linux-gnu)
        #[arg(long)]
        target: Option<String>,
        /// Build in release mode (default: true)
        #[arg(long, default_value = "true")]
        release: bool,
    },
    /// Format a Vox source file in place
    Fmt {
        /// Path to the .vox source file
        #[arg(required = true)]
        file: std::path::PathBuf,
    },
    /// Install a component or package via vox-pm
    Install {
        /// Name of package to retrieve
        #[arg(required = true)]
        package_name: String,
    },
    /// Start the Vox Language Server
    Lsp,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    match cli {
        Cli::Build { file, out_dir } => {
            commands::build::run(&file, &out_dir).await?;
        }
        Cli::Check { file } => {
            commands::check::run(&file)?;
        }
        Cli::Run { file, args } => {
            commands::run::run(&file, &args).await?;
        }
        Cli::Test { file } => {
            commands::test::run(&file).await?;
        }
        Cli::Bundle {
            file,
            out_dir,
            target,
            release,
        } => {
            commands::bundle::run(&file, &out_dir, target.as_deref(), release).await?;
        }
        Cli::Fmt { file } => {
            commands::fmt::run(&file)?;
        }
        Cli::Install { package_name } => {
            commands::install::run(&package_name).await?;
        }
        Cli::Lsp => {
            commands::lsp::run()?;
        }
    }

    Ok(())
}
