//! `vox graphify` — corpus registry and freshness status (Tier D maps).

use anyhow::Context;
use chrono::Utc;
use clap::Subcommand;
use vox_config::graphify::{
    CorpusStatus, GraphifyCorporaRegistry, GraphifyCorpus, GraphifyError, GraphifyKnowledgeNode,
    assess_corpus_status, load_all_corpora, load_graphify_corpora, project_graph_nodes_for_ingest,
    upsert_registered_corpus,
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
    /// Register an external target repository as a corpus and build it.
    Index {
        /// Path to the target repository (or subdirectory) to index.
        path: String,
        /// Corpus id (default: sanitized final path component).
        #[arg(long)]
        id: Option<String>,
        /// Extraction mode / semantic lens ("structural", "modules").
        #[arg(long, default_value = "structural")]
        mode: String,
    },
    /// Assess all corpora and (with --auto) rebuild/ingest each stale one per policy.
    Refresh {
        /// Corpus id (default: all corpora).
        #[arg(long)]
        corpus: Option<String>,
        /// Execute the chosen action; without it, only print what would happen.
        #[arg(long)]
        auto: bool,
    },
}

/// What an autonomous refresh should do for a corpus, given its stale reasons.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) enum RefreshAction {
    Rebuild,
    Ingest,
    Skip,
}

/// Deterministic cost/value gate: a structural change (missing/corrupt/drift/ttl) needs a
/// native rebuild; a lexical-only lag needs a cheap re-ingest; otherwise do nothing.
pub(crate) fn refresh_action(stale_reasons: &[String]) -> RefreshAction {
    let has = |r: &str| stale_reasons.iter().any(|s| s == r);
    if has("graph_missing") || has("graph_corrupt") || has("git_drift") || has("ttl_expired") {
        RefreshAction::Rebuild
    } else if has("lexical_lag") {
        RefreshAction::Ingest
    } else {
        RefreshAction::Skip
    }
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

pub(crate) fn resolve_source_dir(
    repo_root: &std::path::Path,
    corpus: &GraphifyCorpus,
) -> std::path::PathBuf {
    corpus
        .source_root
        .as_ref()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| repo_root.to_path_buf())
        .join(&corpus.scope_path)
}

