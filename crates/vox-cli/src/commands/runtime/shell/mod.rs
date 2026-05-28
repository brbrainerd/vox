//! `vox shell` — developer **micro-REPL** or **policy check** for PowerShell source strings.
//!
//! ## Non-goals (product boundaries)
//!
//! - **`repl` is not a shell emulator**: no pipelines, session `cd`, robust quoting, or
//!   policy-enforced passthrough of arbitrary shell lines.
//! - **Host / agent shell work**: use real **`pwsh`** when available and validate with
//!   `check_terminal::run_check` against `contracts/terminal/exec-policy.v1.yaml` (repo root).
//! - **Inside `.vox`**: use typed `std.process` / `std.fs` / `std.path` (argv-first), not a
//!   shell-string interpreter.
//!
//! Human-readable boundary doc: `docs/src/architecture/vox-shell-operations-boundaries.md`.

pub(crate) mod check_terminal;

pub mod aci_adapter;
pub mod backends;

use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Result;
use clap::Subcommand;
use tokio::process::Command;

use vox_bounded_fs::read_utf8_path_capped_async;

pub use check_terminal::{DEFAULT_POLICY_REL, validate_policy_file};

/// `vox shell` subcommands (`repl` when omitted).
#[derive(Subcommand, Clone, Debug)]
pub enum ShellCmd {
    /// Validate a PowerShell command string against `contracts/terminal/exec-policy.v1.yaml` using pwsh AST.
    Check {
        /// PowerShell source line or script fragment to analyze.
        #[arg(long)]
        payload: String,
        /// Policy YAML (default: repo `contracts/terminal/exec-policy.v1.yaml`).
        #[arg(long, value_name = "PATH")]
        policy: Option<PathBuf>,
    },
    /// Dev-only micro-REPL (`pwd`/`ls`/`cat` + whitespace-split OS passthrough). Not `pwsh` and not policy-checked.
    Repl,
}

/// Dispatch `vox shell [repl|check …]`.
pub async fn run(cmd: Option<ShellCmd>) -> Result<()> {
    match cmd.unwrap_or(ShellCmd::Repl) {
        ShellCmd::Repl => run_shell().await,
        ShellCmd::Check { payload, policy } => {
            check_terminal::run_check(&payload, policy.as_deref())?;
            println!("shell check: OK (exec policy)");
            Ok(())
        }
    }
}

/// Run the `vox shell` REPL.
pub async fn run_shell() -> anyhow::Result<()> {
    static PASSTHROUGH_WARNED: AtomicBool = AtomicBool::new(false);

    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║          Vox Shell (micro-REPL, dev-only)                ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  Built-ins: pwd | ls | cat                               ║");
    println!("║  Optional: whitespace-split OS passthrough (see note)   ║");
    println!("║  Real shells: use `pwsh`; validate via `vox shell check`  ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!("Type 'exit' or 'quit' to leave.\n");

    let stdin = io::stdin();

    loop {
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        print!("vox {} > ", cwd.display());
        io::stdout().flush()?;

        let mut input = String::new();
        stdin.read_line(&mut input)?;

        let line = input.trim();
        if line.is_empty() {
            continue;
        }

        if line == "exit" || line == "quit" {
            break;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        let cmd = parts[0];
        let args = &parts[1..];

        match cmd {
            "pwd" => {
                println!("{}", cwd.display());
            }
            "ls" => match tokio::fs::read_dir(&cwd).await {
                Ok(mut rd) => {
                    while let Ok(Some(entry)) = rd.next_entry().await {
                        let is_dir = entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false);
                        let type_str = if is_dir { "DIR " } else { "FILE" };
                        println!("  {} {}", type_str, entry.file_name().to_string_lossy());
                    }
                }
                Err(e) => eprintln!("Error listing directory: {e}"),
            },
            "cat" => {
                if args.is_empty() {
                    eprintln!("Usage: cat <file>");
                } else {
                    let path = cwd.join(args[0]);
                    match read_utf8_path_capped_async(&path).await {
                        Ok(content) => print!("{content}"),
                        Err(e) => eprintln!("Error reading file: {e}"),
                    }
                }
            }
            _ => {
                if !PASSTHROUGH_WARNED.swap(true, Ordering::Relaxed) {
                    eprintln!(
                        "note: vox shell repl passthrough splits on whitespace only — no quotes, pipes, or redirection. Prefer `pwsh` for real shell semantics."
                    );
                }
                match Command::new(cmd).args(args).status().await {
                    Ok(status) if !status.success() => {
                        eprintln!("Command exited with status: {status}");
                    }
                    Err(e) if e.kind() == io::ErrorKind::NotFound => {
                        eprintln!("vox shell: command not found: {cmd}");
                    }
                    Err(e) => eprintln!("{e}"),
                    Ok(_) => {}
                }
            }
        }
    }

    Ok(())
}
