mod cache;
mod fix_pipeline;

use std::path::PathBuf;

use anyhow::Context;

use owo_colors::OwoColorize;
use vox_bounded_fs::{read_utf8_path_capped, read_utf8_path_capped_async};
use vox_code_audit::detectors::all_rules;
use vox_code_audit::diagnostics::catalog::{ALL_KNOWN_IDS, explain_url, is_known_id};
use vox_code_audit::diagnostics::explain_ai_fixture_diagnostic;
use vox_code_audit::rules::{Language, Severity};
use vox_code_audit::{Finding, OutputFormat, ToestubConfig, ToestubEngine};

use vox_db::{
    Codex, add_suppression, load_baseline as db_load_baseline, load_latest_task_queue,
    save_baseline as db_save_baseline, save_task_queue,
};
/// Run the TOESTUB analysis.
#[allow(clippy::too_many_arguments)]
pub async fn run(
    path: &std::path::Path,
    format: Option<&str>,
    severity: Option<&str>,
    suggest_fixes: bool,
    rules: Option<&str>,
    excludes: &[String],
    langs: Option<&str>,
    baseline: Option<&str>,
    save_baseline: Option<&str>,
    task_list: bool,
    import_suppressions: bool,
    ingest_findings: Option<&std::path::Path>,
    fix_pipeline: bool,
    fix_pipeline_apply: bool,
    gate: Option<&str>,
    gate_budget_path: Option<&std::path::Path>,
    _verify_impacted: bool,
    _max_escalation: u8,
    self_heal_safe_mode: bool,
) -> anyhow::Result<()> {
    // --task-list: show last saved queue from VoxDB and exit
    if task_list {
        if let Ok(db) = Codex::connect_default().await {
            let user_id = vox_gamify::db::canonical_user_id();
            if let Ok(Some((total_findings, fix_suggestions_json))) =
                load_latest_task_queue(&db, &user_id).await
            {
                let fix_suggestions: Vec<vox_code_audit::task_queue::FixSuggestion> =
                    serde_json::from_str(&fix_suggestions_json).unwrap_or_default();
                let queue = vox_code_audit::TaskQueue {
                    total_findings: total_findings as usize,
                    fix_suggestions,
                };
                println!("{}", queue.to_markdown_checklist());
            } else {
                println!("No saved task queue found. Run a scan first.");
            }
            db.shutdown_for_drop();
        } else {
            println!("Could not connect to VoxDB. Run a scan to save a task queue.");
        }
        return Ok(());
    }

    // --import-suppressions: load toestub.toml and upsert into VoxDB, then exit
    if import_suppressions {
        let db = Codex::connect_default().await?;
        let toml_path = path.join("toestub.toml");
        let content = read_utf8_path_capped_async(&toml_path)
            .await
            .with_context(|| format!("Failed to read {}", toml_path.display()))?;
        let parsed: toml::Value = toml::from_str(&content)?;
        let count = if let Some(arr) = parsed.get("suppress").and_then(|s| s.as_array()) {
            let mut n = 0u32;
            for item in arr {
                if let Some(t) = item.as_table() {
                    let path_str = t
                        .get("path")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| anyhow::anyhow!("suppress entry missing 'path'"))?
                        .to_string();
                    let line = t
                        .get("line")
                        .and_then(|v| v.as_integer())
                        .ok_or_else(|| anyhow::anyhow!("suppress entry missing 'line'"))?;
                    let rule_id = t
                        .get("rule")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| anyhow::anyhow!("suppress entry missing 'rule'"))?
                        .to_string();
                    let reason = t.get("reason").and_then(|v| v.as_str());
                    add_suppression(&db, &path_str, line, &rule_id, reason).await?;
                    n += 1;
                }
            }
            n
        } else {
            0
        };
        println!(
            "Imported {} suppression(s) from toestub.toml into VoxDB.",
            count
        );
        db.shutdown_for_drop();
        return Ok(());
    }

    if let Some(ingest_path) = ingest_findings {
        let content = read_utf8_path_capped(ingest_path)
            .with_context(|| format!("Failed to read {}", ingest_path.display()))?;
        let findings: Vec<Finding> = serde_json::from_str(&content)?;
        let ingest_path = ingest_path.to_path_buf();
        let run_scope = path.to_string_lossy().to_string();
        let total = findings.len();
        let handle = tokio::spawn(async move {
            let db = Codex::connect_default()
                .await
                .map_err(|e| anyhow::anyhow!("connect: {:?}", e))?;
            let user_id = vox_gamify::db::canonical_user_id();
            let task_queue = vox_code_audit::TaskQueue::from_findings(&findings);
            let fix_suggestions_json = serde_json::to_string(&task_queue.fix_suggestions)
                .unwrap_or_else(|_| "[]".to_string());

            save_task_queue(
                &db,
                &user_id,
                &run_scope,
                task_queue.total_findings as i64,
                &fix_suggestions_json,
            )
            .await
            .map_err(|e| anyhow::anyhow!("save_task_queue: {:?}", e))?;

            println!(
                "Ingested {} findings from {} into VoxDB task queue (user_id={}).",
                total,
                ingest_path.display(),
                user_id
            );
            db.shutdown_for_drop();
            Ok::<(), anyhow::Error>(())
        });
        handle
            .await
            .map_err(|_| anyhow::anyhow!("ingest task panicked"))??;
        return Ok(());
    }

    // Connect to VoxDB early (for cache + baseline/task persistence)
    let db_opt = Codex::connect_default().await.ok().map(std::sync::Arc::new);

    let schema_path = tokio::task::spawn_blocking(|| std::fs::canonicalize("vox-schema.json").ok())
        .await
        .ok()
        .flatten();

    let config = ToestubConfig {
        roots: vec![PathBuf::from(path)],
        min_severity: match severity.unwrap_or("warning") {
            "info" => Severity::Info,
            "error" => Severity::Error,
            "critical" => Severity::Critical,
            _ => Severity::Warning,
        },
        format: OutputFormat::parse_format(format.unwrap_or("terminal")),
        suggest_fixes,
        languages: langs.map(|l| {
            l.split(',')
                .filter_map(|s| match s.trim() {
                    "rust" | "rs" => Some(Language::Rust),
                    "ts" | "typescript" => Some(Language::TypeScript),
                    "python" | "py" => Some(Language::Python),
                    "gdscript" | "gd" => Some(Language::GDScript),
                    "vox" => Some(Language::Vox),
                    _ => None,
                })
                .collect()
        }),
        excludes: excludes.to_vec(),
        rule_filter: rules.map(|r| r.split(',').map(|s| s.trim().to_string()).collect()),
        schema_path,
        ..Default::default()
    };

    // CPU-heavy scan: keep off the Tokio worker thread (nested async DB calls are not used here).
    let (result, output) = tokio::task::spawn_blocking(move || {
        let engine = ToestubEngine::new(config);
        engine.run_and_report()
    })
    .await
    .context("spawn_blocking join (stub-check engine)")?;

    // Print the formatted output
    println!("{}", output);

    // Print summary footer
    let summary = result.summary();
    if result.findings.is_empty() {
        println!(
            "{}",
            "🦶 TOESTUB: All clear — no issues found.".green().bold()
        );
    } else {
        println!(
            "{} Scanned {} files with {} rules, found {} issues.",
            "🦶 TOESTUB:".bold(),
            result.files_scanned,
            result.rules_applied,
            result.findings.len(),
        );
        if summary.critical > 0 || summary.error > 0 {
            println!(
                "{}",
                format!(
                    "   ⚠  {} critical, {} errors require attention.",
                    summary.critical, summary.error,
                )
                .red()
            );
        }
    }

    // If fix suggestions were requested, also dump the task queue
    if suggest_fixes && !result.task_queue.fix_suggestions.is_empty() {
        println!("\n{}", result.task_queue.to_markdown_checklist());
    }

    // ── Fix pipeline: staged passes for high-volume rules ──
    if fix_pipeline && !result.findings.is_empty() {
        let apply = fix_pipeline_apply && !self_heal_safe_mode;
        let findings = result.findings.clone();
        let path_buf = path.to_path_buf();
        tokio::task::spawn_blocking(move || {
            fix_pipeline::run_fix_pipeline(&findings, &path_buf, apply)
        })
        .await
        .context("fix pipeline task join")??;
    }

    // ── Save baseline / task queue to VoxDB ──
    if let Some(ref db) = db_opt {
        let user_id = vox_gamify::db::canonical_user_id();
        let run_scope = path.to_string_lossy().to_string();

        if let Some(name) = save_baseline {
            let findings_json =
                serde_json::to_string(&result.findings).unwrap_or_else(|_| "[]".to_string());
            if db_save_baseline(db, name, &run_scope, &findings_json)
                .await
                .is_ok()
            {
                println!("\n  Saved baseline '{}' to VoxDB.", name);
            }
        }

        if suggest_fixes {
            let fix_suggestions_json = serde_json::to_string(&result.task_queue.fix_suggestions)
                .unwrap_or_else(|_| "[]".to_string());
            let _ = save_task_queue(
                db,
                &user_id,
                &run_scope,
                result.task_queue.total_findings as i64,
                &fix_suggestions_json,
            )
            .await;
        }
    }

    // ── Tiered CI gate: warning budget and ratchet ──
    if let (Some(gate_mode), Some(budget_path)) = (gate, gate_budget_path) {
        let by_rule: std::collections::HashMap<String, u32> =
            result
                .findings
                .iter()
                .fold(std::collections::HashMap::new(), |mut m, f| {
                    *m.entry(f.rule_id.clone()).or_insert(0) += 1;
                    m
                });
        match gate_mode.to_lowercase().as_str() {
            "ratchet" => {
                let arr: Vec<serde_json::Value> = by_rule
                    .into_iter()
                    .map(|(rule_id, count)| serde_json::json!({ "rule_id": rule_id, "count": count }))
                    .collect();
                let json = serde_json::to_string_pretty(&arr).unwrap_or_else(|_| "[]".to_string());
                if let Some(p) = budget_path.parent() {
                    let _ = tokio::fs::create_dir_all(p).await;
                }
                tokio::fs::write(budget_path, json).await?;
                println!(
                    "\n  Gate ratchet: saved budget to {}",
                    budget_path.display()
                );
            }
            "warnings" => {
                let content = read_utf8_path_capped_async(budget_path)
                    .await
                    .with_context(|| format!("Failed to read budget {}", budget_path.display()))?;
                let budget_arr: Vec<serde_json::Value> = serde_json::from_str(&content)?;
                let budget: std::collections::HashMap<String, u32> = budget_arr
                    .iter()
                    .filter_map(|o| {
                        let id = o.get("rule_id")?.as_str()?.to_string();
                        let c = o.get("count")?.as_u64()? as u32;
                        Some((id, c))
                    })
                    .collect();
                let gated_rules = ["stub/", "doc/missing-frontmatter", "unwired/module"]; // toestub-ignore(stub)
                let mut over = Vec::new();
                for (rule_id, &current) in &by_rule {
                    let budget_count = budget.get(rule_id).copied().unwrap_or(0);
                    if current <= budget_count {
                        continue;
                    }
                    if gated_rules
                        .iter()
                        .any(|p| rule_id == *p || rule_id.starts_with(*p))
                    {
                        over.push((rule_id.clone(), current, budget_count));
                    }
                }
                if !over.is_empty() {
                    for (id, cur, b) in &over {
                        eprintln!("  {}: current {} > budget {}", id, cur, b);
                    }
                    anyhow::bail!(
                        "Gate warnings: {} rule(s) exceed budget. Ratchet with --gate ratchet --gate-budget-path {}",
                        over.len(),
                        budget_path.display()
                    );
                }
            }
            _ => {}
        }
    }

    // ── Baseline diff: exit 1 on new or regressed findings ──
    if let Some(baseline_arg) = baseline {
        let baseline_map = if baseline_arg.contains('/')
            || baseline_arg.contains('\\')
            || baseline_arg.ends_with(".json")
        {
            let path = PathBuf::from(baseline_arg);
            let s = read_utf8_path_capped_async(&path).await?;
            cache::baseline_from_json(&s)?
        } else {
            let db = db_opt
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("VoxDB not available."))?;
            if let Some((_scope, findings_json)) =
                db_load_baseline(db.as_ref(), baseline_arg).await?
            {
                cache::baseline_from_json(&findings_json)?
            } else {
                anyhow::bail!("Baseline '{}' not found in VoxDB.", baseline_arg);
            }
        };

        let mut new = Vec::new();
        let mut regressed = Vec::new();
        for f in &result.findings {
            let key = fix_pipeline::norm_key(&f.file, f.line, &f.rule_id);
            match baseline_map.get(&key) {
                None => new.push(f.clone()),
                Some(sev) if f.severity > *sev => regressed.push((f.clone(), *sev)),
                _ => {}
            }
        }
        if !new.is_empty() || !regressed.is_empty() {
            if !new.is_empty() {
                eprintln!("{} new finding(s) not in baseline:", new.len());
                for f in new.iter().take(20) {
                    eprintln!("  {}:{} {}", f.file.display(), f.line, f.rule_id);
                }
                if new.len() > 20 {
                    eprintln!("  ... and {} more", new.len() - 20);
                }
            }
            if !regressed.is_empty() {
                eprintln!(
                    "{} regressed (severity increased) vs baseline:",
                    regressed.len()
                );
                for (f, _) in regressed.iter().take(20) {
                    eprintln!("  {}:{} {}", f.file.display(), f.line, f.rule_id);
                }
                if regressed.len() > 20 {
                    eprintln!("  ... and {} more", regressed.len() - 20);
                }
            }
            anyhow::bail!(
                "Baseline comparison failed: {} new, {} regressed.",
                new.len(),
                regressed.len()
            );
        }
    }

    // ── Ludus: canonical router (clean-scan rewards + debt teaching) ──
    if let Some(ref db) = db_opt {
        let user_id = vox_gamify::db::canonical_user_id();
        if result.findings.is_empty() {
            let ev = serde_json::json!({
                "type": "toestub_scan_clean",
                "agent_id": 0u64,
            });
            match vox_gamify::event_router::route_event_auto_user(db.as_ref(), &ev).await {
                Ok(res) => print_stub_check_ludus_route(&res),
                Err(e) => tracing::warn!(error = %e, "ludus route_event (toestub clean)"),
            }
        } else {
            println!(
                "\n  {} Want extra rewards? Run {} to fight these bugs in a battle.",
                "🔮".bright_magenta(),
                "vox ludus battle start".bright_green()
            );
            let debt_signal = summary.critical + summary.error > 0
                || result.findings.len() >= 10
                || summary.warning >= 15;
            if debt_signal {
                let dedupe = stub_check_debt_dedupe_key(path, &user_id);
                let ev = serde_json::json!({
                    "type": "stub_check_debt",
                    "agent_id": 0u64,
                    "findings": result.findings.len(),
                    "critical": summary.critical,
                    "errors": summary.error,
                    "warnings": summary.warning,
                    "ludus_dedupe_id": dedupe,
                });
                if let Err(e) =
                    vox_gamify::event_router::route_event_auto_user(db.as_ref(), &ev).await
                {
                    tracing::warn!(error = %e, "ludus route_event (stub_check_debt)");
                }
            }
        }
    }

    if let Some(ref db) = db_opt {
        db.shutdown_for_drop();
    }

    if result.has_errors() {
        anyhow::bail!(
            "TOESTUB found {} error-level issues.",
            summary.error + summary.critical
        );
    }

    Ok(())
}

