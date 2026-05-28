//! Publication and scholarly pipeline subcommands for `vox db`.

use clap::Subcommand;
use std::path::PathBuf;

use super::types::{
    ArxivHandoffStageCli, DbPreflightProfileCli, DiscoveryIntakeGateCli, PublicationPrepareBodyCli,
    ScholarlyVenueCli,
};

pub(crate) const PUBLICATION_SYNC_BATCH_DEFAULT_LIMIT: i64 = 25;
pub(crate) const PUBLICATION_EXTERNAL_JOBS_DEFAULT_LIMIT: i64 = 50;
pub(crate) const PUBLICATION_EXTERNAL_JOBS_TICK_DEFAULT_LIMIT: i64 = 10;
pub(crate) const PUBLICATION_EXTERNAL_JOBS_TICK_DEFAULT_LOCK_TTL_MS: i64 = 120_000;
pub(crate) const PUBLICATION_WORKER_DEFAULT_ITERATIONS: u32 = 1;
pub(crate) const PUBLICATION_WORKER_DEFAULT_INTERVAL_SECS: u64 = 0;
pub(crate) const PUBLICATION_WORKER_DEFAULT_JITTER_SECS: u64 = 0;
pub(crate) const PUBLICATION_EXTERNAL_METRICS_DEFAULT_SINCE_HOURS: i64 = 168;

