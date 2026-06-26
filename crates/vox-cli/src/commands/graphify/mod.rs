//! `vox graphify` — corpus registry and freshness status (Tier D maps).

use anyhow::Context;
use chrono::Utc;
use clap::Subcommand;
use vox_config::graphify::{
    CorpusStatus, GraphifyCorporaRegistry, GraphifyCorpus, GraphifyError, GraphifyKnowledgeNode,
    GraphifyManifest, assess_corpus_status, load_all_corpora, load_graphify_corpora,
    project_graph_nodes_for_ingest, upsert_registered_corpus, write_manifest,
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
    /// Classify each backend node of a `kind` as Surfaced/OrphanBackend/DeadEnd.
    Coverage {
        /// Corpus id (default: registry `default_corpus_id`).
        #[arg(long)]
        corpus: Option<String>,
        /// Node kind to score (command | tool | surface).
        #[arg(long, default_value = "command")]
        kind: String,
        /// Write JSON to this path instead of printing to stdout.
        #[arg(long)]
        out: Option<String>,
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
    /// Prune corpus graph snapshots, keeping the newest N per corpus.
    Gc {
        /// Corpus id (default: all corpora).
        #[arg(long)]
        corpus: Option<String>,
        /// How many snapshots to keep per corpus.
        #[arg(long, default_value_t = 5)]
        keep: usize,
    },
    /// Build the crate build-time x dependency map (deterministic Leiden communities +
    /// blast-radius) from contracts/ci/crate-graph.v1.json + graphify-out/crate_audit.json.
    /// With `--write-summary`, also emit the committed gate SSOT.
    CrateMap {
        /// Skip regenerating crate-graph.v1.json from cargo metadata (use the committed snapshot).
        #[arg(long)]
        no_refresh_graph: bool,
        /// Also write the committed SSOT to this path
        /// (bare flag → contracts/ci/crate-build-map.v1.json).
        #[arg(long, num_args = 0..=1, default_missing_value = "contracts/ci/crate-build-map.v1.json")]
        write_summary: Option<String>,
        /// After building, project the crate-map into Turso for agent recall
        /// (stamps lexical_ingest_sha256).
        #[arg(long)]
        ingest: bool,
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
    // Route through vox_git read-only exec (honors the concurrency policy), not a
    // raw git subprocess — enforced by the arch-check raw-git-exec rule.
    match vox_git::read_only(std::path::Path::new("."), &["rev-parse", "HEAD"]) {
        Ok(out) => {
            let sha = out.trim().to_string();
            Ok(if sha.is_empty() { None } else { Some(sha) })
        }
        // Not a git repo / git unavailable → treat as "no HEAD", same as the
        // prior non-zero-exit branch.
        Err(_) => Ok(None),
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
    // vox_git::read_only already runs `git -C <repo> <args>`; pass `dir` as the repo.
    match vox_git::read_only(dir, &["rev-parse", "HEAD"]) {
        Ok(out) => {
            let sha = out.trim().to_string();
            Ok(if sha.is_empty() { None } else { Some(sha) })
        }
        Err(_) => Ok(None),
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

fn regenerate_crate_graph(repo_root: &std::path::Path) -> anyhow::Result<()> {
    let exe = std::env::current_exe().context("current exe")?;
    let status = std::process::Command::new(&exe)
        .current_dir(repo_root)
        .args(["ci", "affected-crates", "--regen"])
        .status()
        .context("spawn crate-graph regenerator")?;
    if !status.success() {
        anyhow::bail!("crate-graph regenerator exited non-zero");
    }
    Ok(())
}

/// Project a corpus's graph nodes into Turso and stamp lexical_ingest_sha256.
/// Shared by `graphify ingest` and `graphify crate-map --ingest`.
async fn run_graphify_ingest(
    repo_root: &std::path::Path,
    corpus: Option<String>,
    dry_run: bool,
) -> anyhow::Result<()> {
    let reg = load_all_corpora(repo_root).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let corpus_id =
        resolve_ingest_corpus_id(&reg, corpus).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let nodes = load_projected_nodes(repo_root, &reg, &corpus_id)?;
    if dry_run {
        println!("dry-run: corpus={corpus_id} nodes={}", nodes.len());
        return Ok(());
    }
    let upserted = upsert_projected_nodes(&nodes).await?;
    println!("graphify ingest: corpus={corpus_id} upserted={upserted}");
    let corpus = corpus_by_id(&reg, &corpus_id).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let graph_bytes = std::fs::read(repo_root.join(&corpus.graph_path))
        .with_context(|| format!("read graph for digest: {}", corpus.graph_path))?;
    let digest = vox_graphify_reader::graph_digest(&graph_bytes);
    vox_config::graphify::set_lexical_ingest_sha256(
        &repo_root.join(&corpus.manifest_path),
        &digest,
    )
    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    Ok(())
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
            run_graphify_ingest(repo_root, corpus, dry_run).await?;
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
            // Preserve the previous graph as a bounded history before overwriting.
            if output_file.is_file() {
                if let Some(corpus_dir) = output_file.parent() {
                    let stamp = Utc::now().to_rfc3339().replace(':', "-");
                    let _ = vox_graphify_reader::snapshot::snapshot_corpus(corpus_dir, &stamp);
                    let _ = vox_graphify_reader::snapshot::prune_snapshots(corpus_dir, 5);
                }
            }
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
        GraphifyCmd::Coverage { corpus, kind, out } => {
            let reg = load_all_corpora(repo_root).map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let corpus_id = resolve_ingest_corpus_id(&reg, corpus)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let corpus =
                corpus_by_id(&reg, &corpus_id).map_err(|e| anyhow::anyhow!(e.to_string()))?;

            let graph_path = repo_root.join(&corpus.graph_path);
            let raw = std::fs::read_to_string(&graph_path)
                .with_context(|| format!("read graph {}", graph_path.display()))?;
            let graph: serde_json::Value = serde_json::from_str(&raw)
                .with_context(|| format!("parse graph JSON {}", graph_path.display()))?;

            let report = vox_graphify_reader::coverage::compute_coverage(&graph, &kind);
            let json = serde_json::to_string_pretty(&report)?;
            match out {
                Some(path) => {
                    let abs = repo_root.join(&path);
                    if let Some(parent) = abs.parent() {
                        std::fs::create_dir_all(parent)
                            .with_context(|| format!("create dir {}", parent.display()))?;
                    }
                    std::fs::write(&abs, &json)
                        .with_context(|| format!("write coverage {}", abs.display()))?;
                    println!(
                        "coverage: corpus={corpus_id} kind={kind} entries={} -> {}",
                        report.entries.len(),
                        path
                    );
                }
                None => println!("{json}"),
            }
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
        GraphifyCmd::Gc { corpus, keep } => {
            let reg = load_all_corpora(repo_root).map_err(|e| anyhow::anyhow!(e.to_string()))?;
            for c in selected_corpora(&reg, &corpus).map_err(|e| anyhow::anyhow!(e.to_string()))? {
                let output_file = repo_root.join(&c.graph_path);
                if let Some(corpus_dir) = output_file.parent() {
                    let removed = vox_graphify_reader::snapshot::prune_snapshots(corpus_dir, keep)
                        .map_err(|e| anyhow::anyhow!("prune {}: {e}", c.id))?;
                    println!("gc {} kept<= {keep} removed={removed}", c.id);
                }
            }
        }
        GraphifyCmd::CrateMap {
            no_refresh_graph,
            write_summary,
            ingest,
        } => {
            // 1. Freshen the committed dependency graph from cargo metadata unless suppressed.
            if !no_refresh_graph {
                if let Err(e) = regenerate_crate_graph(repo_root) {
                    tracing::warn!("crate-graph regen failed, using committed snapshot: {e}");
                }
            }
            let graph_path = repo_root.join("contracts/ci/crate-graph.v1.json");
            let crate_graph: serde_json::Value = serde_json::from_str(
                &std::fs::read_to_string(&graph_path)
                    .with_context(|| format!("read {}", graph_path.display()))?,
            )
            .with_context(|| format!("parse {}", graph_path.display()))?;

            // 2. Audit times are OPTIONAL (graphify-out/ is gitignored; absent on fresh checkout).
            let audit_path = repo_root.join("graphify-out/crate_audit.json");
            let audit: serde_json::Value = match std::fs::read_to_string(&audit_path) {
                Ok(s) => serde_json::from_str(&s).unwrap_or_else(|_| serde_json::json!([])),
                Err(_) => {
                    println!(
                        "note: {} absent — building count-only map (run scripts/crate-build-audit.vox for compile times)",
                        audit_path.display()
                    );
                    serde_json::json!([])
                }
            };

            // 3. Build + persist.
            let map = vox_graphify_reader::crate_model::build_crate_map(&crate_graph, &audit);
            let out_dir = repo_root.join(".vox/cache/graphify/crate-map");
            std::fs::create_dir_all(&out_dir).context("create crate-map cache dir")?;
            let bytes = serde_json::to_string_pretty(&map)?;
            std::fs::write(out_dir.join("graph.json"), &bytes)
                .context("write crate-map graph.json")?;
            let node_count = map["nodes"].as_array().map(|a| a.len() as u64).unwrap_or(0);
            let edge_count = map["links"].as_array().map(|a| a.len() as u64).unwrap_or(0);
            let manifest = GraphifyManifest {
                corpus_id: Some("crate-map".to_string()),
                built_at: Some(Utc::now().to_rfc3339()),
                git_sha: resolve_head_sha()?,
                scope_path: Some(".".to_string()),
                node_count: Some(node_count),
                edge_count: Some(edge_count),
                graph_json_sha256: Some(vox_graphify_reader::graph_digest(bytes.as_bytes())),
                extraction_mode: Some("crate-map".to_string()),
                lexical_ingest_sha256: None,
            };
            write_manifest(&out_dir.join(".graphify_manifest.v1.json"), &manifest)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            println!(
                "crate-map: {node_count} crates, {edge_count} edges -> .vox/cache/graphify/crate-map/graph.json"
            );
            println!("persist for agent recall: vox graphify ingest --corpus crate-map");

            // 4. Optionally emit the committed gate SSOT (small; parity-checked in CI).
            if let Some(summary_path) = write_summary {
                use std::collections::HashMap;
                let mut compile_times: HashMap<String, f64> = HashMap::new();
                if let Some(arr) = audit.as_array() {
                    for r in arr {
                        if let (Some(name), Some(cs)) = (
                            r.get("crate").and_then(|v| v.as_str()),
                            r.get("compile_s").and_then(|v| {
                                v.as_f64()
                                    .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
                            }),
                        ) {
                            compile_times.insert(name.to_string(), cs);
                        }
                    }
                }
                let summary = vox_graphify_reader::crate_model::build_crate_summary(
                    &crate_graph,
                    &compile_times,
                );
                let summary_abs = repo_root.join(&summary_path);
                std::fs::write(&summary_abs, serde_json::to_string_pretty(&summary)?)
                    .with_context(|| format!("write {}", summary_abs.display()))?;
                let has_times = summary["has_compile_times"].as_bool().unwrap_or(false);
                println!(
                    "summary -> {} (has_compile_times={has_times}, missing={})",
                    summary_path, summary["crates_without_compile_times"]
                );
                if !has_times {
                    println!(
                        "WARNING: no compile times — run scripts/crate-build-audit.vox first \
                         (needs `cargo build --timings` to populate target/cargo-timings/)."
                    );
                }
            }

            if ingest {
                run_graphify_ingest(repo_root, Some("crate-map".to_string()), false).await?;
                println!("ingested crate-map (lexical_ingest_sha256 stamped)");
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
