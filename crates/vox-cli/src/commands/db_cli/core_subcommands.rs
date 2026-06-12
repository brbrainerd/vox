//! Core `vox db` subcommands (schema, research, preferences, reliability).

use clap::Subcommand;
use std::path::PathBuf;

/// Core DB and research subcommands for `vox db`.
#[derive(Subcommand)]
pub enum DbCliCore {
    /// Print schema version and data directory
    Status,
    /// Print row counts, table inventory, and key PRAGMAs (read-only diagnostics)
    Audit {
        /// Include per-table MIN/MAX for a heuristic time column (extra queries)
        #[arg(long, default_value_t = false)]
        timestamps: bool,
    },
    /// Drop user tables and re-run migrations from a `.vox` module
    Reset {
        /// Optional specific schema source file.
        #[arg(long)]
        file: Option<PathBuf>,
    },
    /// Print schema digest for LLM context from a `.vox` file
    Schema {
        /// Source file defining compiler entities.
        #[arg(long)]
        file: Option<PathBuf>,
    },
    /// Print lowered DB query plans (`HirDbQueryPlan`) from a `.vox` file
    Explain {
        /// Source file defining query functions.
        #[arg(long)]
        file: Option<PathBuf>,
        /// Optional query function name filter.
        #[arg(long)]
        query: Option<String>,
        /// Emit compact JSON instead of pretty JSON (ignored with `--jsonl`).
        #[arg(long, default_value_t = false)]
        compact: bool,
        /// Emit one JSON object per query-plan row (newline-delimited JSON for piping).
        #[arg(long, default_value_t = false)]
        jsonl: bool,
    },
    /// Introspect live app-plane SQL schema via VOX_APP_DB_URL/VOX_DB_URL.
    Introspect {
        /// Optional explicit DB URL override.
        #[arg(long)]
        url: Option<String>,
        /// Emit compact JSON.
        #[arg(long, default_value_t = false)]
        compact: bool,
    },
    /// Verify declared `@table` surfaces against live app-plane SQL schema.
    Verify {
        /// Source file defining table declarations.
        #[arg(long)]
        file: Option<PathBuf>,
        /// Optional explicit DB URL override.
        #[arg(long)]
        url: Option<String>,
        /// Fail on unsupported-type mappings in addition to hard drift.
        #[arg(long, default_value_t = false)]
        strict: bool,
        /// Emit compact JSON.
        #[arg(long, default_value_t = false)]
        compact: bool,
    },
    /// Print sample rows from a table
    Sample {
        /// Target table name.
        #[arg(long)]
        table: String,
        /// Maximum number of rows to print.
        #[arg(long, default_value_t = 10)]
        limit: i64,
    },
    /// Apply schema migrations from declarations in a `.vox` file
    Migrate {
        /// Source file containing schema states.
        #[arg(long)]
        file: Option<PathBuf>,
        /// Optional app-plane SQL URL override (`postgres://`, `mysql://`, `libsql://`, `sqlite:`).
        #[arg(long)]
        url: Option<String>,
        /// Plan only; do not execute DDL.
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },
    /// Export preferences and memory for a user to JSON
    Export {
        /// User identifier to query.
        #[arg(long)]
        user_id: String,
        /// File to write JSON to.
        #[arg(long, short = 'o')]
        output: Option<PathBuf>,
    },
    /// Import from JSON produced by `export`
    Import {
        /// Source JSON file.
        #[arg(required = true)]
        path: PathBuf,
    },
    /// Run VACUUM on the local database
    Vacuum,
    /// Delete old `memories` rows for a user / agent id
    Prune {
        /// User identifier.
        #[arg(long)]
        user_id: String,
        /// Retain rows from the last N days.
        #[arg(long, default_value_t = 30)]
        days: u32,
    },
    /// Dry-run JSON for `contracts/db/retention-policy.yaml` (`days`, `ms_days`, `expires_lt_now`).
    ///
    /// SSOT: `contracts/db/retention-policy.yaml`; rationale in mdBook `telemetry-retention-sensitivity-ssot`.
    #[command(name = "prune-plan")]
    PrunePlan {
        /// Policy file (default: repo `contracts/db/retention-policy.yaml` next to vox-cli)
        #[arg(long)]
        policy: Option<PathBuf>,
    },
    /// Apply policy-driven deletes (`days`, `ms_days`, `expires_lt_now`) — destructive
    #[command(name = "prune-apply")]
    PruneApply {
        #[arg(long)]
        policy: Option<PathBuf>,
        /// Required acknowledgement flag for destructive deletes
        #[arg(long, default_value_t = false)]
        i_understand: bool,
    },
    /// Get one preference key
    #[command(name = "pref-get")]
    PrefGet {
        /// User identifier.
        #[arg(long)]
        user_id: String,
        /// Target preference key string.
        #[arg(long)]
        key: String,
    },
    /// Set one preference key
    #[command(name = "pref-set")]
    PrefSet {
        /// User identifier.
        #[arg(long)]
        user_id: String,
        /// String key to override.
        #[arg(long)]
        key: String,
        /// JSON-compatible string value.
        #[arg(long)]
        value: String,
    },
    /// List preferences (optional key prefix)
    #[command(name = "pref-list")]
    PrefList {
        /// User identifier.
        #[arg(long)]
        user_id: String,
        /// Optional substring prefix to match against keys.
        #[arg(long)]
        prefix: Option<String>,
    },
    /// List Codex MCP invocable bindings
    #[command(name = "capability-list")]
    CapabilityList,
    /// Sync invocables from a JSON file into Codex
    #[command(name = "sync-invocables")]
    SyncInvocables {
        /// Input JSON file path.
        #[arg(required = true)]
        path: PathBuf,
    },
    /// Show retrieval / embedding diagnostics
    #[command(name = "retrieval-status")]
    RetrievalStatus,
    /// Mirror a markdown tree into Codex `search_documents` / chunks (local RAG corpus).
    #[command(name = "mirror-search-corpus")]
    MirrorSearchCorpus {
        /// Root directory to scan recursively for `*.md` files.
        #[arg(long)]
        root: PathBuf,
        /// Prefix for `search_documents.source_uri` (e.g. `vox-docs:`).
        #[arg(long, default_value = "vox-docs:")]
        source_uri_prefix: String,
    },
    /// Ingest research from a URL
    #[command(name = "research-ingest-url")]
    ResearchIngestUrl {
        /// Namespace / vendor name.
        #[arg(long)]
        vendor: String,
        /// Logical research topic grouping.
        #[arg(long)]
        topic: String,
        /// Canonical HTTP URL.
        #[arg(long)]
        url: String,
        /// Optional display title.
        #[arg(long)]
        title: Option<String>,
        /// Short excerpt or AI summary.
        #[arg(long)]
        summary: Option<String>,
        /// Classification (e.g. `web`, `pdf`).
        #[arg(long, default_value = "web")]
        source_type: String,
        /// Agentic mapping sub-area.
        #[arg(long)]
        area: Option<String>,
        /// Upstream knowledge-base ID for diffing.
        #[arg(long)]
        kb_id: Option<String>,
        /// Search indexes and comma-separated context values.
        #[arg(long)]
        tags: Option<String>,
        /// Trust baseline rating (0.0 to 1.0).
        #[arg(long, default_value_t = 0.85)]
        confidence: f64,
    },
    /// Ingest a local markdown file as research
    #[command(name = "research-ingest-file")]
    ResearchIngestFile {
        /// Namespace / vendor name.
        #[arg(long)]
        vendor: String,
        /// Logical research topic grouping.
        #[arg(long)]
        topic: String,
        /// Path to local ingestible content.
        #[arg(required = true)]
        path: PathBuf,
        /// Agentic mapping sub-area.
        #[arg(long)]
        area: Option<String>,
        /// Upstream knowledge base ID.
        #[arg(long)]
        kb_id: Option<String>,
        /// Optional tagging dimensions.
        #[arg(long)]
        tags: Option<String>,
        /// Trust baseline (0.0 to 1.0).
        #[arg(long, default_value_t = 0.85)]
        confidence: f64,
    },
    /// Refresh bundled research sources (e.g. openclaw, context_engineering)
    #[command(name = "research-refresh")]
    ResearchRefresh {
        /// Specific vendor path to refresh.
        #[arg(long)]
        vendor: String,
        /// Avoid writing to DB if true.
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },
    /// List stored research packets
    #[command(name = "research-list")]
    ResearchList {
        /// Optional namespace/vendor filter.
        #[arg(long)]
        vendor: Option<String>,
        /// Optional specific topic filter.
        #[arg(long)]
        topic: Option<String>,
        /// Row limit for listing.
        #[arg(long, default_value_t = 50)]
        limit: i64,
    },
    /// Add one capability-map row
    #[command(name = "research-map-add")]
    ResearchMapAdd {
        /// Provider name (e.g. `openai`).
        #[arg(long)]
        vendor: String,
        /// Logical domain (e.g. `vision`).
        #[arg(long)]
        topic: String,
        /// Specialization area.
        #[arg(long)]
        area: String,
        /// OpenClaw capability identifier.
        #[arg(long)]
        openclaw_capability: String,
        /// Local test ID that demonstrates this feature.
        #[arg(long)]
        vox_evidence: String,
        /// Support level: `stable`, `beta`, `unsupported`.
        #[arg(long)]
        status: String,
        /// Indicates if Vox or the vendor currently handles this better.
        #[arg(long)]
        advantage_direction: String,
        /// Suggested action for the planner based on this map.
        #[arg(long)]
        recommended_action: String,
        /// Comma-separated list of test paths.
        #[arg(long)]
        linked_paths: Option<String>,
    },
    /// List capability-map rows
    #[command(name = "research-map-list")]
    ResearchMapList {
        /// Filter by vendor name.
        #[arg(long)]
        vendor: Option<String>,
        /// Filter by capability topic.
        #[arg(long)]
        topic: Option<String>,
        /// Query length limit.
        #[arg(long, default_value_t = 50)]
        limit: i64,
    },
    /// List research metrics for a session id (`research_metrics` — trust / sensitivity: `docs/src/architecture/telemetry-trust-ssot.md`).
    #[command(name = "research-metrics")]
    ResearchMetrics {
        /// `research_metrics.session_id` prefix (TEXT), e.g. `mcp:<repository_id>`, `bench:<repository_id>`, or a linked session key.
        #[arg(long)]
        session_id: String,
        /// When set, filter to this `metric_type` (exact match), e.g. `socrates_surface`, `benchmark_event`.
        #[arg(long)]
        metric_type: Option<String>,
    },
    /// List reliability scores for LLM endpoints, skills, workflows, or repositories
    #[command(name = "reliability-list")]
    ReliabilityList {
        /// Target domain: endpoints, skills, workflows, repositories, trust
        #[arg(long, default_value = "endpoints")]
        domain: String,
        /// Row limit for listing.
        #[arg(long, default_value_t = 50)]
        limit: i64,
    },
    /// List reliability scores and stats for execution agents
    #[command(name = "reliability-agents")]
    ReliabilityAgents {
        /// Row limit for listing.
        #[arg(long, default_value_t = 50)]
        limit: i64,
        /// Minimum reliability score (0.0 to 1.0)
        #[arg(long)]
        min_score: Option<f64>,
    },
    /// Query historical execution time for tools
    #[command(name = "exec-history")]
    ExecHistory {
        /// Optional tool key to filter by
        #[arg(long)]
        tool_key: Option<String>,
        /// Optional repository ID to filter by
        #[arg(long)]
        repo: Option<String>,
        /// Row limit for listing
        #[arg(long, default_value_t = 20)]
        limit: i64,
        /// Output raw JSON
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    /// List historical MENS training runs from `populi_training_run` (Row limit defaults to 20).
    #[command(name = "mens-runs")]
    MensRuns {
        /// Row limit for listing.
        #[arg(long, default_value_t = 20)]
        limit: u32,
    },
    /// List MENS-specific telemetry (entropy, diversity, sample counts) from `research_metrics` (limit 50).
    #[command(name = "mens-metrics")]
    MensMetrics {
        /// Optional domain filter (vox-lang, rust-expert, agents).
        #[arg(long)]
        domain: Option<String>,
        /// Row limit for listing.
        #[arg(long, default_value_t = 50)]
        limit: u32,
    },
    /// Show dynamic build health for a repository (slowest crates, cache hits, fingerprint drift).
    #[command(name = "build-health")]
    BuildHealth {
        /// Specific repository ID (defaults to current discovery).
        #[arg(long)]
        repo: Option<String>,
        /// Output raw JSON for scripting.
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    /// Show build-time regressions vs historical averages for a repository.
    #[command(name = "build-regressions")]
    BuildRegressions {
        /// Specific repository ID (defaults to current discovery).
        #[arg(long)]
        repo: Option<String>,
        /// Run ID to audit (defaults to latest).
        #[arg(long)]
        run_id: Option<i64>,
        /// Regression ratio threshold (e.g. 1.25 for 25% slower).
        #[arg(long, default_value_t = 1.5)]
        threshold: f64,
        /// Output raw JSON for scripting.
        #[arg(long, default_value_t = false)]
        json: bool,
    },
}