/// One teaching / debt signal per local day per scan root (idempotency key for `route_event`).
fn stub_check_debt_dedupe_key(path: &std::path::Path, user_id: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    user_id.hash(&mut h);
    path.to_string_lossy().hash(&mut h);
    let day = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() / 86_400)
        .unwrap_or(0);
    day.hash(&mut h);
    h.finish()
}

fn print_stub_check_ludus_route(res: &vox_gamify::reward_policy::RouteResult) {
    use owo_colors::OwoColorize;
    let mut header = false;
    if let Some(reward) = &res.reward {
        if reward.xp > 0 || reward.crystals > 0 || reward.lumens != 0 || reward.grant_shield {
            println!();
            println!("{}", "🎉 Ludus — clean TOESTUB scan".bright_yellow().bold());
            header = true;
            if reward.xp > 0 {
                println!("  +{} XP", reward.xp.to_string().bright_cyan());
            }
            if reward.crystals > 0 {
                println!("  +{} 💎", reward.crystals.to_string().bright_cyan());
            }
            if reward.lumens > 0 {
                println!("  +{} ✦", reward.lumens.to_string().bright_magenta());
            } else if reward.lumens < 0 {
                println!("  {} ✦", reward.lumens.to_string().bright_red());
            }
            if reward.grant_shield {
                println!("  {}", "🛡️ Streak shield granted".bright_green().bold());
            }
        }
    }
    if let Some((lvl, title)) = &res.leveled_up {
        if !header {
            println!();
            println!("{}", "🎉 Ludus — clean TOESTUB scan".bright_yellow().bold());
        }
        println!(
            "{}",
            format!("  ⚡ LEVEL {}! You are now: {} ⚡", lvl, title)
                .bright_yellow()
                .bold()
        );
    }
}

