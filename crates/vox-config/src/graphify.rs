//! Graphify corpus registry and freshness assessment (Tier D cache maps).
//!
//! SSOT contract: `contracts/retrieval/vox-graph-corpora.v1.yaml`

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

/// Relative path to the corpora registry YAML from repo root.
pub const CORPORA_REL_PATH: &str = "contracts/retrieval/vox-graph-corpora.v1.yaml";

/// One-release legacy path for corpora-registry back-compat (VG-1 G4).
const LEGACY_CORPORA_REL_PATH: &str = "contracts/retrieval/graphify-corpora.v1.yaml";

/// Runtime registration overlay (corpora created by `vox graphify index`).
pub const REGISTERED_REL_PATH: &str = ".vox/cache/vox-graph/registered.v1.json";

/// One-release legacy path for the registry overlay back-compat (VG-1 G3).
const LEGACY_REGISTERED_REL_PATH: &str = ".vox/cache/graphify/registered.v1.json";

/// Legacy graphify output directory (shared with non-graphify CI artifacts — see research doc).
pub const LEGACY_GRAPHIFY_OUT_DIR: &str = "graphify-out";

/// Basename for per-corpus manifest files written beside `graph.json`.
pub const MANIFEST_BASENAME: &str = ".graphify_manifest.v1.json";

/// Env var to override TTL in days for all graphify corpora.
pub const GRAPHIFY_TTL_DAYS_ENV: &str = "VOX_GRAPHIFY_TTL_DAYS";

/// Resolve the TTL (in days) using `VOX_GRAPHIFY_TTL_DAYS` if present, falling back to a default value.
pub fn resolve_ttl_days(default_ttl: u64) -> u64 {
    if let Ok(val) = std::env::var(GRAPHIFY_TTL_DAYS_ENV)
        && let Ok(parsed) = val.parse::<u64>()
    {
        return parsed;
    }
    default_ttl
}

/// True when `raw` (the value of `VOX_GRAPHIFY_TTL_DAYS`) is what `resolve_ttl_days`
/// will return — i.e. the env var, not the contract, is in control.
///
/// Keyed off presence, never off the resolved value differing from the contract:
/// setting the env var to the same number is still an active override.
/// Pure over the raw value so it is testable without mutating process-wide env.
#[must_use]
pub fn ttl_env_override_active(raw: Option<&str>) -> bool {
    raw.is_some_and(|v| v.parse::<u64>().is_ok())
}

/// [`ttl_env_override_active`] applied to the current process environment.
#[must_use]
pub fn ttl_env_override_active_now() -> bool {
    ttl_env_override_active(std::env::var(GRAPHIFY_TTL_DAYS_ENV).ok().as_deref())
}

/// Env var overriding the base dir under which per-corpus graphify cache
/// directories are composed (`<base>/<corpus_id>`), in place of the default
/// `<repo>/.vox/cache/graphify`. See [`repo_graphify_cache_dir`].
pub const GRAPHIFY_CACHE_DIR_ENV: &str = "VOX_GRAPHIFY_CACHE_DIR";

/// Env var disabling the graphify cache outright, regardless of
/// `VOX_GRAPHIFY_CACHE_DIR`. See [`graphify_disable_active`] for accepted values.
pub const GRAPHIFY_DISABLE_ENV: &str = "VOX_GRAPHIFY_DISABLE";

/// True when `raw` (the value of `VOX_GRAPHIFY_CACHE_DIR`) overrides the cache base
/// dir — i.e. [`repo_graphify_cache_dir`] uses it instead of the default
/// `<repo>/.vox/cache/graphify`.
///
/// [`ttl_env_override_active`]'s shape: keyed off presence of a non-blank value,
/// pure over the raw value so it is testable without mutating process-wide env.
#[must_use]
pub fn cache_dir_env_override_active(raw: Option<&str>) -> bool {
    raw.is_some_and(|v| !v.trim().is_empty())
}

/// [`cache_dir_env_override_active`] applied to the current process environment.
#[must_use]
pub fn cache_dir_env_override_active_now() -> bool {
    cache_dir_env_override_active(std::env::var(GRAPHIFY_CACHE_DIR_ENV).ok().as_deref())
}

