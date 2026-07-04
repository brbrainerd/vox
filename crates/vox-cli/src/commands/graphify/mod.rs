//! `vox graphify` — corpus registry and freshness status (Tier D maps).

use anyhow::Context;
use chrono::Utc;
use clap::Subcommand;
use vox_config::graphify::{
    CorpusStatus, GraphifyCorporaRegistry, GraphifyCorpus, GraphifyError, GraphifyKnowledgeNode,
    GraphifyManifest, assess_corpus_status, load_all_corpora, load_graphify_corpora,
    project_graph_nodes_for_ingest, upsert_registered_corpus, write_manifest,
};

/// Returns the cache directory for a corpus: `<repo_root>/.vox/cache/vox-graph/<corpus_id>`.
///
/// Note: the Rebuild/Index/IngestAll paths do NOT call this — they write to the
/// authoritative registry `graph_path` (`output_file.parent()`). This helper is
/// only used by the crate-map ingest path.
fn primary_cache_dir(repo_root: &std::path::Path, corpus_id: &str) -> std::path::PathBuf {
    repo_root
        .join(vox_config::paths::REPO_VOX_GRAPH_CACHE_DIR)
        .join(corpus_id)
}

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
        /// Simulate cutting one dependency edge; print blast_s deltas as JSON.
        /// Analysis flags are mutually exclusive: stdout carries exactly ONE
        /// JSON document per invocation (AI-first contract).
        #[arg(long, value_name = "FROM:TO",
              conflicts_with_all = ["what_if_split", "top_cuts", "edges"])]
        what_if_cut: Option<String>,
        /// Simulate splitting CRATE by moving DEPS to a new leaf crate (JSON).
        #[arg(long, value_name = "CRATE=DEP1,DEP2",
              conflicts_with_all = ["top_cuts", "edges"])]
        what_if_split: Option<String>,
        /// Rank the N best single-edge cuts by total blast_s saved (JSON).
        /// workspace-hack targets are excluded (deliberate coupling).
        #[arg(long, value_name = "N", conflicts_with = "edges")]
        top_cuts: Option<usize>,
        /// Emit symbol-weighted dependency edges to graphify-out/edge_weights.json.
        #[arg(long)]
        edges: bool,
    },
    /// Classify why crates recompiled (cargo fingerprint-log analysis).
    /// Observes CHECK units: link-time-only pain is invisible here.
    WhyRebuilt {
        /// Parse this previously captured fingerprint log file.
        #[arg(long, conflicts_with = "capture")]
        log: Option<String>,
        /// Run `cargo check --workspace --exclude vox-gui` twice (second run
        /// instrumented) and analyze the second run. Check, not build: never
        /// relinks a running vox.exe.
        #[arg(long)]
        capture: bool,
        /// Write classification JSON here.
        #[arg(long, default_value = "graphify-out/rebuild_causes.json")]
        out: String,
    },
}

/// What an autonomous refresh should do for a corpus, given its stale reasons.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) enum RefreshAction {
    Rebuild,
    Ingest,
    Skip,
}

