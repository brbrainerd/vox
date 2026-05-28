use clap::Subcommand;

pub mod eval;
pub mod infra;

#[derive(Subcommand)]
pub enum ResearchCmd {
    /// Start the SearXNG sidecar (requires Docker).
    Up,
    /// Stop the SearXNG sidecar.
    Down,
    /// Check the health of research backends (SearXNG, DDG, Tavily).
    Status,
    /// Run the orchestrator deep-research pipeline (`run_research`).
    Run {
        /// Topic / question tokens (join with spaces).
        #[arg(trailing_var_arg = true, required = true)]
        query: Vec<String>,
        /// Emit JSON [`vox_dei_shim::research::ResearchResult`] to stdout.
        #[arg(long, default_value_t = false)]
        json: bool,
        /// Retrieval scope — `both` (default), `web`, or `local`.
        #[arg(long)]
        scope: Option<String>,
        #[arg(long)]
        max_sources: Option<usize>,
        #[arg(long, default_value_t = false)]
        verify_claims: bool,
        /// Restrict hits to this registrable domain (no scheme), e.g. `example.com`.
        #[arg(long)]
        site_scope: Option<String>,
        /// Create an async research session and return its id without running inline.
        #[arg(long = "async", default_value_t = false)]
        async_run: bool,
    },
    /// Preview an editable research plan without executing retrieval.
    Preview {
        /// Topic / question tokens (join with spaces).
        #[arg(trailing_var_arg = true, required = true)]
        query: Vec<String>,
        /// Emit JSON instead of Markdown.
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    /// List recent persisted research sessions.
    History {
        /// Maximum sessions to show.
        #[arg(long, default_value_t = 20)]
        limit: u32,
    },
    /// Show one persisted research session.
    Show {
        /// Numeric `scientia_research_sessions.id`.
        session_id: i64,
    },
    /// Watch a persisted research session until it reaches a terminal state.
    Watch { session_id: i64 },
    /// Print the latest persisted result/status for a research session.
    Result {
        session_id: i64,
        /// Output format: markdown or json.
        #[arg(long, default_value = "markdown")]
        format: String,
        /// Optional export path for the report.
        #[arg(long)]
        output: Option<std::path::PathBuf>,
    },
    /// Run the research evaluation harness against golden queries.
    Eval {
        /// Optional path to a golden query JSONL file.
        #[arg(long)]
        queries: Option<std::path::PathBuf>,
        /// Output path for the evaluation report.
        #[arg(long)]
        output: Option<std::path::PathBuf>,
        /// Number of parallel queries to run.
        #[arg(long, default_value_t = 4)]
        concurrency: usize,
    },
}

pub async fn run(cmd: ResearchCmd) -> anyhow::Result<()> {
    match cmd {
        ResearchCmd::Up => infra::up().await,
        ResearchCmd::Down => infra::down().await,
        ResearchCmd::Status => infra::status().await,
        ResearchCmd::Run {
            query,
            json,
            scope,
            max_sources,
            verify_claims,
            site_scope,
            async_run,
        } => {
            let q = query.join(" ").trim().to_string();
            run_research_query(
                q,
                json,
                scope,
                max_sources,
                verify_claims,
                site_scope,
                async_run,
            )
            .await
        }
        ResearchCmd::History { limit } => research_history(limit).await,
        ResearchCmd::Show { session_id } => research_show(session_id).await,
        ResearchCmd::Watch { session_id } => research_watch(session_id).await,
        ResearchCmd::Preview { query, json } => research_preview(query.join(" "), json).await,
        ResearchCmd::Result {
            session_id,
            format,
            output,
        } => research_result(session_id, &format, output).await,
        ResearchCmd::Eval {
            queries,
            output,
            concurrency,
        } => eval::run_eval(queries, output, concurrency).await,
    }
}

async fn connect_research_db() -> anyhow::Result<vox_db::VoxDb> {
    let cfg = vox_db::DbConfig::resolve_canonical().map_err(anyhow::Error::msg)?;
    Ok(vox_db::VoxDb::connect(cfg).await?)
}

pub async fn research_history(limit: u32) -> anyhow::Result<()> {
    let db = connect_research_db().await?;
    let rows = db.list_recent_research_sessions(limit).await?;
    if rows.is_empty() {
        println!("No research sessions found.");
        return Ok(());
    }
    for row in rows {
        println!(
            "{}\t{}\t{}\t{}",
            row.id, row.status, row.started_at_ms, row.query_text
        );
    }
    Ok(())
}

pub async fn research_show(session_id: i64) -> anyhow::Result<()> {
    let db = connect_research_db().await?;
    let Some(row) = db.get_research_session(session_id).await? else {
        anyhow::bail!("research session {session_id} not found");
    };
    println!("session_id: {}", row.id);
    println!("session_key: {}", row.session_key);
    println!("status: {}", row.status);
    println!("started_at_ms: {}", row.started_at_ms);
    if let Some(finished) = row.finished_at_ms {
        println!("finished_at_ms: {finished}");
    }
    println!("query: {}", row.query_text);
    if let Some(artifact) = db.get_research_artifact(session_id).await? {
        println!("\n{}", artifact.report_markdown);
    } else {
        println!("\nNo durable research artifact found for this session.");
    }
    Ok(())
}

pub async fn research_preview(query: String, json: bool) -> anyhow::Result<()> {
    let preview = research_plan_preview(&query);
    if json {
        println!("{}", serde_json::to_string_pretty(&preview)?);
    } else {
        println!("# Research Plan Preview\n");
        println!("Query: {}\n", preview["query"].as_str().unwrap_or(""));
        println!("Editable: true\n");
        println!("Subqueries:");
        for subquery in preview["subqueries"].as_array().into_iter().flatten() {
            println!("- {}", subquery.as_str().unwrap_or(""));
        }
        println!("\nPolicy:");
        println!("- scope: {}", preview["scope"].as_str().unwrap_or("both"));
        println!(
            "- max_sources_per_subquery: {}",
            preview["max_sources_per_subquery"].as_u64().unwrap_or(10)
        );
    }
    Ok(())
}

pub async fn research_result(
    session_id: i64,
    format: &str,
    output: Option<std::path::PathBuf>,
) -> anyhow::Result<()> {
    let db = connect_research_db().await?;
    let Some(artifact) = db.get_research_artifact(session_id).await? else {
        anyhow::bail!("no durable research artifact found for session {session_id}");
    };
    let rendered = match format {
        "markdown" | "md" => artifact.report_markdown,
        "json" => artifact.artifact_json,
        other => anyhow::bail!("unsupported result format {other:?}: use markdown|json"),
    };
    if let Some(path) = output {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, rendered)?;
    } else {
        println!("{rendered}");
    }
    Ok(())
}