/// True when `raw` (the value of `VOX_GRAPHIFY_DISABLE`) disables the graphify
/// cache. Blank/absent, `"0"`, `"false"`, `"no"`, and `"off"` (case-insensitive,
/// surrounding whitespace ignored) are treated as *not* disabling; every other
/// non-blank value disables — so a typo like `VOX_GRAPHIFY_DISABLE=1` or
/// `=please` still disables rather than being silently ignored.
#[must_use]
pub fn graphify_disable_active(raw: Option<&str>) -> bool {
    match raw.map(str::trim) {
        Some(v) if !v.is_empty() => !matches!(
            v.to_ascii_lowercase().as_str(),
            "0" | "false" | "no" | "off"
        ),
        _ => false,
    }
}

/// [`graphify_disable_active`] applied to the current process environment.
#[must_use]
pub fn graphify_disable_active_now() -> bool {
    graphify_disable_active(std::env::var(GRAPHIFY_DISABLE_ENV).ok().as_deref())
}

/// Where a corpus's on-disk graphify cache directory lives, or that the cache is
/// disabled outright.
///
/// Deliberately not a bare `PathBuf`: a disabled cache has no harmless path to
/// point at, and a plain path return would let a caller skip checking
/// `VOX_GRAPHIFY_DISABLE` and write anyway. Matching on this enum is required to
/// get a directory out, so the disable switch cannot be silently ignored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphifyCacheDir {
    /// The cache is enabled; reads/writes may use this directory.
    Enabled(PathBuf),
    /// `VOX_GRAPHIFY_DISABLE` is active — callers MUST NOT read or write any
    /// graphify cache for this corpus.
    Disabled,
}

impl GraphifyCacheDir {
    /// The directory, if the cache is enabled.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        match self {
            GraphifyCacheDir::Enabled(p) => Some(p.as_path()),
            GraphifyCacheDir::Disabled => None,
        }
    }

    /// True iff the cache is disabled.
    #[must_use]
    pub fn is_disabled(&self) -> bool {
        matches!(self, GraphifyCacheDir::Disabled)
    }
}

#[derive(Debug, Clone, Deserialize)]
struct CorporaFile {
    default_corpus_id: String,
    #[serde(default = "default_ttl_days")]
    ttl_days_default: u64,
    corpora: Vec<GraphifyCorpus>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
struct RegisteredCorporaFile {
    #[serde(default)]
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
    /// Absolute path to an external source repository to index. `None` = the Vox repo root.
    /// The graph is stored under the Vox repo's `.vox/cache/graphify/<id>/` regardless.
    #[serde(default)]
    pub source_root: Option<String>,
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
/// Repo-relative path of the corpora registry actually in use.
///
/// One-release back-compat (VG-1 G4): the legacy name is honoured when the new
/// one is absent. Readers AND writers must both go through this — resolving the
/// path in only one of them lets `set_ttl_days` write a different file from the
/// one `load_graphify_corpora` read.
pub fn corpora_rel_path(repo_root: &Path) -> &'static str {
    if repo_root.join(CORPORA_REL_PATH).exists() {
        CORPORA_REL_PATH
    } else if repo_root.join(LEGACY_CORPORA_REL_PATH).exists() {
        LEGACY_CORPORA_REL_PATH
    } else {
        CORPORA_REL_PATH
    }
}

pub fn load_graphify_corpora(repo_root: &Path) -> Result<GraphifyCorporaRegistry, GraphifyError> {
    let path = repo_root.join(corpora_rel_path(repo_root));
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

/// Accepted TTL range for the corpora registry, in days.
/// Zero would mark every corpus permanently stale; the upper bound is ten
/// years, past which the value is certainly a typo rather than an intent.
const TTL_DAYS_MIN: u64 = 1;
const TTL_DAYS_MAX: u64 = 3650;

/// Validate a TTL in days. Pure — callable from a command boundary before any
/// write, so an absurd value is rejected with a message rather than persisted
/// and discovered later.
pub fn validate_ttl_days(days: u64) -> Result<u64, String> {
    if (TTL_DAYS_MIN..=TTL_DAYS_MAX).contains(&days) {
        Ok(days)
    } else {
        Err(format!(
            "ttl_days must be between {TTL_DAYS_MIN} and {TTL_DAYS_MAX} (got {days})"
        ))
    }
}

/// Rewrite `ttl_days_default` in the corpora contract, leaving every other byte
/// of the file untouched.
///
/// This is a surgical single-line edit rather than a `serde_yaml` round-trip on
/// purpose: the contract is hand-authored with comments and a deliberate key
/// order, and reserializing it would strip both for a one-number change.
///
/// Errors if the key is absent, because `ttl_days_default` is serde-defaulted
/// and a missing key would otherwise make this a silent no-op.
pub fn set_ttl_days(repo_root: &Path, days: u64) -> std::io::Result<()> {
    // Same resolution the loader uses: writing CORPORA_REL_PATH unconditionally
    // would fail with a confusing NotFound on a checkout still on the legacy name.
    let path = repo_root.join(corpora_rel_path(repo_root));
    let raw = fs::read_to_string(&path)?;
    let mut found = false;
    let mut out = String::with_capacity(raw.len());
    for line in raw.split_inclusive('\n') {
        let body = line.strip_suffix('\n').unwrap_or(line);
        let body = body.strip_suffix('\r').unwrap_or(body);
        // Top-level key only (no leading whitespace), first occurrence only.
        if !found && body.starts_with("ttl_days_default:") {
            found = true;
            out.push_str(&format!("ttl_days_default: {days}"));
            // Preserve whatever line ending the file already uses.
            out.push_str(&line[body.len()..]);
        } else {
            out.push_str(line);
        }
    }
    if !found {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "ttl_days_default key not found in {}; refusing to write",
                path.display()
            ),
        ));
    }
    // Write-then-rename: `fs::write` truncates first, so an interruption would
    // leave the user's tracked contract empty or half-written.
    let tmp_path = path.with_extension("yaml.tmp");
    fs::write(&tmp_path, out)?;
    fs::rename(&tmp_path, &path)
}

