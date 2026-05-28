//! Arca SQL: Automated research discovery, manuscript lifecycle, and scholarly publication.
pub const SCHEMA_SCIENTIA: &str = r#"
-- Canonical record of a discovered insight before manuscript generation.
CREATE TABLE IF NOT EXISTS scientia_discoveries (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    discovery_id      TEXT    NOT NULL UNIQUE,
    session_key       TEXT    NOT NULL,
    repository_id     TEXT    NOT NULL DEFAULT '',
    title             TEXT    NOT NULL,
    summary           TEXT    NOT NULL,
    claims_json       TEXT    NOT NULL,                      -- structured ClaimRecord[]
    evidence_json     TEXT    NOT NULL,                    -- citation/evidence packets
    novelty_score     REAL    NOT NULL DEFAULT 0.0,        -- RAG similarity vs. corpus
    confidence_score  REAL    NOT NULL DEFAULT 0.0,        -- Socrates confidence_at_stop
    human_gate_status TEXT    NOT NULL DEFAULT 'pending',  -- pending|approved|rejected
    human_gate_reason TEXT,
    publication_id    TEXT,                                -- FK to publication_manifests
    created_at_ms     INTEGER NOT NULL,
    updated_at_ms     INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_scientia_discoveries_session 
    ON scientia_discoveries(session_key, created_at_ms);
CREATE INDEX IF NOT EXISTS idx_scientia_discoveries_status 
    ON scientia_discoveries(human_gate_status);

-- Structured citation tracking aligned with discovery claims.
CREATE TABLE IF NOT EXISTS scientia_citations (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    discovery_id      TEXT    NOT NULL,
    citation_key      TEXT    NOT NULL,
    source_type       TEXT    NOT NULL,          -- 'knowledge_node', 'external_url', 'snippet', 'eval_run'
    source_ref        TEXT    NOT NULL,
    title             TEXT,
    authors_json      TEXT,
    year              INTEGER,
    doi               TEXT,
    url               TEXT,
    created_at_ms     INTEGER NOT NULL,
    UNIQUE(discovery_id, citation_key)
);

-- Orchestration queue for the multi-step publication flow.
CREATE TABLE IF NOT EXISTS scientia_publication_queue (
    id                    INTEGER PRIMARY KEY AUTOINCREMENT,
    discovery_id          TEXT    NOT NULL UNIQUE,
    publication_id        TEXT    NOT NULL,
    stage                 TEXT    NOT NULL DEFAULT 'draft',   
    -- stages: draft | doi_reserved | orcid_attributed | approved | submitted | published | failed
    zenodo_deposition_id  TEXT,
    prereserved_doi       TEXT,
    last_error            TEXT,
    attempt_count         INTEGER NOT NULL DEFAULT 0,
    next_retry_at_ms      INTEGER,
    created_at_ms         INTEGER NOT NULL,
    updated_at_ms         INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_scientia_queue_stage 
    ON scientia_publication_queue(stage, next_retry_at_ms);

-- High-performance append-only telemetry projection for dashboards and agent self-awareness.
-- Aggregates execution, cost, a2a, and trust observations into a single sequential table.
CREATE TABLE IF NOT EXISTS agent_telemetry_flat (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    agent_id          TEXT    NOT NULL,
    session_id        TEXT    NOT NULL,
    tenant_id         TEXT,
    repository_id     TEXT    NOT NULL DEFAULT '',
    event_kind        TEXT    NOT NULL,           -- 'exec', 'cost', 'trust_obs', 'a2a', 'budget_alert'
    tool_name         TEXT,
    model_id          TEXT,
    provider          TEXT,
    duration_ms       INTEGER,
    input_tokens      INTEGER,
    output_tokens     INTEGER,
    cost_usd          REAL,
    trust_score       REAL,
    payload_json      TEXT,                  -- narrow, non-PII subset
    recorded_at_ms    INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_agt_tel_flat_agent_ev 
    ON agent_telemetry_flat(agent_id, event_kind, recorded_at_ms);
CREATE INDEX IF NOT EXISTS idx_agt_tel_flat_session 
    ON agent_telemetry_flat(session_id, recorded_at_ms);
CREATE INDEX IF NOT EXISTS idx_agt_tel_flat_repo 
    ON agent_telemetry_flat(repository_id, recorded_at_ms);

-- Consolidated reliability scores for all entities (agents, skills, workflows, repositories).
-- Replaces agent_reliability, skill_reliability, workflow_reliability, repository_reliability.
CREATE TABLE IF NOT EXISTS reliability_scores (
    entity_type      TEXT    NOT NULL,
    entity_id        TEXT    NOT NULL,
    reliability      REAL    NOT NULL DEFAULT 0.5,
    success_count    INTEGER NOT NULL DEFAULT 0,
    failure_count    INTEGER NOT NULL DEFAULT 0,
    updated_at_ms    INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (entity_type, entity_id)
);

CREATE INDEX IF NOT EXISTS idx_reliability_scores_val 
    ON reliability_scores(reliability);

-- Quantitative evaluation of autonomous research quality (localized vs. Tavily).
CREATE TABLE IF NOT EXISTS research_eval_runs (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id            TEXT    NOT NULL UNIQUE,
    model_id          TEXT    NOT NULL,
    config_json       TEXT    NOT NULL,           -- search policy, depth, backends
    metrics_json      TEXT    NOT NULL,           -- RAGAS rollup (recall, groundedness, quality)
    latency_p50_ms    INTEGER,
    latency_p99_ms    INTEGER,
    tier_distribution_json TEXT,                    -- Tier 1/2/3/4 % breakdown
    created_at_ms     INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS research_eval_samples (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id            TEXT    NOT NULL,
    query             TEXT    NOT NULL,
    gold_answer       TEXT,                         -- optional reference
    model_answer      TEXT    NOT NULL,
    recall_at_5       REAL,
    groundedness      REAL,
    quality_score     REAL,                         -- search_execution quality rollup
    latency_ms        INTEGER,
    evidence_json     TEXT,                         -- array of [url, score, engine, snippet]
    recorded_at_ms    INTEGER NOT NULL,
    FOREIGN KEY(run_id) REFERENCES research_eval_runs(run_id)
);

CREATE INDEX IF NOT EXISTS idx_res_eval_samples_run 
    ON research_eval_samples(run_id);

-- Telemetry: Scoreboard for intelligent routing based on historical execution outcomes.
CREATE TABLE IF NOT EXISTS model_scoreboard (
    model_id              TEXT    NOT NULL,
    task_category         TEXT    NOT NULL,
    strength_tag          TEXT    NOT NULL,
    window_days           INTEGER NOT NULL,
    n_calls               INTEGER NOT NULL DEFAULT 0,
    success_rate          REAL    NOT NULL DEFAULT 0.0,
    p50_latency_ms        INTEGER,
    p99_latency_ms        INTEGER,
    cost_per_success_usd  REAL,
    quality_score         REAL    NOT NULL DEFAULT 1.0,
    updated_at_ms         INTEGER NOT NULL,
    PRIMARY KEY (model_id, task_category, strength_tag, window_days)
);

CREATE INDEX IF NOT EXISTS idx_model_scoreboard_task 
    ON model_scoreboard(task_category, strength_tag, success_rate);

-- Observed pricing SSOT updated from llm_interactions
CREATE TABLE IF NOT EXISTS model_pricing_catalog (
    model_id                   TEXT    NOT NULL,
    provider                   TEXT    NOT NULL,
    observed_blended_per_1k    REAL,
    observed_input_per_1k      REAL,
    observed_output_per_1k     REAL,
    catalog_input_per_1k       REAL    NOT NULL DEFAULT 0.0,
    catalog_output_per_1k      REAL    NOT NULL DEFAULT 0.0,
    n_provider_reported        INTEGER NOT NULL DEFAULT 0,
    n_estimated                INTEGER NOT NULL DEFAULT 0,
    n_free                     INTEGER NOT NULL DEFAULT 0,
    confidence                 TEXT    NOT NULL DEFAULT 'low',
    last_observed_at_ms        INTEGER,
    updated_at_ms              INTEGER NOT NULL,
    PRIMARY KEY (model_id, provider)
);

CREATE INDEX IF NOT EXISTS idx_model_pricing_catalog_model
    ON model_pricing_catalog(model_id, confidence);

-- Research session tracking for the SCIENTIA pipeline (Phase 0d).
CREATE TABLE IF NOT EXISTS scientia_research_sessions (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    session_key       TEXT    NOT NULL UNIQUE,
    status            TEXT    NOT NULL DEFAULT 'active',  -- active|completed|failed
    started_at_ms     INTEGER NOT NULL,
    finished_at_ms    INTEGER,
    query_text        TEXT,
    hit_count         INTEGER NOT NULL DEFAULT 0,
    claim_count       INTEGER NOT NULL DEFAULT 0,
    quality_score     INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_scientia_sessions_status ON scientia_research_sessions(status, started_at_ms);

-- Durable deep-research result artifact for async/CLI/MCP result retrieval.
CREATE TABLE IF NOT EXISTS scientia_research_artifacts (
    session_id        INTEGER PRIMARY KEY,
    artifact_json     TEXT    NOT NULL,
    report_markdown   TEXT    NOT NULL,
    created_at_ms     INTEGER NOT NULL,
    updated_at_ms     INTEGER NOT NULL
);

-- Atomic claims extracted from T1 aggregates (Phase 0d).
CREATE TABLE IF NOT EXISTS scientia_claims (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    claim_id          INTEGER NOT NULL UNIQUE,  -- FNV-1a hash of claim text
    session_id        INTEGER NOT NULL DEFAULT 0,
    text              TEXT    NOT NULL,
    is_numeric        INTEGER NOT NULL DEFAULT 0,
    is_recent         INTEGER NOT NULL DEFAULT 0,
    is_named_event    INTEGER NOT NULL DEFAULT 0,
    verifiability_score REAL,
    created_at_ms     INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_scientia_claims_session ON scientia_claims(session_id, created_at_ms);

-- Verification verdicts per claim (Phase 0d).
CREATE TABLE IF NOT EXISTS scientia_claim_verdicts (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    claim_id          INTEGER NOT NULL,
    verdict           TEXT    NOT NULL,  -- Supported|Contradicted|Contested|Unverified
    confidence        REAL    NOT NULL DEFAULT 0.0,
    verifier_model    TEXT,
    span_start        INTEGER,
    span_end          INTEGER,
    span_text         TEXT,
    created_at_ms     INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_scientia_verdicts_claim ON scientia_claim_verdicts(claim_id);

-- Pre-registration records (Phase 0d).
CREATE TABLE IF NOT EXISTS scientia_prereg (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    prereg_id         TEXT    NOT NULL UNIQUE,  -- Nanopub Trusty URI
    hypothesis        TEXT    NOT NULL,
    signed_at_ms      INTEGER NOT NULL,
    signing_key       TEXT    NOT NULL,
    payload_json      TEXT    NOT NULL,  -- full PreregistrationV1 JSON
    supersedes_id     TEXT,
    created_at_ms     INTEGER NOT NULL
);

-- T4 publication attempt log (Phase 0d).
CREATE TABLE IF NOT EXISTS scientia_publication_attempts (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    manifest_id       TEXT    NOT NULL,
    venue             TEXT    NOT NULL,
    attempt_number    INTEGER NOT NULL DEFAULT 1,
    status            TEXT    NOT NULL DEFAULT 'pending',  -- pending|submitted|accepted|rejected|failed
    doi               TEXT,
    error             TEXT,
    attempted_at_ms   INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_scientia_pub_attempts ON scientia_publication_attempts(manifest_id, attempted_at_ms);

-- Learned model behavior profiles for the Provider Atlas (Phase 0d).
CREATE TABLE IF NOT EXISTS scientia_model_profile_learning (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    provider          TEXT    NOT NULL,
    model_id          TEXT    NOT NULL,
    profile_key       TEXT    NOT NULL,  -- e.g. p95_latency_ms, refusal_rate, malformation_rate
    profile_value     REAL    NOT NULL,
    sample_count      INTEGER NOT NULL DEFAULT 0,
    window_start_ms   INTEGER NOT NULL,
    window_end_ms     INTEGER NOT NULL,
    updated_at_ms     INTEGER NOT NULL,
    UNIQUE(provider, model_id, profile_key)
);

-- Provider search runs within a research session (Phase 0d).
CREATE TABLE IF NOT EXISTS scientia_provider_runs (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id        INTEGER NOT NULL,
    provider_name     TEXT    NOT NULL,
    hit_count         INTEGER NOT NULL DEFAULT 0,
    elapsed_ms        INTEGER NOT NULL DEFAULT 0,
    started_at_ms     INTEGER NOT NULL,
    finished_at_ms    INTEGER
);
CREATE INDEX IF NOT EXISTS idx_scientia_provider_runs_session ON scientia_provider_runs(session_id);

-- Training pairs for model learning (quality-scored query/answer pairs) (Phase 0d).
CREATE TABLE IF NOT EXISTS scientia_training_pairs (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id        INTEGER NOT NULL,
    query_text        TEXT    NOT NULL,
    answer_text       TEXT    NOT NULL,
    quality_score     INTEGER NOT NULL DEFAULT 0,
    created_at_ms     INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_scientia_training_pairs_session ON scientia_training_pairs(session_id);

-- Phase A — self-observation signal producers ledger.
-- `vox-scientia-producers` writes rows here from commit-graph, benchmark-history,
-- and Socrates-telemetry detectors. Schema mirrors
-- `contracts/scientia/finding-candidate.v1.schema.json`.
CREATE TABLE IF NOT EXISTS scientia_finding_candidates (
    id                          INTEGER PRIMARY KEY AUTOINCREMENT,
    candidate_id                TEXT    NOT NULL UNIQUE,
    -- candidate_class enum: 'algorithmic_improvement' | 'reproducibility_infra'
    --                      | 'policy_governance' | 'telemetry_trust' | 'other'.
    -- Validity is enforced by `vox_db::FindingCandidateClass::from_str` in
    -- ops_finding_candidates.rs; Turso does not support CHECK constraints.
    candidate_class             TEXT    NOT NULL,
    publication_id              TEXT,
    title_hint                  TEXT,
    internal_signals_json       TEXT    NOT NULL,
    novelty_evidence_bundle_id  TEXT,
    worthiness_decision_ref     TEXT,
    confidence_json             TEXT,
    repository_id               TEXT,
    producer_name               TEXT    NOT NULL,
    signal_fingerprint          TEXT    NOT NULL,
    created_at_ms               INTEGER NOT NULL,
    updated_at_ms               INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_scientia_finding_candidates_class
    ON scientia_finding_candidates(candidate_class);
CREATE INDEX IF NOT EXISTS idx_scientia_finding_candidates_repo
    ON scientia_finding_candidates(repository_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_scientia_finding_candidates_fingerprint
    ON scientia_finding_candidates(producer_name, signal_fingerprint);
"#;
