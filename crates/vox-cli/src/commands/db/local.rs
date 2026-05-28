//! Local VoxDB inspection, maintenance, and preferences.

use crate::commands::db_retention;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use vox_bounded_fs::read_utf8_path_capped_async;

/// Print current VoxDB schema version and connection path.
pub async fn status() -> Result<()> {
    let db = vox_db::VoxDb::connect_default().await?;
    let version = db.schema_version().await?;
    let data_dir = vox_db::VoxDb::data_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    println!("VoxDB Status");
    println!("  Data directory : {data_dir}");
    println!("  Schema version : v{version}");
    Ok(())
}

fn format_research_metric_time(created_at: &str) -> String {
    let s = created_at.trim();
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        return dt.format("%H:%M:%S").to_string();
    }
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return dt.format("%H:%M:%S").to_string();
    }
    s.chars().take(32).collect()
}

/// Read-only audit: table row counts + storage PRAGMAs (JSON to stdout).
pub async fn audit(timestamps: bool) -> Result<()> {
    let db = vox_db::VoxDb::connect_default().await?;
    let out = vox_db::audit_database_json(&db, timestamps)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    println!("{}", serde_json::to_string_pretty(&out)?);
    Ok(())
}

/// Reset the database by dropping all tables and re-applying migrations.
pub async fn reset(file: Option<&PathBuf>) -> Result<()> {
    println!("Resetting database...");
    let path = vox_db::paths::default_db_path()
        .ok_or_else(|| anyhow::anyhow!("Could not resolve default Codex database path"))?;
    let path_str = path.to_string_lossy().into_owned();
    let _db = vox_db::VoxDb::open_local_reset_to_baseline(&path_str)
        .await
        .map_err(|e| anyhow::anyhow!("Reset failed: {e}"))?;
    println!("  Cleared {} and re-applied baseline.", path.display());

    println!("Re-migrating from .vox declarations...");
    migrate(file).await?;
    println!("Reset complete.");
    Ok(())
}

/// Print the current schema digest for LLM context.
pub async fn schema(file: Option<&PathBuf>) -> Result<()> {
    let path = file
        .cloned()
        .unwrap_or_else(|| PathBuf::from("src/main.vox"));

    if !path.exists() {
        anyhow::bail!(
            "No source file found at {}. Run `vox db schema --file <path>` to specify one.",
            path.display()
        );
    }

    let result = crate::pipeline::run_frontend(&path, false)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to parse source for schema: {}", e))?;

    let digest = vox_db::generate_schema_digest(&result.module, None);
    println!("{}", vox_db::format_llm_context(&digest));

    // Also print JSON for tool consumption
    println!("\n--- JSON DIGEST ---");
    println!("{}", serde_json::to_string_pretty(&digest)?);

    Ok(())
}

/// Print sample data from a table or collection.
pub async fn sample(table: &str, limit: i64) -> Result<()> {
    let db = vox_db::VoxDb::connect_default().await?;
    println!("Sample data from '{}' (limit {}):", table, limit);

    let rows = vox_db::sample_table_json_objects(&db, table, limit)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    if rows.is_empty() {
        println!("(no rows)");
        return Ok(());
    }

    for map in rows {
        println!("{}", serde_json::to_string(&map)?);
    }

    Ok(())
}

/// Apply any pending schema migrations.
pub async fn migrate(file: Option<&PathBuf>) -> Result<()> {
    let db = vox_db::VoxDb::connect_default().await?;
    let path = file
        .cloned()
        .unwrap_or_else(|| PathBuf::from("src/main.vox"));

    if !path.exists() {
        println!(
            "No source file found at {}. Run `vox db migrate --file <path>` to specify one.",
            path.display()
        );
        return Ok(());
    }

    let result = crate::pipeline::run_frontend(&path, false)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to parse source for migration: {}", e))?;
    let module = &result.module;

    let mut tables = Vec::new();
    let mut collections = Vec::new();
    let mut indexes = Vec::new();

    for decl in &module.declarations {
        match decl {
            vox_compiler::ast::decl::Decl::Table(t) => tables.push(t),
            vox_compiler::ast::decl::Decl::Collection(c) => collections.push(c),
            vox_compiler::ast::decl::Decl::Index(i) => indexes.push(i),
            _ => {}
        }
    }

    let migrator = vox_db::AutoMigrator::new(db.connection());
    let plan = migrator
        .sync_schema(&tables, &collections, &indexes)
        .await?;

    println!("{}", plan.describe());

    Ok(())
}