/// First corpus id whose `default_for_intents` contains `intent`, if any.
/// Activates the otherwise-dormant intent-routing field.
pub fn select_corpus_for_intent(reg: &GraphifyCorporaRegistry, intent: &str) -> Option<String> {
    reg.corpora
        .iter()
        .find(|c| c.default_for_intents.iter().any(|i| i == intent))
        .map(|c| c.id.clone())
}

/// Load runtime-registered corpora (empty if the overlay file is absent/unparseable).
pub fn load_registered_corpora(repo_root: &Path) -> Vec<GraphifyCorpus> {
    let path = repo_root.join(REGISTERED_REL_PATH);
    // One-release back-compat (VG-1 G3): if the new overlay is absent, read the legacy path.
    let path = if path.exists() {
        path
    } else {
        let legacy = repo_root.join(LEGACY_REGISTERED_REL_PATH);
        if legacy.exists() { legacy } else { path }
    };
    let Ok(raw) = fs::read_to_string(&path) else {
        return Vec::new();
    };
    serde_json::from_str::<RegisteredCorporaFile>(&raw)
        .map(|f| f.corpora)
        .unwrap_or_default()
}

/// Insert-or-replace a corpus (by `id`) in the overlay.
pub fn upsert_registered_corpus(repo_root: &Path, corpus: &GraphifyCorpus) -> std::io::Result<()> {
    let path = repo_root.join(REGISTERED_REL_PATH);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut corpora = load_registered_corpora(repo_root);
    corpora.retain(|c| c.id != corpus.id);
    corpora.push(corpus.clone());
    let raw = serde_json::to_string_pretty(&RegisteredCorporaFile { corpora })
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    fs::write(path, raw)
}

/// Canonical YAML corpora + runtime-registered corpora. YAML wins id collisions.
pub fn load_all_corpora(repo_root: &Path) -> Result<GraphifyCorporaRegistry, GraphifyError> {
    let mut reg = load_graphify_corpora(repo_root)?;
    let existing: HashSet<String> = reg.corpora.iter().map(|c| c.id.clone()).collect();
    for c in load_registered_corpora(repo_root) {
        if !existing.contains(&c.id) {
            reg.corpora.push(c);
        }
    }
    Ok(reg)
}

/// Pure resolver behind [`repo_graphify_cache_dir`]: takes the raw `VOX_GRAPHIFY_DISABLE`
/// and `VOX_GRAPHIFY_CACHE_DIR` env values as arguments so tests need neither `unsafe`
/// env mutation (required under edition 2024) nor a real process environment.
///
/// `VOX_GRAPHIFY_DISABLE` wins outright; otherwise `VOX_GRAPHIFY_CACHE_DIR` overrides
/// the base dir (default `<repo>/.vox/cache/graphify`), with the `<corpus_id>` leaf
/// always preserved so corpora stay separated under either base.
#[must_use]
pub fn resolve_graphify_cache_dir(
    repo_root: &Path,
    corpus_id: &str,
    disable_raw: Option<&str>,
    cache_dir_raw: Option<&str>,
) -> GraphifyCacheDir {
    if graphify_disable_active(disable_raw) {
        return GraphifyCacheDir::Disabled;
    }
    let base = match cache_dir_raw {
        Some(v) if !v.trim().is_empty() => PathBuf::from(v),
        _ => repo_root
            .join(super::paths::REPO_CACHE_DIR)
            .join(super::paths::REPO_GRAPHIFY_CACHE_SUBDIR),
    };
    GraphifyCacheDir::Enabled(base.join(corpus_id))
}

