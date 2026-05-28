use owo_colors::OwoColorize;
use std::path::PathBuf;

use uuid::Uuid;
use vox_db::{DbConfig, ResearchEvalRunRecord, ResearchEvalSampleRecord, VoxDb, now_unix_ms};
use vox_search::context::SearchRuntimeContext;
use vox_search::policy::SearchPolicy;

pub async fn run_eval(
    queries_path: Option<PathBuf>,
    output_path: Option<PathBuf>,
    _concurrency: usize,
) -> anyhow::Result<()> {
    println!(
        "{} Initializing Research Evaluation Harness...",
        "INIT".blue().bold()
    );

    // 1. Establish database connection
    let db = VoxDb::connect(DbConfig::resolve_canonical().map_err(anyhow::Error::msg)?).await?;
    let run_id = Uuid::new_v4().to_string();
    let start_at = now_unix_ms();

    // 2. Load Queries
    let queries = if let Some(path) = &queries_path {
        load_queries_file(path)?
    } else {
        default_golden_queries()
    };
    println!("{} Loaded {} queries", "INFO".blue(), queries.len());

    // 3. Execution Loop
    let mut results = Vec::new();
    let policy = SearchPolicy::default();
    let current_dir = std::env::current_dir()?;
    let ctx = SearchRuntimeContext::new(
        current_dir.clone(),
        Some(std::sync::Arc::new(db.clone())),
        current_dir.clone(),
        current_dir.join("memory.md"),
    );

    for query in &queries {
        println!("{} Evaluating: {}", "RUN".green(), query);
        let start = std::time::Instant::now();

        let plan = vox_db::heuristic_search_plan(query, false, None);
        let execution =
            vox_search::execution::execute_search_plan(&ctx, query, &plan, 5, &policy, None)
                .await
                .map_err(|e| anyhow::anyhow!(e))?;

        let duration = start.elapsed().as_millis() as i64;

        let model_answer = execution.web_lines.join("\n");
        let evidence_snippets = execution.web_lines.clone();

        // 3.1 Calculate Metrics
        let recall = Some(0.0); // No gold answer provided in default loop
        let groundedness =
            vox_search::evaluation::calculate_groundedness(&model_answer, &evidence_snippets);
        let citation_precision =
            citation_precision_from_answer(&model_answer, evidence_snippets.len());
        let abstained = answer_abstained(&model_answer, evidence_snippets.is_empty());
        let multi_hop_score =
            multi_hop_completion_score(query, &execution.backend_mix, &execution.web_lines);

        let sample = ResearchEvalSampleRecord {
            run_id: run_id.clone(),
            query: query.clone(),
            gold_answer: None,
            model_answer,
            recall_at_5: recall,
            groundedness: Some(groundedness),
            quality_score: Some(
                (execution.evidence_quality + citation_precision + multi_hop_score) / 3.0,
            ),
            latency_ms: Some(duration),
            evidence: serde_json::json!({
                "web_lines": execution.web_lines,
                "citation_precision": citation_precision,
                "abstained": abstained,
                "multi_hop_score": multi_hop_score,
                "evidence_quality": execution.evidence_quality,
                "citation_coverage": execution.citation_coverage,
                "recommended_next_action": execution.recommended_next_action,
            }),
            recorded_at_ms: now_unix_ms() as i64,
        };

        results.push(sample);
    }

    // 4. Summarize and Persist Run
    let avg_quality = results
        .iter()
        .map(|s| s.quality_score.unwrap_or(0.0))
        .sum::<f64>()
        / results.len() as f64;
    let avg_latency = results
        .iter()
        .map(|s| s.latency_ms.unwrap_or(0))
        .sum::<i64>() as f64
        / results.len() as f64;
    let avg_citation_precision = average_evidence_metric(&results, "citation_precision");
    let abstention_rate = average_bool_metric(&results, "abstained");
    let avg_multi_hop_score = average_evidence_metric(&results, "multi_hop_score");

    let run_record = ResearchEvalRunRecord {
        run_id,
        model_id: "localized-dispatcher-0.1".into(),
        config: serde_json::json!({ "policy_version": 1 }),
        metrics: serde_json::json!({
            "avg_quality": avg_quality,
            "avg_latency_ms": avg_latency,
            "total_samples": results.len(),
            "citation_precision": avg_citation_precision,
            "abstention_rate": abstention_rate,
            "multi_hop_completion": avg_multi_hop_score,
        }),
        latency_p50_ms: Some(avg_latency as i64),
        latency_p99_ms: None,
        tier_distribution: serde_json::json!({}),
        created_at_ms: start_at as i64,
    };

    db.record_research_eval_run(&run_record).await?;

    for sample in &results {
        db.record_research_eval_sample(sample).await?;
    }

    let report = serde_json::json!({
        "schema_version": 1,
        "run_id": run_record.run_id,
        "suite": if queries_path.is_some() { "custom" } else { "local" },
        "total_samples": results.len(),
        "metrics": {
            "citation_precision": avg_citation_precision,
            "citation_recall": results.iter().map(|s| s.recall_at_5.unwrap_or(0.0)).sum::<f64>() / results.len() as f64,
            "answer_factuality": avg_quality,
            "abstention_rate": abstention_rate,
            "multi_hop_completion": avg_multi_hop_score,
            "avg_latency_ms": avg_latency
        }
    });

    if let Some(path) = output_path {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, serde_json::to_string_pretty(&report)?)?;
    }

    print_styled_summary(&run_record, avg_quality);

    Ok(())
}

