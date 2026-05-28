use clap::{Parser, Subcommand};
use tracing::{info, Level};

mod install;
mod manifest;

#[derive(Parser)]
#[command(name = "voxup", about = "The Vox toolchain multiplexer")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Install or update the Vox toolchain
    Install {
        #[arg(default_value = "default")]
        profile: String,
    },
    /// Run the proxy for a vox command
    Proxy {
        /// The vox arguments
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    let cli = Cli::parse();

    match &cli.command {
        Commands::Install { profile } => {
            info!("Installing voxup profile: {}", profile);
            install::run_install(profile).await?;
        }
        Commands::Proxy { args } => {
            info!("voxup proxy intercept: forwarding args: {:?}", args);
            install::run_proxy(args).await?;
        }
    }

    Ok(())
}
