//! Shared library for CLI primitives.

pub mod artifact_policy;
pub mod benchmark_telemetry;
pub mod build_service;
pub mod cli_actions;
pub mod cli_args;
pub mod command_contract;
pub mod command_registry_model;

pub mod constants;
pub mod daemon_ipc;
pub mod db_types;
pub mod diagnostics;
pub mod fs_utils;
pub mod gamify_shim;
pub mod orchestrator_driver;
pub mod scientia;
pub mod workflow_journal_codex;

/// Global flags available before every subcommand.
#[derive(clap::Args, Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct GlobalOpts {
    /// When to emit ANSI colors (`NO_COLOR` still disables).
    #[arg(long, global = true, value_name = "WHEN", value_enum)]
    pub color: Option<crate::diagnostics::ColorChoice>,
    /// Hint subcommands to prefer machine JSON where supported (`VOX_CLI_GLOBAL_JSON=1`).
    #[arg(long, global = true)]
    pub json: bool,
    /// More verbose logs.
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,
    /// Less noisy logs and hints (`VOX_CLI_QUIET=1` for supported subcommands).
    #[arg(short = 'q', long, global = true)]
    pub quiet: bool,
}

/// Initialize [`tracing`] for CLI tools.
pub fn init_tracing_for_cli() {
    vox_foundation::tracing::try_init_cli_default_info_fallback();
}

/// Apply global opts to the environment.
pub fn apply_global_opts(opts: &GlobalOpts) {
    if let Some(color) = opts.color {
        crate::diagnostics::set_color_choice(color);
    }
}

#[cfg(test)]
mod global_opts_tests {
    use super::GlobalOpts;
    use clap::Parser;

    #[derive(Parser)]
    struct Root {
        #[command(flatten)]
        global: GlobalOpts,
    }

    #[test]
    fn quiet_short_flag_parses() {
        let r = Root::try_parse_from(["vox", "-q"]).expect("parse");
        assert!(r.global.quiet);
    }

    #[test]
    fn quiet_long_flag_parses() {
        let r = Root::try_parse_from(["vox", "--quiet"]).expect("parse");
        assert!(r.global.quiet);
    }
}