/// Publication / scholarly subcommands for `vox db`.
#[derive(Subcommand)]
pub enum DbCliPublication {
    /// Upsert a publication manifest from markdown for scientia/journal flows.
    #[command(name = "publication-prepare")]
    PublicationPrepare {
        /// Content type (`scientia`, `news`, `paper`, ...).
        #[arg(long, default_value = "scientia")]
        content_type: String,
        #[command(flatten)]
        body: PublicationPrepareBodyCli,
        /// Run [`vox_publisher::publication_preflight`] before upsert; fail on error-level findings.
        #[arg(long, default_value_t = false)]
        preflight: bool,
        #[arg(long, value_enum, default_value_t = DbPreflightProfileCli::Default)]
        preflight_profile: DbPreflightProfileCli,
        /// Optional scientia-only discovery intake gate (blocks low-signal or non-auto-eligible drafts).
        #[arg(long, value_enum, default_value_t = DiscoveryIntakeGateCli::None)]
        discovery_intake_gate: DiscoveryIntakeGateCli,
    },
    /// Like `publication-prepare` but always runs preflight (same as `--preflight`).
    #[command(name = "publication-prepare-validated")]
    PublicationPrepareValidated {
        /// Content type (`scientia`, `news`, `paper`, ...).
        #[arg(long, default_value = "scientia")]
        content_type: String,
        #[command(flatten)]
        body: PublicationPrepareBodyCli,
        #[arg(long, value_enum, default_value_t = DbPreflightProfileCli::Default)]
        preflight_profile: DbPreflightProfileCli,
        #[arg(long, value_enum, default_value_t = DiscoveryIntakeGateCli::None)]
        discovery_intake_gate: DiscoveryIntakeGateCli,
    },
    /// Print publication preflight report JSON for an existing manifest (no writes).
    #[command(name = "publication-preflight")]
    PublicationPreflight {
        #[arg(long)]
        publication_id: String,
        #[arg(long, value_enum, default_value_t = DbPreflightProfileCli::Default)]
        profile: DbPreflightProfileCli,
        /// Attach conservative [`vox_publisher::publication_worthiness`] estimate (loads default contract from repo root).
        #[arg(long, default_value_t = false)]
        with_worthiness: bool,
    },
    /// Print Zenodo deposit `metadata` JSON for an existing manifest (stdout only; no HTTP).
    #[command(name = "publication-zenodo-metadata")]
    PublicationZenodoMetadata {
        #[arg(long)]
        publication_id: String,
    },
    /// Print merged OpenReview submit profile JSON (`metadata_json.openreview` + env; stdout only; no HTTP).
    #[command(name = "publication-openreview-profile")]
    PublicationOpenreviewProfile {
        #[arg(long)]
        publication_id: String,
    },
    /// Write scholarly staging files (`body.md`, `CITATION.cff`, `crossref_work.json`, optional `zenodo.json`) under `--output-dir`.
    #[command(name = "publication-scholarly-staging-export")]
    PublicationScholarlyStagingExport {
        #[arg(long)]
        publication_id: String,
        #[arg(long)]
        output_dir: PathBuf,
        #[arg(long, value_enum)]
        venue: ScholarlyVenueCli,
    },
    /// Evaluate [`vox_publisher::publication_worthiness`] against a metrics JSON file (stdout only).
    #[command(name = "publication-worthiness-evaluate")]
    PublicationWorthinessEvaluate {
        /// Repo-relative YAML policy (defaults to `contracts/scientia/publication-worthiness.default.yaml`).
        #[arg(long)]
        contract_yaml: Option<PathBuf>,
        /// JSON file with [`vox_publisher::publication_worthiness::WorthinessInputs`].
        #[arg(long)]
        metrics_json: PathBuf,
    },
    /// Record digest-bound approval for a prepared publication.
    #[command(name = "publication-approve")]
    PublicationApprove {
        /// Stable publication id.
        #[arg(long)]
        publication_id: String,
        /// Approver identity (must be distinct across at least two values).
        #[arg(long)]
        approver: String,
    },
    /// Submit a prepared publication using the scholarly adapter (`--adapter` or `VOX_SCHOLARLY_ADAPTER`).
    #[command(name = "publication-submit-local")]
    PublicationSubmitLocal {
        /// Stable publication id.
        #[arg(long)]
        publication_id: String,
        /// Override adapter: `local_ledger`, `echo_ledger`, `zenodo`, `openreview` (default: env).
        #[arg(long)]
        adapter: Option<String>,
    },
    /// Print publication manifest, approvals, and scholarly submission rows.
    #[command(name = "publication-status")]
    PublicationStatus {
        /// Stable publication id.
        #[arg(long)]
        publication_id: String,
        /// Attach worthiness scoring to the embedded preflight report.
        #[arg(long, default_value_t = false)]
        with_worthiness: bool,
    },
    /// Rank SCIENTIA publication candidates from DB manifests (deterministic signal score).
    #[command(name = "publication-discovery-scan")]
    PublicationDiscoveryScan {
        #[arg(long)]
        content_type: Option<String>,
        #[arg(long)]
        state: Option<String>,
        #[arg(long, default_value_t = 50)]
        limit: i64,
    },
    /// Auto-publish `auto_draft_eligible` Scientia findings to the local RSS feed (feed.xml).
    /// Scans candidate publications, filters to those with a StrongCandidate discovery rank,
    /// and appends each new finding as an RSS item. No gate / dual-approval required — RSS is
    /// the locally-owned channel. Skips items already present in the feed (idempotent).
    #[command(name = "publication-discovery-publish-rss")]
    PublicationDiscoveryPublishRss {
        /// Only consider publications of this content type (default: all).
        #[arg(long)]
        content_type: Option<String>,
        /// Override the path to feed.xml (default: docs/src/feed.xml relative to repo root).
        #[arg(long)]
        feed_path: Option<std::path::PathBuf>,
        /// Maximum candidates to scan before selecting eligible ones.
        #[arg(long, default_value_t = 100)]
        limit: i64,
        /// Emit JSON result summary instead of human-readable output.
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    /// Explain discovery rank, manifest completion, and transform preview for one id.
    /// Explain the heuristic scoring for publication discovery
    #[command(name = "publication-discovery-explain")]
    PublicationDiscoveryExplain {
        #[arg(long)]
        publication_id: String,
    },
    /// Emit destination transform preview JSON (scholarly/social stubs; `machine_suggested`).
    #[command(name = "publication-transform-preview")]
    PublicationTransformPreview {
        #[arg(long)]
        publication_id: String,
    },
    /// Merge live Socrates / sidecars into `metadata_json`, recompute discovery evidence, upsert digest (scientia only).
    #[command(name = "publication-discovery-refresh-evidence")]
    PublicationDiscoveryRefreshEvidence {
        #[arg(long)]
        publication_id: String,
    },
    /// Federated prior-art fetch (OpenAlex, Crossref, Semantic Scholar) → stdout JSON; optional persist into `metadata_json.scientia_novelty_bundle`.
    #[command(name = "publication-novelty-fetch")]
    PublicationNoveltyFetch {
        #[arg(long)]
        publication_id: String,
        #[arg(long, default_value_t = false)]
        offline: bool,
        #[arg(long, default_value_t = false)]
        persist_metadata: bool,
    },
    /// Decision snapshot: preflight + worthiness + discovery rank, with optional live prior-art merge (no DB write unless already embedded).
    #[command(name = "publication-decision-explain")]
    PublicationDecisionExplain {
        #[arg(long)]
        publication_id: String,
        #[arg(long, default_value_t = false)]
        live_prior_art: bool,
        #[arg(long, default_value_t = false)]
        offline: bool,
    },
    /// One-shot operator JSON: prior-art bundle + finding-candidate row + worthiness (scientia only; stdout only).
    #[command(name = "publication-novelty-happy-path")]
    PublicationNoveltyHappyPath {
        #[arg(long)]
        publication_id: String,
        #[arg(long, default_value_t = false)]
        offline: bool,
    },
    /// Poll remote repository status for a scholarly submission (latest row, or match `--external-submission-id`).
    #[command(name = "publication-scholarly-remote-status")]
    PublicationScholarlyRemoteStatus {
        /// Stable publication id.
        #[arg(long)]
        publication_id: String,
        /// Restrict to this `scholarly_submissions.external_submission_id` (e.g. Zenodo deposition id, OpenReview note id).
        #[arg(long)]
        external_submission_id: Option<String>,
    },
    /// Poll remote status for every stored scholarly submission on this publication.
    #[command(name = "publication-scholarly-remote-status-sync-all")]
    PublicationScholarlyRemoteStatusSyncAll {
        #[arg(long)]
        publication_id: String,
    },
    /// Batch remote status poll: sync-all for each distinct publication (by recent scholarly submission activity).
    #[command(name = "publication-scholarly-remote-status-sync-batch")]
    PublicationScholarlyRemoteStatusSyncBatch {
        #[arg(long, default_value_t = PUBLICATION_SYNC_BATCH_DEFAULT_LIMIT)]
        limit: i64,
        /// Run the batch this many times (supervised worker; default 1).
        #[arg(long, default_value_t = PUBLICATION_WORKER_DEFAULT_ITERATIONS)]
        iterations: u32,
        /// Seconds to sleep between iterations (0 = no pause; max 3600).
        #[arg(long, default_value_t = PUBLICATION_WORKER_DEFAULT_INTERVAL_SECS)]
        interval_secs: u64,
        /// Stop early after this many wall-clock seconds (loop mode only; max 86400).
        #[arg(long)]
        max_runtime_secs: Option<u64>,
        /// Extra seconds 0..=interval added via subsecond hash jitter (loop mode only; capped at interval).
        #[arg(long, default_value_t = PUBLICATION_WORKER_DEFAULT_JITTER_SECS)]
        jitter_secs: u64,
    },
    /// Append-only operator milestone for arXiv-assist handoff (does not change manifest `state`).
    #[command(name = "publication-arxiv-handoff-record")]
    PublicationArxivHandoffRecord {
        #[arg(long)]
        publication_id: String,
        #[arg(long)]
        stage: ArxivHandoffStageCli,
        /// Operator id or handle (logged in event detail).
        #[arg(long)]
        operator: Option<String>,
        /// Free-form note (logged in event detail).
        #[arg(long)]
        note: Option<String>,
        /// Required when `stage` is `published` (e.g. `arXiv:2501.01234v1`).
        #[arg(long)]
        arxiv_id: Option<String>,
    },
    /// List `external_submission_jobs` rows due for retry (queued or retryable_failed with elapsed `next_retry_at_ms`).
    #[command(name = "publication-external-jobs-due")]
    PublicationExternalJobsDue {
        #[arg(long, default_value_t = PUBLICATION_EXTERNAL_JOBS_DEFAULT_LIMIT)]
        limit: i64,
    },
    /// List `external_submission_jobs` in terminal `failed` state (dead-letter).
    #[command(name = "publication-external-jobs-dead-letter")]
    PublicationExternalJobsDeadLetter {
        #[arg(long, default_value_t = PUBLICATION_EXTERNAL_JOBS_DEFAULT_LIMIT)]
        limit: i64,
    },
    /// Requeue one dead-letter job (`failed` → `queued`) by primary key for the next worker tick.
    #[command(name = "publication-external-jobs-replay")]
    PublicationExternalJobsReplay {
        #[arg(long)]
        job_id: i64,
    },
    /// Run one batch of due scholarly submit jobs (preflight, lease, submit with job.adapter).
    #[command(name = "publication-external-jobs-tick")]
    PublicationExternalJobsTick {
        #[arg(long, default_value_t = PUBLICATION_EXTERNAL_JOBS_TICK_DEFAULT_LIMIT)]
        limit: i64,
        /// Worker lease duration in milliseconds (5s–1h).
        #[arg(long, default_value_t = PUBLICATION_EXTERNAL_JOBS_TICK_DEFAULT_LOCK_TTL_MS)]
        lock_ttl_ms: i64,
        /// Override lock owner id (default: `vox:<pid>` or `VOX_SCHOLARLY_JOB_LOCK_OWNER`).
        #[arg(long)]
        lock_owner: Option<String>,
        /// Process this many back-to-back ticks (default 1; max 10000).
        #[arg(long, default_value_t = PUBLICATION_WORKER_DEFAULT_ITERATIONS)]
        iterations: u32,
        /// Seconds between ticks when `iterations` > 1 (max 3600).
        #[arg(long, default_value_t = PUBLICATION_WORKER_DEFAULT_INTERVAL_SECS)]
        interval_secs: u64,
        /// Stop early after this wall-clock budget (loop mode only; max 86400).
        #[arg(long)]
        max_runtime_secs: Option<u64>,
        /// Jitter bound added to each interval sleep in loop mode (0 = off).
        #[arg(long, default_value_t = PUBLICATION_WORKER_DEFAULT_JITTER_SECS)]
        jitter_secs: u64,
    },
    /// One-shot scholarly pipeline: manifest preflight, dual-approval gate, optional staging export, then submit.
    #[command(name = "publication-scholarly-pipeline-run")]
    PublicationScholarlyPipelineRun {
        #[arg(long)]
        publication_id: String,
        #[arg(long, value_enum, default_value_t = DbPreflightProfileCli::Default)]
        preflight_profile: DbPreflightProfileCli,
        /// Evaluate preflight + approval (+ optional staging) without calling submit.
        #[arg(long, default_value_t = false)]
        dry_run: bool,
        /// When set with `--venue`, write staging bundle under this directory.
        #[arg(long)]
        staging_output_dir: Option<PathBuf>,
        #[arg(long, value_enum)]
        venue: Option<ScholarlyVenueCli>,
        /// Passed through to submit (`VOX_SCHOLARLY_ADAPTER` when omitted).
        #[arg(long)]
        adapter: Option<String>,
        /// Single-line JSON on stdout (default: pretty-printed).
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    /// Read-only JSON rollup: scholarly external jobs, attempts, status snapshots, submission rows, and publication attempts (by channel).
    #[command(name = "publication-external-pipeline-metrics")]
    PublicationExternalPipelineMetrics {
        /// Lower bound for time-windowed series: now minus this many hours (0 = since Unix epoch). Clamped to 8760 (one year). Default 168 (7 days).
        #[arg(long, default_value_t = PUBLICATION_EXTERNAL_METRICS_DEFAULT_SINCE_HOURS)]
        since_hours: i64,
    },
    /// Upsert one publication media asset row.
    #[command(name = "publication-media-upsert")]
    PublicationMediaUpsert {
        /// Stable publication id.
        #[arg(long)]
        publication_id: String,
        /// Local/repository asset ref key.
        #[arg(long)]
        asset_ref: String,
        /// Media type (`video`, `image`, `dataset`, ...).
        #[arg(long)]
        media_type: String,
        /// Optional storage URI (URL, object key, external id).
        #[arg(long)]
        storage_uri: Option<String>,
        /// Lifecycle status (`pending`, `uploaded`, `failed`, ...).
        #[arg(long, default_value = "pending")]
        status: String,
        /// Optional JSON file path for metadata blob.
        #[arg(long)]
        metadata_json_path: Option<PathBuf>,
    },
    /// List publication media asset rows.
    #[command(name = "publication-media-list")]
    PublicationMediaList {
        /// Stable publication id.
        #[arg(long)]
        publication_id: String,
    },
    /// Delete one publication media asset row.
    #[command(name = "publication-media-delete")]
    PublicationMediaDelete {
        /// Stable publication id.
        #[arg(long)]
        publication_id: String,
        /// Local/repository asset ref key.
        #[arg(long)]
        asset_ref: String,
    },
    /// Simulate channel routing outcomes for a prepared publication id.
    #[command(name = "publication-route-simulate")]
    PublicationRouteSimulate {
        /// Stable publication id.
        #[arg(long)]
        publication_id: String,
        /// Emit compact single-line JSON (machine-friendly).
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    /// Publish a prepared publication to selected channels.
    #[command(name = "publication-publish")]
    PublicationPublish {
        /// Stable publication id.
        #[arg(long)]
        publication_id: String,
        /// Optional comma-separated channel allowlist.
        #[arg(long)]
        channels: Option<String>,
        /// Force dry-run mode for this invocation.
        #[arg(long, default_value_t = true)]
        dry_run: bool,
        /// Emit compact single-line JSON (machine-friendly).
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    /// Retry failed channels from latest recorded attempt.
    #[command(name = "publication-retry-failed")]
    PublicationRetryFailed {
        /// Stable publication id.
        #[arg(long)]
        publication_id: String,
        /// Optional single channel to retry.
        #[arg(long)]
        channel: Option<String>,
        /// Force dry-run mode for retry.
        #[arg(long, default_value_t = true)]
        dry_run: bool,
        /// Emit compact single-line JSON (machine-friendly).
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    /// Run one batch of Scientist RSS/Atom crawling and deduplication tick.
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
}
