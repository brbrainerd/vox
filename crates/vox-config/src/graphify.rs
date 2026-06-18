//! Graphify corpus registry and freshness assessment (Tier D cache maps).
//!
//! SSOT contract: `contracts/retrieval/graphify-corpora.v1.yaml`

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

/// Relative path to the corpora registry YAML from repo root.
pub const CORPORA_REL_PATH: &str = "contracts/retrieval/graphify-corpora.v1.yaml";

/// Legacy graphify output directory (shared with non-graphify CI artifacts — see research doc).
pub const LEGACY_GRAPHIFY_OUT_DIR: &str = "graphify-out";

/// Basename for per-corpus manifest files written beside `graph.json`.
pub const MANIFEST_BASENAME: &str = ".graphify_manifest.v1.json";

#[derive(Debug, Clone, Deserialize)]
struct CorporaFile {
    default_corpus_id: String,
    #[serde(default = "default_ttl_days")]
    ttl_days_default: u64,
    corpora: Vec<GraphifyCorpus>,
}

fn default_ttl_days() -> u64 {
    30
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GraphifyCorpus {
    pub id: String,
    pub title: String,
    pub scope_path: String,
    pub graph_path: String,
    pub manifest_path: String,
    #[serde(default)]
    pub extraction_mode: Option<String>,
    #[serde(default)]
    pub default_for_intents: Vec<String>,
    /// When true, this corpus is Turso-backed (no on-disk graph.json).
    /// `assess_corpus_status` skips all disk checks and returns fresh unconditionally.
    #[serde(default)]
    pub is_virtual: bool,
}

#[derive(Debug, Clone)]
pub struct GraphifyCorporaRegistry {
    pub default_corpus_id: String,
    pub ttl_days_default: u64,
    pub corpora: Vec<GraphifyCorpus>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct GraphifyManifest {
    pub corpus_id: Option<String>,
    pub built_at: Option<String>,
    pub git_sha: Option<String>,
    pub scope_path: Option<String>,
    pub node_count: Option<u64>,
    pub edge_count: Option<u64>,
    pub graph_json_sha256: Option<String>,
    pub extraction_mode: Option<String>,
    /// SHA256 of the graph file at last `vox graphify ingest` run.
    /// If this differs from `graph_json_sha256`, the Turso index is behind the graph.
    pub lexical_ingest_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CorpusStatus {
    pub corpus_id: String,
    pub title: String,
    pub graph_path: PathBuf,
    pub manifest_path: PathBuf,
    pub graph_exists: bool,
    pub manifest_exists: bool,
    pub node_count: Option<u64>,
    pub edge_count: Option<u64>,
    pub built_at: Option<String>,
    pub manifest_git_sha: Option<String>,
    pub head_git_sha: Option<String>,
    pub stale_reasons: Vec<String>,
    pub warnings: Vec<String>,
    pub is_fresh: bool,
}

#[derive(Debug)]
pub enum GraphifyError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Parse {
        path: PathBuf,
        detail: String,
    },
    UnknownCorpus(String),
}

impl std::fmt::Display for GraphifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GraphifyError::Io { path, source } => {
                write!(f, "read {}: {source}", path.display())
            }
            GraphifyError::Parse { path, detail } => {
                write!(f, "parse {}: {detail}", path.display())
            }
            GraphifyError::UnknownCorpus(id) => write!(f, "unknown corpus id `{id}`"),
        }
    }
}

impl std::error::Error for GraphifyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            GraphifyError::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Load the corpora registry from the repo contract file.
pub fn load_graphify_corpora(repo_root: &Path) -> Result<GraphifyCorporaRegistry, GraphifyError> {
    let path = repo_root.join(CORPORA_REL_PATH);
    let raw = fs::read_to_string(&path).map_err(|source| GraphifyError::Io {
        path: path.clone(),
        source,
    })?;
    let file: CorporaFile = serde_yaml::from_str(&raw).map_err(|e| GraphifyError::Parse {
        path: path.clone(),
        detail: e.to_string(),
    })?;
    Ok(GraphifyCorporaRegistry {
        default_corpus_id: file.default_corpus_id,
        ttl_days_default: file.ttl_days_default,
        corpora: file.corpora,
    })
}

/// Tier D cache dir for a named corpus: `<repo>/.vox/cache/graphify/<corpus_id>/`.
pub fn repo_graphify_cache_dir(repo_root: &Path, corpus_id: &str) -> PathBuf {
    repo_root
        .join(super::paths::REPO_CACHE_DIR)
        .join(super::paths::REPO_GRAPHIFY_CACHE_SUBDIR)
        .join(corpus_id)
}