/// Print the explanation for a single diagnostic ID and exit.
///
/// Looks up the registered detector (if any) for its `explain()` text, then
/// prints the stable ID, severity, URL, and rationale.
pub fn explain_diagnostic(id: &str) -> anyhow::Result<()> {
    if !is_known_id(id) {
        // Attempt a fuzzy match to be helpful
        let similar: Vec<&&str> = ALL_KNOWN_IDS
            .iter()
            .filter(|k| k.contains(id.split('/').last().unwrap_or(id)))
            .collect();
        if similar.is_empty() {
            anyhow::bail!(
                "Unknown diagnostic ID: `{id}`.\n\
                 Run `vox check --list-diagnostics` to see all known IDs."
            );
        }
        eprintln!(
            "Unknown ID `{id}`. Did you mean one of:\n{}",
            similar
                .iter()
                .map(|k| format!("  {k}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
        anyhow::bail!("ID not found");
    }

    let url = explain_url(id);

    // Find the registered detector to get its explain() text and minimal_repro.
    let rules = all_rules(None);
    let matching_rule = rules.iter().find(|r| r.diagnostic_id() == Some(id));
    let explain_text = explain_ai_fixture_diagnostic(id)
        .unwrap_or_else(|| matching_rule.map(|r| r.explain()).unwrap_or(""));
    let minimal_repro: Option<&'static str> = matching_rule.and_then(|r| r.minimal_repro());

    println!("{}", "─".repeat(70));
    println!("  Diagnostic: {}", id.bright_cyan().bold());
    println!("  URL:        {}", url.dimmed());
    println!("{}", "─".repeat(70));

    if explain_text.is_empty() {
        println!("  (No extended explanation available for this diagnostic yet.)");
        println!("  See: {url}");
    } else {
        for line in explain_text.lines() {
            println!("  {line}");
        }
    }

    // Show the minimal reproduction snippet when the detector provides one.
    if let Some(repro) = minimal_repro {
        println!();
        println!("  {}", "Example:".bold());
        for line in repro.lines() {
            println!("  {line}");
        }
    }

    println!("{}", "─".repeat(70));
    Ok(())
}

/// List all known stable diagnostic IDs.
pub fn list_diagnostics() {
    println!("{}", "─".repeat(70));
    println!(
        "  All known Vox diagnostic IDs ({} total)",
        ALL_KNOWN_IDS.len()
    );
    println!("{}", "─".repeat(70));
    for id in ALL_KNOWN_IDS {
        println!("  {}", id.bright_cyan());
    }
    println!("{}", "─".repeat(70));
    println!("  Use `vox check --explain <ID>` for details on any ID.");
}

/// Check that all suppression comments in a set of findings have a rationale.
///
/// Returns an error listing any suppressions that lack a `— <reason>` of ≥ 20 chars.
pub fn check_rationale_required(path: &std::path::Path) -> anyhow::Result<()> {
    let mut violations: Vec<String> = Vec::new();
    let patterns = [
        regex::Regex::new(r#"//\s*toestub-ignore\([^)]+\)\s*$"#).unwrap(),
        regex::Regex::new(r#"//\s*vox:skip\s*$"#).unwrap(),
    ];

    fn walk_files(dir: &std::path::Path, out: &mut Vec<String>, patterns: &[regex::Regex]) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                walk_files(&p, out, patterns);
            } else if matches!(
                p.extension().and_then(|e| e.to_str()),
                Some("rs" | "vox" | "ts")
            ) {
                let Ok(content) = std::fs::read_to_string(&p) else {
                    continue;
                };
                for (line_num, line) in content.lines().enumerate() {
                    for pat in patterns {
                        if pat.is_match(line) {
                            let reason_part = line
                                .split("—")
                                .nth(1)
                                .or_else(|| line.split("--").nth(1))
                                .unwrap_or("")
                                .trim();
                            if reason_part.chars().filter(|c| !c.is_whitespace()).count() < 20 {
                                out.push(format!(
                                    "{}:{}: suppression lacks a rationale (≥ 20 chars after '—'): {}",
                                    p.display(),
                                    line_num + 1,
                                    line.trim()
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    walk_files(path, &mut violations, &patterns);

    if violations.is_empty() {
        println!("All suppression comments have adequate rationale.");
        Ok(())
    } else {
        for v in &violations {
            eprintln!("  {}", v);
        }
        anyhow::bail!(
            "{} suppression(s) missing rationale. Add '— <reason>' with ≥ 20 chars.",
            violations.len()
        )
    }
}
