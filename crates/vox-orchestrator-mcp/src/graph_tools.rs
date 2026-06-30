//! Graphify corpus tools (`vox_graphify_status`, `vox_graphify_search`).

use chrono::Utc;
use serde::Deserialize;
use std::fs;
use vox_graph_reader;

use crate::git_exec::{GitExec, GitExecError};
use crate::params::ToolResult;
use crate::server_state::ServerState;
use vox_config::graphify::{
    CorpusStatus, GraphifyError, assess_corpus_status, lexical_search_graph, load_graphify_corpora,
};

const REM_GRAPHIFY: &str =
    "Ensure `contracts/retrieval/vox-graph-corpora.v1.yaml` exists and graph paths are readable.";

const DEFAULT_SEARCH_LIMIT: u32 = 10;

#[derive(Debug, Deserialize)]
pub struct GraphifyStatusParams {
    /// Corpus id; omit to report all corpora in the registry.
    pub corpus: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GraphifySearchParams {
    /// Corpus id from the registry; omit to use `default_corpus_id`.
    pub corpus: Option<String>,
    /// Optional intent; when `corpus` is omitted, routes to the corpus registered for this
    /// intent (`default_for_intents`) before falling back to `default_corpus_id`.
    #[serde(default)]
    pub intent: Option<String>,
    pub query: String,
    pub limit: Option<u32>,
    /// When true (default), upsert each hit into `knowledge_nodes` for future agent recall.
    /// Pass false for ephemeral searches that must not be recorded.
    #[serde(default = "default_persist_true")]
    pub persist: bool,
}

fn default_persist_true() -> bool {
    true
}

fn corpus_by_id<'a>(
    reg: &'a vox_config::graphify::GraphifyCorporaRegistry,
    id: &str,
) -> Result<&'a vox_config::graphify::GraphifyCorpus, GraphifyError> {
    reg.corpora
        .iter()
        .find(|c| c.id == id)
        .ok_or_else(|| GraphifyError::UnknownCorpus(id.to_string()))
}

fn selected_corpora<'a>(
    reg: &'a vox_config::graphify::GraphifyCorporaRegistry,
    corpus: &Option<String>,
) -> Result<Vec<&'a vox_config::graphify::GraphifyCorpus>, GraphifyError> {
    match corpus {
        Some(id) => Ok(vec![corpus_by_id(reg, id)?]),
        None => Ok(reg.corpora.iter().collect()),
    }
}

fn resolve_search_corpus<'a>(
    reg: &'a vox_config::graphify::GraphifyCorporaRegistry,
    corpus: &Option<String>,
    intent: &Option<String>,
) -> Result<(&'a vox_config::graphify::GraphifyCorpus, String), GraphifyError> {
    if let Some(id) = corpus {
        let c = corpus_by_id(reg, id)?;
        Ok((c, id.clone()))
    } else if let Some(i) = intent {
        if let Some(id) = vox_config::graphify::select_corpus_for_intent(reg, i) {
            let c = corpus_by_id(reg, &id)?;
            Ok((c, id))
        } else {
            let id = reg.default_corpus_id.clone();
            let c = corpus_by_id(reg, &id)?;
            Ok((c, id))
        }
    } else {
        let id = reg.default_corpus_id.clone();
        let c = corpus_by_id(reg, &id)?;
        Ok((c, id))
    }
}

fn knowledge_id(corpus_id: &str, node_id: &str) -> String {
    format!("graphify:{corpus_id}:node:{node_id}")
}

/// Build a `corpus_health` block from a single corpus's freshness assessment so query/search/path
/// payloads carry a recency signal the AI caller can act on. Never fails the query — staleness is
/// reported, not enforced.
fn corpus_health_block(
    repo_root: &std::path::Path,
    reg: &vox_config::graphify::GraphifyCorporaRegistry,
    corpus: &vox_config::graphify::GraphifyCorpus,
    head_sha: Option<&str>,
) -> serde_json::Value {
    let now = Utc::now();
    let ttl = vox_config::graphify::resolve_ttl_days(reg.ttl_days_default);
    let status = assess_corpus_status(repo_root, corpus, head_sha, now, ttl);
    let confidence = if status.is_fresh { "fresh" } else { "stale" };
    serde_json::json!({
        "is_fresh": status.is_fresh,
        "stale_reasons": status.stale_reasons,
        "built_at": status.built_at,
        "head_git_sha": status.head_git_sha,
        "confidence": confidence,
    })
}

async fn resolve_head_sha(state: &ServerState) -> Option<String> {
    let cwd = state
        .repository
        .git_root
        .clone()
        .unwrap_or_else(|| state.repository.root.clone());
    let exec = GitExec::new(cwd);
    match exec.run(&["rev-parse", "HEAD"]).await {
        Ok(o) => {
            let sha = o.stdout.trim();
            if sha.is_empty() {
                None
            } else {
                Some(sha.to_string())
            }
        }
        Err(GitExecError::Banned(_))
        | Err(GitExecError::NonZero { .. })
        | Err(GitExecError::Spawn { .. }) => None,
    }
}

