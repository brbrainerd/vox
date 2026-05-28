//! Scientia subcommand definitions.

use crate::constants::*;
use crate::db_types::*;
use clap::Subcommand;

/// Subcommands for `vox scientia`.
#[derive(Subcommand, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum ScientiaCmd {
    /// Validate a finding-candidate JSON document.
    #[command(name = "finding-candidate-validate")]
    FindingCandidateValidate {
        /// Path to JSON instance.
        #[arg(long)]
        json: std::path::PathBuf,
    },
    /// Validate a novelty-evidence-bundle JSON document.
    #[command(name = "novelty-evidence-bundle-validate")]
    NoveltyEvidenceBundleValidate {
        /// Path to JSON instance.
        #[arg(long)]
        json: std::path::PathBuf,
    },
    /// List Codex MCP invocable bindings.
    #[command(name = "capability-list")]
    CapabilityList,
    /// List stored research packets.
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
    /// List capability-map rows.
    #[command(name = "research-map-list")]
    ResearchMapList {
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
    /// Retrieval / embedding diagnostics.
    #[command(name = "retrieval-status")]
    RetrievalStatus,
    /// Mirror markdown into Codex search corpus.
    #[command(name = "mirror-search-corpus")]
    MirrorSearchCorpus {
        /// Root directory to scan recursively for `*.md` files.
        #[arg(long)]
        root: std::path::PathBuf,
        /// Prefix for `search_documents.source_uri`.
        #[arg(long, default_value = "vox-docs:")]
        source_uri_prefix: String,
    },
    /// Refresh bundled research sources.
    #[command(name = "research-refresh")]
    ResearchRefresh {
        /// Specific vendor/provider path to refresh.
        #[arg(long)]
        vendor: String,
        /// Only check sync status without executing the refresh.
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },
    /// Prepare a scientific publication manifest from markdown.
    #[command(name = "publication-prepare")]
    PublicationPrepare {
        #[command(flatten)]
        body: PublicationPrepareBodyCli,
        #[arg(long, default_value_t = false)]
        preflight: bool,
        #[arg(long, value_enum, default_value_t = DbPreflightProfileCli::Default)]
        preflight_profile: DbPreflightProfileCli,
        #[arg(long, value_enum, default_value_t = DiscoveryIntakeGateCli::None)]
        discovery_intake_gate: DiscoveryIntakeGateCli,
    },
    /// Same as `publication-prepare` with mandatory preflight.
    #[command(name = "publication-prepare-validated")]
    PublicationPrepareValidated {
        #[command(flatten)]
        body: PublicationPrepareBodyCli,
        #[arg(long, value_enum, default_value_t = DbPreflightProfileCli::Default)]
        preflight_profile: DbPreflightProfileCli,
        #[arg(long, value_enum, default_value_t = DiscoveryIntakeGateCli::None)]
        discovery_intake_gate: DiscoveryIntakeGateCli,
    },
    /// JSON preflight report for an existing publication id.
    #[command(name = "publication-preflight")]
    PublicationPreflight {
        #[arg(long)]
        publication_id: String,
        #[arg(long, value_enum, default_value_t = DbPreflightProfileCli::Default)]
        profile: DbPreflightProfileCli,
        #[arg(long, default_value_t = false)]
        with_worthiness: bool,
    },
    /// Print Zenodo metadata
    #[command(name = "publication-zenodo-metadata")]
    PublicationZenodoMetadata {
        #[arg(long)]
        publication_id: String,
    },
    /// Merged OpenReview invitation/signature/readers.
    #[command(name = "publication-openreview-profile")]
    PublicationOpenreviewProfile {
        #[arg(long)]
        publication_id: String,
    },
    /// Export scholarly staging files
    #[command(name = "publication-scholarly-staging-export")]
    PublicationScholarlyStagingExport {
        #[arg(long)]
        publication_id: String,
        #[arg(long)]
        output_dir: std::path::PathBuf,
        #[arg(long, value_enum)]
        venue: ScholarlyVenueCli,
    },
    /// Worthiness rubric evaluation JSON.
    #[command(name = "publication-worthiness-evaluate")]
    PublicationWorthinessEvaluate {
        #[arg(long)]
        contract_yaml: Option<std::path::PathBuf>,
        #[arg(long)]
        metrics_json: std::path::PathBuf,
    },
    /// Record digest-bound approval for a prepared publication.
    #[command(name = "publication-approve")]
    PublicationApprove {
        /// Stable publication id.
        #[arg(long)]
        publication_id: String,
        /// Approver identity.
        #[arg(long)]
        approver: String,
    },
    /// Submit through the scholarly adapter.
    #[command(name = "publication-submit-local")]
    PublicationSubmitLocal {
        /// Stable publication id.
        #[arg(long)]
        publication_id: String,
        /// Override adapter.
        #[arg(long)]
        adapter: Option<String>,
    },
    /// Show manifest + approval + scholarly status.
    #[command(name = "publication-status")]
    PublicationStatus {
        /// Stable publication id.
        #[arg(long)]
        publication_id: String,
        #[arg(long, default_value_t = false)]
        with_worthiness: bool,
    },
    /// Rank SCIENTIA publication candidates.
    #[command(name = "publication-discovery-scan")]
    PublicationDiscoveryScan {
        #[arg(long)]
        state: Option<String>,
        #[arg(long, default_value_t = 50)]
        limit: i64,
    },
    /// Auto-publish StrongCandidate findings to the local RSS feed (feed.xml). Idempotent.
    #[command(name = "publication-discovery-publish-rss")]
    PublicationDiscoveryPublishRss {
        /// Override the path to feed.xml (default: docs/src/feed.xml relative to repo root).
        #[arg(long)]
        feed_path: Option<std::path::PathBuf>,
        /// Maximum candidates to scan.
        #[arg(long, default_value_t = 100)]
        limit: i64,
        /// Emit JSON result summary.
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    /// Explain the heuristic scoring for publication discovery
    #[command(name = "publication-discovery-explain")]
    PublicationDiscoveryExplain {
        #[arg(long)]
        publication_id: String,
    },
    /// Emit destination transform preview JSON
    #[command(name = "publication-transform-preview")]
    PublicationTransformPreview {
        #[arg(long)]
        publication_id: String,
    },
    /// Prior-art fetch JSON.
    #[command(name = "publication-novelty-fetch")]
    PublicationNoveltyFetch {
        #[arg(long)]
        publication_id: String,
        #[arg(long, default_value_t = false)]
        offline: bool,
        #[arg(long, default_value_t = false)]
        persist_metadata: bool,
    },
    /// Decision snapshot JSON.
    #[command(name = "publication-decision-explain")]
    PublicationDecisionExplain {
        #[arg(long)]
        publication_id: String,
        #[arg(long, default_value_t = false)]
        live_prior_art: bool,
        #[arg(long, default_value_t = false)]
        offline: bool,
    },
    /// Happy-path bundle + candidate + worthiness JSON.
    #[command(name = "publication-novelty-happy-path")]
    PublicationNoveltyHappyPath {
        #[arg(long)]
        publication_id: String,
        #[arg(long, default_value_t = false)]
        offline: bool,
    },
    /// Poll remote scholarly repository status.
    #[command(name = "publication-scholarly-remote-status")]
    PublicationScholarlyRemoteStatus {
        #[arg(long)]
        publication_id: String,
        #[arg(long)]
        external_submission_id: Option<String>,
    },
    /// Poll remote status for every scholarly submission row.
    #[command(name = "publication-scholarly-remote-status-sync-all")]
    PublicationScholarlyRemoteStatusSyncAll {
        #[arg(long)]
        publication_id: String,
    },
    /// Batch remote status poll across publications.
    #[command(name = "publication-scholarly-remote-status-sync-batch")]
    PublicationScholarlyRemoteStatusSyncBatch {
        #[arg(long, default_value_t = PUBLICATION_SYNC_BATCH_DEFAULT_LIMIT)]
        limit: i64,
        #[arg(long, default_value_t = PUBLICATION_WORKER_DEFAULT_ITERATIONS)]
        iterations: u32,
        #[arg(long, default_value_t = PUBLICATION_WORKER_DEFAULT_INTERVAL_SECS)]
        interval_secs: u64,
        #[arg(long)]
        max_runtime_secs: Option<u64>,
        #[arg(long, default_value_t = PUBLICATION_WORKER_DEFAULT_JITTER_SECS)]
        jitter_secs: u64,
    },
    /// Record an arXiv-assist operator milestone.
    #[command(name = "publication-arxiv-handoff-record")]
    PublicationArxivHandoffRecord {
        #[arg(long)]
        publication_id: String,
        #[arg(long)]
        stage: ArxivHandoffStageCli,
        #[arg(long)]
        operator: Option<String>,
        #[arg(long)]
        note: Option<String>,
        #[arg(long)]
        arxiv_id: Option<String>,
    },
    /// List due scholarly outbound jobs
    #[command(name = "publication-external-jobs-due")]
    PublicationExternalJobsDue {
        #[arg(long, default_value_t = PUBLICATION_EXTERNAL_JOBS_DEFAULT_LIMIT)]
        limit: i64,
    },
    /// List scholarly outbound jobs in terminal `failed` state.
    #[command(name = "publication-external-jobs-dead-letter")]
    PublicationExternalJobsDeadLetter {
        #[arg(long, default_value_t = PUBLICATION_EXTERNAL_JOBS_DEFAULT_LIMIT)]
        limit: i64,
    },
    /// Requeue one dead-letter scholarly job.
    #[command(name = "publication-external-jobs-replay")]
    PublicationExternalJobsReplay {
        #[arg(long)]
        job_id: i64,
    },
    /// Run one batch of due scholarly submit jobs
    #[command(name = "publication-external-jobs-tick")]
    PublicationExternalJobsTick {
        #[arg(long, default_value_t = PUBLICATION_EXTERNAL_JOBS_TICK_DEFAULT_LIMIT)]
        limit: i64,
        #[arg(long, default_value_t = PUBLICATION_EXTERNAL_JOBS_TICK_DEFAULT_LOCK_TTL_MS)]
        lock_ttl_ms: i64,
        #[arg(long)]
        lock_owner: Option<String>,
        #[arg(long, default_value_t = PUBLICATION_WORKER_DEFAULT_ITERATIONS)]
        iterations: u32,
        #[arg(long, default_value_t = PUBLICATION_WORKER_DEFAULT_INTERVAL_SECS)]
        interval_secs: u64,
        #[arg(long)]
        max_runtime_secs: Option<u64>,
        #[arg(long, default_value_t = PUBLICATION_WORKER_DEFAULT_JITTER_SECS)]
        jitter_secs: u64,
    },
    /// One-command scholarly path.
    #[command(name = "publication-scholarly-pipeline-run")]
    PublicationScholarlyPipelineRun {
        #[arg(long)]
        publication_id: String,
        #[arg(long, value_enum, default_value_t = DbPreflightProfileCli::Default)]
        preflight_profile: DbPreflightProfileCli,
        #[arg(long, default_value_t = false)]
        dry_run: bool,
        #[arg(long)]
        staging_output_dir: Option<std::path::PathBuf>,
        #[arg(long, value_enum)]
        venue: Option<ScholarlyVenueCli>,
        #[arg(long)]
        adapter: Option<String>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    /// JSON rollup of external scholarly pipeline metrics.
    #[command(name = "publication-external-pipeline-metrics")]
    PublicationExternalPipelineMetrics {
        #[arg(long, default_value_t = PUBLICATION_EXTERNAL_METRICS_DEFAULT_SINCE_HOURS)]
        since_hours: i64,
    },
    /// Run one batch of Scientist RSS/Atom crawling.
    #[command(name = "ingest-tick")]
    IngestTick {
        /// Optional specific feed id to tick.
        #[arg(long)]
        feed_id: Option<String>,
        #[arg(long, default_value_t = 10)]
        limit: usize,
    },
    /// Register or update a feed source for inbound intelligence.
    #[command(name = "feed-source-add")]
    FeedSourceAdd {
        #[arg(long)]
        id: String,
        #[arg(long)]
        url: String,
        #[arg(long, default_value = "rss")]
        kind: String,
        #[arg(long, default_value_t = 3600000)]
        interval_ms: i64,
    },
    /// List registered feed sources.
    #[command(name = "feed-source-list")]
    FeedSourceList,
    /// Diagnose syndication adapter health.
    #[command(name = "diagnose")]
    Diagnose {
        /// Force live heartbeat probes.
        #[arg(long, default_value_t = false)]
        live: bool,
    },

    /// Scout the current workspace for publication candidates (Phase A
    /// signal producers + Phase F single-command surface).
    ///
    /// Surveys recent commit-graph activity, benchmark history, and
    /// Socrates telemetry, persists new candidates to
    /// `scientia_finding_candidates`, and prints a ranked report.
    #[command(name = "scout")]
    Scout {
        /// Maximum commits to scan back from HEAD.
        #[arg(long, default_value_t = 100)]
        commit_window: usize,
        /// Activity window in days (currently informational; reserved for
        /// future bench/telemetry windowing).
        #[arg(long, default_value_t = 30)]
        days_window: u32,
        /// Output format.
        #[arg(long, value_enum, default_value_t = ScoutOutput::Table)]
        output: ScoutOutput,
        /// Restrict to a single candidate class
        /// (`algorithmic_improvement` | `reproducibility_infra`
        ///  | `policy_governance` | `telemetry_trust`).
        #[arg(long)]
        candidate_class: Option<String>,
    },

    /// Phase B — Re-execute a manifest's RO-Crate `mainEntity` in a sandbox
    /// and emit the measured `ReplayReport` JSON to stdout.
    #[command(name = "publication-replay-execute")]
    PublicationReplayExecute {
        /// Path to a JSON file matching
        /// `vox_scientia::ro_crate::MainEntity`.
        #[arg(long)]
        main_entity: std::path::PathBuf,
        /// Directory containing the materialized RO-Crate; entry-point runs
        /// here.
        #[arg(long)]
        stage_dir: std::path::PathBuf,
    },

    /// Phase C — Render an IMRaD manuscript skeleton from a JSON
    /// `ScaffoldInput`, with provenance-bound safe slots filled and
    /// rubric-forbidden sections as TODO blocks.
    #[command(name = "publication-manuscript-draft")]
    PublicationManuscriptDraft {
        /// Path to a JSON file matching
        /// `vox_manuscript_scaffold::ScaffoldInput`.
        #[arg(long)]
        scaffold: std::path::PathBuf,
    },

    /// Phase D — Evaluate the solo-author critic gate against supplied
    /// approver + fingerprint inputs and emit the `GateOutcome` JSON.
    #[command(name = "publication-critic-gate-check")]
    PublicationCriticGateCheck {
        /// Path to a JSON file matching the owned analog of
        /// `vox_critic_gate::GateInputs`.
        #[arg(long)]
        inputs: std::path::PathBuf,
    },

    /// Phase D wiring — Record a critic approval for a publication digest,
    /// after running the solo-author critic gate over existing human
    /// approvers + the proposed critic. Refuses to persist if the gate
    /// returns a non-clearing reason.
    #[command(name = "publication-critic-approve")]
    PublicationCriticApprove {
        /// Publication id (lookup current manifest + content digest).
        #[arg(long)]
        publication_id: String,
        /// Stable critic identity. Recorded as the `approver` column value.
        #[arg(long)]
        critic_id: String,
        /// Path to a JSON file matching `CriticApproveInputsJson`
        /// (critic fingerprint + recommendation + artifact-side
        /// fingerprints + venue policy + optional report URI).
        #[arg(long)]
        inputs: std::path::PathBuf,
    },

    /// Phase 1 wiring — Run the SCIENTIA claim-extraction pipeline (VeriScore
    /// gate → atomic decomposition → span check → MiniCheck verifier) over
    /// the manifest's `body_markdown` and persist the summary into
    /// `metadata_json.scientia_evidence.extracted_claims`. Subsequent
    /// preflight + worthiness runs derive `claim_evidence_coverage` from the
    /// measured support ratio instead of the citation-presence heuristic.
    ///
    /// `VOX_MINICHECK_ENDPOINT` selects the HTTP verifier; absent it falls
    /// back to the deterministic mock backend.
    #[command(name = "publication-extract-claims")]
    PublicationExtractClaims {
        /// Publication id whose manifest body should be processed.
        #[arg(long)]
        publication_id: String,
    },

    /// Phase 3 — Render a `ScaffoldInput` JSON to a standalone LaTeX
    /// document (`\documentclass{article}`) and write it to stdout or
    /// `--output`. Suitable for PDF generation via `tectonic` /
    /// `pdflatex`. Preserves the same safe-slots / TODO discipline as the
    /// markdown scaffolder.
    #[command(name = "publication-render-latex")]
    PublicationRenderLatex {
        /// Path to a JSON file matching `vox_manuscript_scaffold::ScaffoldInput`.
        #[arg(long)]
        scaffold: std::path::PathBuf,
        /// Optional output path (default: stdout).
        #[arg(long)]
        output: Option<std::path::PathBuf>,
    },

    /// Phase 4 — Build an arXiv-ready `.tar.gz` bundle from a
    /// `ScaffoldInput` JSON + a directory of figure assets. The bundle
    /// contains `main.tex` plus each `figures[*].path` resolved against
    /// `--figures-dir`. Output is the canonical arXiv submission layout.
    #[command(name = "publication-arxiv-bundle")]
    PublicationArxivBundle {
        /// Path to a JSON file matching `vox_manuscript_scaffold::ScaffoldInput`.
        #[arg(long)]
        scaffold: std::path::PathBuf,
        /// Directory containing figure assets at paths matching
        /// `scaffold.figures[*].path`. May be empty if the scaffold has
        /// no figures.
        #[arg(long)]
        figures_dir: std::path::PathBuf,
        /// Output path for the `.tar.gz` bundle.
        #[arg(long)]
        output: std::path::PathBuf,
    },

    /// Phase E — Print per-class venue routing + policy defaults for the
    /// AI/SWE micro-publication track (non-Atlas).
    #[command(name = "publication-venue-recommend")]
    PublicationVenueRecommend {
        /// One of `algorithmic_improvement`, `reproducibility_infra`,
        /// `policy_governance`, `telemetry_trust`, `other`,
        /// `model_capability_atlas`, `provider_reliability_atlas`.
        #[arg(long)]
        candidate_class: String,
        /// Optional path to YAML matching
        /// `contracts/scientia/finding-class-defaults.v1.yaml`. Built-in
        /// defaults are used when absent.
        #[arg(long)]
        yaml_config: Option<std::path::PathBuf>,
    },

    /// Phase G — Render the canonical HTML page for one
    /// `/findings/<trusty-uri>` finding (Highwire meta tags + version
    /// history + retraction banner + verified-claims sidebar).
    #[command(name = "publication-finding-page-render")]
    PublicationFindingPageRender {
        /// Path to a JSON file matching `vox_findings_site::FindingPage`.
        #[arg(long)]
        page: std::path::PathBuf,
    },

    /// Phase H — Build a `QueueSnapshot` JSON for the dashboard panel from
    /// supplied candidate / claims-pending / reply-window / retraction-queue
    /// inputs.
    #[command(name = "publication-dashboard-snapshot")]
    PublicationDashboardSnapshot {
        /// Path to a JSON file matching the owned analog of
        /// `vox_scientia_dashboard::DashboardInputs`.
        #[arg(long)]
        inputs: std::path::PathBuf,
    },
}

/// Output format for `vox scientia scout`.
#[derive(Copy, Clone, Debug, clap::ValueEnum, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScoutOutput {
    /// Human-readable table on stdout.
    Table,
    /// Machine-readable JSON array on stdout.
    Json,
}