/// Export memory, patterns, and preferences for a user to JSON.
pub async fn export(user_id: &str, output: Option<&PathBuf>) -> Result<()> {
    let db = vox_db::VoxDb::connect_default().await?;

    let prefs = db.list_user_preferences(user_id, None).await?;
    let memories = db.recall_memory(user_id, None, 200, None).await?;
    let patterns = db.get_learned_patterns(user_id, 200).await?;

    let data = serde_json::json!({
        "user_id": user_id,
        "exported_at": chrono::Utc::now().to_rfc3339(),
        "preferences": prefs.iter().map(|(k, v)| serde_json::json!({"key": k, "value": v})).collect::<Vec<_>>(),
        "memories": memories.iter().map(|m| serde_json::json!({
            "type": m.memory_type,
            "content": m.content,
            "importance": m.importance,
        })).collect::<Vec<_>>(),
        "learned_patterns": patterns.iter().map(|p| serde_json::json!({
            "pattern_type": p.pattern_type,
            "category": p.category,
            "description": p.description,
            "confidence": p.confidence,
        })).collect::<Vec<_>>(),
    });

    let json_str = serde_json::to_string_pretty(&data)?;
    match output {
        Some(path) => {
            tokio::fs::write(path, &json_str).await?;
            println!("Exported to {}", path.display());
        }
        None => println!("{json_str}"),
    }
    Ok(())
}

/// Import preferences and memory from a JSON file previously exported with `vox db export`.
pub async fn import(path: &Path) -> Result<()> {
    let db = vox_db::VoxDb::connect_default().await?;
    let json_str = read_utf8_path_capped_async(path).await?;
    let data: serde_json::Value = serde_json::from_str(&json_str)?;

    let user_id = data["user_id"].as_str().unwrap_or("default");
    let mut pref_count = 0usize;
    let mut mem_count = 0usize;

    if let Some(prefs) = data["preferences"].as_array() {
        for pref in prefs {
            let key = pref["key"].as_str().unwrap_or("");
            let value = pref["value"].as_str().unwrap_or("");
            if !key.is_empty() {
                db.set_user_preference(user_id, key, value).await?;
                pref_count += 1;
            }
        }
    }

    if let Some(mems) = data["memories"].as_array() {
        for m in mems {
            let mtype = m["type"].as_str().unwrap_or("general");
            let content = m["content"].as_str().unwrap_or("");
            let importance = m["importance"].as_f64().unwrap_or(1.0);
            if !content.is_empty() {
                db.save_memory(vox_db::MemoryParams {
                    agent_id: user_id,
                    session_id: "import",
                    memory_type: mtype,
                    content,
                    metadata: None,
                    importance,
                    vcs_snapshot_id: None,
                })
                .await?;
                mem_count += 1;
            }
        }
    }

    println!("Imported from {}", path.display());
    println!("  Preferences : {pref_count}");
    println!("  Memory facts: {mem_count}");
    Ok(())
}

/// Run SQLite VACUUM to reclaim space and defragment the database.
pub async fn vacuum() -> Result<()> {
    let db = vox_db::VoxDb::connect_default().await?;
    db.run_sqlite_vacuum()
        .await
        .map_err(|e| anyhow::anyhow!("VACUUM failed: {e}"))?;
    println!("VACUUM complete. Database has been compacted.");
    Ok(())
}

/// Delete memory entries older than `days` days for a given agent/user.
pub async fn prune(user_id: &str, days: u32) -> Result<()> {
    let db = vox_db::VoxDb::connect_default().await?;
    let threshold = chrono::Utc::now()
        .checked_sub_signed(chrono::Duration::days(days as i64))
        .unwrap_or(chrono::Utc::now());
    let threshold_str = threshold.format("%Y-%m-%d %H:%M:%S").to_string();
    let deleted = db
        .delete_memories_created_before(user_id, threshold_str.as_str())
        .await
        .map_err(|e| anyhow::anyhow!("Prune failed: {e}"))?;
    println!("Pruned {deleted} rows from `memories` older than {days} days for '{user_id}'.");
    Ok(())
}

