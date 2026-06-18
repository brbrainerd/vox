//! Reliability and research-metric listing (`vox db reliability-*`, `research-metrics`).
//!
//! `research_metrics` metadata can be **S1–S2** (MCP sessions, paths in JSON). Treat stdout as operator-local.

use comfy_table::Table;
use comfy_table::presets::UTF8_FULL;

use super::helpers::summarize_text;

fn format_table(headers: &[&str], rows: &[Vec<String>]) -> Option<String> {
    if rows.is_empty() {
        return None;
    }
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(headers.iter().map(|h| (*h).to_string()).collect::<Vec<_>>());
    for row in rows {
        table.add_row(row.clone());
    }
    Some(table.to_string())
}

fn print_table(headers: &[&str], rows: Vec<Vec<String>>) {
    match format_table(headers, &rows) {
        Some(table) => println!("{table}"),
        None => println!("(no records found)"),
    }
}

/// Show telemetry metrics for `research_metrics` rows matching a session id prefix.
pub async fn research_metrics(session_id: &str, metric_type: Option<&str>) -> anyhow::Result<()> {
    let sid = session_id.trim();
    if sid.is_empty() {
        anyhow::bail!(
            "--session-id must be non-empty (examples: `mcp:<repository_id>`, `bench:<repo>`, `sess-key-1`)"
        );
    }
    let db = vox_db::VoxDb::connect_default().await?;
    let metrics = db
        .list_research_metrics_by_session(sid, metric_type, 500)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    if metrics.is_empty() {
        println!("(no research_metrics rows for session prefix {sid:?})");
    } else {
        eprintln!(
            "Note: metadata_json may include workspace-adjacent fields (S2). Do not paste into public channels without review — see docs/src/architecture/telemetry-trust-ssot.md"
        );
        println!("research_metrics (session_id LIKE {sid:?}…)");
        for (sess, mtype, value, meta) in metrics {
            print!(
                "  - [{}] {}  value={}",
                mtype,
                sess,
                value.map_or_else(|| "null".to_string(), |v| v.to_string())
            );
            if let Some(m) = meta {
                print!("  metadata: {m}");
            }
            println!();
        }
    }
    Ok(())
}

/// List reliability scores for LLM endpoints, skills, workflows, or repositories.
pub async fn reliability_list(domain: &str, limit: i64) -> anyhow::Result<()> {
    let db = vox_db::VoxDb::connect_default().await?;

    println!("Reliability stats for: {}", domain);

    match domain {
        "endpoints" => {
            let entries = db
                .list_endpoint_reliability(limit)
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            let rows: Vec<Vec<String>> = entries
                .into_iter()
                .map(|e| {
                    vec![
                        summarize_text(&e.endpoint_url, 40),
                        summarize_text(&e.model_id, 40),
                        e.total_requests.to_string(),
                        format!("{:.4}", e.hallucination_proxy_ewma),
                        format!("{:.4}", e.contradiction_ratio_ewma),
                        format!("{:.4}", e.infra_failure_ewma),
                    ]
                })
                .collect();
            print_table(
                &[
                    "Endpoint",
                    "Model",
                    "Reqs",
                    "Hallucina",
                    "Contradic",
                    "InfraFail",
                ],
                rows,
            );
        }
        "skills" => {
            let rows_data = db
                .list_skill_reliability_worst_first(limit)
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            let rows: Vec<Vec<String>> = rows_data
                .into_iter()
                .map(|(id, rel, succ, fail)| {
                    vec![
                        summarize_text(&id, 40),
                        format!("{:.4}", rel),
                        succ.to_string(),
                        fail.to_string(),
                    ]
                })
                .collect();
            print_table(&["Skill ID", "Reliability", "Success", "Failure"], rows);
        }
        "workflows" => {
            let rows_data = db
                .list_workflow_reliability_worst_first(limit)
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            let rows: Vec<Vec<String>> = rows_data
                .into_iter()
                .map(|(id, rel, succ, fail)| {
                    vec![
                        summarize_text(&id, 40),
                        format!("{:.4}", rel),
                        succ.to_string(),
                        fail.to_string(),
                    ]
                })
                .collect();
            print_table(&["Workflow", "Reliability", "Success", "Failure"], rows);
        }
        "repositories" => {
            let rows_data = db
                .list_repository_reliability_worst_first(limit)
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            let rows: Vec<Vec<String>> = rows_data
                .into_iter()
                .map(|(id, rel, succ, fail)| {
                    vec![
                        summarize_text(&id, 40),
                        format!("{:.4}", rel),
                        succ.to_string(),
                        fail.to_string(),
                    ]
                })
                .collect();
            print_table(
                &["Repository ID", "Reliability", "Success", "Failure"],
                rows,
            );
        }
        "trust" | "trust-rollups" => {
            let rows_data = db
                .list_trust_rollups(None, None, None, None, limit)
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            let rows: Vec<Vec<String>> = rows_data
                .into_iter()
                .map(|row| {
                    vec![
                        summarize_text(&row.entity_type, 32),
                        summarize_text(&row.entity_id, 32),
                        summarize_text(&row.dimension, 32),
                        summarize_text(&row.domain, 32),
                        format!("{:.4}", row.score),
                        row.sample_size.to_string(),
                        summarize_text(&row.model_id, 32),
                    ]
                })
                .collect();
            print_table(
                &[
                    "EntityType",
                    "EntityID",
                    "Dimension",
                    "Domain",
                    "Score",
                    "Samples",
                    "Model",
                ],
                rows,
            );
        }
        _ => anyhow::bail!(
            "Unknown reliability domain '{}'. Use endpoints, skills, workflows, repositories, or trust.",
            domain
        ),
    }
    Ok(())
}

/// List reliability scores for execution agents.
pub async fn reliability_agents(limit: i64, min_score: Option<f64>) -> anyhow::Result<()> {
    let db = vox_db::VoxDb::connect_default().await?;

    let min = min_score.unwrap_or(0.0);
    let rows_data = db
        .list_agent_reliability_above(min, limit)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    println!("Agent Reliability (min score: {:.2})", min);
    let rows: Vec<Vec<String>> = rows_data
        .into_iter()
        .map(|(aid, rel, succ, fail)| {
            vec![
                summarize_text(&aid, 48),
                format!("{:.4}", rel),
                succ.to_string(),
                fail.to_string(),
            ]
        })
        .collect();
    print_table(&["Agent ID", "Reliability", "Success", "Failure"], rows);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_table_renders_headers_and_rows() {
        let rows = vec![vec![
            "my-skill".to_string(),
            "0.9500".to_string(),
            "19".to_string(),
            "1".to_string(),
        ]];
        let rendered = format_table(&["Skill ID", "Reliability", "Success", "Failure"], &rows)
            .expect("non-empty table");
        assert!(rendered.contains("Skill ID"));
        assert!(rendered.contains("my-skill"));
        assert!(rendered.contains("0.9500"));
    }

    #[test]
    fn format_table_empty_rows_returns_none() {
        assert!(format_table(&["Agent ID"], &[]).is_none());
    }
}