fn research_plan_preview(query: &str) -> serde_json::Value {
    let trimmed = query.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut subqueries = vec![trimmed.clone()];
    if trimmed.contains("compare") {
        subqueries.push(format!("{trimmed} comparison criteria"));
        subqueries.push(format!("{trimmed} independent source corroboration"));
    } else if trimmed.contains("trace") || trimmed.contains("lineage") {
        subqueries.push(format!("{trimmed} timeline primary sources"));
        subqueries.push(format!("{trimmed} current state"));
    } else {
        subqueries.push(format!("{trimmed} primary sources"));
        subqueries.push(format!("{trimmed} recent independent analysis"));
    }
    serde_json::json!({
        "schema_version": 1,
        "editable": true,
        "query": trimmed,
        "scope": "both",
        "max_sources_per_subquery": 10,
        "subqueries": subqueries,
        "progress_states": [
            "queued",
            "planning",
            "retrieving",
            "verifying_claims",
            "synthesizing",
            "auditing_citations",
            "persisting_artifact",
            "completed"
        ],
        "free_baseline": {
            "required_paid_services": false,
            "optional_tavily_when_user_configured": true
        }
    })
}

pub async fn research_watch(session_id: i64) -> anyhow::Result<()> {
    let db = connect_research_db().await?;
    loop {
        let Some(row) = db.get_research_session(session_id).await? else {
            anyhow::bail!("research session {session_id} not found");
        };
        println!("{}\t{}\t{}", row.id, row.status, row.query_text);
        if matches!(row.status.as_str(), "completed" | "failed" | "orphaned") {
            return Ok(());
        }
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    }
}