/// Emit JSON plan for rows that would be deleted per `contracts/db/retention-policy.yaml`.
pub async fn prune_plan(policy: Option<&Path>) -> Result<()> {
    let path = policy
        .map(PathBuf::from)
        .unwrap_or_else(db_retention::default_policy_path);
    let pol = db_retention::load_policy(&path)?;
    let db = vox_db::VoxDb::connect_default().await?;
    let mut rows_out = Vec::new();
    for (table, rule) in pol.tables.iter() {
        if rule.kind == "days" {
            let (Some(days), Some(col)) = (rule.days, rule.time_column.as_deref()) else {
                anyhow::bail!(
                    "retention policy: table `{table}` kind=days requires `days` and `time_column`"
                );
            };
            let n = db
                .retention_count_older_than_days(table, col, days)
                .await
                .with_context(|| format!("plan {table}"))?;
            rows_out.push(serde_json::json!({
                "table": table,
                "mode": "days",
                "days": days,
                "time_column": col,
                "would_delete": n,
            }));
            continue;
        }
        if rule.kind == "ms_days" {
            let (Some(days), Some(col)) = (rule.days, rule.time_column.as_deref()) else {
                anyhow::bail!(
                    "retention policy: table `{table}` kind=ms_days requires `days` and `time_column`"
                );
            };
            let cutoff = vox_db::VoxDb::retention_cutoff_ms_exclusive_for_days(days);
            let n = db
                .retention_count_older_than_ms_cutoff(table, col, cutoff)
                .await
                .with_context(|| format!("plan {table}"))?;
            rows_out.push(serde_json::json!({
                "table": table,
                "mode": "ms_days",
                "days": days,
                "time_column": col,
                "would_delete": n,
            }));
            continue;
        }
        if rule.kind == "expires_lt_now" {
            let Some(col) = rule.time_column.as_deref() else {
                anyhow::bail!(
                    "retention policy: table `{table}` kind=expires_lt_now requires `time_column`"
                );
            };
            let n = db
                .retention_count_expires_lt_now(table, col)
                .await
                .with_context(|| format!("plan {table}"))?;
            rows_out.push(serde_json::json!({
                "table": table,
                "mode": "expires_lt_now",
                "time_column": col,
                "would_delete": n,
            }));
            continue;
        }
        rows_out.push(serde_json::json!({
            "table": table,
            "mode": rule.kind,
            "would_delete": serde_json::Value::Null,
        }));
    }
    let out = serde_json::json!({
        "policy": path.display().to_string(),
        "tables": rows_out,
    });
    println!("{}", serde_json::to_string_pretty(&out)?);
    Ok(())
}

/// Execute `days` and `ms_days` rules from the retention policy (DELETE).
pub async fn prune_apply(policy: Option<&Path>, i_understand: bool) -> Result<()> {
    if !i_understand {
        anyhow::bail!("refusing prune-apply without `--i-understand` (destructive deletes)");
    }
    let path = policy
        .map(PathBuf::from)
        .unwrap_or_else(db_retention::default_policy_path);
    let pol = db_retention::load_policy(&path)?;
    let db = vox_db::VoxDb::connect_default().await?;
    let mut total: u64 = 0;
    for (table, rule) in pol.tables.iter() {
        let n = if rule.kind == "days" {
            let (Some(days), Some(col)) = (rule.days, rule.time_column.as_deref()) else {
                anyhow::bail!(
                    "retention policy: table `{table}` kind=days requires `days` and `time_column`"
                );
            };
            db.retention_delete_older_than_days(table, col, days)
                .await
                .with_context(|| format!("delete {table}"))?
        } else if rule.kind == "ms_days" {
            let (Some(days), Some(col)) = (rule.days, rule.time_column.as_deref()) else {
                anyhow::bail!(
                    "retention policy: table `{table}` kind=ms_days requires `days` and `time_column`"
                );
            };
            db.retention_delete_all_ms_older_than_days(table, col, days)
                .await
                .with_context(|| format!("delete {table}"))?
        } else if rule.kind == "expires_lt_now" {
            let Some(col) = rule.time_column.as_deref() else {
                anyhow::bail!(
                    "retention policy: table `{table}` kind=expires_lt_now requires `time_column`"
                );
            };
            db.retention_delete_expires_lt_now(table, col)
                .await
                .with_context(|| format!("delete {table}"))?
        } else {
            continue;
        };
        total += n;
    }
    // Also prune Tavily search documents with fixed 7-day TTL (Wave 1 operational hardening).
    let tavily_pruned = db
        .retention_prune_tavily_search_documents()
        .await
        .unwrap_or(0);

    println!(
        "prune-apply: deleted {total} rows total (policy {}), plus {tavily_pruned} stale Tavily search documents.",
        path.display()
    );
    Ok(())
}

