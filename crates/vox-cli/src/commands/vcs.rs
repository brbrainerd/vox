//! `vox vcs` — Jujutsu colocated version control bridge.
//!
//! All subcommands invoke the local `jj` binary via `std::process::Command`.
//! No dependency on `vox-dei` is required — the bridge is intentionally thin
//! so it can live inside the lean CLI binary without pulling in the full agent
//! orchestration stack.

use anyhow::{bail, Context, Result};

use crate::cli_actions::VcsAction;

/// Run a `jj` subprocess and return its combined stdout/stderr output.
async fn jj(args: &[&str]) -> Result<String> {
    let out = tokio::process::Command::new("jj")
        .args(args)
        .output()
        .await
        .with_context(|| format!("failed to execute `jj {}`", args.join(" ")))?;

    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();

    if out.status.success() {
        Ok(stdout)
    } else {
        bail!(
            "`jj {}` exited {:?}\nstdout: {}\nstderr: {}",
            args.join(" "),
            out.status,
            stdout.trim(),
            stderr.trim()
        )
    }
}

/// Dispatch a [`VcsAction`] subcommand.
pub async fn run(action: VcsAction) -> Result<()> {
    let json_mode = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxJsonOutput)
        .expose()
        .is_some_and(|v| v == "1");

    match action {
        VcsAction::Init => {
            // Use forward slashes for the repo root on Windows to avoid jj path issues.
            let cwd = std::env::current_dir()?;
            let cwd_str = cwd.to_string_lossy().replace('\\', "/");
            let out = jj(&["git", "init", "--colocated", &cwd_str])
                .await
                .context("vox vcs init: `jj git init --colocated` failed")?;
            println!("{}", out.trim());
        }

        VcsAction::Status => {
            let out = jj(&["status"]).await?;
            println!("{}", out.trim());
        }

        VcsAction::Diff { stat, file } => {
            let out = match (stat, file) {
                (true, Some(path)) => {
                    let p = path_str(&path);
                    jj(&["diff", "--stat", &p]).await?
                }
                (true, None) => jj(&["diff", "--stat"]).await?,
                (false, Some(path)) => {
                    let p = path_str(&path);
                    jj(&["diff", &p]).await?
                }
                (false, None) => jj(&["diff"]).await?,
            };
            println!("{}", out.trim());
        }

        VcsAction::Log { n } => {
            let n_str = n.to_string();
            let out = jj(&["log", "-n", &n_str]).await?;
            println!("{}", out.trim());
        }

        VcsAction::Annotate { file, json } => {
            let p = path_str(&file);
            let out = jj(&["file", "annotate", &p]).await?;
            if json {
                // Parse into structured lines: change_id, author, line_no, content
                let lines: Vec<serde_json::Value> = out
                    .lines()
                    .enumerate()
                    .map(|(i, line)| {
                        // Format: "<change_id> <author>: <content>"
                        let (meta, content) =
                            line.split_once(": ").unwrap_or(("unknown unknown", line));
                        let parts: Vec<&str> = meta.splitn(2, ' ').collect();
                        serde_json::json!({
                            "line": i + 1,
                            "change_id": parts.first().copied().unwrap_or(""),
                            "author": parts.get(1).copied().unwrap_or(""),
                            "content": content,
                        })
                    })
                    .collect();
                println!("{}", serde_json::to_string_pretty(&lines)?);
            } else {
                println!("{}", out.trim());
            }
        }

        VcsAction::Heatmap { file, json } => {
            let p = path_str(&file);
            let out = jj(&["file", "annotate", &p]).await?;
            let hot_regions = compute_heatmap(&out);
            if json {
                println!("{}", serde_json::to_string_pretty(&hot_regions)?);
            } else {
                if hot_regions.is_empty() {
                    println!("No hot regions detected.");
                } else {
                    println!("Hot regions in {}:", file.display());
                    for r in &hot_regions {
                        println!(
                            "  lines {}-{} ({} changes)",
                            r.start, r.end, r.change_count
                        );
                    }
                }
            }
        }

        VcsAction::Merge { rev, push } => {
            let out =
                jj(&["rebase", "--destination", "@-", "--destination", &rev])
                    .await
                    .with_context(|| format!("vox vcs merge: failed to merge rev `{rev}`"))?;
            println!("{}", out.trim());

            if push && !json_mode {
                println!("Pushing to remote…");
            }
            if push {
                let push_out = jj(&["git", "push"])
                    .await
                    .context("vox vcs merge: post-merge_push failed")?;
                println!("{}", push_out.trim());
            }
        }
    }

    Ok(())
}

/// Convert a PathBuf to a forward-slash string (cross-platform jj compat).
fn path_str(p: &std::path::Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

/// Heatmap: find line spans touched by the same change ID.
/// Returns up to 5 regions (min/max line range) sorted by descending change count.
fn compute_heatmap(annotate_output: &str) -> Vec<HotRegion> {
    use std::collections::HashMap;

    // Count how many lines belong to each change_id.
    let mut change_lines: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, line) in annotate_output.lines().enumerate() {
        let change_id = line.split(' ').next().unwrap_or("");
        if !change_id.is_empty() {
            change_lines.entry(change_id).or_default().push(i + 1);
        }
    }

    // Build HotRegion for each change_id that touches > 1 line.
    let mut regions: Vec<HotRegion> = change_lines
        .values()
        .filter(|lines| lines.len() > 1)
        .map(|lines| {
            let start = lines.iter().copied().min().unwrap_or(0);
            let end = lines.iter().copied().max().unwrap_or(0);
            HotRegion {
                start,
                end,
                change_count: lines.len(),
            }
        })
        .collect();

    regions.sort_by_key(|r| std::cmp::Reverse(r.change_count));
    regions.truncate(5);
    regions
}

/// The line span (min to max) touched by a specific change ID.
/// Note: regions may be sparse (non-contiguous) but are reported as a single range.
#[derive(Debug, serde::Serialize)]
struct HotRegion {
    /// First line number of the region (1-indexed).
    start: usize,
    /// Last line number of the region (1-indexed).
    end: usize,
    /// Number of change-annotated lines that land in this region.
    change_count: usize,
}
