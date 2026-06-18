use clap::{Parser, Subcommand};
use tracing::{Level, info};

mod channel;
mod download;
mod install;
mod proxy;
mod shell;
mod update;

#[derive(Parser)]
#[command(name = "voxup", about = "The Vox toolchain installer and multiplexer", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Install or update the Vox toolchain from GitHub Releases.
    Install {
        #[arg(default_value = "default")]
        profile: String,
    },
    /// Check for a newer Vox release and upgrade if one is available.
    Update,
    /// Proxy a vox command through the hermetic environment.
    Proxy {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();
    let cli = Cli::parse();
    match &cli.command {
        Commands::Install { profile } => {
            info!("Installing Vox (profile: {profile})");
            install::run_install(profile).await?;
        }
        Commands::Update => {
            info!("Checking for Vox updates…");
            update::run_update().await?;
        }
        Commands::Proxy { args } => {
            proxy::run_proxy(args).await?;
        }
    }
    Ok(())
}
