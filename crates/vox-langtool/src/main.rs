//! `vox-langtool` — DB-free CLI for writing in the Vox language.
//!
//! Provides `check`, `fmt`, `run`, and `build` subcommands without pulling
//! the heavy runtime / orchestrator / database stack.

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "vox-langtool", about = "DB-free Vox language tooling", version)]
struct Cli {
    #[command(flatten)]
    global: vox_cli_core::GlobalOpts,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Type-check a .vox file; non-zero exit on errors.
    Check {
        /// File to check.
        file: PathBuf,
    },
    /// Format a .vox file in place; --check verifies only.
    Fmt {
        /// File to format.
        file: PathBuf,
        /// Verify the file is already formatted (errors if not).
        #[arg(long)]
        check: bool,
    },
    /// Execute a .vox script via the tree-walking interpreter.
    Run {
        /// File to run.
        file: PathBuf,
        /// Arguments forwarded to the script (after `--`).
        #[arg(last = true)]
        args: Vec<String>,
    },
    /// Emit Rust script source via codegen.
    Build {
        /// File to build.
        file: PathBuf,
        /// Output directory for generated files.
        #[arg(long)]
        out_dir: PathBuf,
    },
}

fn main() -> Result<()> {
    vox_cli_core::init_tracing_for_cli();
    let cli = Cli::parse();
    vox_cli_core::apply_global_opts(&cli.global);

    match &cli.command {
        Commands::Check { file } => vox_langtool::commands::check::run(file),
        Commands::Fmt { file, check } => vox_langtool::commands::fmt::run(file, *check),
        Commands::Run { file, args } => vox_langtool::commands::run::run(file, args),
        Commands::Build { file, out_dir } => vox_langtool::commands::build::run(file, out_dir),
    }
}