fn assess_all(
    repo_root: &std::path::Path,
    reg: &vox_config::graphify::GraphifyCorporaRegistry,
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

/// `vox_graphify_status`: read-only freshness report for graphify corpora.
pub async fn graphify_status(state: &ServerState, params: GraphifyStatusParams) -> String {
    let repo_root = &state.repository.root;
    let reg = match load_graphify_corpora(repo_root) {
        Ok(r) => r,
        Err(e) => {
            return ToolResult::<serde_json::Value>::err_with_remediation(
                e.to_string(),
                REM_GRAPHIFY,
            )
            .to_json();
        }
    };
    let head = resolve_head_sha(state).await;
    let statuses = match assess_all(repo_root, &reg, &params.corpus, head.as_deref()) {
        Ok(s) => s,
        Err(e) => {
            return ToolResult::<serde_json::Value>::err_with_remediation(
                e.to_string(),
                REM_GRAPHIFY,
            )
            .to_json();
        }
    };
    // Augment each corpus with `rebuild_recommended` (true when not fresh) so the AI caller can
    // close the loop via `vox_search_rebuild` without re-deriving freshness.
    let corpora: Vec<serde_json::Value> = statuses
        .iter()
        .map(|s| {
            let mut v = serde_json::to_value(s).unwrap_or_else(|_| serde_json::json!({}));
            if let Some(obj) = v.as_object_mut() {
                obj.insert(
                    "rebuild_recommended".to_string(),
                    serde_json::json!(!s.is_fresh),
                );
            }
            v
        })
        .collect();
    let payload = serde_json::json!({
        "head_git_sha": head,
        "default_corpus_id": reg.default_corpus_id,
        "corpora": corpora,
    });
    ToolResult::ok(payload).to_json()
}

/// URL-safe slug from a query string with a 8-char FNV-64 hash suffix to prevent collisions.
///
/// Two different queries that share a 32-char prefix will still produce unique slugs because
/// the hash suffix is derived from the **full** original query string.
fn query_slug(query: &str) -> String {
    // Normalize: lowercase, non-alphanumeric → hyphen, collapse runs, trim.
    let normalized: String = query
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let prefix: String = normalized.chars().take(32).collect();
    // FNV-64 of the original query (full, before normalization) for collision resistance.
    let hash = {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        query.hash(&mut h);
        format!("{:08x}", h.finish() & 0xffff_ffff)
    };
    if prefix.is_empty() {
        hash
    } else {
        format!("{prefix}-{hash}")
    }
}

/// `vox_graphify_search`: lexical search over an on-disk graphify corpus graph.
pub async fn graphify_search(state: &ServerState, params: GraphifySearchParams) -> String {
    let repo_root = &state.repository.root;
    let reg = match load_graphify_corpora(repo_root) {
        Ok(r) => r,
        Err(e) => {
            return ToolResult::<serde_json::Value>::err_with_remediation(
                e.to_string(),
                REM_GRAPHIFY,
            )
            .to_json();
        }
    };
    let (corpus, corpus_id) = match resolve_search_corpus(&reg, &params.corpus, &params.intent) {
        Ok(v) => v,
        Err(e) => {
            return ToolResult::<serde_json::Value>::err_with_remediation(
                e.to_string(),
                REM_GRAPHIFY,
            )
            .to_json();
        }
    };
    let graph_path = repo_root.join(&corpus.graph_path);
    let raw = match fs::read_to_string(&graph_path) {
        Ok(s) => s,
        Err(e) => {
            return ToolResult::<serde_json::Value>::err_with_remediation(
                format!("read {}: {e}", graph_path.display()),
                REM_GRAPHIFY,
            )
            .to_json();
        }
    };
    let graph: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            return ToolResult::<serde_json::Value>::err_with_remediation(
                format!("parse {}: {e}", graph_path.display()),
                REM_GRAPHIFY,
            )
            .to_json();
        }
    };
    let limit = params.limit.unwrap_or(DEFAULT_SEARCH_LIMIT).max(1);
    let hits = lexical_search_graph(&graph, &corpus_id, &params.query, limit as usize);

    // Record searched_at before any async work.
    let searched_at = chrono::Utc::now().to_rfc3339();
    let head_sha_for_meta = resolve_head_sha(state).await;

    // Persist hits to Turso so future agents can recall this search.
    // NOTE: Recall consumers MUST compare metadata.git_sha against HEAD to detect stale hits.
    if params.persist && !hits.is_empty() {
        let slug = query_slug(&params.query);
        if let Ok(db) = vox_db::VoxDb::connect_default().await {
            for hit in &hits {
                let node_id = format!("graphify:{corpus_id}:search:{slug}:{}", hit.node_id);
                let metadata = serde_json::json!({
                    "corpus_id": corpus_id,
                    "query": params.query,
                    "searched_at": searched_at,
                    "git_sha": head_sha_for_meta,
                    "source": "graphify_search_hit",
                })
                .to_string();
                // Non-fatal: DB unavailability must not fail the search response.
                // We DO log non-unavailability errors so schema/auth problems surface.
                if let Err(e) = db
                    .upsert_knowledge_node(
                        &node_id,
                        &hit.label,
                        &format!(
                            "{} [corpus: {corpus_id}, query: {}]",
                            hit.label, params.query
                        ),
                        Some("graphify_search_hit"),
                        Some(&metadata),
                        None,
                    )
                    .await
                {
                    tracing::warn!(
                        corpus_id = %corpus_id,
                        node_id = %node_id,
                        error = %e,
                        "graphify search-hit persist failed (non-fatal)"
                    );
                }
            }
        }
    }

    let payload_hits: Vec<serde_json::Value> = hits
        .into_iter()
        .map(|h| {
            serde_json::json!({
                "node_id": h.node_id,
                "label": h.label,
                "score": h.score,
                "knowledge_id": knowledge_id(&corpus_id, &h.node_id),
            })
        })
        .collect();
    let corpus_health = corpus_health_block(repo_root, &reg, corpus, head_sha_for_meta.as_deref());
    let payload = serde_json::json!({
        "corpus_id": corpus_id,
        "searched_at": searched_at,
        "corpus_health": corpus_health,
        "hits": payload_hits,
    });
    ToolResult::ok(payload).to_json()
}

