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

    println!("Building for tests: {}...", file.display());
    build::run(
        file,
        &out_dir,
        None,
        None,
        false,
        false,
        BuildMode::App,
        vox_codegen::codegen_rust::RustAppShell::default(),
    )
    .await?;

    let generated_dir = Path::new("target").join("generated");
    println!("Running tests in {}...", generated_dir.display());

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

    if !status.success() {
        anyhow::bail!("Tests failed with exit code: {:?}", status.code());
    }

    Ok(())
}

async fn run_watch(args: &crate::cli_args::TestArgs) -> Result<()> {
    use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

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

    let debounce = Duration::from_millis(300);
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