/// Advisory rebuild lock in `corpus_dir/refresh.lock`. Returns `Ok(Some(result))` when the
/// guarded closure ran, or `Ok(None)` when a *fresh* lock (mtime < 1h) is already held — so the
/// caller skips instead of racing concurrent writes to `graph.json`. An older/unreadable mtime is
/// treated as stale and reclaimed (self-heals after a `kill -9`/power loss). An RAII guard
/// releases the lock on normal return, on `?` early-return, AND on panic-unwind, so a crashed
/// rebuild does not wedge the corpus until the 1h reclaim. Advisory only: the check→write window
/// has a benign TOCTOU race, mitigated by the scheduler's `MultipleInstances IgnoreNew`.
pub(crate) fn with_graph_lock<T>(
    corpus_dir: &std::path::Path,
    f: impl FnOnce() -> anyhow::Result<T>,
) -> anyhow::Result<Option<T>> {
    let lock_path = corpus_dir.join("refresh.lock");
    if let Ok(meta) = std::fs::metadata(&lock_path) {
        let fresh = meta
            .modified()
            .ok()
            .and_then(|m| m.elapsed().ok())
            .map(|age| age < std::time::Duration::from_secs(3600))
            .unwrap_or(false);
        if fresh {
            return Ok(None);
        }
    }
    std::fs::create_dir_all(corpus_dir).ok();
    std::fs::write(&lock_path, chrono::Utc::now().to_rfc3339())?;

    // RAII: release on normal return, on `?` early-return, and on panic-unwind.
    struct LockGuard(std::path::PathBuf);
    impl Drop for LockGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let _guard = LockGuard(lock_path);

    f().map(Some)
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

/// Top-level subcommands of the feature-gated AI/ML groups (`mens`, `populi`,
/// `oratio`) that are compiled out of a default (non-`mens`/`gpu`/`oratio`) binary
/// and therefore absent from `build_catalog()`'s clap walk. Recovered by hand from
/// the `vox-ml-cli` subcommand enums so the structural index sees the full CLI tree
/// even in a default build:
/// - `mens`   ← `PopuliAction`   (`commands/mens/populi/action_populi_enum.rs`)
/// - `populi` ← `PopuliCli`      (`commands/populi_cli.rs`)
/// - `oratio` ← `OratioAction`   (`commands/oratio_cmd.rs`)
///
/// Listing the leaf NAMES here (rather than importing the enums) avoids a
/// `vox-cli → vox-ml-cli` build coupling in the default binary. The audit counts
/// (PopuliAction≈22, PopuliCli≈18, OratioAction≈9) are the acceptance check; the
/// `cli_catalog_json_includes_gated_*` test pins the group names + entry-count floor.
const GATED_CLI_SUBCOMMANDS: &[(&str, &[&str])] = &[
    (
        "mens",
        &[
            "pipeline",
            "train",
            "dogfood",
            "train-uv",
            "serve",
            "corpus",
            "probe",
            "status",
            "watch-telemetry",
            "models",
            "merge-qlora",
            "export-gguf",
            "generate",
            "review",
            "workflow",
            "check",
            "fix",
            "eval-local",
        ],
    ),
    (
        "populi",
        &[
            "init",
            "up",
            "down",
            "status",
            "registry-snapshot",
            "serve",
            "config",
            "admin",
            "node",
            "dispatch",
            "result",
            "stats",
            "pair",
            "federation",
            "corpus",
            "identity",
            "attest",
            "join",
        ],
    ),
    (
        "oratio",
        &[
            "transcribe",
            "listen",
            "record-transcribe",
            "doctor",
            "status",
            "eval",
            "eval-history",
            "subtitle",
            "serve",
        ],
    ),
];

/// Serialize the clap command catalog to JSON for `cli:` ingest, substituting the
/// gated-corrected `mens`/`populi`/`oratio` leaf rows so a default binary still
/// emits the full leaf set. Consumed by `vox_graph_reader::registry::cli_command_nodes`.
pub fn cli_catalog_json() -> String {
    use crate::command_catalog::{CatalogTier, CommandCatalog, CommandCatalogEntry, build_catalog};
    let mut catalog: CommandCatalog = build_catalog();
    // Index the leaf paths already present so synthetic rows never duplicate a
    // compiled-in gated subcommand (honesty: don't double-count).
    let existing: std::collections::HashSet<Vec<String>> =
        catalog.entries.iter().map(|e| e.path.clone()).collect();
    for (group, subs) in GATED_CLI_SUBCOMMANDS {
        for sub in *subs {
            let path = vec![(*group).to_string(), (*sub).to_string()];
            if existing.contains(&path) {
                continue;
            }
            catalog.entries.push(CommandCatalogEntry {
                command: format!("vox {group} {sub}"),
                about: "(feature-gated; recovered for structural ingest)".to_string(),
                aliases: Vec::new(),
                has_subcommands: false,
                compiled_in: false,
                source_group: (*group).to_string(),
                feature_gate: Some((*group).to_string()),
                path,
                tier: CatalogTier::FeatureGated,
                capability_id: None,
                arguments: Vec::new(),
            });
        }
    }
    serde_json::to_string(&catalog).unwrap_or_else(|_| "{\"entries\":[]}".to_string())
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

/// Compute the GUI content-manifest emit inputs for a rebuild.
///
/// Returns `(gui_source_dir, surface_registry_yaml)` — both `Some` only when the corpus is in
/// `gui-wiring` extraction mode (so `emit_content_manifest` runs only for the GUI surface graph).
/// `gui_source_dir` is the `ui/src/` root under the corpus source dir (where surface component
/// modules referenced by the graph's `surface:→module:` edges live); `surface_registry_yaml` is
/// the contents of `contracts/gui/surface-registry.v1.yaml`. Outside gui-wiring mode both are
/// `None` and `rebuild_graph` skips the content-manifest emit.
fn gui_manifest_inputs(
    repo_root: &std::path::Path,
    extraction_mode: Option<&str>,
    source_dir: &std::path::Path,
) -> (Option<std::path::PathBuf>, Option<String>) {
    if extraction_mode != Some("gui-wiring") {
        return (None, None);
    }
    let gui_source_dir = Some(source_dir.join("ui/src"));
    let surface_registry_yaml =
        std::fs::read_to_string(repo_root.join("contracts/gui/surface-registry.v1.yaml")).ok();
    (gui_source_dir, surface_registry_yaml)
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
    let digest = vox_graph_reader::graph_digest(&graph_bytes);
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

/// `{crates:{name:[deps]}}` -> adjacency map (shared by the analysis flags).
fn adj_from_crate_graph(
    crate_graph: &serde_json::Value,
) -> std::collections::HashMap<String, Vec<String>> {
    let mut adj = std::collections::HashMap::new();
    if let Some(m) = crate_graph.get("crates").and_then(|v| v.as_object()) {
        for (c, ds) in m {
            let deps = ds
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            adj.insert(c.clone(), deps);
        }
    }
    adj
}

/// crate_audit.json rows -> crate name -> compile seconds (string or number).
fn times_from_audit(audit: &serde_json::Value) -> std::collections::HashMap<String, f64> {
    let mut out = std::collections::HashMap::new();
    if let Some(arr) = audit.as_array() {
        for r in arr {
            if let (Some(name), Some(cs)) = (
                r.get("crate").and_then(|v| v.as_str()),
                r.get("compile_s").and_then(|v| {
                    v.as_f64()
                        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
                }),
            ) {
                out.insert(name.to_string(), cs);
            }
        }
    }
    out
}

/// Parse `--what-if-split` spec "crate=d1,d2" -> (crate, deps).
fn parse_split_spec(spec: &str) -> anyhow::Result<(String, Vec<String>)> {
    let (krate, deps) = spec
        .split_once('=')
        .ok_or_else(|| anyhow::anyhow!("expected CRATE=DEP1,DEP2, got '{spec}'"))?;
    let deps: Vec<String> = deps
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect();
    if krate.trim().is_empty() || deps.is_empty() {
        anyhow::bail!("expected CRATE=DEP1,DEP2, got '{spec}'");
    }
    Ok((krate.trim().to_string(), deps))
}

/// Atomic write: temp file in the same dir, then rename over the target.
fn write_atomic(path: &std::path::Path, contents: &str) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create dir {}", parent.display()))?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, contents).with_context(|| format!("write {}", tmp.display()))?;
    std::fs::rename(&tmp, path).with_context(|| format!("rename to {}", path.display()))?;
    Ok(())
}

/// AI-first artifact envelope: schema_version + provenance around a result.
fn with_provenance(generated_by: &str, result: serde_json::Value) -> serde_json::Value {
    let git_sha = resolve_head_sha().ok().flatten();
    serde_json::json!({
        "schema_version": 1,
        "provenance": { "generated_by": generated_by, "git_sha": git_sha },
        "result": result,
    })
}

/// Run `cargo check` twice; the second run has fingerprint tracing enabled and
/// its stderr is returned (and saved to graphify-out/rebuild_fingerprint.log).
/// An idle second run SHOULD be a no-op: every dirty line it emits is a
/// rebuild-hygiene finding.
fn capture_fingerprint_log(repo_root: &std::path::Path) -> anyhow::Result<String> {
    let check_args = ["check", "--workspace", "--exclude", "vox-gui"];
    eprintln!("why-rebuilt: warm-up cargo check (this may take a while)...");
    let warm = std::process::Command::new("cargo")
        .current_dir(repo_root)
        .args(check_args)
        .status()
        .context("spawn warm-up cargo check")?;
    if !warm.success() {
        anyhow::bail!("warm-up cargo check failed — fix the build first");
    }
    eprintln!("why-rebuilt: instrumented cargo check...");
    let out = std::process::Command::new("cargo")
        .current_dir(repo_root)
        .args(check_args)
        .env("CARGO_LOG", "cargo::core::compiler::fingerprint=info")
        .output()
        .context("spawn instrumented cargo check")?;
    if !out.status.success() {
        // The warm-up run streams live via .status(); this run captures
        // silently via .output() to feed the classifier — so on failure the
        // diagnostics never reached the terminal. Print them now or the user
        // has no idea what broke.
        eprint!("{}", String::from_utf8_lossy(&out.stderr));
        anyhow::bail!("instrumented cargo check failed (see output above) — fix the build first");
    }
    let log_text = String::from_utf8_lossy(&out.stderr).to_string();
    let log_path = repo_root.join("graphify-out/rebuild_fingerprint.log");
    write_atomic(&log_path, &log_text)?;
    eprintln!("why-rebuilt: raw log -> {}", log_path.display());
    Ok(log_text)
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
            let (gui_source_dir, surface_registry_yaml) =
                gui_manifest_inputs(repo_root, corpus.extraction_mode.as_deref(), &source_dir);
            let meta = vox_graph_reader::rebuild::RebuildMeta {
                corpus_id: corpus_id.clone(),
                git_sha: resolve_head_sha()?,
                scope_path: corpus.scope_path.clone(),
                extraction_mode: corpus.extraction_mode.clone(),
                built_at_rfc3339: Utc::now().to_rfc3339(),
                cli_catalog_json: Some(cli_catalog_json()),
                gui_source_dir,
                surface_registry_yaml,
            };
            // Preserve the previous graph as a bounded history before overwriting.
            if output_file.is_file() {
                if let Some(corpus_dir) = output_file.parent() {
                    let stamp = Utc::now().to_rfc3339().replace(':', "-");
                    let _ = vox_graph_reader::snapshot::snapshot_corpus(corpus_dir, &stamp);
                    let _ = vox_graph_reader::snapshot::prune_snapshots(corpus_dir, 5);
                }
            }
            let corpus_dir = output_file
                .parent()
                .ok_or_else(|| anyhow::anyhow!("graph_path has no parent"))?
                .to_path_buf();
            let ran = with_graph_lock(&corpus_dir, || {
                vox_graph_reader::rebuild::rebuild_graph(
                    repo_root,
                    &source_dir,
                    &output_file,
                    &cache_dir,
                    &meta,
                )
                .map_err(|e| anyhow::anyhow!("Rebuild failed: {}", e))
            })?;
            match ran {
                Some(()) => println!("Graphify rebuild successful!"),
                None => println!("Rebuild skipped: another rebuild is in progress (lock held)."),
            }
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

            let report = vox_graph_reader::coverage::compute_coverage(&graph, &kind);
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
                graph_path: format!(".vox/cache/vox-graph/{corpus_id}/graph.json"),
                manifest_path: format!(
                    ".vox/cache/vox-graph/{corpus_id}/.graphify_manifest.v1.json"
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
            let (gui_source_dir, surface_registry_yaml) =
                gui_manifest_inputs(repo_root, corpus.extraction_mode.as_deref(), &source_dir);
            let meta = vox_graph_reader::rebuild::RebuildMeta {
                corpus_id: corpus_id.clone(),
                git_sha: resolve_head_sha_in(&abs).ok().flatten(),
                scope_path: corpus.scope_path.clone(),
                extraction_mode: corpus.extraction_mode.clone(),
                built_at_rfc3339: Utc::now().to_rfc3339(),
                cli_catalog_json: Some(cli_catalog_json()),
                gui_source_dir,
                surface_registry_yaml,
            };
            println!("Indexing '{}' as corpus '{}'...", abs.display(), corpus_id);
            vox_graph_reader::rebuild::rebuild_graph(
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
                        let (gui_source_dir, surface_registry_yaml) = gui_manifest_inputs(
                            repo_root,
                            c.extraction_mode.as_deref(),
                            &source_dir,
                        );
                        let meta = vox_graph_reader::rebuild::RebuildMeta {
                            corpus_id: c.id.clone(),
                            git_sha: head.clone(),
                            scope_path: c.scope_path.clone(),
                            extraction_mode: c.extraction_mode.clone(),
                            built_at_rfc3339: Utc::now().to_rfc3339(),
                            cli_catalog_json: Some(cli_catalog_json()),
                            gui_source_dir,
                            surface_registry_yaml,
                        };
                        let corpus_dir = output_file
                            .parent()
                            .ok_or_else(|| anyhow::anyhow!("graph_path has no parent"))?
                            .to_path_buf();
                        let ran = with_graph_lock(&corpus_dir, || {
                            vox_graph_reader::rebuild::rebuild_graph(
                                repo_root,
                                &source_dir,
                                &output_file,
                                &cache_dir,
                                &meta,
                            )
                            .map_err(|e| anyhow::anyhow!("refresh rebuild {}: {e}", c.id))
                        })?;
                        match ran {
                            Some(()) => println!("  rebuilt {}", c.id),
                            None => println!("  skipped {} (rebuild lock held)", c.id),
                        }
                    }
                    RefreshAction::Ingest => {
                        let nodes = load_projected_nodes(repo_root, &reg, &c.id)?;
                        let upserted = upsert_projected_nodes(&nodes).await?;
                        let graph_bytes = std::fs::read(repo_root.join(&c.graph_path))
                            .with_context(|| format!("read graph for digest: {}", c.graph_path))?;
                        vox_config::graphify::set_lexical_ingest_sha256(
                            &repo_root.join(&c.manifest_path),
                            &vox_graph_reader::graph_digest(&graph_bytes),
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
                    let removed = vox_graph_reader::snapshot::prune_snapshots(corpus_dir, keep)
                        .map_err(|e| anyhow::anyhow!("prune {}: {e}", c.id))?;
                    println!("gc {} kept<= {keep} removed={removed}", c.id);
                }
            }
        }
        GraphifyCmd::CrateMap {
            no_refresh_graph,
            write_summary,
            ingest,
            what_if_cut,
            what_if_split,
            top_cuts,
            edges,
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
                    eprintln!(
                        "note: {} absent — building count-only map (run scripts/crate-build-audit.vox for compile times)",
                        audit_path.display()
                    );
                    serde_json::json!([])
                }
            };

            // Analysis flags: run the requested analysis and return early —
            // read-only questions; they skip persisting the map/manifest.
            // stdout = exactly one JSON document; warnings -> stderr.
            let analysis_requested =
                what_if_cut.is_some() || what_if_split.is_some() || top_cuts.is_some() || edges;
            if analysis_requested {
                if write_summary.is_some() || ingest {
                    eprintln!(
                        "note: --write-summary/--ingest are ignored during an analysis-only run \
                         (--what-if-cut/--what-if-split/--top-cuts/--edges)"
                    );
                }
                let adj = adj_from_crate_graph(&crate_graph);
                let times = times_from_audit(&audit);
                if times.is_empty() {
                    eprintln!(
                        "WARNING: no compile times — deltas are dependents-only (blast_s=0). \
                         Run scripts/crate-build-audit.vox first."
                    );
                }
                if let Some(spec) = &what_if_cut {
                    let (from, to) = spec
                        .split_once(':')
                        .ok_or_else(|| anyhow::anyhow!("expected FROM:TO, got '{spec}'"))?;
                    if from.is_empty() || to.is_empty() {
                        anyhow::bail!("expected FROM:TO with non-empty crate names, got '{spec}'");
                    }
                    let d = vox_graph_reader::what_if::what_if_cut(&adj, &times, from, to)
                        .map_err(|e| anyhow::anyhow!(e))?;
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&with_provenance(
                            &format!("vox graphify crate-map --what-if-cut {spec}"),
                            serde_json::to_value(&d)?
                        ))?
                    );
                }
                if let Some(spec) = &what_if_split {
                    let (krate, moved) = parse_split_spec(spec)?;
                    let d = vox_graph_reader::what_if::what_if_split(&adj, &times, &krate, &moved)
                        .map_err(|e| anyhow::anyhow!(e))?;
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&with_provenance(
                            &format!("vox graphify crate-map --what-if-split {spec}"),
                            serde_json::to_value(&d)?
                        ))?
                    );
                }
                if let Some(n) = top_cuts {
                    let exclude: Vec<String> =
                        vox_graph_reader::what_if::DEFAULT_EXCLUDED_CUT_TARGETS
                            .iter()
                            .map(|s| s.to_string())
                            .collect();
                    let cuts = vox_graph_reader::what_if::top_cuts(&adj, &times, n, &exclude);
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&with_provenance(
                            &format!("vox graphify crate-map --top-cuts {n}"),
                            serde_json::to_value(&cuts)?
                        ))?
                    );
                }
                if edges {
                    // Symbol corpus: repo-code-graph, registry-resolved (native schema).
                    let reg =
                        load_all_corpora(repo_root).map_err(|e| anyhow::anyhow!(e.to_string()))?;
                    let corpus = corpus_by_id(&reg, "repo-code-graph")
                        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
                    let sg_path = repo_root.join(&corpus.graph_path);
                    if !sg_path.is_file() {
                        anyhow::bail!(
                            "repo-code-graph corpus not built yet ({} missing). Run: \
                             vox graphify rebuild --corpus repo-code-graph",
                            sg_path.display()
                        );
                    }
                    let sg: serde_json::Value = serde_json::from_str(
                        &std::fs::read_to_string(&sg_path)
                            .with_context(|| format!("read symbol corpus {}", sg_path.display()))?,
                    )?;
                    // Note: --edges emits a distinct shape (no "result" wrapper) because
                    // this JSON is also the file written to disk at edge_weights.json —
                    // it must be self-describing on its own, not just as a CLI response.
                    let mut out = vox_graph_reader::edge_weights::weigh_edges(&sg, &adj, &times);
                    out["provenance"] = serde_json::json!({
                        "generated_by": "vox graphify crate-map --edges",
                        "git_sha": resolve_head_sha().ok().flatten(),
                        "corpus_path": corpus.graph_path,
                    });
                    let out_path = repo_root.join("graphify-out/edge_weights.json");
                    write_atomic(&out_path, &serde_json::to_string_pretty(&out)?)?;
                    eprintln!(
                        "edge weights -> {} ({} edges, {} candidates; corpus partial — candidates only)",
                        out_path.display(),
                        out["edges"].as_array().map(|a| a.len()).unwrap_or(0),
                        out["meta"]["candidate_count"]
                    );
                    println!("{}", serde_json::to_string_pretty(&out)?);
                }
                return Ok(());
            }

            // 3. Build + persist.
            let map = vox_graph_reader::crate_model::build_crate_map(&crate_graph, &audit);
            let out_dir = primary_cache_dir(repo_root, "crate-map");
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
                graph_json_sha256: Some(vox_graph_reader::graph_digest(bytes.as_bytes())),
                extraction_mode: Some("crate-map".to_string()),
                lexical_ingest_sha256: None,
            };
            write_manifest(&out_dir.join(".graphify_manifest.v1.json"), &manifest)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            println!(
                "crate-map: {node_count} crates, {edge_count} edges -> .vox/cache/vox-graph/crate-map/graph.json"
            );
            println!("persist for agent recall: vox graphify ingest --corpus crate-map");

            // 4. Optionally emit the committed gate SSOT (small; parity-checked in CI).
            if let Some(summary_path) = write_summary {
                let compile_times = times_from_audit(&audit);
                let summary = vox_graph_reader::crate_model::build_crate_summary(
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
        GraphifyCmd::WhyRebuilt { log, capture, out } => {
            let generated_by = if capture {
                "vox graphify why-rebuilt --capture".to_string()
            } else {
                format!(
                    "vox graphify why-rebuilt --log {}",
                    log.as_deref().unwrap_or("")
                )
            };
            let log_text = if capture {
                capture_fingerprint_log(repo_root)?
            } else {
                let path = log.ok_or_else(|| anyhow::anyhow!("pass --log <file> or --capture"))?;
                std::fs::read_to_string(repo_root.join(&path))
                    .with_context(|| format!("read log {path}"))?
            };
            let causes = vox_graph_reader::rebuild_causes::parse_fingerprint_log(&log_text);
            let summary = vox_graph_reader::rebuild_causes::summarize(&causes);
            let per = vox_graph_reader::rebuild_causes::per_crate(&causes);

            if causes.is_empty() {
                eprintln!(
                    "why-rebuilt: no fingerprint-dirty lines — nothing recompiled (clean) \
                     or the log lacks CARGO_LOG fingerprint tracing."
                );
            }
            let payload = with_provenance(
                &generated_by,
                serde_json::json!({
                    "summary": summary, "per_crate": per, "causes": causes,
                    "limitations": [
                        "observes check units; link-time-only pain invisible",
                        "per_crate keeps first specific cause; full counts in summary"
                    ],
                }),
            );
            write_atomic(
                &repo_root.join(&out),
                &serde_json::to_string_pretty(&payload)?,
            )?;
            // Never guess: a high unknown rate means cargo's log shape moved.
            // Gated on the PER-CRATE rate, not summary.unknown_rate (line
            // level): every dirty target unavoidably emits one reason-less
            // header line, so the line-level rate is structurally inflated
            // even when every crate resolved correctly via its detail line
            // (measured 2026-07-02: a real capture where every crate
            // resolved still showed 45% at the line level).
            let crate_unknown_rate = vox_graph_reader::rebuild_causes::per_crate_unknown_rate(&per);
            if !per.is_empty() && crate_unknown_rate > 0.2 {
                anyhow::bail!(
                    "{:.0}% of crates never resolved to a specific cause (exceeds 20%) — \
                     cargo's fingerprint log format likely changed; extend \
                     rebuild_causes::classify from the raw lines preserved in {} and add \
                     them to the fixture",
                    crate_unknown_rate * 100.0,
                    out
                );
            }
            eprintln!(
                "why-rebuilt: {} dirty lines across {} crates -> {}",
                summary.total,
                per.len(),
                out
            );
            for (class, count) in &summary.counts {
                eprintln!("  {class:<20} {count}");
            }
            let hygiene: Vec<&String> = per
                .iter()
                .filter(|(_, c)| {
                    !matches!(
                        c,
                        vox_graph_reader::rebuild_causes::CauseClass::FileDirty
                            | vox_graph_reader::rebuild_causes::CauseClass::DepRebuilt
                    )
                })
                .map(|(k, _)| k)
                .collect();
            if !hygiene.is_empty() {
                eprintln!(
                    "HYGIENE FINDINGS (recompiled without source changes): {}",
                    hygiene
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
            println!("{}", serde_json::to_string_pretty(&payload)?);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    #[test]
    fn parse_split_spec_shapes() {
        assert_eq!(
            parse_split_spec("vox-cli=vox-db,vox-forge").unwrap(),
            (
                "vox-cli".to_string(),
                vec!["vox-db".to_string(), "vox-forge".to_string()]
            )
        );
        assert!(parse_split_spec("vox-cli").is_err());
        assert!(parse_split_spec("=a").is_err());
        assert!(parse_split_spec("a=").is_err());
    }

    #[test]
    fn adj_and_times_extractors() {
        let g = serde_json::json!({"crates": {"a": ["b"], "b": []}});
        let adj = adj_from_crate_graph(&g);
        assert_eq!(adj.get("a").unwrap(), &vec!["b".to_string()]);
        let audit = serde_json::json!([
            {"crate": "a", "compile_s": "1.5"},
            {"crate": "b", "compile_s": 2.5}
        ]);
        let t = times_from_audit(&audit);
        assert_eq!(t.get("a"), Some(&1.5));
        assert_eq!(t.get("b"), Some(&2.5));
    }

    fn write_registry(repo: &Path) {
        let dir = repo.join("contracts/retrieval");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("vox-graph-corpora.v1.yaml"),
            include_str!("../../../../../contracts/retrieval/vox-graph-corpora.v1.yaml"),
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
        let graph_dir = tmp
            .path()
            .join(vox_config::paths::REPO_GRAPHIFY_REPO_CODE_GRAPH_DIR);
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
        let graph_dir = tmp
            .path()
            .join(vox_config::paths::REPO_GRAPHIFY_REPO_CODE_GRAPH_DIR);
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
            graph_path: vox_config::paths::REPO_GRAPHIFY_EXT_GRAPH_FILE.into(),
            manifest_path: vox_config::paths::REPO_GRAPHIFY_EXT_MANIFEST_FILE.into(),
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

#[cfg(test)]
mod vg1_cache_path_tests {
    use super::*;

    #[test]
    fn new_cache_path_is_vox_graph() {
        let tmp = tempfile::tempdir().unwrap();
        let corpus_id = "repo-code-graph";
        // The primary path must be .vox/cache/vox-graph/<corpus_id>
        let expected = tmp
            .path()
            .join(vox_config::paths::REPO_VOX_GRAPH_CACHE_DIR)
            .join(corpus_id);
        let actual = primary_cache_dir(tmp.path(), corpus_id);
        assert_eq!(actual, expected);
    }
}

#[cfg(test)]
mod lock_tests {
    use super::{RefreshAction, refresh_action, with_graph_lock};

    #[test]
    fn refresh_action_skips_worktree_drift_only() {
        assert_eq!(
            refresh_action(&["worktree_drift".into()]),
            RefreshAction::Skip
        );
        assert_eq!(
            refresh_action(&["git_drift".into()]),
            RefreshAction::Rebuild
        );
        assert_eq!(
            refresh_action(&["worktree_drift".into(), "git_drift".into()]),
            RefreshAction::Rebuild
        );
        assert_eq!(
            refresh_action(&["lexical_lag".into()]),
            RefreshAction::Ingest
        );
    }

    #[test]
    fn lock_runs_and_releases_when_free() {
        let tmp = tempfile::tempdir().unwrap();
        let r = with_graph_lock(tmp.path(), || Ok(42)).unwrap();
        assert_eq!(r, Some(42));
        assert!(
            !tmp.path().join("refresh.lock").exists(),
            "lock released after run"
        );
    }

    #[test]
    fn lock_skips_when_fresh_lock_held() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("refresh.lock"), "held").unwrap(); // mtime = now
        let mut ran = false;
        let r = with_graph_lock(tmp.path(), || {
            ran = true;
            Ok(())
        })
        .unwrap();
        assert!(r.is_none(), "must skip when a fresh lock is held");
        assert!(!ran, "guarded closure must not run while locked");
    }
}
