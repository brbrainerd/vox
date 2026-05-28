use anyhow::{Context, Result};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

pub async fn run(write_to: Option<PathBuf>) -> Result<()> {
    let token = vox_secrets::resolve_secret(vox_secrets::SecretId::ForgeToken)
        .expose()
        .unwrap_or_default()
        .to_string();

    if token.is_empty() {
        anyhow::bail!("VOX_GITHUB_TOKEN is required to fetch deploy status");
    }

    // Call GitHub API to get the latest deploy-hetzner.yml run
    let url = "https://api.github.com/repos/vox-foundation/vox/actions/runs?branch=main&event=push&per_page=10";
    let client = vox_http_client::client();
    let resp = client
        .get(url)
        .header("Authorization", format!("Bearer {}", token))
        .header("User-Agent", "vox-cli")
        .header("Accept", "application/vnd.github.v3+json")
        .send()
        .await
        .context("Failed to fetch workflow runs from GitHub API")?;

    let json: Value = resp.json().await?;
    let runs = json["workflow_runs"]
        .as_array()
        .context("Invalid GitHub API response")?;

    let deploy_run = runs
        .iter()
        .find(|r| r["name"].as_str().unwrap_or("") == "Deploy Hetzner (Coolify)");

    let deploy_run = match deploy_run {
        Some(r) => r,
        None => {
            println!("No recent Deploy Hetzner runs found.");
            return Ok(());
        }
    };

    let _run_id = deploy_run["id"].as_i64().unwrap_or(0);
    let status = deploy_run["status"].as_str().unwrap_or("unknown");
    let conclusion = deploy_run["conclusion"].as_str().unwrap_or("pending");
    let html_url = deploy_run["html_url"].as_str().unwrap_or("");
    let head_sha = deploy_run["head_sha"].as_str().unwrap_or("unknown");
    let head_commit_msg = deploy_run["head_commit"]["message"]
        .as_str()
        .unwrap_or("")
        .lines()
        .next()
        .unwrap_or("");

    // Try to fetch jobs to see which stage failed
    let jobs_url = deploy_run["jobs_url"].as_str().unwrap_or("");
    let mut failed_job = String::new();
    if !jobs_url.is_empty() {
        if let Ok(jobs_resp) = client
            .get(jobs_url)
            .header("Authorization", format!("Bearer {}", token))
            .header("User-Agent", "vox-cli")
            .send()
            .await
        {
            if let Ok(jobs_json) = jobs_resp.json::<Value>().await {
                if let Some(jobs_arr) = jobs_json["jobs"].as_array() {
                    for job in jobs_arr {
                        let j_conclusion = job["conclusion"].as_str().unwrap_or("");
                        if j_conclusion == "failure" {
                            failed_job = job["name"].as_str().unwrap_or("").to_string();
                            break;
                        }
                    }
                }
            }
        }
    }

    let emoji = if conclusion == "success" {
        "✅ SUCCESS"
    } else if conclusion == "failure" {
        "❌ FAILED"
    } else {
        "⏳ PENDING"
    };
    let mut md = format!(
        "# Deploy Status — vox main @ {} ({})\n\n**Status**: {}  |  **Stage**: {}\n**Run**: {}\n\n",
        &head_sha[0..std::cmp::min(7, head_sha.len())],
        head_commit_msg,
        emoji,
        if failed_job.is_empty() {
            status
        } else {
            &failed_job
        },
        html_url
    );

    if conclusion == "failure" {
        md.push_str("## Next Steps\n- [ ] View the run URL above for detailed logs.\n- [ ] Run `vox deploy --target coolify --wait` to retry locally.\n");
    } else if conclusion == "success" {
        md.push_str("## Next Steps\n- [ ] Deployment was successful. Service should be running.\n");
    }

    if let Some(path) = write_to {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, &md)?;
        println!("Deploy status written to {}", path.display());
    } else {
        println!("{}", md);
    }

    if conclusion == "failure" {
        std::process::exit(1);
    }

    Ok(())
}