// ── Graphify Query (BFS expansion) ────────────────────────────────────────

use vox_graph_reader::Direction;

/// Parse the optional `direction` param. Unknown/missing → `Both` (legacy).
fn parse_direction(raw: &Option<String>) -> Direction {
    match raw.as_deref() {
        Some("in") => Direction::In,
        Some("out") => Direction::Out,
        _ => Direction::Both,
    }
}

fn direction_label(d: Direction) -> &'static str {
    match d {
        Direction::In => "in",
        Direction::Out => "out",
        Direction::Both => "both",
    }
}

#[derive(Debug, Deserialize)]
pub struct GraphifyQueryParams {
    pub corpus: Option<String>,
    /// Seed node IDs to BFS-expand from.
    pub seeds: Vec<String>,
    /// BFS hop limit (default 2, max 5).
    pub max_depth: Option<u8>,
    /// Max hits returned (default 20).
    pub limit: Option<u32>,
    /// "in" = callers, "out" = callees, "both" = undirected (default).
    #[serde(default)]
    pub direction: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GraphifyPathParams {
    pub corpus: Option<String>,
    /// Source node ID.
    pub from: String,
    /// Destination node ID.
    pub to: String,
    /// "in" = callers, "out" = callees, "both" = undirected (default).
    #[serde(default)]
    pub direction: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GraphifyCompareParams {
    pub corpus_a: String,
    pub corpus_b: String,
}

/// Load and parse a corpus graph.json from disk.
fn load_graph_json(
    repo_root: &std::path::Path,
    corpus: &vox_config::graphify::GraphifyCorpus,
) -> Result<serde_json::Value, String> {
    let p = repo_root.join(&corpus.graph_path);
    let raw = fs::read_to_string(&p).map_err(|e| format!("read {}: {e}", p.display()))?;
    serde_json::from_str(&raw).map_err(|e| format!("parse {}: {e}", p.display()))
}

/// `vox_graphify_query`: BFS neighbor expansion from seed node IDs.
pub async fn graphify_query(state: &ServerState, params: GraphifyQueryParams) -> String {
    let repo_root = &state.repository.root;
    let reg = match load_graphify_corpora(repo_root) {
        Ok(r) => r,
        Err(e) => {
            return ToolResult::<serde_json::Value>::err_with_remediation(
                e.to_string(),
                REM_GRAPHIFY,
            )
            .to_json();
        }
    };
    let (corpus, corpus_id) = match resolve_search_corpus(&reg, &params.corpus, &None) {
        Ok(v) => v,
        Err(e) => {
            return ToolResult::<serde_json::Value>::err_with_remediation(
                e.to_string(),
                REM_GRAPHIFY,
            )
            .to_json();
        }
    };
    let graph = match load_graph_json(repo_root, corpus) {
        Ok(v) => v,
        Err(e) => {
            return ToolResult::<serde_json::Value>::err_with_remediation(e, REM_GRAPHIFY)
                .to_json();
        }
    };
    let reader = match vox_graph_reader::GraphifyReader::from_value(graph) {
        Ok(r) => r,
        Err(e) => {
            return ToolResult::<serde_json::Value>::err_with_remediation(
                e.to_string(),
                REM_GRAPHIFY,
            )
            .to_json();
        }
    };
    let max_depth = params.max_depth.unwrap_or(2).min(5);
    let limit = params.limit.unwrap_or(20).max(1) as usize;
    let seeds: Vec<&str> = params.seeds.iter().map(String::as_str).collect();
    let direction = parse_direction(&params.direction);
    let hits = reader.bfs_from_seeds(&seeds, max_depth, limit, direction);
    let payload_hits: Vec<serde_json::Value> = hits
        .iter()
        .map(|h| {
            serde_json::json!({
                "node_id": h.node_id,
                "label": h.label,
                "depth": h.depth,
                "path": h.path,
                "knowledge_id": knowledge_id(&corpus_id, &h.node_id),
            })
        })
        .collect();
    let head = resolve_head_sha(state).await;
    let corpus_health = corpus_health_block(repo_root, &reg, corpus, head.as_deref());
    ToolResult::ok(serde_json::json!({
        "corpus_id": corpus_id,
        "seeds": params.seeds,
        "direction": direction_label(direction),
        "corpus_health": corpus_health,
        "hits": payload_hits,
    }))
    .to_json()
}

/// `vox_graphify_path`: shortest path between two node IDs.
pub async fn graphify_path(state: &ServerState, params: GraphifyPathParams) -> String {
    let repo_root = &state.repository.root;
    let reg = match load_graphify_corpora(repo_root) {
        Ok(r) => r,
        Err(e) => {
            return ToolResult::<serde_json::Value>::err_with_remediation(
                e.to_string(),
                REM_GRAPHIFY,
            )
            .to_json();
        }
    };
    let (corpus, corpus_id) = match resolve_search_corpus(&reg, &params.corpus, &None) {
        Ok(v) => v,
        Err(e) => {
            return ToolResult::<serde_json::Value>::err_with_remediation(
                e.to_string(),
                REM_GRAPHIFY,
            )
            .to_json();
        }
    };
    let graph = match load_graph_json(repo_root, corpus) {
        Ok(v) => v,
        Err(e) => {
            return ToolResult::<serde_json::Value>::err_with_remediation(e, REM_GRAPHIFY)
                .to_json();
        }
    };
    let reader = match vox_graph_reader::GraphifyReader::from_value(graph) {
        Ok(r) => r,
        Err(e) => {
            return ToolResult::<serde_json::Value>::err_with_remediation(
                e.to_string(),
                REM_GRAPHIFY,
            )
            .to_json();
        }
    };
    let direction = parse_direction(&params.direction);
    let path = reader.shortest_path(&params.from, &params.to, direction);
    let reachable = path.is_some();
    let head = resolve_head_sha(state).await;
    let corpus_health = corpus_health_block(repo_root, &reg, corpus, head.as_deref());
    ToolResult::ok(serde_json::json!({
        "corpus_id": corpus_id,
        "from": params.from,
        "to": params.to,
        "direction": direction_label(direction),
        "path": path,
        "reachable": reachable,
        "corpus_health": corpus_health,
    }))
    .to_json()
}

/// `vox_graphify_compare`: diff two corpus manifests (node/edge/community delta).
pub async fn graphify_compare(state: &ServerState, params: GraphifyCompareParams) -> String {
    let repo_root = &state.repository.root;
    let reg = match load_graphify_corpora(repo_root) {
        Ok(r) => r,
        Err(e) => {
            return ToolResult::<serde_json::Value>::err_with_remediation(
                e.to_string(),
                REM_GRAPHIFY,
            )
            .to_json();
        }
    };
    let corpus_a = match reg.corpora.iter().find(|c| c.id == params.corpus_a) {
        Some(c) => c,
        None => {
            return ToolResult::<serde_json::Value>::err_with_remediation(
                format!("unknown corpus_a: {}", params.corpus_a),
                REM_GRAPHIFY,
            )
            .to_json();
        }
    };
    let corpus_b = match reg.corpora.iter().find(|c| c.id == params.corpus_b) {
        Some(c) => c,
        None => {
            return ToolResult::<serde_json::Value>::err_with_remediation(
                format!("unknown corpus_b: {}", params.corpus_b),
                REM_GRAPHIFY,
            )
            .to_json();
        }
    };
    let now = chrono::Utc::now();
    let ttl = reg.ttl_days_default;
    let head = resolve_head_sha(state).await;
    let status_a = assess_corpus_status(repo_root, corpus_a, head.as_deref(), now, ttl);
    let status_b = assess_corpus_status(repo_root, corpus_b, head.as_deref(), now, ttl);
    let summary_a = vox_graph_reader::compare::ManifestSummary {
        node_count: status_a.node_count.unwrap_or(0),
        edge_count: status_a.edge_count.unwrap_or(0),
        community_count: 0, // not in CorpusStatus; reserved for future manifest field
    };
    let summary_b = vox_graph_reader::compare::ManifestSummary {
        node_count: status_b.node_count.unwrap_or(0),
        edge_count: status_b.edge_count.unwrap_or(0),
        community_count: 0,
    };
    let diff = vox_graph_reader::compare::diff_manifests(&summary_a, &summary_b);
    ToolResult::ok(serde_json::json!({
        "corpus_a": {
            "id": params.corpus_a,
            "node_count": summary_a.node_count,
            "edge_count": summary_a.edge_count,
            "is_fresh": status_a.is_fresh,
        },
        "corpus_b": {
            "id": params.corpus_b,
            "node_count": summary_b.node_count,
            "edge_count": summary_b.edge_count,
            "is_fresh": status_b.is_fresh,
        },
        "diff": {
            "node_delta": diff.node_delta,
            "edge_delta": diff.edge_delta,
            "community_delta": diff.community_delta,
        },
    }))
    .to_json()
}

// ── Graphify Rebuild (P1 close-the-loop) ──────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct GraphifyRebuildParams {
    /// Corpus id to rebuild; omit to rebuild the registry `default_corpus_id`.
    pub corpus: Option<String>,
}

/// `vox_search_rebuild`: WRITE/mutating — rebuild a corpus's AST code graph (same entrypoint as
/// the CLI `vox graphify rebuild`). Snapshots the previous graph, regenerates it, and reports the
/// fresh node/edge counts so an agent can close the staleness loop after seeing
/// `rebuild_recommended: true` in `vox_graphify_status`.
pub async fn graphify_rebuild(state: &ServerState, params: GraphifyRebuildParams) -> String {
    let repo_root = &state.repository.root;
    let reg = match load_graphify_corpora(repo_root) {
        Ok(r) => r,
        Err(e) => {
            return ToolResult::<serde_json::Value>::err_with_remediation(
                e.to_string(),
                REM_GRAPHIFY,
            )
            .to_json();
        }
    };
    let corpus_id = match &params.corpus {
        Some(id) => id.clone(),
        None => reg.default_corpus_id.clone(),
    };
    let corpus = match corpus_by_id(&reg, &corpus_id) {
        Ok(c) => c,
        Err(e) => {
            return ToolResult::<serde_json::Value>::err_with_remediation(
                e.to_string(),
                REM_GRAPHIFY,
            )
            .to_json();
        }
    };

    let source_dir = corpus
        .source_root
        .as_ref()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| repo_root.to_path_buf())
        .join(&corpus.scope_path);
    let output_file = repo_root.join(&corpus.graph_path);
    let cache_dir = match output_file.parent() {
        Some(p) => p.join("file_cache"),
        None => {
            return ToolResult::<serde_json::Value>::err_with_remediation(
                "graph_path has no parent directory".to_string(),
                REM_GRAPHIFY,
            )
            .to_json();
        }
    };