fn citation_precision_from_answer(answer: &str, evidence_count: usize) -> f64 {
    if evidence_count == 0 {
        return if answer_abstained(answer, true) {
            1.0
        } else {
            0.0
        };
    }
    let mut cited = 0usize;
    let mut supported = 0usize;
    for token in answer.split(|c: char| c.is_whitespace() || c == ',' || c == '.') {
        let Some(inner) = token.strip_prefix('[').and_then(|t| t.strip_suffix(']')) else {
            continue;
        };
        if let Ok(n) = inner.parse::<usize>() {
            cited += 1;
            if (1..=evidence_count).contains(&n) {
                supported += 1;
            }
        }
    }
    if cited == 0 {
        0.0
    } else {
        supported as f64 / cited as f64
    }
}

fn answer_abstained(answer: &str, no_evidence: bool) -> bool {
    let lower = answer.to_ascii_lowercase();
    no_evidence
        || lower.contains("insufficient evidence")
        || lower.contains("not enough evidence")
        || lower.contains("no external sources were found")
}

fn multi_hop_completion_score(
    query: &str,
    backend_mix: &[vox_db::SearchBackend],
    web_lines: &[String],
) -> f64 {
    let multi_hop_query = query.contains("compare")
        || query.contains("trace")
        || query.contains("then")
        || query.contains("FRAMES-style")
        || query.contains("BrowseComp-style");
    if !multi_hop_query {
        return 1.0;
    }
    let backend_score = (backend_mix.len() as f64 / 2.0).min(1.0);
    let source_score = (web_lines.len() as f64 / 3.0).min(1.0);
    ((backend_score + source_score) / 2.0).clamp(0.0, 1.0)
}

fn average_evidence_metric(samples: &[ResearchEvalSampleRecord], key: &str) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    samples
        .iter()
        .map(|sample| {
            sample
                .evidence
                .get(key)
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0)
        })
        .sum::<f64>()
        / samples.len() as f64
}

fn average_bool_metric(samples: &[ResearchEvalSampleRecord], key: &str) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    samples
        .iter()
        .filter(|sample| {
            sample
                .evidence
                .get(key)
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        })
        .count() as f64
        / samples.len() as f64
}

fn load_queries_file(path: &std::path::Path) -> anyhow::Result<Vec<String>> {
    let content = std::fs::read_to_string(path)?;
    let mut queries = Vec::new();
    for line in content.lines() {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
            if let Some(q) = json.get("query").and_then(|v| v.as_str()) {
                queries.push(q.to_string());
            }
        }
    }
    Ok(queries)
}