/// Count nodes and edges/links in a graphify NetworkX export JSON value.
pub fn graph_stats_from_json(value: &serde_json::Value) -> Option<(u64, u64)> {
    let nodes = value
        .get("nodes")
        .and_then(|n| n.as_array())
        .map(|a| a.len() as u64)?;
    let edges = value
        .get("links")
        .or_else(|| value.get("edges"))
        .and_then(|n| n.as_array())
        .map(|a| a.len() as u64)
        .unwrap_or(0);
    Some((nodes, edges))
}

/// Minimum token length for lexical graph search (matches `scientia_prior_art` default: len > 2).
const LEXICAL_TOKEN_MIN_LEN: usize = 2;

/// Lexical hit from a graphify `graph.json` nodes array.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexicalGraphHit {
    pub node_id: String,
    pub label: String,
    pub score: usize,
}

/// Knowledge-node projection for graphify ingest pipelines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphifyKnowledgeNode {
    pub id: String,
    pub label: String,
    pub content: String,
    pub node_type: String,
    pub metadata: String,
}

fn lexical_tokenize(s: &str) -> HashSet<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.len() > LEXICAL_TOKEN_MIN_LEN)
        .map(str::to_string)
        .collect()
}

fn node_label_from_json(node: &serde_json::Value) -> Option<&str> {
    node.get("label")
        .or_else(|| node.get("id"))
        .or_else(|| node.get("name"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
}

fn node_id_from_json(node: &serde_json::Value, label: &str) -> String {
    node.get("id")
        .and_then(|v| v.as_str())
        .unwrap_or(label)
        .to_string()
}

/// Score nodes in a graphify export by token overlap with `query`; return top `limit` hits.
///
/// `corpus_id` is reserved for future corpus-scoped filtering; lexical scoring is graph-local.
#[must_use]
pub fn lexical_search_graph(
    value: &serde_json::Value,
    _corpus_id: &str,
    query: &str,
    limit: usize,
) -> Vec<LexicalGraphHit> {
    let Some(nodes) = value.get("nodes").and_then(|n| n.as_array()) else {
        return Vec::new();
    };
    let query_tokens = lexical_tokenize(query);
    if query_tokens.is_empty() || limit == 0 {
        return Vec::new();
    }

    let mut scored: Vec<LexicalGraphHit> = Vec::new();
    for node in nodes {
        let Some(label) = node_label_from_json(node) else {
            continue;
        };
        let node_tokens = lexical_tokenize(label);
        let overlap = query_tokens.intersection(&node_tokens).count();
        if overlap == 0 {
            continue;
        }
        scored.push(LexicalGraphHit {
            node_id: node_id_from_json(node, label),
            label: label.to_string(),
            score: overlap,
        });
    }
    scored.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.label.cmp(&b.label)));
    scored.truncate(limit);
    scored
}

/// Project graph nodes into knowledge-node records for lexical ingest.
#[must_use]
pub fn project_graph_nodes_for_ingest(
    value: &serde_json::Value,
    corpus_id: &str,
) -> Vec<GraphifyKnowledgeNode> {
    let Some(nodes) = value.get("nodes").and_then(|n| n.as_array()) else {
        return Vec::new();
    };

    nodes
        .iter()
        .filter_map(|node| {
            let label = node_label_from_json(node)?;
            let node_id = node_id_from_json(node, label);
            let node_type = node
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("graph_node")
                .to_string();
            let content = serde_json::to_string(node).unwrap_or_else(|_| label.to_string());
            let metadata = serde_json::json!({
                "corpus_id": corpus_id,
                "source": "graphify_lexical_ingest",
            })
            .to_string();
            Some(GraphifyKnowledgeNode {
                id: format!("graphify:{corpus_id}:node:{node_id}"),
                label: label.to_string(),
                content,
                node_type,
                metadata,
            })
        })
        .collect()
}