    let head = resolve_head_sha(state).await;
    let built_at = Utc::now().to_rfc3339();
    let meta = vox_graph_reader::rebuild::RebuildMeta {
        corpus_id: corpus_id.clone(),
        git_sha: head,
        scope_path: corpus.scope_path.clone(),
        extraction_mode: corpus.extraction_mode.clone(),
        built_at_rfc3339: built_at.clone(),
        cli_catalog_json: None,
        ..Default::default()
    };

    // Preserve the previous graph as a bounded history before overwriting (mirrors the CLI).
    if output_file.is_file() {
        if let Some(corpus_dir) = output_file.parent() {
            let stamp = Utc::now().to_rfc3339().replace(':', "-");
            let _ = vox_graph_reader::snapshot::snapshot_corpus(corpus_dir, &stamp);
            let _ = vox_graph_reader::snapshot::prune_snapshots(corpus_dir, 5);
        }
    }

    if let Err(e) = vox_graph_reader::rebuild::rebuild_graph(
        repo_root,
        &source_dir,
        &output_file,
        &cache_dir,
        &meta,
    ) {
        return ToolResult::<serde_json::Value>::err_with_remediation(
            format!("rebuild {corpus_id} failed: {e}"),
            REM_GRAPHIFY,
        )
        .to_json();
    }

