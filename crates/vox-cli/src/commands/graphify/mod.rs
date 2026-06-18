//! `vox graphify` — corpus registry and freshness status (Tier D maps).

use anyhow::Context;
use chrono::Utc;
use clap::Subcommand;
use vox_config::graphify::{
    CorpusStatus, GraphifyCorporaRegistry, GraphifyCorpus, GraphifyError, GraphifyKnowledgeNode,
    assess_corpus_status, load_graphify_corpora, project_graph_nodes_for_ingest,
};

#[derive(Debug, Subcommand)]
pub enum GraphifyCmd {
    /// Report graphify corpus freshness (read-only).
    Status {
        /// Corpus id (default: all corpora in registry).
        #[arg(long)]
        corpus: Option<String>,
        /// Exit non-zero when any reported corpus is stale.
        #[arg(long)]
        strict: bool,
        /// Emit JSON instead of human-readable lines.
        #[arg(long)]
        json: bool,
    },
    /// Project graph nodes into Turso `knowledge_nodes` via VoxDb.
    Ingest {
        /// Corpus id (default: registry `default_corpus_id`).
        #[arg(long)]
        corpus: Option<String>,
        /// Dry-run: print counts only, no DB writes.
        #[arg(long)]
        dry_run: bool,
    },
    /// Rebuild the base AST code graph and cluster it.
    Rebuild {
        /// Corpus id (default: registry `default_corpus_id`).
        #[arg(long)]
        corpus: Option<String>,
    },
}

fn resolve_head_sha() -> anyhow::Result<Option<String>> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .context("git rev-parse HEAD")?;
    if !output.status.success() {
        return Ok(None);
    }
    let sha = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if sha.is_empty() {
        Ok(None)
    } else {
        Ok(Some(sha))
    }
}

fn corpus_by_id<'a>(
    reg: &'a GraphifyCorporaRegistry,
    id: &str,
) -> Result<&'a GraphifyCorpus, GraphifyError> {
    reg.corpora
        .iter()
        .find(|c| c.id == id)
        .ok_or_else(|| GraphifyError::UnknownCorpus(id.to_string()))
}

fn selected_corpora<'a>(
    reg: &'a GraphifyCorporaRegistry,
    corpus: &Option<String>,
) -> Result<Vec<&'a GraphifyCorpus>, GraphifyError> {
    match corpus {
        Some(id) => Ok(vec![corpus_by_id(reg, id)?]),
        None => Ok(reg.corpora.iter().collect()),
    }
}

fn assess_all(
    repo_root: &std::path::Path,
    reg: &GraphifyCorporaRegistry,
    corpus: &Option<String>,
    head_sha: Option<&str>,
) -> Result<Vec<CorpusStatus>, GraphifyError> {
    let now = Utc::now();
    let ttl = vox_config::graphify::resolve_ttl_days(reg.ttl_days_default);
    selected_corpora(reg, corpus)?
        .into_iter()
        .map(|c| Ok(assess_corpus_status(repo_root, c, head_sha, now, ttl)))
        .collect()
}

fn resolve_ingest_corpus_id(
    reg: &GraphifyCorporaRegistry,
    corpus: Option<String>,
) -> Result<String, GraphifyError> {
    match corpus {
        Some(id) => {
            corpus_by_id(reg, &id)?;
            Ok(id)
        }
        None => Ok(reg.default_corpus_id.clone()),
    }
}

fn load_projected_nodes(
    repo_root: &std::path::Path,
    reg: &GraphifyCorporaRegistry,
    corpus_id: &str,
) -> anyhow::Result<Vec<GraphifyKnowledgeNode>> {
    let corpus = corpus_by_id(reg, corpus_id).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let graph_path = repo_root.join(&corpus.graph_path);
    let raw = std::fs::read_to_string(&graph_path)
        .with_context(|| format!("read graph {}", graph_path.display()))?;
    let graph: serde_json::Value = serde_json::from_str(&raw)
        .with_context(|| format!("parse graph JSON {}", graph_path.display()))?;
    Ok(project_graph_nodes_for_ingest(&graph, corpus_id))
}

async fn upsert_projected_nodes(nodes: &[GraphifyKnowledgeNode]) -> anyhow::Result<usize> {
    let db = vox_db::VoxDb::connect_default()
        .await
        .context("connect to VoxDb")?;
    let mut upserted = 0usize;
    for node in nodes {
        db.upsert_knowledge_node(
            &node.id,
            &node.label,
            &node.content,
            Some(node.node_type.as_str()),
            Some(node.metadata.as_str()),
            None,
        )
        .await
        .with_context(|| format!("upsert knowledge node {}", node.id))?;
        upserted += 1;
    }
    Ok(upserted)
}

/// Load registry, read corpus graph JSON, and project nodes for ingest (no DB I/O).
#[allow(dead_code)]
pub(crate) fn ingest_graph_corpus(
    repo_root: &std::path::Path,
    corpus_id: &str,
) -> anyhow::Result<Vec<GraphifyKnowledgeNode>> {
    let reg = load_graphify_corpora(repo_root).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    load_projected_nodes(repo_root, &reg, corpus_id)
}

fn render_status_line(s: &CorpusStatus) -> String {
    let fresh = if s.is_fresh { "fresh" } else { "stale" };
    let nodes = s
        .node_count
        .map(|n| n.to_string())
        .unwrap_or_else(|| "-".into());
    let edges = s
        .edge_count
        .map(|n| n.to_string())
        .unwrap_or_else(|| "-".into());
    format!(
        "{:<20} {fresh:<6} nodes={nodes:<6} edges={edges:<6} graph={}",
        s.corpus_id,
        s.graph_path.display()
    )
}

