use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum ExtCmd {
    /// Ludus gamification
    #[cfg(feature = "extras-ludus")]
    Ludus {
        #[command(subcommand)]
        cmd: crate::commands::extras::ludus_cli::LudusCli,
    },
    /// ARS skill registry + promote / context
    #[cfg(feature = "ars")]
    Skill {
        #[command(subcommand)]
        cmd: crate::commands::extras::skill_cmd::SkillCmd,
    },
    /// OpenClaw / ClawHub gateway
    #[cfg(feature = "ars")]
    #[command(visible_alias = "oc")]
    Openclaw {
        #[command(subcommand)]
        action: crate::commands::openclaw::OpenClawAction,
    },
    /// Craft / skills lane
    Ars {
        #[command(subcommand)]
        cmd: crate::latin_cmd::ArsCmd,
    },
    /// Mens: train, serve, corpus, eval (delegated to vox-ml-cli)
    #[command(hide = true)]
    Mens {
        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Oratio: speech-to-text / transcripts (delegated to vox-ml-cli)
    #[command(hide = true)]
    Oratio {
        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Populi registry + HTTP control plane (delegated to vox-ml-cli)
    #[command(hide = true)]
    Populi {
        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Fine-tune: legacy entry (delegated to vox-ml-cli)
    #[command(hide = true)]
    Train {
        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
        args: Vec<String>,
    },
}

pub async fn run(cmd: ExtCmd) -> Result<()> {
    match cmd {
        #[cfg(feature = "extras-ludus")]
        ExtCmd::Ludus { cmd } => {
            crate::cli_dispatch::run_ars_cmd(crate::latin_cmd::ArsCmd::Ludus { cmd }).await
        }
        #[cfg(feature = "ars")]
        ExtCmd::Skill { cmd } => {
            crate::cli_dispatch::run_ars_cmd(crate::latin_cmd::ArsCmd::Skill { cmd }).await
        }
        #[cfg(feature = "ars")]
        ExtCmd::Openclaw { action } => crate::cli_dispatch::run_openclaw_subcommand(action).await,
        ExtCmd::Ars { cmd } => crate::cli_dispatch::run_ars_cmd(cmd).await,
        ExtCmd::Mens { .. }
        | ExtCmd::Oratio { .. }
        | ExtCmd::Populi { .. }
        | ExtCmd::Train { .. } => {
            unreachable!("ML commands in ext should be intercepted by main.rs")
        }
    }
}