fn default_golden_queries() -> Vec<String> {
    vec![
        "What are the latest developments in Rust 2024 edition?".into(),
        "How do I configure SearXNG for private JSON output?".into(),
        "What is the current price of Ethereum in USD?".into(),
        "Vox Dei orchestrator architecture overview".into(),
        // Deep-research-style multi-hop prompts (evidence spread across sources)
        "Compare MLX vs CUDA for on-device LLM fine-tuning: hardware requirements, tooling, and community adoption in 2025.".into(),
        "Trace the lineage from ReAct agents to modern web-browsing research assistants; name key papers and vendor products.".into(),
        "Summarize EU AI Act transparency obligations for general-purpose AI models and cite primary regulator sources.".into(),
        "Where does the Vox research pipeline create Codex sessions?".into(),
        "Which Vox module converts local search execution rows into research hits?".into(),
        "How does Vox choose local Mens versus OpenRouter for research LLM calls?".into(),
        "What contract file declares MCP research tools?".into(),
        "Which CLI command shows persisted research sessions?".into(),
        "What DB table stores Scientia research sessions?".into(),
        "Which verifier verdicts are supported by Vox research?".into(),
        "What is the fallback behavior when claim extraction has no LLM?".into(),
        "Which search context fields are required for local retrieval?".into(),
        "How are research cache keys normalized?".into(),
        "FRAMES-style: identify the component that plans subqueries, then name the metric recorded after planning.".into(),
        "FRAMES-style: find the local retrieval bridge and describe how repo hits are cited.".into(),
        "FRAMES-style: compare synchronous research run with async session status reporting.".into(),
        "BrowseComp-style: find primary documentation for OpenRouter chat completions and summarize endpoint shape.".into(),
        "BrowseComp-style: find current Ollama OpenAI compatibility notes and cite the local endpoint.".into(),
        "BrowseComp-style: find Tavily search API result fields relevant to citations.".into(),
        "BrowseComp-style: find SearXNG JSON output configuration guidance.".into(),
        "BrowseComp-style: find CRAG prior art and identify its correction trigger.".into(),
        "BrowseComp-style: find citation precision evaluation approaches for web QA.".into(),
        "BrowseComp-style: find Gemini Deep Research public product behavior and compare async expectations.".into(),
        "BrowseComp-style: find OpenClaw research assistant claims and cite product docs.".into(),
        "BrowseComp-style: find MiniCheck claim verification model details.".into(),
        "BrowseComp-style: find CoVE self-verification paper and summarize its loop.".into(),
    ]
}

fn print_styled_summary(run_record: &ResearchEvalRunRecord, avg_quality: f64) {
    println!(
        "\n{}",
        " RESEARCH EVALUATION COMPLETE ".on_blue().white().bold()
    );
    println!(
        "{:<15} {}",
        "Run ID:".dimmed(),
        run_record.run_id.bright_white()
    );
    println!(
        "{:<15} {}",
        "Model:".dimmed(),
        run_record.model_id.bright_white()
    );

    let quality_line = format!("{:.2}", avg_quality);
    let styled = if avg_quality > 0.8 {
        quality_line.green().bold().to_string()
    } else if avg_quality > 0.5 {
        quality_line.yellow().bold().to_string()
    } else {
        quality_line.red().bold().to_string()
    };
    println!("{:<15} {}", "Avg Quality:".dimmed(), styled);

    if let Some(lat) = run_record.latency_p50_ms {
        println!(
            "{:<15} {}ms",
            "P50 Latency:".dimmed(),
            lat.to_string().bright_cyan()
        );
    }

    println!("{}", "─".repeat(40).dimmed());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn citation_precision_counts_only_existing_sources() {
        let precision = citation_precision_from_answer("Claim [1] and unsupported [4].", 2);
        assert_eq!(precision, 0.5);
    }

    #[test]
    fn abstention_counts_empty_evidence_as_valid_abstention() {
        assert!(answer_abstained("No external sources were found.", true));
        assert!(!answer_abstained("The answer is well-supported.", false));
    }

    #[test]
    fn multi_hop_queries_require_backend_and_source_coverage() {
        let score = multi_hop_completion_score(
            "FRAMES-style: find one thing then compare another",
            &[
                vox_db::SearchBackend::MemoryBm25,
                vox_db::SearchBackend::Web,
            ],
            &[
                "source 1".to_string(),
                "source 2".to_string(),
                "source 3".to_string(),
            ],
        );

        assert_eq!(score, 1.0);
    }
}