/// `git -C <dir> rev-parse HEAD`, or Ok(None) if not a git repo.
fn resolve_head_sha_in(dir: &std::path::Path) -> anyhow::Result<Option<String>> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
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
    vox_head: Option<&str>,
) -> Result<Vec<CorpusStatus>, GraphifyError> {
    let now = Utc::now();
    let ttl = vox_config::graphify::resolve_ttl_days(reg.ttl_days_default);
    selected_corpora(reg, corpus)?
        .into_iter()
        .map(|c| {
            let head = match &c.source_root {
                Some(root) => resolve_head_sha_in(std::path::Path::new(root)).unwrap_or(None),
                None => vox_head.map(str::to_string),
            };
            Ok(assess_corpus_status(
                repo_root,
                c,
                head.as_deref(),
                now,
                ttl,
            ))
        })
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
pub async fn run(cmd: GraphifyCmd, repo_root: &std::path::Path) -> anyhow::Result<()> {
    match cmd {
        GraphifyCmd::Status {
            corpus,
            strict,
            json,
        } => {
            let reg = load_all_corpora(repo_root).map_err(|e| anyhow::anyhow!(e.to_string()))?;
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
            let reg = load_all_corpora(repo_root).map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let corpus_id = resolve_ingest_corpus_id(&reg, corpus)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let nodes = load_projected_nodes(repo_root, &reg, &corpus_id)?;

            if dry_run {
                println!("dry-run: corpus={corpus_id} nodes={}", nodes.len());
                return Ok(());
            }

            let upserted = upsert_projected_nodes(&nodes).await?;
            println!("graphify ingest: corpus={corpus_id} upserted={upserted}");

            // Stamp lexical_ingest_sha256 = digest of the graph just projected, so lexical_lag
            // clears now and re-fires after a later rebuild changes graph_json_sha256.
            let corpus =
                corpus_by_id(&reg, &corpus_id).map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let graph_bytes = std::fs::read(repo_root.join(&corpus.graph_path))
                .with_context(|| format!("read graph for digest: {}", corpus.graph_path))?;
            let digest = vox_graphify_reader::graph_digest(&graph_bytes);
            vox_config::graphify::set_lexical_ingest_sha256(
                &repo_root.join(&corpus.manifest_path),
                &digest,
            )
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        }
        GraphifyCmd::Rebuild { corpus } => {
            let reg = load_all_corpora(repo_root).map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let corpus_id = resolve_ingest_corpus_id(&reg, corpus)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let corpus =
                corpus_by_id(&reg, &corpus_id).map_err(|e| anyhow::anyhow!(e.to_string()))?;

            let source_dir = resolve_source_dir(repo_root, corpus);
            let output_file = repo_root.join(&corpus.graph_path);
            let cache_dir = output_file.parent().unwrap().join("file_cache");

            println!("Rebuilding Graphify graph for corpus: {}...", corpus_id);
            let meta = vox_graphify_reader::rebuild::RebuildMeta {
                corpus_id: corpus_id.clone(),
                git_sha: resolve_head_sha()?,
                scope_path: corpus.scope_path.clone(),
                extraction_mode: corpus.extraction_mode.clone(),
                built_at_rfc3339: Utc::now().to_rfc3339(),
            };
            vox_graphify_reader::rebuild::rebuild_graph(
                repo_root,
                &source_dir,
                &output_file,
                &cache_dir,
                &meta,
            )
            .map_err(|e| anyhow::anyhow!("Rebuild failed: {}", e))?;
            println!("Graphify rebuild successful!");
        }
        GraphifyCmd::Index { path, id, mode } => {
            let abs = std::fs::canonicalize(&path)
                .with_context(|| format!("canonicalize target path {path}"))?;
            // NOTE (Windows): canonicalize yields a verbatim `\\?\` prefix; it round-trips
            // through PathBuf/join/git -C fine. Do not strip it manually.
            let corpus_id = id
                .unwrap_or_else(|| {
                    abs.file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| "target".to_string())
                })
                .chars()
                .map(|c| {
                    if c.is_alphanumeric() || c == '-' || c == '_' {
                        c
                    } else {
                        '-'
                    }
                })
                .collect::<String>();
            let corpus = GraphifyCorpus {
                id: corpus_id.clone(),
                title: format!("Indexed target: {}", abs.display()),
                scope_path: ".".to_string(),
                graph_path: format!(".vox/cache/graphify/{corpus_id}/graph.json"),
                manifest_path: format!(
                    ".vox/cache/graphify/{corpus_id}/.graphify_manifest.v1.json"
                ),
                extraction_mode: Some(mode),
                default_for_intents: vec![],
                is_virtual: false,
                source_root: Some(abs.to_string_lossy().to_string()),
            };
            upsert_registered_corpus(repo_root, &corpus)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let source_dir = resolve_source_dir(repo_root, &corpus);
            let output_file = repo_root.join(&corpus.graph_path);
            let cache_dir = output_file
                .parent()
                .ok_or_else(|| anyhow::anyhow!("graph_path has no parent"))?
                .join("file_cache");
            let meta = vox_graphify_reader::rebuild::RebuildMeta {
                corpus_id: corpus_id.clone(),
                git_sha: resolve_head_sha_in(&abs).ok().flatten(),
                scope_path: corpus.scope_path.clone(),
                extraction_mode: corpus.extraction_mode.clone(),
                built_at_rfc3339: Utc::now().to_rfc3339(),
            };
            println!("Indexing '{}' as corpus '{}'...", abs.display(), corpus_id);
            vox_graphify_reader::rebuild::rebuild_graph(
                repo_root,
                &source_dir,
                &output_file,
                &cache_dir,
                &meta,
            )
            .map_err(|e| anyhow::anyhow!("Index rebuild failed: {}", e))?;
            println!("Corpus '{corpus_id}' registered and built.");
        }
        GraphifyCmd::Refresh { corpus, auto } => {
            let reg = load_all_corpora(repo_root).map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let head = resolve_head_sha()?;
            let statuses = assess_all(repo_root, &reg, &corpus, head.as_deref())
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            for s in &statuses {
                if s.is_fresh {
                    println!("fresh   {}", s.corpus_id);
                    continue;
                }
                let action = refresh_action(&s.stale_reasons);
                println!(
                    "{:?}  {} (stale: {})",
                    action,
                    s.corpus_id,
                    s.stale_reasons.join(",")
                );
                if !auto {
                    continue;
                }
                let c =
                    corpus_by_id(&reg, &s.corpus_id).map_err(|e| anyhow::anyhow!(e.to_string()))?;
                match action {
                    RefreshAction::Rebuild => {
                        let source_dir = resolve_source_dir(repo_root, c);
                        let output_file = repo_root.join(&c.graph_path);
                        let cache_dir = output_file
                            .parent()
                            .ok_or_else(|| anyhow::anyhow!("graph_path has no parent"))?
                            .join("file_cache");
                        let meta = vox_graphify_reader::rebuild::RebuildMeta {
                            corpus_id: c.id.clone(),
                            git_sha: head.clone(),
                            scope_path: c.scope_path.clone(),
                            extraction_mode: c.extraction_mode.clone(),
                            built_at_rfc3339: Utc::now().to_rfc3339(),
                        };
                        vox_graphify_reader::rebuild::rebuild_graph(
                            repo_root,
                            &source_dir,
                            &output_file,
                            &cache_dir,
                            &meta,
                        )
                        .map_err(|e| anyhow::anyhow!("refresh rebuild {}: {e}", c.id))?;
                        println!("  rebuilt {}", c.id);
                    }
                    RefreshAction::Ingest => {
                        let nodes = load_projected_nodes(repo_root, &reg, &c.id)?;
                        let upserted = upsert_projected_nodes(&nodes).await?;
                        let graph_bytes = std::fs::read(repo_root.join(&c.graph_path))
                            .with_context(|| format!("read graph for digest: {}", c.graph_path))?;
                        vox_config::graphify::set_lexical_ingest_sha256(
                            &repo_root.join(&c.manifest_path),
                            &vox_graphify_reader::graph_digest(&graph_bytes),
                        )
                        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
                        println!("  ingested {} ({} nodes)", c.id, upserted);
                    }
                    RefreshAction::Skip => {}
                }
            }
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

    #[test]
    fn source_root_overrides_repo_root_for_source_dir() {
        use vox_config::graphify::GraphifyCorpus;
        let repo = std::path::Path::new("/repo");
        let ext = GraphifyCorpus {
            id: "ext".into(),
            title: "ext".into(),
            scope_path: "src".into(),
            graph_path: ".vox/cache/graphify/ext/graph.json".into(),
            manifest_path: ".vox/cache/graphify/ext/.graphify_manifest.v1.json".into(),
            extraction_mode: Some("structural".into()),
            default_for_intents: vec![],
            is_virtual: false,
            source_root: Some("/other/target".into()),
        };
        assert_eq!(
            resolve_source_dir(repo, &ext),
            std::path::Path::new("/other/target").join("src")
        );
        let local = GraphifyCorpus {
            source_root: None,
            ..ext
        };
        assert_eq!(resolve_source_dir(repo, &local), repo.join("src"));
    }

    #[test]
    fn refresh_action_maps_reasons() {
        use super::{RefreshAction, refresh_action};
        assert_eq!(
            refresh_action(&["graph_missing".into()]),
            RefreshAction::Rebuild
        );
        assert_eq!(
            refresh_action(&["git_drift".into()]),
            RefreshAction::Rebuild
        );
        assert_eq!(
            refresh_action(&["ttl_expired".into()]),
            RefreshAction::Rebuild
        );
        assert_eq!(
            refresh_action(&["lexical_lag".into()]),
            RefreshAction::Ingest
        );
        assert_eq!(refresh_action(&[]), RefreshAction::Skip);
        // rebuild dominates a co-occurring lexical_lag
        assert_eq!(
            refresh_action(&["git_drift".into(), "lexical_lag".into()]),
            RefreshAction::Rebuild
        );
    }
}
