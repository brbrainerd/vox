//! `vox test` — runs `cargo test` in the generated Rust crate under `target/generated`.

use crate::cli_args::BuildMode;
use crate::commands::build;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Build `file` into `dist/` / `target/generated`, then execute `cargo test` in the backend workspace.
pub async fn run(args: &crate::cli_args::TestArgs) -> Result<()> {
    if args.watch {
        run_watch(args).await
    } else {
        run_once(args).await
    }
}

async fn run_once(args: &crate::cli_args::TestArgs) -> Result<()> {
    let out_dir = PathBuf::from("dist");
    let file = &args.file;

    if vox_config::VoxConfig::load().build_target == vox_config::BuildTarget::Client {
        anyhow::bail!(
            "`vox test` requires Rust codegen; `[build] target = \"client\"` / `VOX_BUILD_TARGET=client` emits TypeScript only. Use fullstack or server."
        );
    }

    let json = crate::pipeline::global_json_enabled();

    crate::vox_note!(json, "Building for tests: {}...", file.display());
    // `build::run` already emits exactly one `"command":"build"` envelope on
    // stdout under `--json` (success or failure — see build.rs). If it fails
    // here, that envelope has already been printed and fully describes the
    // failure, so we must propagate the error WITHOUT printing a second,
    // redundant `"command":"test"` envelope — the test-run stage was never
    // reached.
    build::run(
        file,
        &out_dir,
        None,
        None,
        false,
        false,
        BuildMode::App,
        vox_codegen::codegen_rust::RustAppShell::default(),
        None,
        None,
    )
    .await?;

    let generated_dir = Path::new("target").join("generated");
    crate::vox_note!(json, "Running tests in {}...", generated_dir.display());

    // `vox test` compiles the generated backend crate to run its tests,
    // exactly like `vox run` compiles it to serve — same genuine toolchain
    // requirement (spec §9.1: state it honestly rather than crash on a raw
    // spawn failure), same preflight shape (see `run.rs`'s
    // `missing_cargo_toolchain_message`).
    if which::which("cargo").is_err() {
        anyhow::bail!(missing_cargo_toolchain_message());
    }

    let mut cmd = Command::new("cargo");
    cmd.arg("test").current_dir(&generated_dir);
    if let Some(f) = &args.filter {
        cmd.arg(f);
    }
    if args.coverage {
        cmd.env(
            "RUSTFLAGS",
            "-C instrument-coverage -C llvm-args=--instrprof-output-path=coverage.profraw",
        );
        cmd.env("LLVM_PROFILE_FILE", "coverage-%p-%m.profraw");
    }
    if args.update_snapshots {
        cmd.env("UPDATE_EXPECT", "1");
        cmd.env("INSTA_UPDATE", "always");
    }
    if let Some(iters) = args.forall_iterations {
        cmd.env("VOX_FORALL_ITERATIONS", iters.to_string());
    }

    let status = cmd.status().context("Failed to execute cargo test")?;

    if json {
        // The preceding build stage already emitted its own `"command":"build"`
        // envelope only on failure (see above); reaching this point means the
        // build succeeded and produced no envelope yet, so this is the single
        // envelope for the run — reporting the actual test outcome.
        println!(
            "{}",
            crate::pipeline::format_command_result_envelope_json(
                "test",
                file,
                status.success(),
                status.code(),
            )
        );
    }

    if !status.success() {
        anyhow::bail!("Tests failed with exit code: {:?}", status.code());
    }

    Ok(())
}

async fn run_watch(args: &crate::cli_args::TestArgs) -> Result<()> {
    use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
    use std::sync::mpsc;
    use std::time::Instant;

    let watch_root = args
        .file
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();

    let (tx, rx) = mpsc::channel::<notify::Result<Event>>();
    let mut watcher = RecommendedWatcher::new(tx, Config::default())?;
    watcher.watch(&watch_root, RecursiveMode::Recursive)?;

    println!(
        "Watching {} for .vox changes (Ctrl-C to stop)...",
        watch_root.display()
    );

    // Run once immediately before waiting for changes.
    let _ = run_once(args).await;

    let debounce = vox_config::timeouts::D_300MS;
    let mut last_run = Instant::now();

    for event in rx {
        match event {
            Ok(ev) => {
                let is_vox = ev
                    .paths
                    .iter()
                    .any(|p| p.extension().map(|e| e == "vox").unwrap_or(false));
                if is_vox && last_run.elapsed() >= debounce {
                    last_run = Instant::now();
                    println!("\n--- file changed, re-running tests ---");
                    let _ = run_once(args).await;
                }
            }
            Err(e) => eprintln!("watch error: {e}"),
        }
    }

    Ok(())
}

/// Message for a missing `cargo`, mirroring `run.rs`'s
/// `missing_cargo_toolchain_message` — `vox test` has the identical genuine
/// toolchain requirement (compiling the generated backend crate), just for
/// `cargo test` instead of `cargo run`.
fn missing_cargo_toolchain_message() -> String {
    "`vox test` compiles the generated backend to run its tests, which \
     requires a Rust toolchain, but `cargo` was not found on PATH. Install \
     Rust from https://rustup.rs and try again."
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// No cargo, no repo-relative path — an installed user has neither.
    #[test]
    fn missing_cargo_message_names_no_repo_path() {
        let msg = missing_cargo_toolchain_message();
        assert!(
            !msg.contains("crates/"),
            "must not reference a repo path: {msg}"
        );
        assert!(
            msg.contains("rustup.rs"),
            "must point at the real remedy: {msg}"
        );
    }
}