    // Re-read the freshly written graph to report node/edge counts.
    let (node_count, edge_count) = match load_graph_json(repo_root, corpus) {
        Ok(g) => {
            let n = g.get("nodes").and_then(|v| v.as_array()).map(|a| a.len());
            let e = g
                .get("links")
                .or_else(|| g.get("edges"))
                .and_then(|v| v.as_array())
                .map(|a| a.len());
            (n, e)
        }
        Err(_) => (None, None),
    };

    ToolResult::ok(serde_json::json!({
        "corpus_id": corpus_id,
        "rebuilt": true,
        "built_at": built_at,
        "node_count": node_count,
        "edge_count": edge_count,
    }))
    .to_json()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use vox_orchestrator::{
        AffinityGroupRegistry, Orchestrator, OrchestratorConfig, SessionConfig, SessionManager,
    };
    use vox_repository::{RepoCapabilities, RepositoryContext};
    use vox_skills::new_registry_arc;

    fn write_registry(repo: &Path) {
        let dir = repo.join("contracts/retrieval");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("vox-graph-corpora.v1.yaml"),
            include_str!("../../../contracts/retrieval/vox-graph-corpora.v1.yaml"),
        )
        .unwrap();
    }

    fn write_sample_graph(repo: &Path) {
        let dir = repo.join(vox_config::paths::REPO_GRAPHIFY_REPO_CODE_GRAPH_DIR);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("graph.json"),
            r#"{"nodes":[{"id":"auth","label":"authentication module","type":"module"}],"links":[]}"#,
        )
        .unwrap();
    }

    fn test_state_for_repo(root: std::path::PathBuf) -> ServerState {
        let cfg = OrchestratorConfig::for_testing();
        let orch_cfg = cfg.clone();
        let groups = AffinityGroupRegistry::new(vec![]);
        let session_cfg = SessionConfig {
            persist: false,
            sessions_dir: std::env::temp_dir().join("vox-mcp-graphify-tools-test-sessions"),
            ..SessionConfig::default()
        };
        let session_manager = SessionManager::new(session_cfg).expect("session manager");
        let repository = RepositoryContext {
            root,
            git_root: None,
            repository_id: "graphify-tools-test".into(),
            origin_url: None,
            capabilities: RepoCapabilities {
                vox_project: false,
                cargo_workspace: false,
                cargo_package: false,
                node_workspace: false,
                python_project: false,
                go_module: false,
                git: false,
            },
            has_vox_agents_dir: false,
            vox_toml: None,
        };
        ServerState::test_stub(
            cfg,
            repository,
            Arc::new(Orchestrator::with_groups(orch_cfg, groups)),
            Arc::new(Mutex::new(session_manager)),
            new_registry_arc(),
        )
    }

    #[test]
    fn unknown_corpus_returns_error_from_assess_all() {
        let tmp = tempfile::tempdir().unwrap();
        write_registry(tmp.path());
        let reg = load_graphify_corpora(tmp.path()).unwrap();
        let err = assess_all(tmp.path(), &reg, &Some("missing".into()), None).unwrap_err();
        assert!(matches!(err, GraphifyError::UnknownCorpus(_)));
    }

    #[tokio::test]
    async fn graphify_search_returns_matching_hit() {
        let tmp = tempfile::tempdir().unwrap();
        write_registry(tmp.path());
        write_sample_graph(tmp.path());
        let state = test_state_for_repo(tmp.path().to_path_buf());
        let json = graphify_search(
            &state,
            GraphifySearchParams {
                corpus: Some("repo-code-graph".into()),
                intent: None,
                query: "authentication".into(),
                limit: None,
                persist: false, // avoid DB in unit test
            },
        )
        .await;
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        assert_eq!(parsed.get("success"), Some(&serde_json::json!(true)));
        let data = parsed.get("data").expect("data");
        assert_eq!(
            data.get("corpus_id"),
            Some(&serde_json::json!("repo-code-graph"))
        );
        let hits = data
            .get("hits")
            .and_then(|h| h.as_array())
            .expect("hits array");
        assert!(!hits.is_empty(), "expected at least one hit: {json}");
        let first = &hits[0];
        assert_eq!(first.get("node_id"), Some(&serde_json::json!("auth")));
        assert_eq!(
            first.get("knowledge_id"),
            Some(&serde_json::json!("graphify:repo-code-graph:node:auth"))
        );
        assert!(
            first
                .get("label")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .contains("authentication")
        );
    }

    #[tokio::test]
    async fn graphify_search_routes_by_intent_when_no_corpus() {
        let tmp = tempfile::tempdir().unwrap();
        write_registry(tmp.path());
        write_sample_graph(tmp.path());
        let state = test_state_for_repo(tmp.path().to_path_buf());
        let json = graphify_search(
            &state,
            GraphifySearchParams {
                corpus: None,
                intent: Some("code_navigation".into()), // → repo-code-graph
                query: "authentication".into(),
                limit: None,
                persist: false,
            },
        )
        .await;
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        assert_eq!(parsed.get("success"), Some(&serde_json::json!(true)));
        let data = parsed.get("data").expect("data");
        assert_eq!(
            data.get("corpus_id"),
            Some(&serde_json::json!("repo-code-graph"))
        );
    }

    #[tokio::test]
    async fn graphify_search_response_includes_searched_at() {
        let tmp = tempfile::tempdir().unwrap();
        write_registry(tmp.path());
        write_sample_graph(tmp.path());
        let state = test_state_for_repo(tmp.path().to_path_buf());
        let json = graphify_search(
            &state,
            GraphifySearchParams {
                corpus: Some("repo-code-graph".into()),
                intent: None,
                query: "authentication".into(),
                limit: None,
                persist: false, // skip DB in unit test
            },
        )
        .await;
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        assert_eq!(parsed["success"], serde_json::json!(true));
        let data = parsed.get("data").expect("data field");
        assert!(
            data.get("searched_at").and_then(|v| v.as_str()).is_some(),
            "searched_at must be a string: {data}"
        );
        assert_eq!(data["corpus_id"], serde_json::json!("repo-code-graph"));
    }

    #[tokio::test]
    async fn graphify_search_payload_includes_corpus_health() {
        let tmp = tempfile::tempdir().unwrap();
        write_registry(tmp.path());
        write_sample_graph(tmp.path());
        let state = test_state_for_repo(tmp.path().to_path_buf());
        let json = graphify_search(
            &state,
            GraphifySearchParams {
                corpus: Some("repo-code-graph".into()),
                intent: None,
                query: "authentication".into(),
                limit: None,
                persist: false,
            },
        )
        .await;
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        let health = parsed["data"]["corpus_health"]
            .as_object()
            .expect("corpus_health object");
        assert!(
            health.get("is_fresh").and_then(|v| v.as_bool()).is_some(),
            "is_fresh present: {json}"
        );
        assert!(
            health
                .get("stale_reasons")
                .and_then(|v| v.as_array())
                .is_some(),
            "stale_reasons present: {json}"
        );
        assert!(
            health.get("confidence").and_then(|v| v.as_str()).is_some(),
            "confidence present: {json}"
        );
    }

    #[tokio::test]
    async fn graphify_path_payload_includes_corpus_health() {
        let tmp = tempfile::tempdir().unwrap();
        write_registry(tmp.path());
        let dir = tmp
            .path()
            .join(vox_config::paths::REPO_GRAPHIFY_REPO_CODE_GRAPH_DIR);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("graph.json"),
            r#"{"nodes":[{"id":"a"},{"id":"b"}],"links":[{"source":"a","target":"b"}]}"#,
        )
        .unwrap();
        let state = test_state_for_repo(tmp.path().to_path_buf());
        let json = graphify_path(
            &state,
            GraphifyPathParams {
                corpus: Some("repo-code-graph".into()),
                from: "a".into(),
                to: "b".into(),
                direction: None,
            },
        )
        .await;
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        let health = parsed["data"]["corpus_health"]
            .as_object()
            .expect("corpus_health object");
        assert!(health.get("is_fresh").and_then(|v| v.as_bool()).is_some());
        assert!(
            health
                .get("stale_reasons")
                .and_then(|v| v.as_array())
                .is_some()
        );
    }

    #[tokio::test]
    async fn graphify_query_returns_bfs_neighbors() {
        let tmp = tempfile::tempdir().unwrap();
        write_registry(tmp.path());
        // Graph: auth --edge--> crypto
        let dir = tmp
            .path()
            .join(vox_config::paths::REPO_GRAPHIFY_REPO_CODE_GRAPH_DIR);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("graph.json"),
            r#"{"nodes":[{"id":"auth","label":"authentication module","type":"module"},{"id":"crypto","label":"crypto lib","type":"lib"}],"links":[{"source":"auth","target":"crypto"}]}"#,
        )
        .unwrap();
        let state = test_state_for_repo(tmp.path().to_path_buf());
        let json = graphify_query(
            &state,
            GraphifyQueryParams {
                corpus: Some("repo-code-graph".into()),
                seeds: vec!["auth".into()],
                max_depth: Some(1),
                limit: Some(10),
                direction: None,
            },
        )
        .await;
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        assert_eq!(
            parsed["success"],
            serde_json::json!(true),
            "tool error: {json}"
        );
        let hits = parsed["data"]["hits"]
            .as_array()
            .expect("hits must be an array");
        assert!(!hits.is_empty(), "expected BFS hits: {json}");
        assert_eq!(hits[0]["node_id"], serde_json::json!("crypto"));
    }

    #[tokio::test]
    async fn graphify_path_returns_node_route() {
        let tmp = tempfile::tempdir().unwrap();
        write_registry(tmp.path());
        let dir = tmp
            .path()
            .join(vox_config::paths::REPO_GRAPHIFY_REPO_CODE_GRAPH_DIR);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("graph.json"),
            r#"{"nodes":[{"id":"a"},{"id":"b"},{"id":"c"}],"links":[{"source":"a","target":"b"},{"source":"b","target":"c"}]}"#,
        )
        .unwrap();
        let state = test_state_for_repo(tmp.path().to_path_buf());
        let json = graphify_path(
            &state,
            GraphifyPathParams {
                corpus: Some("repo-code-graph".into()),
                from: "a".into(),
                to: "c".into(),
                direction: None,
            },
        )
        .await;
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        assert_eq!(
            parsed["success"],
            serde_json::json!(true),
            "tool error: {json}"
        );
        assert_eq!(parsed["data"]["path"], serde_json::json!(["a", "b", "c"]));
        assert_eq!(parsed["data"]["reachable"], serde_json::json!(true));
    }

    #[tokio::test]
    async fn graphify_compare_returns_delta_fields() {
        let tmp = tempfile::tempdir().unwrap();
        write_registry(tmp.path());
        // Write graph files for both corpora being compared.
        let dir_a = tmp
            .path()
            .join(vox_config::paths::REPO_GRAPHIFY_REPO_CODE_GRAPH_DIR);
        fs::create_dir_all(&dir_a).unwrap();
        fs::write(
            dir_a.join("graph.json"),
            r#"{"nodes":[{"id":"a"},{"id":"b"}],"links":[{"source":"a","target":"b"}]}"#,
        )
        .unwrap();
        let dir_b = tmp
            .path()
            .join(vox_config::paths::REPO_GRAPHIFY_GUI_SURFACE_DIR);
        fs::create_dir_all(&dir_b).unwrap();
        fs::write(
            dir_b.join("graph.json"),
            r#"{"nodes":[{"id":"x"}],"links":[]}"#,
        )
        .unwrap();
        let state = test_state_for_repo(tmp.path().to_path_buf());
        let json = graphify_compare(
            &state,
            GraphifyCompareParams {
                corpus_a: "repo-code-graph".into(),
                corpus_b: "vox-gui-surface".into(),
            },
        )
        .await;
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        assert_eq!(
            parsed["success"],
            serde_json::json!(true),
            "compare error: {json}"
        );
        // corpus_a has 2 nodes, corpus_b has 1 → node_delta = -1
        assert_eq!(parsed["data"]["diff"]["node_delta"], serde_json::json!(-1));
    }

    #[tokio::test]
    async fn graphify_compare_unknown_corpus_returns_error() {
        let tmp = tempfile::tempdir().unwrap();
        write_registry(tmp.path());
        let state = test_state_for_repo(tmp.path().to_path_buf());
        let json = graphify_compare(
            &state,
            GraphifyCompareParams {
                corpus_a: "no-such-corpus".into(),
                corpus_b: "repo-code-graph".into(),
            },
        )
        .await;
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        assert_eq!(
            parsed["success"],
            serde_json::json!(false),
            "expected error: {json}"
        );
    }

    #[tokio::test]
    async fn graphify_query_payload_includes_corpus_health() {
        let tmp = tempfile::tempdir().unwrap();
        write_registry(tmp.path());
        let dir = tmp
            .path()
            .join(vox_config::paths::REPO_GRAPHIFY_REPO_CODE_GRAPH_DIR);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("graph.json"),
            r#"{"nodes":[{"id":"auth","label":"auth"},{"id":"crypto","label":"crypto"}],"links":[{"source":"auth","target":"crypto"}]}"#,
        )
        .unwrap();
        let state = test_state_for_repo(tmp.path().to_path_buf());
        let json = graphify_query(
            &state,
            GraphifyQueryParams {
                corpus: Some("repo-code-graph".into()),
                seeds: vec!["auth".into()],
                max_depth: Some(1),
                limit: Some(10),
                direction: None,
            },
        )
        .await;
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        let health = parsed["data"]["corpus_health"]
            .as_object()
            .expect("corpus_health object");
        assert!(health.get("is_fresh").and_then(|v| v.as_bool()).is_some());
        assert!(
            health
                .get("stale_reasons")
                .and_then(|v| v.as_array())
                .is_some()
        );
    }

    #[tokio::test]
    async fn graphify_query_echoes_direction() {
        let tmp = tempfile::tempdir().unwrap();
        write_registry(tmp.path());
        let dir = tmp
            .path()
            .join(vox_config::paths::REPO_GRAPHIFY_REPO_CODE_GRAPH_DIR);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("graph.json"),
            r#"{"nodes":[{"id":"a","label":"a"},{"id":"b","label":"b"}],"links":[{"source":"a","target":"b"}]}"#,
        )
        .unwrap();
        let state = test_state_for_repo(tmp.path().to_path_buf());
        let json = graphify_query(
            &state,
            GraphifyQueryParams {
                corpus: Some("repo-code-graph".into()),
                seeds: vec!["a".into()],
                max_depth: Some(1),
                limit: Some(10),
                direction: Some("out".into()),
            },
        )
        .await;
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(
            parsed["success"],
            serde_json::json!(true),
            "tool error: {json}"
        );
        assert_eq!(parsed["data"]["direction"], serde_json::json!("out"));
        // "out" from `a` reaches `b`; reverse would be empty.
        let hits = parsed["data"]["hits"].as_array().unwrap();
        assert_eq!(hits[0]["node_id"], serde_json::json!("b"));
    }

    #[tokio::test]
    async fn graphify_status_reports_rebuild_recommended() {
        let tmp = tempfile::tempdir().unwrap();
        write_registry(tmp.path());
        // No graph files written → every corpus is stale → rebuild_recommended must be true.
        let state = test_state_for_repo(tmp.path().to_path_buf());
        let json = graphify_status(&state, GraphifyStatusParams { corpus: None }).await;
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        let corpora = parsed["data"]["corpora"].as_array().expect("corpora array");
        assert!(!corpora.is_empty(), "expected corpora: {json}");
        for c in corpora {
            assert!(
                c.get("rebuild_recommended")
                    .and_then(|v| v.as_bool())
                    .is_some(),
                "rebuild_recommended present: {c}"
            );
        }
    }

    #[tokio::test]
    async fn graphify_rebuild_unknown_corpus_returns_error() {
        let tmp = tempfile::tempdir().unwrap();
        write_registry(tmp.path());
        let state = test_state_for_repo(tmp.path().to_path_buf());
        let json = graphify_rebuild(
            &state,
            GraphifyRebuildParams {
                corpus: Some("no-such-corpus".into()),
            },
        )
        .await;
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        assert_eq!(
            parsed["success"],
            serde_json::json!(false),
            "expected error for unknown corpus: {json}"
        );
    }
}
