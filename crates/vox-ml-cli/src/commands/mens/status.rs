//! `vox mens status` — training run status from telemetry.

use anyhow::Result;
use std::path::PathBuf;

use vox_bounded_fs::read_utf8_path_capped;

pub async fn run_status(
    run_dir: Option<PathBuf>,
    as_json: bool,
    quotas: bool,
    config: bool,
    db: bool,
) -> Result<()> {
    use owo_colors::OwoColorize;
    if config {
        #[cfg(feature = "codex")]
        {
            return display_config(as_json).await;
        }
        #[cfg(not(feature = "codex"))]
        {
            anyhow::bail!(
                "Config display requires the codex feature. Rebuild with: cargo build -p vox-cli --features codex"
            );
        }
    }

    if quotas {
        #[cfg(feature = "codex")]
        {
            return display_quotas(as_json).await;
        }
        #[cfg(not(feature = "codex"))]
        {
            anyhow::bail!(
                "BYOK quota display requires the codex feature. Rebuild with: cargo build -p vox-cli --features codex"
            );
        }
    }
    if db {
        #[cfg(feature = "codex")]
        {
            return display_db_intelligence(as_json).await;
        }
        #[cfg(not(feature = "codex"))]
        {
            anyhow::bail!(
                "Database intelligence display requires the codex feature. Rebuild with: cargo build -p vox-cli --features codex"
            );
        }
    }

    let base =
        run_dir.unwrap_or_else(|| PathBuf::from(vox_scaling_policy::DEFAULT_MENS_RUNS_LATEST));
    let telemetry_path = if base.join("metrics.jsonl").exists() {
        base.join("metrics.jsonl")
    } else {
        base.join("telemetry.jsonl")
    };

    let run_state_path = base.join("run_state.json");
    let run_state = if run_state_path.exists() {
        read_utf8_path_capped(&run_state_path)
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
    } else {
        None
    };
    if !telemetry_path.exists() && run_state.is_none() {
        eprintln!(
            "  {} No telemetry found at {}",
            "⚠".yellow(),
            telemetry_path.display()
        );
        eprintln!("  Run `vox mens train` to generate training metrics.");
        return Ok(());
    }

    let content = if telemetry_path.exists() {
        read_utf8_path_capped(&telemetry_path)?
    } else {
        String::new()
    };
    let lines: Vec<serde_json::Value> = content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();

    let status_str = run_state
        .as_ref()
        .and_then(|v| v.get("status"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            if base.join("model_final.bin").exists()
                || base.join("candle_qlora_adapter.safetensors").exists()
            {
                Some("completed".to_string())
            } else if lines.iter().any(|v| v.get("step").is_some()) {
                Some("running".to_string())
            } else {
                Some("unknown".to_string())
            }
        })
        .unwrap_or_else(|| "unknown".to_string());

    let step_records: Vec<&serde_json::Value> =
        lines.iter().filter(|v| v.get("step").is_some()).collect();

    let mut used_valid_tokens_alias = false;
    let total_valid_tokens: u64 = step_records
        .iter()
        .map(|v| {
            v.get("valid_tokens")
                .and_then(|x| x.as_u64())
                .or_else(|| {
                    used_valid_tokens_alias = true;
                    v.get("supervised_tokens").and_then(|x| x.as_u64())
                })
                .unwrap_or(0)
        })
        .sum();
    let mut used_theoretical_tokens_alias = false;
    let total_theoretical_tokens: u64 = step_records
        .iter()
        .map(|v| {
            v.get("theoretical_tokens")
                .and_then(|x| x.as_u64())
                .or_else(|| {
                    used_theoretical_tokens_alias = true;
                    v.get("total_tokens").and_then(|x| x.as_u64())
                })
                .unwrap_or(0)
        })
        .sum();
    let total_truncated: u64 = step_records
        .iter()
        .map(|v| {
            v.get("truncated_samples")
                .and_then(|x| x.as_u64())
                .unwrap_or(0)
        })
        .sum();
    let total_zero_supervision: u64 = step_records
        .iter()
        .map(|v| {
            v.get("zero_supervision_samples")
                .and_then(|x| x.as_u64())
                .unwrap_or(0)
        })
        .sum();
    let supervised_ratio_pct = if total_theoretical_tokens == 0 {
        0.0
    } else {
        (total_valid_tokens as f64 / total_theoretical_tokens as f64) * 100.0
    };

    // Last nonzero loss: find the most-recent step where loss > 1e-7
    let last_nonzero_loss: Option<(u64, f64)> = step_records.iter().rev().find_map(|v| {
        let loss = v.get("loss").and_then(|x| x.as_f64())?;
        let step = v.get("global_step").and_then(|x| x.as_u64())?;
        if loss > 1e-7 {
            Some((step, loss))
        } else {
            None
        }
    });

    // Stall detection: last 10 steps all have zero supervision AND loss < 1e-7
    let recent_window: Vec<_> = step_records.iter().rev().take(10).collect();
    let is_stalled = recent_window.len() >= 5
        && recent_window.iter().all(|v| {
            let valid = v.get("valid_tokens").and_then(|x| x.as_u64()).unwrap_or(1);
            let loss = v.get("loss").and_then(|x| x.as_f64()).unwrap_or(1.0);
            valid == 0 && loss < 1e-7
        });

    let effective_status = if is_stalled && status_str == "running" {
        "stalled".to_string()
    } else {
        status_str.clone()
    };

    let last_warning = step_records
        .iter()
        .rev()
        .find_map(|v| v.get("warning").and_then(|w| w.as_str()));

    if as_json {
        let summary = serde_json::json!({
            "telemetry_path": telemetry_path.to_string_lossy(),
            "run_state_path": run_state_path.to_string_lossy(),
            "status": effective_status,
            "stalled": is_stalled,
            "total_records": lines.len(),
            "total_valid_tokens": total_valid_tokens,
            "total_theoretical_tokens": total_theoretical_tokens,
            "supervised_ratio_pct": supervised_ratio_pct,
            "total_truncated_samples": total_truncated,
            "total_zero_supervision_samples": total_zero_supervision,
            "last_nonzero_loss_step": last_nonzero_loss.map(|(s, _)| s),
            "last_nonzero_loss_value": last_nonzero_loss.map(|(_, l)| l),
            "last_warning": last_warning,
            "run_state": run_state,
            "records": lines,
        });
        println!("{}", serde_json::to_string_pretty(&summary)?);
        return Ok(());
    }

    let last = lines.iter().rfind(|v| v.get("step").is_some());
    let last_epoch = lines.iter().rfind(|v| v.get("avg_loss").is_some());

    // Status label with color coding
    let status_label = match effective_status.as_str() {
        "completed" => effective_status.green().to_string(),
        "running" => effective_status.cyan().to_string(),
        "stalled" => effective_status.yellow().to_string(),
        "oom_failed" | "panic_failed" | "failed" => effective_status.red().to_string(),
        other => other.to_string(),
    };

    eprintln!(
        "{}",
        "┌─ Mens Training Status ─────────────────────────┐".cyan()
    );
    eprintln!(
        "│ Telemetry: {:<40}│",
        format!("{}", telemetry_path.display())
            .chars()
            .take(40)
            .collect::<String>()
    );
    eprintln!("│ Records:   {:<40}│", lines.len());
    eprintln!("│ Status:    {:<40}│", status_label);

    if is_stalled {
        eprintln!(
            "│ {} Stall detected: last {} steps have zero supervision/loss      │",
            "⚠".yellow(),
            recent_window.len()
        );
    }

    if let Some(step) = last {
        let step_n = step
            .get("global_step")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let loss = step.get("loss").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let lr = step
            .get("learning_rate")
            .and_then(|v| v.as_f64())
            .or_else(|| step.get("lr").and_then(|v| v.as_f64()))
            .unwrap_or(0.0);
        let sup_pct = step
            .get("supervised_ratio_pct")
            .and_then(|v| v.as_f64())
            .or_else(|| {
                let valid = step.get("valid_tokens").and_then(|x| x.as_u64())?;
                let theoretical = step.get("theoretical_tokens").and_then(|x| x.as_u64())?;
                if theoretical == 0 {
                    Some(0.0)
                } else {
                    Some((valid as f64 / theoretical as f64) * 100.0)
                }
            })
            .unwrap_or(0.0);
        eprintln!(
            "│ Last step: {:<40}│",
            format!(
                "step={} loss={:.4} lr={:.2e} sup={:.1}%",
                step_n, loss, lr, sup_pct
            )
        );
    }

    if let Some((nonzero_step, nonzero_loss)) = last_nonzero_loss {
        eprintln!(
            "│ Last>0 loss:{:<40}│",
            format!("step={} loss={:.4}", nonzero_step, nonzero_loss)
        );
    } else if !step_records.is_empty() {
        eprintln!(
            "│ {} No nonzero loss observed — supervision may be zero throughout     │",
            "⚠".yellow()
        );
    }

    if let Some(epoch) = last_epoch {
        let e = epoch.get("epoch").and_then(|v| v.as_u64()).unwrap_or(0);
        let avg = epoch
            .get("avg_loss")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        eprintln!(
            "│ Last epoch:{:<40}│",
            format!("epoch={} avg_loss={:.4}", e, avg)
        );
        if let Some(ckpt) = epoch.get("checkpoint_path").and_then(|v| v.as_str()) {
            eprintln!(
                "│ Checkpoint:{:<40}│",
                ckpt.chars().take(40).collect::<String>()
            );
        }
    }

    eprintln!(
        "│ Supervised:{:<40}│",
        format!(
            "{:.1}% ({} / {})",
            supervised_ratio_pct, total_valid_tokens, total_theoretical_tokens
        )
    );
    eprintln!(
        "│ Truncation:{:<40}│",
        format!(
            "samples={} zero_sup={}",
            total_truncated, total_zero_supervision
        )
    );

    // Model card
    let model_card = base.join("MODEL_CARD.md");
    if model_card.exists() {
        eprintln!(
            "│ Model card:{:<40}│",
            model_card
                .display()
                .to_string()
                .chars()
                .take(40)
                .collect::<String>()
        );
    }

    if let Some(w) = last_warning {
        eprintln!(
            "│ {} Warning: {:<38}│",
            "⚠".yellow(),
            w.chars().take(38).collect::<String>()
        );
    }
    if used_valid_tokens_alias || used_theoretical_tokens_alias {
        eprintln!(
            "  {} status consumed deprecated telemetry aliases; emit canonical valid_tokens/theoretical_tokens",
            "⚠".yellow()
        );
    }
    if let Some(err) = run_state
        .as_ref()
        .and_then(|v| v.get("error"))
        .and_then(|v| v.as_str())
    {
        eprintln!(
            "│ {} Error:   {:<38}│",
            "✗".red(),
            err.chars().take(38).collect::<String>()
        );
    }

    eprintln!(
        "{}",
        "└──────────────────────────────────────────────────┘".cyan()
    );

    // Action guidance based on status
    match effective_status.as_str() {
        "oom_failed" => {
            eprintln!();
            eprintln!(
                "  {} OOM failure. Reduce --batch-size, --seq-len, or --rank, or use --preset safe.",
                "→".cyan()
            );
        }
        "panic_failed" => {
            eprintln!();
            eprintln!(
                "  {} Training panicked. Check run_state.json for details. Try --preset tiny for diagnosis.",
                "→".cyan()
            );
        }
        "stalled" => {
            eprintln!();
            eprintln!(
                "  {} Stalled run. Supervision is zero. Consider increasing --seq-len or reducing system prompt.",
                "→".cyan()
            );
        }
        _ => {}
    }

    Ok(())
}

