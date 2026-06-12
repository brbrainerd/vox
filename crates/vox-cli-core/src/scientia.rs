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
    /// Deterministic archive-metadata autofill: fills MISSING fields (never overwrites),
    /// records per-field provenance, and reports before/after completeness. No LLM.
    #[command(name = "publication-autofill")]
    PublicationAutofill {
        /// Stable publication id.
        #[arg(long)]
        publication_id: String,
        /// Apply the proposed fills to the stored manifest (persists via upsert + digest recompute).
        #[arg(long, default_value_t = false)]
        apply: bool,
    },
    /// Archive the publication's code repository via Software Heritage Save Code Now.
    ///
    /// On success (or accepted-without-wait) merges `scientia.swh_save` and
    /// (when available) `scientia.swhid` into the stored manifest so Zenodo
    /// `related_identifiers` picks up the SWHID.
    #[command(name = "publication-archive-code")]
    PublicationArchiveCode {
        /// Stable publication id.
        #[arg(long)]
        publication_id: String,
        /// Code repository URL to archive.
        /// Defaults to `metadata_json.scientia.reproducibility.code_repository_url`.
        /// If neither is present, run publication-autofill first.
        #[arg(long)]
        origin_url: Option<String>,
        /// Poll up to 5 minutes (10 s interval) until task_status is succeeded/failed.
        #[arg(long, default_value_t = false)]
        wait: bool,
    },
    /// Run the archive pipeline end-to-end (Zenodo deposit + Software Heritage).
    ///
    /// Requires a complete manifest and at least one digest-bound approval
    /// (`publication-approve`). Sandbox Zenodo is the default; `--production`
    /// targets production, `--publish` publishes the deposition.
    #[command(name = "publication-archive-run")]
    PublicationArchiveRun {
        /// Stable publication id.
        #[arg(long)]
        publication_id: String,
        /// Target production Zenodo instead of the sandbox (default: sandbox).
        #[arg(long, default_value_t = false)]
        production: bool,
        /// Publish the Zenodo deposition rather than leaving it as a draft.
        #[arg(long, default_value_t = false)]
        publish: bool,
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

    /// Track C — Scan new commits for research-worthy signals and create DRAFT
    /// finding candidates (surfaced for human review, NEVER auto-published).
    /// Advances a per-producer cursor only after the batch's draft inserts
    /// succeed. When an embedding provider + code vector index are configured,
    /// folds a (Supporting-only) code-uniqueness signal into each candidate.
    #[command(name = "discovery-watch")]
    DiscoveryWatch {
        /// Run a single pass and exit (currently the only mode).
        #[arg(long, default_value_t = false)]
        once: bool,
        /// Repository path to scan (default: resolved repo root).
        #[arg(long)]
        repo: Option<std::path::PathBuf>,
        /// When no cursor exists, scan at most this many commits back from HEAD.
        #[arg(long, default_value_t = 20)]
        limit: usize,
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

    /// P2 — Record a human review decision (approve | reject | defer) for ONE
    /// extracted claim. The decision is bound to the publication's CURRENT
    /// content digest, so a later content edit invalidates a prior approval.
    /// `publication-nanopub-build` refuses to emit unless the latest decision is
    /// an approval bound to the current digest.
    #[command(name = "publication-claim-review")]
    PublicationClaimReview {
        /// Publication id the claim belongs to.
        #[arg(long)]
        publication_id: String,
        /// `claim_id` of the extracted claim to decide on (see `vox scientia claims`).
        #[arg(long)]
        claim_id: i64,
        /// The review decision.
        #[arg(long, value_enum)]
        decision: ClaimReviewDecisionCli,
        /// Optional free-text rationale recorded with the decision.
        #[arg(long)]
        reason: Option<String>,
    },

    /// P1 — Build a spec-compliant nanopublication for ONE extracted claim:
    /// resolve (or create) the per-user RSA + ORCID signing identity, assemble
    /// the enriched assertion, RSA-sign it, VALIDATE it OFFLINE (trusty hash +
    /// signature; NO network), and persist the signed artifact to
    /// `scientia_nanopubs` with `published_state="local"`. Prints the resulting
    /// Trusty URI.
    ///
    /// With `--publish-test-server`: after the build succeeds, also publish the
    /// signed nanopub to the nanopub TEST server (requires both a human-approved
    /// claim token AND `VOX_NANOPUB_TEST_SERVER=1` in the environment). The test
    /// server is a public registry that is periodically wiped — NOT production.
    /// Prints the published URI. Production publishing is deliberately unimplemented.
    #[command(name = "publication-nanopub-build")]
    PublicationNanopubBuild {
        /// Publication id the claim belongs to (selects the claim bucket).
        #[arg(long)]
        publication_id: String,
        /// `claim_id` of the extracted claim to sign (see `vox scientia claims`).
        #[arg(long)]
        claim_id: i64,
        /// Optional ORCID URL (e.g. <https://orcid.org/0000-0002-1825-0097>).
        /// Overrides the stored identity ORCID; required if none is stored.
        #[arg(long)]
        orcid: Option<String>,
        /// After building locally, also publish to the nanopub TEST server.
        /// Requires `VOX_NANOPUB_TEST_SERVER=1` to be set in the environment
        /// and a persisted human-approved review decision for this claim.
        /// The test server is public and periodically wiped — NOT production.
        #[arg(long, default_value_t = false)]
        publish_test_server: bool,
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
        /// arXiv primary category to embed in the handoff sidecar
        /// (e.g. `cs.SE`, `stat.ML`).  When omitted the default `cs.SE` is
        /// used and `category_origin` is set to `"default"` in the sidecar,
        /// reminding the operator to verify.  When supplied explicitly
        /// `category_origin` is `"flag"`.
        #[arg(long)]
        primary_category: Option<String>,
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

    /// List a publication's extracted claims joined to each claim's latest
    /// verdict, as JSON. Reads `scientia_claims` / `scientia_claim_verdicts`
    /// (populated by `publication-extract-claims`).
    #[command(name = "claims")]
    Claims {
        /// Publication id whose extracted claims to list.
        #[arg(long)]
        publication_id: String,
    },

    /// List a publication's claims awaiting human review as JSON. A claim
    /// appears when it has an extracted (non-`Unverified`) verdict AND its
    /// latest decision is absent or non-terminal (`deferred`/`edited`).
    /// Terminal decisions (`approved`, `rejected`) exclude the claim.
    #[command(name = "publication-review-queue")]
    PublicationReviewQueue {
        /// Publication id whose claims awaiting review to list.
        #[arg(long)]
        publication_id: String,
    },

    /// Phase H — Assemble a dashboard `QueueSnapshot` JSON directly from the
    /// live Codex DB (publication candidates + extracted-claims pending counts +
    /// retraction queue). Unlike `publication-dashboard-snapshot`, this needs
    /// no inputs file.
    #[command(name = "dashboard")]
    Dashboard,

    /// P3 — LLM-assisted evidence/conclusion suggestions for ONE claim in the
    /// review queue (ADVISORY only — never mutates any decision or assertion).
    /// Routed through the model-agnostic actor-runtime LLM facade; prints a JSON
    /// array of suggestions. Degrades to `[]` on any LLM error.
    #[command(name = "evidence-assist")]
    EvidenceAssist {
        /// Publication id the claim belongs to.
        #[arg(long)]
        publication_id: String,
        /// `claim_id` of the claim to get suggestions for (see `vox scientia publication-review-queue`).
        #[arg(long)]
        claim_id: i64,
    },

    /// Phase H — Assemble a `CostRollup` JSON for the current calendar quarter
    /// from the live Codex DB.  Per-provider totals come from
    /// `agent_telemetry_flat` (event_kind='cost'); the four pipeline-phase
    /// category lines are 0.0 until `agent_telemetry_flat` gains a
    /// `pipeline_phase` column.  An empty DB yields an all-zeros rollup.
    #[command(name = "cost")]
    Cost,
}

/// Human review decision for `publication-claim-review`.
///
/// The stored DB vocabulary (`as_stored`) MUST match `vox_db::store::VALID_DECISIONS`.
/// `"edited"` is driven by content edits, not a manual review action, so it is
/// intentionally NOT exposed here.
#[derive(Copy, Clone, Debug, clap::ValueEnum, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClaimReviewDecisionCli {
    Approve,
    Reject,
    Defer,
}

impl ClaimReviewDecisionCli {
    /// Map to the DB vocabulary stored in `scientia_review_decisions.decision`.
    /// These strings MUST match `vox_db::store::VALID_DECISIONS`.
    pub fn as_stored(&self) -> &'static str {
        match self {
            Self::Approve => "approved",
            Self::Reject => "rejected",
            Self::Defer => "deferred",
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Confirm `ClaimReviewDecisionCli::as_stored` maps each variant to the
    /// exact DB vocabulary required by `vox_db::store::VALID_DECISIONS`.
    #[test]
    fn claim_review_decision_as_stored_maps_correctly() {
        assert_eq!(ClaimReviewDecisionCli::Approve.as_stored(), "approved");
        assert_eq!(ClaimReviewDecisionCli::Reject.as_stored(), "rejected");
        assert_eq!(ClaimReviewDecisionCli::Defer.as_stored(), "deferred");
    }

    /// Guard against drift: every CLI decision MUST map to a value the DB layer
    /// accepts. A typo in `as_stored()` would otherwise only surface as a
    /// runtime DB-validation error rather than a caught programming error.
    #[test]
    fn as_stored_values_are_all_valid_db_decisions() {
        for d in [
            ClaimReviewDecisionCli::Approve,
            ClaimReviewDecisionCli::Reject,
            ClaimReviewDecisionCli::Defer,
        ] {
            assert!(
                vox_db::store::VALID_DECISIONS.contains(&d.as_stored()),
                "{:?}.as_stored() = {:?} is not in vox_db::store::VALID_DECISIONS {:?}",
                d,
                d.as_stored(),
                vox_db::store::VALID_DECISIONS,
            );
        }
    }
}
