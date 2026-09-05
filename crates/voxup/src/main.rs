use clap::{Parser, Subcommand};
use tracing::{Level, info};

mod channel;
mod download;
mod install;
mod profiles;
mod proxy;
mod shell;
mod update;

#[derive(Parser)]
#[command(
    name = "voxup",
    about = "The Vox toolchain installer and multiplexer",
    version
)]
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
        /// Install a specific release tag instead of the latest stable
        /// release — e.g. a release-candidate prerelease (`v0.6.0-rc.4`).
        /// `/releases/latest` excludes prereleases, so this is the only way
        /// to fetch one.
        #[arg(long)]
        tag: Option<String>,
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

    let args: Vec<String> = std::env::args().collect();
    let current_exe = std::env::current_exe().ok();
    let binary_name = current_exe
        .as_ref()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("");
    let is_proxied =
        binary_name.eq_ignore_ascii_case("vox") || binary_name.eq_ignore_ascii_case("vox.exe");

    if is_proxied {
        let proxy_args = if args.is_empty() {
            Vec::new()
        } else {
            args[1..].to_vec()
        };
        proxy::run_proxy(&proxy_args).await?;
        return Ok(());
    }

    let cli = Cli::parse();
    match &cli.command {
        Commands::Install { profile, tag } => {
            info!("Installing Vox (profile: {profile})");
            install::run_install(profile, tag.as_deref()).await?;
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