#[cfg(feature = "codex")]
async fn display_config(as_json: bool) -> Result<()> {
    let resp = crate::dei_daemon::call(
        crate::dei_daemon::method::CONFIG_GET,
        serde_json::Value::Null,
        false,
    )
    .await?;

    if as_json {
        println!("{}", serde_json::to_string_pretty(&resp)?);
    } else {
        println!("\n  \x1b[1;36mOrchestrator Inference Configuration\x1b[0m");
        if let Some(inf) = resp.get("inference_config") {
            let tier_str = if let Some(tier) = inf.get("tier") {
                if let Some(s) = tier.as_str() {
                    s
                } else {
                    tier.get("kind")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                }
            } else {
                "unknown"
            };
            println!("  Tier      : {}", tier_str);
            println!(
                "  Quality   : {}",
                inf.get("quality")
                    .and_then(|v| v.as_str())
                    .unwrap_or("balanced")
            );
            println!(
                "  Verbosity : {}",
                inf.get("verbosity")
                    .and_then(|v| v.as_str())
                    .unwrap_or("normal")
            );
            if let Some(modality) = inf.get("modalities") {
                let tool = modality
                    .get("tool_calling")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let web = modality
                    .get("web_search")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let vision = modality
                    .get("vision")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let json = modality
                    .get("structured_output")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let stream = modality
                    .get("streaming")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                println!(
                    "  Modalities: tool_calling={}, web_search={}, vision={}, json={}, stream={}",
                    tool, web, vision, json, stream
                );
            }
        } else {
            println!("  (No inference_config found in response)");
        }
        println!();
    }
    Ok(())
}

