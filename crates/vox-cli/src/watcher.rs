//! Shared file-watcher helper for `vox build --watch`, `vox check --watch`, etc.
//!
//! Eliminates the ~30-line `notify::recommended_watcher` + `recv_timeout` loop
//! that was previously copy-pasted in `build.rs`, `check.rs`, and `dev.rs`.

use anyhow::Result;
use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc::channel;
use std::time::Duration;

/// Run `callback` whenever `file` is modified on disk.
///
/// Blocks indefinitely; use `Ctrl-C` to stop.  The callback receives a reference
/// to the watched file path so the caller does not need to capture it separately.
///
/// # Example
/// ```ignore
/// watch_file(Path::new("App.vox"), |path| async move {
///     build::run_once(path, &out_dir, false).await.ok();
/// }).await?;
/// ```
pub async fn watch_file<F, Fut>(file: &Path, label: &str, mut callback: F) -> Result<()>
where
    F: FnMut(&Path) -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    use owo_colors::OwoColorize;
    println!("{} {}", label.cyan().bold(), file.display());

    // Run once immediately before starting to watch.
    callback(file).await;

    let (tx, rx) = channel();
    let mut watcher = notify::recommended_watcher(tx)?;

    let watch_dir = file.parent().unwrap_or(Path::new("."));
    let effective_dir = if watch_dir.as_os_str().is_empty() {
        Path::new(".")
    } else {
        watch_dir
    };
    watcher.watch(effective_dir, RecursiveMode::NonRecursive)?;

    let absolute_file = std::fs::canonicalize(file).unwrap_or_else(|_| file.to_path_buf());

    loop {
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(Ok(Event {
                kind: EventKind::Modify(_),
                paths,
                ..
            })) => {
                for path in paths {
                    let abs = std::fs::canonicalize(&path).unwrap_or(path);
                    if abs == absolute_file {
                        println!("\n{} {}", "File changed:".cyan(), file.display());
                        callback(file).await;
                        break;
                    }
                }
            }
            Ok(Ok(_)) => {}
            Ok(Err(e)) => eprintln!("{} {:?}", "Watch error:".red(), e),
            Err(_) => {
                // recv_timeout elapsed — just keep looping
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
}