/// Get a user preference by key.
pub async fn pref_get(user_id: &str, key: &str) -> Result<()> {
    let db = vox_db::VoxDb::connect_default().await?;
    match db.get_user_preference(user_id, key).await? {
        Some(val) => println!("{key} = {val}"),
        None => println!("(not set)"),
    }
    Ok(())
}

/// Set a user preference key/value.
pub async fn pref_set(user_id: &str, key: &str, value: &str) -> Result<()> {
    let db = vox_db::VoxDb::connect_default().await?;
    db.set_user_preference(user_id, key, value).await?;
    println!("Set '{key}' = '{value}' for user '{user_id}'.");
    Ok(())
}

/// List all preferences for a user.
pub async fn pref_list(user_id: &str, prefix: Option<&str>) -> Result<()> {
    let db = vox_db::VoxDb::connect_default().await?;
    let prefs = db.list_user_preferences(user_id, prefix).await?;
    let filtered: Vec<_> = prefs
        .iter()
        .filter(|(k, _)| prefix.map(|p| k.starts_with(p)).unwrap_or(true))
        .collect();
    if filtered.is_empty() {
        println!("(no preferences)");
    } else {
        for (k, v) in &filtered {
            println!("{k} = {v}");
        }
    }
    Ok(())
}

/// Query historical execution time for tools.
pub async fn exec_history(
    tool_key: Option<&str>,
    repo: Option<&str>,
    limit: i64,
    json: bool,
) -> Result<()> {
    let db = vox_db::VoxDb::connect_default().await?;
    let history = db.query_historical_exec_time(tool_key, repo, limit).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&history)?);
    } else {
        if history.is_empty() {
            println!("No execution history found.");
            return Ok(());
        }
        for item in history {
            println!(
                "[{}] tool='{}' repo='{}' result={} dur={}ms expected={:?}",
                item["recorded_at"],
                item["tool_key"].as_str().unwrap_or(""),
                item["repository_id"].as_str().unwrap_or(""),
                item["outcome"].as_str().unwrap_or(""),
                item["duration_ms"],
                item["timeout_budget_ms"],
            );
        }
    }
    Ok(())
}

/// List historical MENS training runs from `populi_training_run` table.
pub async fn mens_runs(limit: u32) -> Result<()> {
    let db = vox_db::VoxDb::connect_default().await?;
    let runs = db.list_training_runs(limit).await?;
    if runs.is_empty() {
        println!("No training runs found in populi_training_run.");
        return Ok(());
    }
    println!(
        "{:<24} {:<12} {:<10} {:<8} {:<8} {:<16}",
        "Run ID", "Status", "Step", "Loss", "Epoch", "Updated"
    );
    println!("{}", "-".repeat(84));
    for run in runs {
        let loss = run
            .last_loss
            .map(|f| format!("{:.4}", f))
            .unwrap_or_else(|| "N/A".to_string());
        let updated = chrono::DateTime::from_timestamp(run.updated_at, 0)
            .map(|t| t.format("%m-%d %H:%M").to_string())
            .unwrap_or_else(|| "N/A".to_string());

        println!(
            "{:<24} {:<12} {:<10} {:<8} {:<8} {}",
            run.run_id, run.status, run.global_step, loss, run.epoch, updated
        );
    }
    Ok(())
}

