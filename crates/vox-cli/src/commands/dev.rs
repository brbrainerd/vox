//! `vox dev` — build, run, and watch for changes in one command.
//!
//! Equivalent to running `vox build` and `vox run` simultaneously, with
//! automatic rebuild and restart when source files change.

use anyhow::Result;
use notify::{Event, EventKind, RecursiveMode, Watcher};
use owo_colors::OwoColorize;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};
use tokio::process::Command;

pub async fn run(file: &Path, out_dir: &Path, port: Option<u16>) -> Result<()> {
    println!(
        "{} {} → {}",
        "vox dev".cyan().bold(),
        file.display(),
        out_dir.display()
    );
    println!(
        "{}",
        "  Watching for changes. Press Ctrl+C to stop.\n".dimmed()
    );

    let port = port.unwrap_or(3000);
    let file = file.to_path_buf();
    let out_dir = out_dir.to_path_buf();

    // Initial build + run
    let mut server = build_and_run(&file, &out_dir, port).await;

    // Set up file watcher
    let (tx, rx) = channel();
    let mut watcher = notify::recommended_watcher(tx)?;

    // Watch the source directory (or parent of the file)
    let watch_dir = file.parent().unwrap_or(Path::new("."));
    if watch_dir.as_os_str().is_empty() {
        watcher.watch(Path::new("."), RecursiveMode::Recursive)?;
    } else {
        watcher.watch(watch_dir, RecursiveMode::Recursive)?;
    }

    let mut last_rebuild = Instant::now();
    let debounce = Duration::from_millis(300); // debounce rapid saves

    loop {
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(Ok(Event {
                kind: EventKind::Modify(_) | EventKind::Create(_),
                paths,
                ..
            })) => {
                let is_vox_file = paths
                    .iter()
                    .any(|p| p.extension().and_then(|e| e.to_str()) == Some("vox"));
                if is_vox_file && last_rebuild.elapsed() > debounce {
                    last_rebuild = Instant::now();
                    println!(
                        "\n{} {}",
                        "↺  File changed, rebuilding...".yellow().bold(),
                        paths
                            .first()
                            .map(|p| p.display().to_string())
                            .unwrap_or_default()
                            .dimmed()
                    );

                    // Kill old server
                    if let Some(ref mut child) = server {
                        let _ = child.kill().await;
                    }

                    // Rebuild and restart
                    server = build_and_run(&file, &out_dir, port).await;
                }
            }
            Ok(Ok(_)) => {}
            Ok(Err(e)) => eprintln!("{} {:?}", "Watch error:".red(), e),
            Err(_) => {
                // recv_timeout elapsed — just keep looping
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }
    }
}

async fn build_and_run(
    file: &PathBuf,
    out_dir: &PathBuf,
    port: u16,
) -> Option<tokio::process::Child> {
    // Build step
    match crate::commands::build::run_once(file, out_dir, false).await {
        Ok(()) => {}
        Err(e) => {
            eprintln!("\n{} {}", "✗ Build failed:".red().bold(), e);
            return None;
        }
    }

    // Auto-sync schema before running
    println!("{} Syncing database schema...", "↻".yellow());
    if let Err(e) = crate::commands::db::migrate(Some(file)).await {
        eprintln!("  {} Failed to sync schema: {}", "⚠".yellow(), e);
    }

    println!(
        "\n{} Launching on http://localhost:{}",
        "▶".green().bold(),
        port
    );

    // Start generated server
    let generated_dir = PathBuf::from("target/generated");
    match Command::new("cargo")
        .arg("run")
        .arg("--manifest-path")
        .arg(generated_dir.join("Cargo.toml"))
        .env("VOX_PORT", port.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
    {
        Ok(child) => Some(child),
        Err(e) => {
            eprintln!("{} Failed to start server: {}", "✗".red(), e);
            None
        }
    }
}