/// Tier D cache dir for a named corpus: `<repo>/.vox/cache/graphify/<corpus_id>/` by
/// default, relocatable via `VOX_GRAPHIFY_CACHE_DIR` and disableable via
/// `VOX_GRAPHIFY_DISABLE` (see [`GraphifyCacheDir`] for why the disable switch is a
/// type, not a sentinel path).
#[must_use]
pub fn repo_graphify_cache_dir(repo_root: &Path, corpus_id: &str) -> GraphifyCacheDir {
    resolve_graphify_cache_dir(
        repo_root,
        corpus_id,
        std::env::var(GRAPHIFY_DISABLE_ENV).ok().as_deref(),
        std::env::var(GRAPHIFY_CACHE_DIR_ENV).ok().as_deref(),
    )
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
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
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

/// Write a manifest to disk (pretty JSON).
pub fn write_manifest(path: &Path, manifest: &GraphifyManifest) -> Result<(), GraphifyError> {
    let json = serde_json::to_string_pretty(manifest).map_err(|e| GraphifyError::Parse {
        path: path.to_path_buf(),
        detail: e.to_string(),
    })?;
    fs::write(path, json).map_err(|source| GraphifyError::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// Read-modify-write the manifest's `lexical_ingest_sha256` (creates a minimal manifest if absent).
pub fn set_lexical_ingest_sha256(manifest_path: &Path, sha: &str) -> Result<(), GraphifyError> {
    let mut manifest = read_manifest(manifest_path).unwrap_or_default();
    manifest.lexical_ingest_sha256 = Some(sha.to_string());
    write_manifest(manifest_path, &manifest)
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

/// Stale-reason string for uncommitted working-tree edits within a corpus scope.
pub const WORKTREE_DRIFT_REASON: &str = "worktree_drift";

/// Map a tri-state working-tree dirtiness signal to a stale reason.
///
/// - `Some(true)`  => the corpus scope has uncommitted changes => `Some("worktree_drift")`.
/// - `Some(false)` => scope is clean => `None`.
/// - `None`        => git could not be consulted (unknown) => `None` (never flips `is_fresh`
///   to false on a git failure alone; the caller records a non-fatal warning instead).
///
/// Pure and deterministic so it can be unit-tested without invoking git.
#[must_use]
pub fn worktree_drift_stale_reason(scope_dirty: Option<bool>) -> Option<String> {
    match scope_dirty {
        Some(true) => Some(WORKTREE_DRIFT_REASON.to_string()),
        _ => None,
    }
}

/// Decide whether `git status --porcelain` output indicates changes within `scope_path`.
///
/// `porcelain` is the raw stdout of `git status --porcelain -- <scope_path>` (already
/// scope-filtered by git) OR an unscoped `git status --porcelain` that we filter here.
/// A `scope_path` of `"."` (whole repo) treats any non-empty output as dirty.
///
/// Porcelain lines look like `" M crates/foo/bar.rs"` or `"?? new.rs"`; the path begins
/// at column 3. Rename lines (`R  old -> new`) are treated as dirty if either side falls
/// within scope. Pure and deterministic.
#[must_use]
pub fn porcelain_indicates_scope_dirty(porcelain: &str, scope_path: &str) -> bool {
    let scope = scope_path.trim_end_matches('/');
    let whole_repo = scope.is_empty() || scope == ".";
    for line in porcelain.lines() {
        if line.len() < 4 {
            continue;
        }
        // Strip the 2-char XY status + 1 space prefix.
        let rest = &line[3..];
        // Rename/copy entries carry "old -> new"; check both operands.
        let candidates: Vec<&str> = rest.split(" -> ").collect();
        for raw in candidates {
            let path = raw.trim().trim_matches('"');
            if path.is_empty() {
                continue;
            }
            if whole_repo {
                return true;
            }
            let norm = path.trim_start_matches("./");
            if norm == scope || norm.starts_with(&format!("{scope}/")) {
                return true;
            }
        }
    }
    false
}

/// Consult git for uncommitted changes within `scope_path` (read-only, resilient).
///
/// Returns `Some(true|false)` when git answers, or `None` when git is unavailable/errors
/// (caller treats `None` as "unknown" and must NOT mark the corpus stale on that basis).
fn scope_worktree_dirty(repo_root: &Path, scope_path: &str) -> Option<bool> {
    // Prefer a scope-filtered porcelain query; fall back to whole-repo + local filter.
    let scoped = vox_git::read_only(repo_root, &["status", "--porcelain", "--", scope_path]);
    let porcelain = match scoped {
        Ok(out) => out,
        Err(_) => match vox_git::read_only(repo_root, &["status", "--porcelain"]) {
            Ok(out) => out,
            Err(_) => return None,
        },
    };
    Some(porcelain_indicates_scope_dirty(&porcelain, scope_path))
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
        let stats = parse_res.as_ref().and_then(graph_stats_from_json);

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

    if let Some(ref m) = manifest
        && let Some(reason) = lexical_lag_stale_reason(m)
    {
        stale_reasons.push(reason);
    }

    // Working-tree-aware staleness (finding B2): uncommitted edits within the corpus
    // scope mean the on-disk graph no longer matches the source, even at the same HEAD.
    // Resilient: git failure => `None` => "unknown" => warn but never flip is_fresh.
    match scope_worktree_dirty(repo_root, &corpus.scope_path) {
        Some(true) => stale_reasons.push(WORKTREE_DRIFT_REASON.to_string()),
        Some(false) => {}
        None => warnings.push("worktree_status_unknown".into()),
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
    fn validate_ttl_days_rejects_zero_and_absurd() {
        assert_eq!(validate_ttl_days(30), Ok(30));
        assert_eq!(validate_ttl_days(1), Ok(1));
        assert_eq!(validate_ttl_days(3650), Ok(3650));
        assert!(validate_ttl_days(0).is_err());
        assert!(validate_ttl_days(3651).is_err());
    }

    #[test]
    fn set_ttl_days_rewrites_one_line_and_preserves_comments() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(CORPORA_REL_PATH);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        // Shape copied from the real contract, comments included.
        std::fs::write(
            &path,
            "x-vox-version: 1\nschema_version: 1\n\n# Named Graphify knowledge-graph corpora.\n# See docs/...\n\ndefault_corpus_id: repo-code-graph\nttl_days_default: 30\n\ncorpora:\n  - id: repo-code-graph\n    title: Repository code graph\n    scope_path: \".\"\n    graph_path: \"g\"\n    manifest_path: \"m\"\n",
        )
        .unwrap();

        set_ttl_days(tmp.path(), 7).unwrap();

        let after = std::fs::read_to_string(&path).unwrap();
        assert!(after.contains("ttl_days_default: 7"), "value not updated");
        assert!(
            !after.contains("ttl_days_default: 30"),
            "old value still present"
        );
        // Everything else must survive byte-for-byte.
        assert!(after.contains("# Named Graphify knowledge-graph corpora."));
        assert!(after.contains("# See docs/..."));
        assert!(after.contains("default_corpus_id: repo-code-graph"));
        assert!(after.contains("    title: Repository code graph"));
        // And the file must still parse.
        let reg = load_graphify_corpora(tmp.path()).unwrap();
        assert_eq!(reg.ttl_days_default, 7);
    }

    #[test]
    fn set_ttl_days_errors_when_key_absent() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(CORPORA_REL_PATH);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        // No ttl_days_default line: the key is serde-defaulted, so it can be missing.
        std::fs::write(
            &path,
            "x-vox-version: 1\ndefault_corpus_id: a\ncorpora: []\n",
        )
        .unwrap();
        let err = set_ttl_days(tmp.path(), 7)
            .expect_err("must not silently no-op when the key is absent");
        // Must be the refusal, not an incidental IO error on a missing fixture.
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
        assert!(err.to_string().contains("ttl_days_default key not found"));
        // And the file must be untouched.
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "x-vox-version: 1\ndefault_corpus_id: a\ncorpora: []\n"
        );
    }

    #[test]
    fn ttl_env_override_keys_off_presence_not_value() {
        // Present and parseable => active, even when it equals the contract value.
        assert!(ttl_env_override_active(Some("30")));
        assert!(ttl_env_override_active(Some("7")));
        // Absent => inactive.
        assert!(!ttl_env_override_active(None));
        // Present but unparseable => resolve_ttl_days ignores it, so not active.
        assert!(!ttl_env_override_active(Some("")));
        assert!(!ttl_env_override_active(Some("soon")));
    }

    #[test]
    fn graph_stats_empty_graph() {
        let v = serde_json::json!({"nodes": [], "links": []});
        assert_eq!(graph_stats_from_json(&v), Some((0, 0)));
    }

    #[test]
    fn repo_graphify_cache_dir_under_dot_vox_cache() {
        let tmp = tempfile::tempdir().unwrap();
        let status = repo_graphify_cache_dir(tmp.path(), "repo-code-graph");
        let p = status.path().expect("enabled by default").to_path_buf();
        let s = p.to_string_lossy();
        assert!(s.contains(".vox"));
        assert!(s.contains("graphify"));
        assert!(s.ends_with("repo-code-graph") || s.contains("repo-code-graph"));
    }

    #[test]
    fn cache_dir_env_override_replaces_base_but_keeps_corpus_leaf() {
        let tmp = tempfile::tempdir().unwrap();
        let status =
            resolve_graphify_cache_dir(tmp.path(), "repo-code-graph", None, Some("/mnt/vcache"));
        assert_eq!(
            status,
            GraphifyCacheDir::Enabled(PathBuf::from("/mnt/vcache/repo-code-graph"))
        );
    }

    #[test]
    fn cache_dir_env_blank_falls_back_to_default_base() {
        let tmp = tempfile::tempdir().unwrap();
        let status = resolve_graphify_cache_dir(tmp.path(), "repo-code-graph", None, Some("   "));
        assert_eq!(
            status,
            GraphifyCacheDir::Enabled(
                tmp.path()
                    .join(".vox")
                    .join("cache")
                    .join("graphify")
                    .join("repo-code-graph")
            )
        );
    }

    #[test]
    fn disable_wins_over_cache_dir_override() {
        let tmp = tempfile::tempdir().unwrap();
        let status = resolve_graphify_cache_dir(
            tmp.path(),
            "repo-code-graph",
            Some("1"),
            Some("/mnt/vcache"),
        );
        assert_eq!(status, GraphifyCacheDir::Disabled);
        assert!(status.is_disabled());
        assert_eq!(status.path(), None);
    }

    #[test]
    fn graphify_disable_active_recognizes_falsey_and_truthy_spellings() {
        // Falsey / absent => not disabled.
        assert!(!graphify_disable_active(None));
        assert!(!graphify_disable_active(Some("")));
        assert!(!graphify_disable_active(Some("   ")));
        assert!(!graphify_disable_active(Some("0")));
        assert!(!graphify_disable_active(Some("false")));
        assert!(!graphify_disable_active(Some("FALSE")));
        assert!(!graphify_disable_active(Some("no")));
        assert!(!graphify_disable_active(Some("off")));
        // Truthy / anything else non-blank => disabled, including typos.
        assert!(graphify_disable_active(Some("1")));
        assert!(graphify_disable_active(Some("true")));
        assert!(graphify_disable_active(Some("yes")));
        assert!(graphify_disable_active(Some("please")));
    }

    #[test]
    fn cache_dir_env_override_active_keys_off_presence() {
        assert!(!cache_dir_env_override_active(None));
        assert!(!cache_dir_env_override_active(Some("")));
        assert!(!cache_dir_env_override_active(Some("   ")));
        assert!(cache_dir_env_override_active(Some("/mnt/vcache")));
    }

    #[test]
    fn loads_from_vox_graph_corpora_path() {
        let tmp = tempfile::tempdir().unwrap();
        // Write to the NEW path only — must load without fallback.
        let new_path = tmp
            .path()
            .join("contracts/retrieval/vox-graph-corpora.v1.yaml");
        std::fs::create_dir_all(new_path.parent().unwrap()).unwrap();
        std::fs::write(
            &new_path,
            include_str!("../../../contracts/retrieval/vox-graph-corpora.v1.yaml"),
        )
        .unwrap();
        let result = load_graphify_corpora(tmp.path());
        assert!(result.is_ok(), "must load from new path: {result:?}");
    }

    #[test]
    fn falls_back_to_legacy_graphify_corpora() {
        let tmp = tempfile::tempdir().unwrap();
        // Write ONLY to the legacy path — fallback must find it.
        let legacy_path = tmp
            .path()
            .join("contracts/retrieval/graphify-corpora.v1.yaml");
        std::fs::create_dir_all(legacy_path.parent().unwrap()).unwrap();
        std::fs::write(
            &legacy_path,
            include_str!("../../../contracts/retrieval/vox-graph-corpora.v1.yaml"),
        )
        .unwrap();
        let result = load_graphify_corpora(tmp.path());
        assert!(
            result.is_ok(),
            "must fall back to legacy graphify-corpora.v1.yaml: {result:?}"
        );
    }

    #[test]
    fn registered_overlay_writes_new_vox_graph_path() {
        let tmp = tempfile::tempdir().unwrap();
        upsert_registered_corpus(tmp.path(), &sample_corpus("ext")).unwrap();
        // The overlay must be written at the new .vox/cache/vox-graph path.
        let new_overlay = tmp
            .path()
            .join(crate::paths::REPO_VOX_GRAPH_REGISTERED_FILE);
        assert!(new_overlay.exists(), "overlay must write to vox-graph path");
        let loaded = load_registered_corpora(tmp.path());
        assert!(loaded.iter().any(|c| c.id == "ext"));
    }

    #[test]
    fn registered_overlay_falls_back_to_legacy_path() {
        let tmp = tempfile::tempdir().unwrap();
        // Write ONLY the legacy overlay; the new path is absent.
        let legacy = tmp.path().join(crate::paths::REPO_GRAPHIFY_REGISTERED_FILE);
        std::fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        let body = serde_json::to_string_pretty(&RegisteredCorporaFile {
            corpora: vec![sample_corpus("legacy-ext")],
        })
        .unwrap();
        std::fs::write(&legacy, body).unwrap();
        let loaded = load_registered_corpora(tmp.path());
        assert!(
            loaded.iter().any(|c| c.id == "legacy-ext"),
            "must fall back to legacy registered.v1.json overlay"
        );
    }

    fn sample_corpus(id: &str) -> GraphifyCorpus {
        GraphifyCorpus {
            id: id.into(),
            title: "t".into(),
            scope_path: ".".into(),
            graph_path: format!(".vox/cache/graphify/{id}/graph.json"),
            manifest_path: format!(".vox/cache/graphify/{id}/.graphify_manifest.v1.json"),
            extraction_mode: Some("structural".into()),
            default_for_intents: vec![],
            is_virtual: false,
            source_root: Some(
                std::env::temp_dir()
                    .join("target")
                    .to_string_lossy()
                    .into_owned(),
            ),
        }
    }
    fn write_min_registry(repo: &std::path::Path) {
        let dir = repo.join("contracts/retrieval");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("vox-graph-corpora.v1.yaml"),
            "default_corpus_id: repo-code-graph\nttl_days_default: 30\ncorpora:\n  - id: repo-code-graph\n    title: Repo\n    scope_path: \".\"\n    graph_path: \"g\"\n    manifest_path: \"m\"\n"
        ).unwrap();
    }
    #[test]
    fn upsert_then_load_all_includes_registered() {
        let tmp = tempfile::tempdir().unwrap();
        write_min_registry(tmp.path());
        upsert_registered_corpus(tmp.path(), &sample_corpus("ext-a")).unwrap();
        let reg = load_all_corpora(tmp.path()).unwrap();
        assert!(reg.corpora.iter().any(|c| c.id == "ext-a"));
        assert!(reg.corpora.iter().any(|c| c.id == "repo-code-graph"));
    }
    #[test]
    fn upsert_idempotent_by_id() {
        let tmp = tempfile::tempdir().unwrap();
        write_min_registry(tmp.path());
        upsert_registered_corpus(tmp.path(), &sample_corpus("ext-a")).unwrap();
        upsert_registered_corpus(tmp.path(), &sample_corpus("ext-a")).unwrap();
        assert_eq!(
            load_registered_corpora(tmp.path())
                .iter()
                .filter(|c| c.id == "ext-a")
                .count(),
            1
        );
    }
    #[test]
    fn yaml_wins_id_collision() {
        let tmp = tempfile::tempdir().unwrap();
        write_min_registry(tmp.path());
        let mut collide = sample_corpus("repo-code-graph");
        collide.title = "HIJACKED".into();
        upsert_registered_corpus(tmp.path(), &collide).unwrap();
        let reg = load_all_corpora(tmp.path()).unwrap();
        let c = reg
            .corpora
            .iter()
            .find(|c| c.id == "repo-code-graph")
            .unwrap();
        assert_eq!(c.title, "Repo");
        assert_eq!(
            reg.corpora
                .iter()
                .filter(|c| c.id == "repo-code-graph")
                .count(),
            1
        );
    }

    #[test]
    fn lexical_stamp_clears_and_refires_lag() {
        let tmp = tempfile::tempdir().unwrap();
        let mpath = tmp.path().join(".graphify_manifest.v1.json");
        let m = GraphifyManifest {
            graph_json_sha256: Some("x".into()),
            ..Default::default()
        };
        write_manifest(&mpath, &m).unwrap();

        set_lexical_ingest_sha256(&mpath, "x").unwrap();
        let after = read_manifest(&mpath).unwrap();
        assert!(
            lexical_lag_stale_reason(&after).is_none(),
            "matched sha → no lag"
        );

        set_lexical_ingest_sha256(&mpath, "y").unwrap();
        let after2 = read_manifest(&mpath).unwrap();
        assert_eq!(
            lexical_lag_stale_reason(&after2).as_deref(),
            Some("lexical_lag")
        );
    }

    #[test]
    fn worktree_drift_reason_only_fires_on_dirty() {
        assert_eq!(
            worktree_drift_stale_reason(Some(true)).as_deref(),
            Some("worktree_drift")
        );
        // Clean and unknown both yield no stale reason (unknown must not flip is_fresh).
        assert_eq!(worktree_drift_stale_reason(Some(false)), None);
        assert_eq!(worktree_drift_stale_reason(None), None);
    }

    #[test]
    fn porcelain_scope_filtering() {
        let out = " M crates/vox-config/src/graphify.rs\n?? docs/new.md\n";
        // In-scope edit => dirty.
        assert!(porcelain_indicates_scope_dirty(out, "crates/vox-config"));
        // Out-of-scope edit only => clean for that scope.
        assert!(!porcelain_indicates_scope_dirty(out, "crates/vox-gui"));
        // Whole-repo scope => any output is dirty.
        assert!(porcelain_indicates_scope_dirty(out, "."));
        // Empty porcelain => clean regardless of scope.
        assert!(!porcelain_indicates_scope_dirty("", "."));
        assert!(!porcelain_indicates_scope_dirty("", "crates/vox-config"));
    }

    #[test]
    fn porcelain_rename_checks_both_operands() {
        let out = "R  crates/old/a.rs -> crates/vox-config/b.rs\n";
        assert!(porcelain_indicates_scope_dirty(out, "crates/vox-config"));
        assert!(porcelain_indicates_scope_dirty(out, "crates/old"));
        assert!(!porcelain_indicates_scope_dirty(out, "crates/unrelated"));
    }

    #[test]
    fn intent_routing_picks_first_matching_corpus() {
        // synthetic registry
        let mk = |id: &str, intents: &[&str]| GraphifyCorpus {
            id: id.into(),
            title: id.into(),
            scope_path: ".".into(),
            graph_path: "g".into(),
            manifest_path: "m".into(),
            extraction_mode: None,
            default_for_intents: intents.iter().map(|s| s.to_string()).collect(),
            is_virtual: false,
            source_root: None,
        };
        let reg = GraphifyCorporaRegistry {
            default_corpus_id: "a".into(),
            ttl_days_default: 30,
            corpora: vec![mk("a", &["code_navigation"]), mk("b", &["gui_surface"])],
        };
        assert_eq!(
            select_corpus_for_intent(&reg, "gui_surface").as_deref(),
            Some("b")
        );
        assert_eq!(
            select_corpus_for_intent(&reg, "code_navigation").as_deref(),
            Some("a")
        );
        assert_eq!(select_corpus_for_intent(&reg, "nonexistent"), None);
    }

    #[test]
    fn intent_routing_against_bundled_registry() {
        // Exercises the real contract data (repo-code-graph↔code_navigation, vox-gui-surface↔gui_surface).
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("contracts/retrieval");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("vox-graph-corpora.v1.yaml"),
            include_str!("../../../contracts/retrieval/vox-graph-corpora.v1.yaml"),
        )
        .unwrap();
        let reg = load_graphify_corpora(tmp.path()).unwrap();
        assert_eq!(
            select_corpus_for_intent(&reg, "code_navigation").as_deref(),
            Some("repo-code-graph")
        );
        assert_eq!(
            select_corpus_for_intent(&reg, "gui_surface").as_deref(),
            Some("vox-gui-surface")
        );
    }
}
