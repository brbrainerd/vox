//! # `vox` — minimal Vox compiler CLI
//!
//! Parses arguments with **clap** and dispatches to the `commands` module. The happy path for codegen is:
//! **lex → parse → typecheck → HIR →** TypeScript (`vox-codegen-ts`) **+** Rust (`vox-codegen-rust`).
//!
//! ## Globals and discoverability
//!
//! Root flags (before subcommand): **`--color`**, **`--json`**, **`--verbose` / `-v`**, **`--quiet` / `-q`** — see [`VoxCliRoot`](vox_cli::VoxCliRoot). **`vox completions <shell>`** emits shell completions (bash/zsh/fish/powershell/elvish).
//!
//! Latin groupings (same dispatch as flat verbs): **`vox fabrica`**, **`vox mens`**, **`vox ars`**, **`vox recensio`** (feature **`coderabbit`**).
//!
//! ## Subcommands
//!
//! | CLI | Rust module |
//! |-----|-------------|
//! | `vox build <file> [-o DIR]` | `commands::build` |
//! | `vox check <file>` | `commands::check` |
//! | `vox test <file> [--watch]` | `commands::test` |
//! | `vox snapshot orphans [--clean]` | `commands::snapshot` |
//! | `vox run <file> [--port N] [--mode auto\|app\|script] [-- …]` | `commands::run` |
//! | `vox script <file> …` | `commands::runtime::run::script` (needs `script-execution`) |
//! | `vox ci …` | `commands::ci` |
//! | `vox bundle …` (plugin bundles) | `commands::plugin_bundle` |
//! | `vox bundle-app …` / `vox fabrica bundle …` (web app) | `commands::bundle` |
//! | `vox fmt <file>` | `commands::fmt` |
//! | `vox add` / `remove` / `update` / `lock` / `sync` / `upgrade` / `pm` | `commands::add`, `remove`, `update`, `lock`, `sync`, `upgrade`, `pm` |
//! | `vox dev <file>` | `commands::dev` (via `vox-compilerd`) |
//! | `vox emit client <file>` | `commands::emit` (Library TS SDK / openapi only) |
//! | `vox live` | `commands::live` (needs `--features live`) |
//! | `vox db …` | `commands::db_cli` |
//! | `vox memory search …` | `commands::memory_cli` |
//! | `vox scientia …` | `commands::scientia` (research / capability-map facade over `db_cli`) |
//! | `vox telemetry …` | `commands::telemetry` (optional upload queue; ADR 023) |
//! | `vox codex verify \| export-legacy \| import-legacy \| cutover \| import-orchestrator-memory \| import-skill-bundle \| socrates-metrics \| socrates-eval-snapshot` | `commands::codex` |
//! | `vox openclaw …` | `commands::openclaw` (needs `--features ars`) |
//! | `vox snippet …` / `vox share …` | `commands::extras` |
//! | `vox skill …` | `commands::extras::skill_cmd` (needs `--features ars`) |
//! | `vox ludus …` | `commands::extras::ludus_cli` (needs `--features extras-ludus`) |
//! | `vox stub-check …` | `commands::stub_check` (needs `--features stub-check`) |
//! | `vox architect …` | `commands::diagnostics::tools::architect` (needs `--features stub-check` and/or `codex`) |
//! | `vox lsp` | `commands::lsp` |
//! | `vox doctor` (extended: `--build-perf` / `--scope` / `--json` need `--features codex`) | `commands::diagnostics::doctor` |
//! | `vox secrets …` / `vox clavis …` | `commands::secrets` |
//! | `vox train …` (feature `mens-dei` + `gpu`) | `commands::ai::train` |
//! | `vox review coderabbit …` | `commands::review` (needs `--features coderabbit`) |
//!
//! End-user docs: repository file `docs/src/reference/cli.md`. `@v0` integration during `build`: module `v0`.

use clap::Parser;
use std::process::Command;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Intercept ML commands and delegate to vox-ml-cli
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        let cmd = args[1].as_str();
        let is_ml = matches!(
            cmd,
            "mens" | "schola" | "oratio" | "speech" | "populi" | "train"
        );
        let is_ext_ml = cmd == "ext"
            && args.len() > 2
            && matches!(
                args[2].as_str(),
                "mens" | "schola" | "oratio" | "speech" | "populi" | "train"
            );

        if is_ml || is_ext_ml {
            let primary_cmd = if is_ext_ml { args[2].as_str() } else { cmd };
            let binary = if primary_cmd == "schola" {
                "vox-schola"
            } else {
                "vox-ml-cli"
            };

            let mut command = Command::new(binary);
            if primary_cmd == "train" {
                // `vox train` -> `vox-ml-cli mens train`
                command.arg("mens");
            }

            let forward_args = if is_ext_ml { &args[2..] } else { &args[1..] };
            command.args(forward_args);

            // Wait for completion and exit with same status
            match command.status() {
                Ok(status) => {
                    std::process::exit(status.code().unwrap_or(1));
                }
                Err(e) => {
                    eprintln!("Error: {} is not installed or not in PATH.", binary);
                    eprintln!(
                        "The '{}' subsystem has been extracted to a separate crate.",
                        primary_cmd
                    );
                    eprintln!("Please run: cargo install --path crates/{}", binary);
                    eprintln!("Underlying error: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }

    let root = vox_cli::VoxCliRoot::parse();
    vox_cli::run_vox_cli_from_parsed(root).await
}