/// Catalog handler anchor for `research.run` (`contracts/operations/catalog.v1.yaml`).
pub async fn run_research_query(
    query: String,
    json: bool,
    scope: Option<String>,
    max_sources: Option<usize>,
    verify_claims: bool,
    site_scope: Option<String>,
    async_run: bool,
) -> anyhow::Result<()> {
    use std::sync::Arc;
    use vox_db::{DbConfig, VoxDb};
    use vox_dei_shim::research::{
        ResearchConfig, ResearchQuery, ResearchScope, run_research_with_context,
    };
    use vox_repository::discover_repository_or_fallback;
    use vox_search::SearchRuntimeContext;

    if query.is_empty() {
        anyhow::bail!("research run: query must not be empty");
    }

    let scope_label = scope.as_deref().unwrap_or("both").trim();
    let scope = match scope_label.to_ascii_lowercase().as_str() {
        "both" => ResearchScope::Both,
        "local" => ResearchScope::Local,
        "web" => ResearchScope::Web,
        other => anyhow::bail!("invalid scope {other:?}: use web|local|both"),
    };

    let rq = ResearchQuery {
        query,
        scope,
        max_sources: max_sources.unwrap_or(10).clamp(1, 50),
        persist_to_docs: false,
        verify_claims,
        site_scope,
    };

    if async_run {
        let db = connect_research_db().await?;
        let session_key = format!("research_cli_async:{}", uuid::Uuid::new_v4());
        let session_id = db.create_research_session(&session_key, &rq.query).await?;
        println!(
            "{}",
            serde_json::json!({
                "session_id": session_id,
                "task_id": format!("research-{session_id}"),
                "status": "queued"
            })
        );
        return Ok(());
    }

    let config = ResearchConfig::default();
    let cwd = std::env::current_dir()?;
    let repo_ctx = discover_repository_or_fallback(&cwd);
    let mem = vox_orchestrator::MemoryConfig::default();
    let db = match DbConfig::resolve_canonical() {
        Ok(cfg) => VoxDb::connect(cfg).await.ok().map(Arc::new),
        Err(_) => None,
    };
    let ctx = SearchRuntimeContext::new(
        repo_ctx.root,
        db.clone(),
        cwd.join(&mem.log_dir),
        cwd.join(&mem.memory_md_path),
    );
    let result = run_research_with_context(rq, Some(&ctx), db.as_deref(), &config).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!("{}", result.answer);
        if !result.sources.is_empty() {
            println!("\nSources:");
            for h in &result.sources {
                println!("- {} — {}", h.title, h.url);
            }
        }
        println!(
            "\n(routing_tier={:?}, sources={}, quality_score={})",
            result.research_metadata.routing_tier,
            result.sources.len(),
            result.research_metadata.quality_score
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn research_plan_preview_is_editable_and_free_baseline() {
        let preview = research_plan_preview("compare local RAG and Gemini Deep Research");

        assert_eq!(preview["editable"].as_bool(), Some(true));
        assert_eq!(
            preview["free_baseline"]["required_paid_services"].as_bool(),
            Some(false)
        );
        assert!(preview["subqueries"].as_array().expect("subqueries").len() >= 3);
    }
}