/// Entry point for `vox graphify <cmd>`.
pub fn run(cmd: GraphifyCmd, repo_root: &std::path::Path) -> anyhow::Result<()> {
    match cmd {
        GraphifyCmd::Status {
            corpus,
            strict,
            json,
        } => {
            let reg =
                load_graphify_corpora(repo_root).map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let head = resolve_head_sha()?;
            let statuses = assess_all(repo_root, &reg, &corpus, head.as_deref())
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;

            if json {
                println!("{}", serde_json::to_string_pretty(&statuses)?);
            } else {
                if let Some(ref h) = head {
                    println!("# head {h}");
                }
                for s in &statuses {
                    println!("{}", render_status_line(s));
                    if !s.stale_reasons.is_empty() {
                        println!("  stale: {}", s.stale_reasons.join(", "));
                    }
                    if !s.warnings.is_empty() {
                        println!("  warn:  {}", s.warnings.join(", "));
                    }
                }
            }

            if strict && statuses.iter().any(|s| !s.is_fresh) {
                anyhow::bail!("one or more graphify corpora are stale");
            }
        }
        GraphifyCmd::Ingest { corpus, dry_run } => {
            let reg =
                load_graphify_corpora(repo_root).map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let corpus_id = resolve_ingest_corpus_id(&reg, corpus)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let nodes = load_projected_nodes(repo_root, &reg, &corpus_id)?;

            if dry_run {
                println!("dry-run: corpus={corpus_id} nodes={}", nodes.len());
                return Ok(());
            }

            let upserted = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .context("tokio runtime for graphify ingest")?
                .block_on(upsert_projected_nodes(&nodes))?;
            println!("graphify ingest: corpus={corpus_id} upserted={upserted}");
        }
        GraphifyCmd::Rebuild { corpus } => {
            let reg =
                load_graphify_corpora(repo_root).map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let corpus_id = resolve_ingest_corpus_id(&reg, corpus)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let corpus =
                corpus_by_id(&reg, &corpus_id).map_err(|e| anyhow::anyhow!(e.to_string()))?;

            let source_dir = repo_root.join(&corpus.scope_path);
            let output_file = repo_root.join(&corpus.graph_path);
            let cache_dir = output_file.parent().unwrap().join("file_cache");

            println!("Rebuilding Graphify graph for corpus: {}...", corpus_id);
            vox_graphify_reader::rebuild::rebuild_graph(
                repo_root,
                &source_dir,
                &output_file,
                &cache_dir,
            )
            .map_err(|e| anyhow::anyhow!("Rebuild failed: {}", e))?;
            println!("Graphify rebuild successful!");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    fn write_registry(repo: &Path) {
        let dir = repo.join("contracts/retrieval");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("graphify-corpora.v1.yaml"),
            include_str!("../../../../../contracts/retrieval/graphify-corpora.v1.yaml"),
        )
        .unwrap();
    }

    #[test]
    fn status_strict_fails_when_graph_missing() {
        let tmp = tempfile::tempdir().unwrap();
        write_registry(tmp.path());
        let reg = load_graphify_corpora(tmp.path()).unwrap();
        let statuses = assess_all(tmp.path(), &reg, &None, Some("abc")).unwrap();
        assert!(statuses.iter().any(|s| !s.is_fresh));
        assert!(
            statuses
                .iter()
                .all(|s| s.stale_reasons.contains(&"graph_missing".to_string()) || !s.graph_exists)
        );
    }

    #[test]
    fn corpus_filter_unknown_id_errors() {
        let tmp = tempfile::tempdir().unwrap();
        write_registry(tmp.path());
        let reg = load_graphify_corpora(tmp.path()).unwrap();
        let err = selected_corpora(&reg, &Some("nope".into())).unwrap_err();
        assert!(matches!(err, GraphifyError::UnknownCorpus(_)));
    }

    #[test]
    fn ingest_corpus_resolves_cache_dir_path() {
        let tmp = tempfile::tempdir().unwrap();
        write_registry(tmp.path());
        // After path migration, repo-code-graph lives at .vox/cache/graphify/repo-code-graph/
        let graph_dir = tmp.path().join(".vox/cache/graphify/repo-code-graph");
        fs::create_dir_all(&graph_dir).unwrap();
        fs::write(
            graph_dir.join("graph.json"),
            r#"{"nodes":[{"id":"n1","label":"some module","type":"module"}]}"#,
        )
        .unwrap();
        let nodes = ingest_graph_corpus(tmp.path(), "repo-code-graph").unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].id, "graphify:repo-code-graph:node:n1");
    }

    #[test]
    fn ingest_graph_corpus_projects_minimal_graph_nodes() {
        let tmp = tempfile::tempdir().unwrap();
        write_registry(tmp.path());
        let graph_dir = tmp.path().join(".vox/cache/graphify/repo-code-graph");
        fs::create_dir_all(&graph_dir).unwrap();
        fs::write(
            graph_dir.join("graph.json"),
            r#"{"nodes":[{"id":"auth","label":"authentication module","type":"module"}]}"#,
        )
        .unwrap();

        let nodes = ingest_graph_corpus(tmp.path(), "repo-code-graph").unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].id, "graphify:repo-code-graph:node:auth");
        assert_eq!(nodes[0].label, "authentication module");
        assert_eq!(nodes[0].node_type, "module");
        assert!(nodes[0].metadata.contains("repo-code-graph"));
        assert!(nodes[0].metadata.contains("graphify_lexical_ingest"));
    }
}
