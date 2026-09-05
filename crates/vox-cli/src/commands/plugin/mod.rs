//! `vox plugin` — install, remove, list, and inspect Vox plugins.

pub mod doctor;
pub mod info;
pub mod install;
pub mod list;
pub mod publish;
pub mod remove;
pub mod scaffold;

use clap::Subcommand;
use std::path::PathBuf;

/// Subcommands for `vox plugin`.
#[derive(Subcommand)]
pub enum PluginCmd {
    /// List all catalog entries with install status.
    List,
    /// Show manifest and install details for a plugin.
    Info {
        /// Plugin id, e.g. `noop-skill` or `mens-candle-cuda`.
        id: String,
    },
    /// Install a plugin from the catalog, a local directory, or a URL.
    Install {
        /// Plugin id (catalog install) — omit when using `--path` or `--url`.
        id: Option<String>,
        /// Install from a local directory containing Plugin.toml.
        #[arg(long, value_name = "DIR")]
        path: Option<PathBuf>,
        /// Install from an HTTPS URL pointing to a `.zip` archive.
        #[arg(long, value_name = "URL")]
        url: Option<String>,
        /// Skip the confirmation prompt.
        #[arg(long)]
        yes: bool,
        /// Install even when no sha256 is recorded for the plugin. The archive is
        /// loaded as native code — only use this for a source you trust.
        #[arg(long)]
        allow_unverified: bool,
    },
    /// Remove an installed plugin.
    Remove {
        /// Plugin id to remove.
        id: String,
    },
    /// Check installed plugins for ABI version drift and missing native libs.
    Doctor,
    /// Scaffold a new plugin directory with Plugin.toml and starter files.
    Scaffold {
        /// Plugin id, e.g. `my-plugin` (directory will be `vox-plugin-my-plugin`).
        id: String,
        /// Payload kind for the scaffold.
        #[arg(long, value_enum, default_value = "code")]
        kind: scaffold::ScaffoldKind,
        /// Output directory (defaults to current directory).
        #[arg(long, value_name = "DIR", default_value = ".")]
        dir: std::path::PathBuf,
    },
    /// Publish an installed skill plugin to an OpenClaw-compatible gateway.
    Publish {
        /// Plugin id to publish, e.g. `my-skill`.
        id: String,
        /// OpenClaw gateway URL (defaults to <https://api.clawhub.ai>).
        #[arg(long, value_name = "URL")]
        gateway: Option<String>,
        /// API key for the gateway (overrides OPENCLAW_API_KEY env var).
        #[arg(long, value_name = "KEY")]
        api_key: Option<String>,
    },
}

pub async fn run(cmd: PluginCmd) -> anyhow::Result<()> {
    match cmd {
        PluginCmd::List => list::run(),
        PluginCmd::Info { id } => info::run(&id),
        PluginCmd::Install {
            id,
            path,
            url,
            yes,
            allow_unverified,
        } => {
            install::run(
                id.as_deref(),
                path.as_deref(),
                url.as_deref(),
                yes,
                allow_unverified,
            )
            .await
        }
        PluginCmd::Remove { id } => remove::run(&id),
        PluginCmd::Doctor => doctor::run(),
        PluginCmd::Scaffold { id, kind, dir } => scaffold::run(&id, kind, &dir),
        PluginCmd::Publish {
            id,
            gateway,
            api_key,
        } => publish::run(&id, gateway, api_key).await,
    }
}
