use anyhow::Result;
use owo_colors::OwoColorize;
use serde::Deserialize;
use serde_json::json;
use vox_gamify::db::{process_event_rewards, try_claim_processed_event};

use crate::commands::extras::ludus::LudusContext;

#[derive(Debug, Deserialize)]
struct GitHubEvent {
    id: String,
    #[serde(rename = "type")]
    event_type: String,
    #[allow(dead_code)]
    created_at: String,
    payload: serde_json::Value,
    repo: GitHubRepo,
}

#[derive(Debug, Deserialize)]
struct GitHubRepo {
    name: String,
}

pub async fn sync_command() -> Result<()> {
    let ctx = LudusContext::load().await?;

    let token = match vox_secrets::get_registry_token("github.com") {
        Some(t) if !t.is_empty() => t,
        _ => {
            println!("{}", "Error: No GitHub account linked.".bright_red());
            println!("Run `vox ludus auth github` to link your account.");
            return Ok(());
        }
    };

    println!(
        "{}",
        "=== GitHub Contribution Sync ===".bright_cyan().bold()
    );
    println!("{}", "Fetching recent activity from GitHub...".dimmed());

    let client = reqwest::Client::new();
    let res = client
        .get("https://api.github.com/user/events")
        .header("User-Agent", "VoxGamifyCLI/1.0")
        .header("Authorization", format!("token {}", token))
        .send()
        .await?;

    if !res.status().is_success() {
        if res.status().as_u16() == 401 {
            anyhow::bail!(
                "GitHub token is invalid or expired. Please re-authenticate with `vox ludus auth github`."
            );
        }
        anyhow::bail!("Failed to fetch GitHub events: {}", res.status());
    }

    let events: Vec<GitHubEvent> = res.json().await?;

    let mut total_xp = 0;
    let mut total_crystals = 0;
    let mut claimed_count = 0;

    for event in events {
        let dedupe_key = format!("github:event:{}", event.id);

        // Check if already processed
        if !try_claim_processed_event(&ctx.db, &ctx.user_id, &dedupe_key).await? {
            continue;
        }

        // Map GitHub events to Ludus event types
        let ludus_event_type = match event.event_type.as_str() {
            "PullRequestEvent" => {
                let action = event.payload["action"].as_str().unwrap_or("");
                let merged = event.payload["pull_request"]["merged"]
                    .as_bool()
                    .unwrap_or(false);

                if action == "opened" {
                    Some("task_submitted")
                } else if action == "closed" && merged {
                    Some("task_completed")
                } else {
                    None
                }
            }
            "PullRequestReviewEvent" => {
                let state = event.payload["review"]["state"].as_str().unwrap_or("");
                if state == "approved" || state == "changes_requested" {
                    Some("peer_teach_session")
                } else {
                    None
                }
            }
            "PushEvent" => Some("snapshot_captured"),
            "IssuesEvent" => {
                let action = event.payload["action"].as_str().unwrap_or("");
                if action == "opened" {
                    Some("task_started")
                } else if action == "closed" {
                    Some("task_resolved")
                } else {
                    None
                }
            }
            _ => None,
        };

        if let Some(et) = ludus_event_type {
            // Process the reward
            let reward_res = process_event_rewards(
                &ctx.db,
                &ctx.user_id,
                &json!({
                    "type": et,
                    "github_repo": event.repo.name,
                    "github_event_id": event.id,
                    "metadata": format!("Repo: {}", event.repo.name),
                }),
            )
            .await?;

            let xp = reward_res.reward.as_ref().map(|r| r.xp).unwrap_or(0);
            let crystals = reward_res.reward.as_ref().map(|r| r.crystals).unwrap_or(0);

            total_xp += xp;
            total_crystals += crystals;
            claimed_count += 1;

            // Print brief summary for this event
            println!(
                "  {} {} in {} -> {} XP, {} 💎",
                "✔".bright_green(),
                et.bright_yellow(),
                event.repo.name.bright_blue(),
                xp.to_string().bright_green(),
                crystals.to_string().bright_cyan()
            );

            if let Some((lvl, title)) = reward_res.leveled_up {
                println!(
                    "    {}",
                    format!("⭐ LEVEL UP! You are now Level {} - {}", lvl, title)
                        .bright_magenta()
                        .bold()
                );
            }
        }
    }

    println!();
    if claimed_count > 0 {
        println!(
            "✅ Synced {} new contribution events.",
            claimed_count.bright_green().bold()
        );
        println!(
            "Total rewards gained: {} XP | {} 💎",
            total_xp.bright_yellow().bold(),
            total_crystals.bright_cyan().bold()
        );
    } else {
        println!("{}", "No new contributions found since last sync.".dimmed());
    }

    Ok(())
}
