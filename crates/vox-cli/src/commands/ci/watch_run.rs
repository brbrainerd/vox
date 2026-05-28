//! `vox ci watch-run` — Poll GitHub Actions for the current HEAD SHA and surface all
//! failing checks to stdout so AI agents and developers can self-heal without
//! opening the GitHub UI.
//!
//! Designed to be called:
//! - From the post-push git hook installed by `vox ci install-hooks`
//! - Manually: `vox ci watch-run` (blocks until all checks complete or timeout)
//! - With `--sha <SHA>` to poll a specific commit

use anyhow::{Context, Result};
use serde_json::Value;
use std::time::{Duration, Instant};

const GITHUB_REPO: &str = "vox-foundation/vox";
const POLL_INTERVAL_SECS: u64 = 15;
const DEFAULT_TIMEOUT_SECS: u64 = 600; // 10 minutes

pub struct WatchRunArgs {
    pub sha: Option<String>,
    pub timeout_secs: u64,
    /// If true, exit 0 even on failures (used for advisory post-push printing)
    pub advisory: bool,
    /// If true, print only failures (quieter for hook use)
    pub failures_only: bool,
}

impl Default for WatchRunArgs {
    fn default() -> Self {
        Self {
            sha: None,
            timeout_secs: DEFAULT_TIMEOUT_SECS,
            advisory: false,
            failures_only: false,
        }
    }
}

pub async fn run(args: WatchRunArgs) -> Result<()> {
    let token = vox_secrets::resolve_secret(vox_secrets::SecretId::ForgeToken)
        .expose()
        .unwrap_or_default()
        .to_string();

    if token.is_empty() {
        eprintln!(
            "⚠️  vox ci watch-run: VOX_GITHUB_TOKEN / GITHUB_TOKEN not set — \
            cannot poll CI. Set it via `vox secrets` or export the env var.\n\
            Skipping post-push CI check."
        );
        // Advisory: don't block the push if the token is missing.
        return Ok(());
    }

    let head_sha = match &args.sha {
        Some(s) => s.clone(),
        None => resolve_head_sha()?,
    };

    let short_sha = &head_sha[..std::cmp::min(7, head_sha.len())];
    println!(
        "\n📡 Polling GitHub Actions for commit {} (timeout: {}s, interval: {}s)",
        short_sha, args.timeout_secs, POLL_INTERVAL_SECS
    );
    println!(
        "   https://github.com/{}/commit/{}/checks",
        GITHUB_REPO, head_sha
    );

    let client = reqwest::Client::new();
    let start = Instant::now();
    let timeout = Duration::from_secs(args.timeout_secs);

    loop {
        let elapsed = start.elapsed();
        if elapsed > timeout {
            eprintln!(
                "\n⏰ Timeout: CI checks did not complete within {}s for {}.",
                args.timeout_secs, short_sha
            );
            eprintln!(
                "   Check manually: https://github.com/{}/commit/{}/checks",
                GITHUB_REPO, head_sha
            );
            if args.advisory {
                return Ok(());
            }
            anyhow::bail!("CI watch-run timed out");
        }

        let check_runs = fetch_check_runs(&client, &token, &head_sha).await?;

        if check_runs.is_empty() {
            println!("   ⏳ No checks found yet for {} — waiting...", short_sha);
            tokio::time::sleep(Duration::from_secs(POLL_INTERVAL_SECS)).await;
            continue;
        }

        let total = check_runs.len();
        let completed: Vec<&Value> = check_runs
            .iter()
            .filter(|r| r["status"].as_str().unwrap_or("") == "completed")
            .collect();
        let failed: Vec<&Value> = completed
            .iter()
            .filter(|r| {
                matches!(
                    r["conclusion"].as_str().unwrap_or(""),
                    "failure" | "timed_out" | "cancelled"
                )
            })
            .copied()
            .collect();
        let pending: Vec<&Value> = check_runs
            .iter()
            .filter(|r| r["status"].as_str().unwrap_or("") != "completed")
            .collect();
        let succeeded: Vec<&Value> = completed
            .iter()
            .filter(|r| r["conclusion"].as_str().unwrap_or("") == "success")
            .copied()
            .collect();

        // Print current summary line
        println!(
            "\r   [{:>3}s] ✅ {} passed  ❌ {} failed  ⏳ {} pending  (total: {})",
            elapsed.as_secs(),
            succeeded.len(),
            failed.len(),
            pending.len(),
            total
        );

        // Print failures immediately as they land
        if !failed.is_empty() {
            println!("\n❌ FAILING CHECKS (commit {}):", short_sha);
            for run in &failed {
                let name = run["name"].as_str().unwrap_or("unknown");
                let conclusion = run["conclusion"].as_str().unwrap_or("?");
                let url = run["html_url"].as_str().unwrap_or("");
                let duration_ms = run["completed_at"]
                    .as_str()
                    .zip(run["started_at"].as_str())
                    .map(|(end, start)| format!(" ({})", format_duration(start, end)))
                    .unwrap_or_default();
                println!("   ❌ {} — {} {}  → {}", name, conclusion, duration_ms, url);
            }
        }

        if pending.is_empty() {
            // All checks have completed.
            println!("\n📊 CI Summary for {}:", short_sha);
            println!("   ✅ Passed:  {}", succeeded.len());
            println!("   ❌ Failed:  {}", failed.len());
            println!("   Total:     {}", total);

            if failed.is_empty() {
                println!("\n✅ All CI checks passed for {}.", short_sha);
                return Ok(());
            }

            // Print actionable failure list
            println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!(
                "❌ CI FAILURES — {} check(s) require attention:",
                failed.len()
            );
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            for run in &failed {
                let name = run["name"].as_str().unwrap_or("unknown");
                let url = run["html_url"].as_str().unwrap_or("");
                let conclusion = run["conclusion"].as_str().unwrap_or("failure");
                println!("\n  Check:  {}", name);
                println!("  Result: {}", conclusion);
                println!("  URL:    {}", url);

                if let Some(id) = run["id"].as_u64() {
                    println!("  Fetching logs...");
                    match fetch_job_log(&client, &token, id).await {
                        Ok(log) => {
                            let lines: Vec<&str> = log.lines().collect();
                            let tail = if lines.len() > 30 {
                                &lines[lines.len() - 30..]
                            } else {
                                &lines[..]
                            };
                            println!("  --- Last 30 lines of log ---");
                            for line in tail {
                                println!("    {}", line);
                            }
                            println!("  ----------------------------");
                        }
                        Err(e) => {
                            println!("  Could not fetch logs: {}", e);
                        }
                    }
                }
            }
            println!(
                "\n  View all: https://github.com/{}/commit/{}/checks",
                GITHUB_REPO, head_sha
            );
            println!(
                "  Self-heal: run `vox ci watch-run --sha {}` to re-poll after fixes.",
                short_sha
            );
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

            if args.advisory {
                return Ok(());
            }
            std::process::exit(1);
        }

        tokio::time::sleep(Duration::from_secs(POLL_INTERVAL_SECS)).await;
    }
}

