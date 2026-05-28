//! Latin-themed command groupings (`fabrica`, `mens`, `ars`, `recensio`) — aliases for discoverability.

use clap::Subcommand;

#[cfg(feature = "script-execution")]
use crate::cli_args::ScriptArgs;
#[cfg(feature = "stub-check")]
use crate::cli_args::StubCheckArgs;
use crate::cli_args::{
    BuildArgs, BundleArgs, CheckArgs, CompileArgs, DevArgs, DoctorArgs, FmtArgs, RunArgs, TestArgs,
};

/// `vox fabrica …` — workshop / compiler lane (build, check, run, …).
#[derive(Subcommand)]
pub enum FabricaCmd {
    /// Build a Vox source file, producing TypeScript output
    Build(BuildArgs),
    /// Type-check a Vox source file without producing output
    Check(CheckArgs),
    /// Run tests for the Vox program
    Test(TestArgs),
    /// Run a Vox source file (build + cargo run in generated project)
    Run(RunArgs),
    /// Watch and rebuild via `vox-compilerd`
    Dev(DevArgs),
    /// Bundle a Vox source file into a complete web application
    Bundle(BundleArgs),
    /// Compile installable artifacts (native binary, desktop/mobile hints, workspace)
    Compile(CompileArgs),
    /// Format a Vox source file in place
    Fmt(FmtArgs),
    /// Run a `.vox` script (`fn main()`) via the native script cache
    #[cfg(feature = "script-execution")]
    Script(ScriptArgs),
}

/// `vox diag …` — diagnostics lane (doctor, architect, stub-check).
#[derive(Subcommand)]
pub enum DiagCmd {
    /// Check toolchain and local environment readiness
    Doctor(DoctorArgs),
    /// Workspace layout validation + god-object scan
    #[cfg(any(feature = "codex", feature = "stub-check"))]
    Architect {
        #[command(subcommand)]
        cmd: crate::cli_actions::ArchitectAction,
    },
    /// TOESTUB scan + Codex baselines / Ludus rewards
    #[cfg(feature = "stub-check")]
    StubCheck(StubCheckArgs),
}

/// `vox ars …` — craft / skills lane (snippet, share, skill, openclaw, ludus).
#[derive(Subcommand)]
pub enum ArsCmd {
    /// Snippet helpers (local `vox-package` store)
    Snippet {
        #[command(subcommand)]
        cmd: crate::commands::extras::snippet_cli::SnippetCli,
    },
    /// Share / search packages via local Arca index
    Share {
        #[command(subcommand)]
        cmd: crate::commands::extras::share_cli::ShareCli,
    },
    /// ARS skill registry + promote / context
    #[cfg(feature = "ars")]
    Skill {
        #[command(subcommand)]
        cmd: crate::commands::extras::skill_cmd::SkillCmd,
    },
    /// OpenClaw / ClawHub gateway
    #[cfg(feature = "ars")]
    Openclaw {
        #[command(subcommand)]
        action: crate::commands::openclaw::OpenClawAction,
    },
    /// Ludus gamification
    #[cfg(feature = "extras-ludus")]
    Ludus {
        #[command(subcommand)]
        cmd: crate::commands::extras::ludus_cli::LudusCli,
    },
}