fn read_manifest(path: &Path) -> Option<GraphifyManifest> {
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn parse_rfc3339(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

/// Returns `Some("lexical_lag")` when `lexical_ingest_sha256` differs from `graph_json_sha256`.
///
/// Returns `None` when either SHA is absent (never ingested = unknown, not a lag).
pub fn lexical_lag_stale_reason(manifest: &GraphifyManifest) -> Option<String> {
    match (&manifest.graph_json_sha256, &manifest.lexical_ingest_sha256) {
        (Some(graph_sha), Some(ingest_sha)) if graph_sha != ingest_sha => {
            Some("lexical_lag".to_string())
        }
        _ => None,
    }
}

/// Assess on-disk freshness for one corpus (read-only).
pub fn assess_corpus_status(
    repo_root: &Path,
    corpus: &GraphifyCorpus,
    head_git_sha: Option<&str>,
    now: DateTime<Utc>,
    ttl_days: u64,
) -> CorpusStatus {
    // Virtual corpora are Turso-backed; skip all disk checks.
    if corpus.is_virtual {
        return CorpusStatus {
            corpus_id: corpus.id.clone(),
            title: corpus.title.clone(),
            graph_path: repo_root.join(&corpus.graph_path),
            manifest_path: repo_root.join(&corpus.manifest_path),
            graph_exists: false,
            manifest_exists: false,
            node_count: None,
            edge_count: None,
            built_at: None,
            manifest_git_sha: None,
            head_git_sha: head_git_sha.map(str::to_string),
            stale_reasons: vec![],
            warnings: vec!["virtual_corpus".to_string()],
            is_fresh: true,
        };
    }
    let graph_path = repo_root.join(&corpus.graph_path);
    let manifest_path = repo_root.join(&corpus.manifest_path);
    let graph_exists = graph_path.is_file();
    let manifest_exists = manifest_path.is_file();

    let mut stale_reasons = Vec::new();
    let mut warnings = Vec::new();

    if !graph_exists {
        stale_reasons.push("graph_missing".into());
    }
    if !manifest_exists {
        warnings.push("manifest_missing".into());
    }

    let manifest = manifest_exists
        .then(|| read_manifest(&manifest_path))
        .flatten();

    let (node_count, edge_count, built_at, manifest_git_sha) = if graph_exists {
        let raw_res = fs::read_to_string(&graph_path);
        let parse_res = raw_res
            .ok()
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok());
        let stats = parse_res.as_ref().and_then(|v| graph_stats_from_json(v));

        if parse_res.is_none() || stats.is_none() {
            stale_reasons.push("graph_corrupt".into());
        }

        let (n, e) = stats.unwrap_or((0, 0));
        let built = manifest
            .as_ref()
            .and_then(|m| m.built_at.clone())
            .or_else(|| {
                fs::metadata(&graph_path)
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .map(|t| {
                        let dt: DateTime<Utc> = t.into();
                        dt.to_rfc3339()
                    })
            });
        let sha = manifest.as_ref().and_then(|m| m.git_sha.clone());
        (Some(n), Some(e), built, sha)
    } else {
        (
            None,
            None,
            None,
            manifest.as_ref().and_then(|m| m.git_sha.clone()),
        )
    };

    if let (Some(m_sha), Some(h_sha)) = (&manifest_git_sha, head_git_sha)
        && m_sha != h_sha
    {
        stale_reasons.push("git_drift".into());
    }

    if let Some(ref built) = built_at
        && let Some(built_dt) = parse_rfc3339(built)
        && now.signed_duration_since(built_dt) > Duration::days(ttl_days as i64)
    {
        stale_reasons.push("ttl_expired".into());
    }

    if let (Some(m), Some(nc), Some(ec)) = (&manifest, node_count, edge_count) {
        if let Some(expected) = m.node_count
            && expected != nc
        {
            warnings.push("node_count_drift".into());
        }
        if let Some(expected) = m.edge_count
            && expected != ec
        {
            warnings.push("edge_count_drift".into());
        }
    }

    let is_fresh = stale_reasons.is_empty();
    CorpusStatus {
        corpus_id: corpus.id.clone(),
        title: corpus.title.clone(),
        graph_path,
        manifest_path,
        graph_exists,
        manifest_exists,
        node_count,
        edge_count,
        built_at,
        manifest_git_sha,
        head_git_sha: head_git_sha.map(str::to_string),
        stale_reasons,
        warnings,
        is_fresh,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_stats_empty_graph() {
        let v = serde_json::json!({"nodes": [], "links": []});
        assert_eq!(graph_stats_from_json(&v), Some((0, 0)));
    }

    #[test]
    fn repo_graphify_cache_dir_under_dot_vox_cache() {
        let tmp = tempfile::tempdir().unwrap();
        let p = repo_graphify_cache_dir(tmp.path(), "repo-code-graph");
        let s = p.to_string_lossy();
        assert!(s.contains(".vox"));
        assert!(s.contains("graphify"));
        assert!(s.ends_with("repo-code-graph") || s.contains("repo-code-graph"));
    }
}