#[cfg(feature = "codex")]
async fn display_quotas(_as_json: bool) -> Result<()> {
    anyhow::bail!(
        "BYOK quota rollups previously depended on the `vox-orchestrator` usage tracker (not linked in this CLI build). \
         That integration is not shipped in this CLI build. \
         Use `vox mens status` without `--quotas`, or check limits in your provider console."
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn run_status_missing_dir_does_not_panic() {
        let result = run_status(
            Some(PathBuf::from("/nonexistent/run/dir")),
            false,
            false,
            false,
            false,
        )
        .await;
        assert!(
            result.is_ok(),
            "missing run dir should return Ok, not panic"
        );
    }

    #[tokio::test]
    async fn run_status_json_missing_dir_does_not_panic() {
        let result = run_status(
            Some(PathBuf::from("/nonexistent/run/dir")),
            true,
            false,
            false,
            false,
        )
        .await;
        assert!(result.is_ok());
    }

    #[cfg(not(feature = "codex"))]
    #[tokio::test]
    async fn run_status_quotas_without_codex_returns_error() {
        let result = run_status(
            Some(PathBuf::from("/nonexistent")),
            false,
            true,
            false,
            false,
        )
        .await;
        assert!(result.is_err(), "quotas without codex feature should error");
        assert!(
            result.unwrap_err().to_string().contains("codex"),
            "error message should mention codex"
        );
    }

    #[cfg(feature = "codex")]
    #[tokio::test(flavor = "multi_thread")]
    async fn test_display_quotas_smoke() {
        // Just verify it doesn't panic and returns a result.
        // Even if the DB is missing, it should return an Err or Ok(()) gracefully.
        let _ = display_quotas(true).await;
        let _ = display_quotas(false).await;
    }
}

#[cfg(feature = "codex")]
async fn display_db_intelligence(as_json: bool) -> anyhow::Result<()> {
    use owo_colors::OwoColorize;
    let vox_db = vox_db::VoxDb::connect_default().await?;

    let quality = vox_db.query_corpus_quality_summary().await?;
    let steps = vox_db.query_recent_grpo_steps(None, 10).await?;

    let telemetry = vox_db
        .list_research_metrics_by_session("", Some("populi_control_event"), 10)
        .await?;

    if as_json {
        let out = serde_json::json!({
            "corpus_quality": quality,
            "recent_grpo_steps": steps,
            "mesh_telemetry": telemetry,
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        println!(
            "\n  {}",
            "MENS Intelligence Summary (Arca DB)".bold().cyan()
        );
        println!("  ┌─ Corpus Quality ───────────────────────────┐");
        println!("  │ Total Pairs:     {:<26}│", quality.total_pairs);
        println!(
            "  │ Parse Rate:      {:<26.2}%│",
            quality.parse_rate * 100.0
        );
        println!("  │ Avg AST Depth:   {:<26.2}│", quality.avg_ast_depth);
        println!(
            "  │ Avg Constructs:  {:<26.2}│",
            quality.avg_construct_count
        );
        println!("  │ Avg Reward:      {:<26.2}│", quality.avg_reward_score);
        println!("  └────────────────────────────────────────────┘");

        if !steps.is_empty() {
            println!("\n  {}", "Recent GRPO Steps".bold().cyan());
            println!(
                "  {:<12} {:>6} {:>8} {:>10} {:>8}",
                "Run ID", "Step", "Reward", "Loss", "Parse %"
            );
            for s in steps {
                let short_id = if s.run_id.len() > 10 {
                    &s.run_id[..10]
                } else {
                    &s.run_id
                };
                println!(
                    "  {:<12} {:>6} {:>8.4} {:>10.4} {:>8.1}%",
                    short_id,
                    s.step,
                    s.mean_reward,
                    s.policy_loss,
                    s.parse_rate * 100.0
                );
            }
        } else {
            println!("\n  {}", "No GRPO steps found in database.".yellow());
        }

        if !telemetry.is_empty() {
            println!(
                "\n  {}",
                "Recent Mesh Health (Hardware Telemetry)".bold().cyan()
            );
            println!(
                "  {:<12} {:>6} {:>8} {:>10} {:>12}",
                "Node ID", "Util", "Temp", "Power", "VRAM (Used)"
            );
            for (sid, _mtype, _mval, meta) in telemetry {
                let short_id = if sid.len() > 10 { &sid[..10] } else { &sid };
                if let Some(m) =
                    meta.and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                {
                    let util = m.get("gpu_util").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let temp = m.get("gpu_temp_c").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let power = m.get("gpu_power_w").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let vram_used = m.get("vram_used_mb").and_then(|v| v.as_u64()).unwrap_or(0);
                    let vram_total = m.get("vram_total_mb").and_then(|v| v.as_u64()).unwrap_or(1);

                    println!(
                        "  {:<12} {:>5.1}% {:>7.1}°C {:>9.1}W {:>8}MB/{:>2}MB",
                        short_id,
                        util,
                        temp,
                        power,
                        vram_used,
                        vram_total / 1024
                    );
                }
            }
        } else {
            println!("\n  {}", "No mesh telemetry found in database.".yellow());
        }
        println!();
        println!();
    }
    Ok(())
}