/// Fetch all check runs for a commit SHA via the GitHub Checks API.
async fn fetch_check_runs(client: &reqwest::Client, token: &str, sha: &str) -> Result<Vec<Value>> {
    let url = format!(
        "https://api.github.com/repos/{}/commits/{}/check-runs?per_page=100",
        GITHUB_REPO, sha
    );
    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", token))
        .header("User-Agent", "vox-cli")
        .header("Accept", "application/vnd.github.v3+json")
        .send()
        .await
        .context("Failed to fetch check runs from GitHub Checks API")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("GitHub Checks API returned {}: {}", status, body);
    }

    let json: Value = resp
        .json()
        .await
        .context("Failed to parse GitHub Checks API response")?;
    let runs = json["check_runs"].as_array().cloned().unwrap_or_default();
    Ok(runs)
}

/// Fetch logs for a specific job ID
async fn fetch_job_log(client: &reqwest::Client, token: &str, job_id: u64) -> Result<String> {
    let url = format!(
        "https://api.github.com/repos/{}/actions/jobs/{}/logs",
        GITHUB_REPO, job_id
    );
    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", token))
        .header("User-Agent", "vox-cli")
        .send()
        .await?;

    if resp.status().is_success() {
        Ok(resp.text().await?)
    } else {
        anyhow::bail!("API returned {}", resp.status());
    }
}

/// Get HEAD SHA from git.
fn resolve_head_sha() -> Result<String> {
    let output = std::process::// vox-arch-check: allow git-exec
        Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .context("Failed to run git rev-parse HEAD")?;
    if !output.status.success() {
        anyhow::bail!("git rev-parse HEAD failed");
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn format_duration(start: &str, end: &str) -> String {
    // Best-effort: parse ISO8601 and compute seconds
    // Fall back to raw strings if parsing fails
    if let (Ok(s), Ok(e)) = (
        chrono::DateTime::parse_from_rfc3339(start),
        chrono::DateTime::parse_from_rfc3339(end),
    ) {
        let secs = (e - s).num_seconds();
        if secs < 60 {
            return format!("{}s", secs);
        }
        return format!("{}m{}s", secs / 60, secs % 60);
    }
    String::new()
}