/// List MENS-specific telemetry (entropy, diversity, sample counts) from `research_metrics` table.
pub async fn mens_metrics(domain: Option<&str>, limit: u32) -> Result<()> {
    let db = vox_db::VoxDb::connect_default().await?;
    let session_prefix = if let Some(d) = domain {
        format!("mens:{}", d)
    } else {
        "mens:".to_string()
    };

    let rows = db
        .list_research_metrics_for_session_prefix(&session_prefix, limit as i64)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    println!(
        "{:<28} {:<24} {:<10} {:<20}",
        "Session", "Metric", "Value", "Time / Metadata"
    );
    println!("{}", "-".repeat(95));

    let mut count = 0;
    for (session, mtype, val, meta, created_at) in rows {
        count += 1;

        let val_str = val
            .map(|v| format!("{:.4}", v))
            .unwrap_or_else(|| "-".to_string());
        let time = format_research_metric_time(&created_at);

        println!(
            "{:<28} {:<24} {:<10} [{}] {}",
            session,
            mtype,
            val_str,
            time,
            meta.as_deref().unwrap_or("")
        );
    }

    if count == 0 {
        println!(
            "(No MENS metrics found matching prefix '{}')",
            session_prefix
        );
    }

    Ok(())
}

/// Show dynamic build health for the current repository.
pub async fn build_health(repo: Option<String>, json: bool) -> Result<()> {
    let db = vox_db::VoxDb::connect_default().await?;
    let repo_id = if let Some(r) = repo {
        r
    } else {
        let repo =
            vox_repository::discover_repository(&std::env::current_dir().unwrap_or_default());
        repo.as_ref()
            .map(|r| r.repository_id.clone())
            .unwrap_or_else(|_| "local".into())
    };

    let health = db
        .query_build_health(&repo_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("No build telemetry found for repository '{}'", repo_id))?;

    if json {
        println!("{}", serde_json::to_string_pretty(&health)?);
    } else {
        println!("Vox Build Health: {}", repo_id);
        println!("  Latest Run Time: {:.2}s", health.total_ms as f64 / 1000.0);
        println!(
            "  Crate Throughput: {} compiled, {} cached",
            health.compiled, health.cached
        );
        println!("  Warnings       : {}", health.warning_count);
        println!(
            "  Graph Changed  : {}",
            if health.dep_changed { "YES" } else { "no" }
        );

        if !health.slowest.is_empty() {
            println!("\nSlowest Crates:");
            for c in health.slowest {
                println!(
                    "  {:40} {:>6}ms  {}",
                    c.name,
                    c.elapsed_ms.unwrap_or(0),
                    c.hint.as_deref().unwrap_or("")
                );
            }
        }
    }
    Ok(())
}

/// Show build-time regressions for the current repository.
pub async fn build_regressions(
    repo: Option<String>,
    run_id: Option<i64>,
    threshold: f64,
    json: bool,
) -> Result<()> {
    let db = vox_db::VoxDb::connect_default().await?;
    let repo_id = if let Some(r) = repo {
        r
    } else {
        let repo =
            vox_repository::discover_repository(&std::env::current_dir().unwrap_or_default());
        repo.as_ref()
            .map(|r| r.repository_id.clone())
            .unwrap_or_else(|_| "local".into())
    };

    let target_run_id = if let Some(id) = run_id {
        id
    } else {
        db.query_latest_build_run_id(&repo_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("No build runs found for repository '{}'", repo_id))?
    };

    let regressions = db
        .query_build_regressions(&repo_id, target_run_id, threshold)
        .await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&regressions)?);
    } else {
        println!(
            "Vox Build Regressions (Run #{}): {}",
            target_run_id, repo_id
        );
        if regressions.is_empty() {
            println!("  ✅ No regressions detected (>1.5x avg).");
        } else {
            println!("  ❌ {} regressions detected:", regressions.len());
            for r in regressions {
                println!(
                    "  {:40} {:>6}ms (avg {:>6.0}ms) [{:>4.1}x]  {}",
                    r.name,
                    r.elapsed_ms,
                    r.avg_ms,
                    r.ratio,
                    r.hint.as_deref().unwrap_or("")
                );
            }
        }
    }
    Ok(())
}
