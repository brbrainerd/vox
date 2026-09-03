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

-- scientia_citations: quarantined (DEAD, Task 4) — see domains/quarantine.rs.

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
    pipeline_phase    TEXT,                  -- Scientia pipeline phase tag for cost rows: 'extraction' | 'critic' | 'novelty' | 'scholarly'; NULL for non-Scientia telemetry
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
    success_count         INTEGER NOT NULL DEFAULT 0,
    cumulative_cost_usd   REAL    NOT NULL DEFAULT 0.0,
    p50_latency_ms        INTEGER,
    p99_latency_ms        INTEGER,
    cost_per_success_usd  REAL,
    quality_score         REAL    NOT NULL DEFAULT 1.0,
    -- Task M3: filled by the same nearest-rank batch pass as p50/p99_latency_ms (see
    -- VoxDb::refresh_model_scoreboard in ops_scientia.rs), from llm_interactions rows that
    -- have ttft_ms/tpot_ms recorded. NULL until that batch pass has run at least once for a
    -- window with at least one such row.
    p95_ttft_ms           INTEGER,
    p95_tpot_ms           REAL,
    -- Task M3: mean successful-call throughput (output_tokens / (latency_ms / 1000.0)) over
    -- the window -- "goodput" in the LLM-serving sense: throughput of tokens that were part
    -- of an actually-successful response, not raw wire throughput including failed retries.
    goodput_tokens_per_sec REAL,
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

-- scientia_prereg, scientia_publication_attempts: quarantined (DEAD, Task 4)
-- — see domains/quarantine.rs.

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

-- scientia_provider_runs, scientia_training_pairs: quarantined (DORMANT,
-- Task 4) — see domains/quarantine.rs.

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

-- Per-user identity binding for nanopublication signing (design §4.1).
CREATE TABLE IF NOT EXISTS user_identities (
    user_id            TEXT    PRIMARY KEY,
    orcid_id           TEXT,
    nanopub_pubkey_b64 TEXT,                       -- base64 PKCS#8 public key (nanopub crate format)
    nanopub_key_ref    TEXT    NOT NULL DEFAULT '',-- SecretId canonical env that holds the private key
    created_at_ms      INTEGER NOT NULL,
    updated_at_ms      INTEGER NOT NULL
);

-- Emitted (local/staged) nanopublications, one row per signed claim artifact.
CREATE TABLE IF NOT EXISTS scientia_nanopubs (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    trusty_uri        TEXT    NOT NULL UNIQUE,
    claim_id          INTEGER NOT NULL,
    publication_id    TEXT,
    user_id           TEXT    NOT NULL,
    orcid_id          TEXT,
    trig              TEXT    NOT NULL,
    validated_offline INTEGER NOT NULL DEFAULT 0,   -- 1 once the reference validator passes
    published_state   TEXT    NOT NULL DEFAULT 'local',  -- local|test_server|published (only 'local' used in this phase)
    created_at_ms     INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_scientia_nanopubs_claim ON scientia_nanopubs(claim_id);

-- LLM embedding vector cache keyed by sha256(model+text) for novelty scoring.
-- Upsert semantics: INSERT OR REPLACE so the latest call always wins.
CREATE TABLE IF NOT EXISTS scientia_embedding_cache (
    text_sha256   TEXT    PRIMARY KEY,
    model         TEXT    NOT NULL,
    vector_json   TEXT    NOT NULL,
    created_at_ms INTEGER NOT NULL
);

-- Per-producer scan cursor for automated discovery producers (commit-watcher, ...).
-- `last_seen` is producer-defined (e.g. the last scanned commit sha). Single row
-- per producer; advanced ONLY after a batch's draft inserts succeed.
CREATE TABLE IF NOT EXISTS scientia_producer_cursor (
    producer      TEXT    PRIMARY KEY,
    last_seen     TEXT    NOT NULL,
    updated_at_ms INTEGER NOT NULL
);

-- Discovery inbox: a surfacing index over draft publication manifests produced
-- by automated discovery producers (commit-watcher, ...). One row per surfaced
-- candidate; the GUI lists unacknowledged rows as "new research candidate"
-- alerts and a WS poller broadcasts new ids on `scientia.discovery.surfaced`.
-- DERIVED/regenerable: the draft manifest is the source of truth, so this table
-- is in `LEGACY_EXPORT_SKIP_TABLES`. `signal_codes` is a JSON array string.
CREATE TABLE IF NOT EXISTS scientia_discovery_inbox (
    id                 INTEGER PRIMARY KEY AUTOINCREMENT,
    publication_id     TEXT    NOT NULL,
    surfaced_at_ms     INTEGER NOT NULL,
    intake_tier        TEXT    NOT NULL,
    signal_codes       TEXT    NOT NULL,                  -- JSON array of signal codes
    acknowledged_at_ms INTEGER
);
-- NOTE: deliberately NO secondary index on this table. An index covering
-- `acknowledged_at_ms` trips a Turso/libSQL bug on UPDATE when the column moves
-- off NULL (IdxDelete "no matching index entry"). This is a small, derived
-- surfacing table; the unacknowledged scan and the id-diff are cheap without one.

-- Append-only per-claim human review decisions (design §5.1). Latest by decided_at_ms wins.
CREATE TABLE IF NOT EXISTS scientia_review_decisions (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    claim_id          INTEGER NOT NULL,
    publication_id    TEXT,
    bound_digest      TEXT    NOT NULL,         -- publication content_sha3_256 at decision time
    decision          TEXT    NOT NULL,         -- approved|rejected|deferred|edited (validated in Rust)
    actor             TEXT    NOT NULL,         -- human user_id (local_user_id())
    reason            TEXT,
    model_fingerprints_json TEXT,               -- artifact-side model fps present (for AI disclosure)
    decided_at_ms     INTEGER NOT NULL
);
-- Append-only is enforced at the Rust ops boundary (only INSERT + SELECT exist;
-- there is no UPDATE/DELETE op). DB-level triggers are NOT used because Turso/
-- libSQL rejects `CREATE TRIGGER` ("experimental feature; enable with
-- --experimental-triggers"), the same constraint that rules out SQL CHECK here.
CREATE INDEX IF NOT EXISTS idx_scientia_review_decisions_claim
    ON scientia_review_decisions(claim_id, decided_at_ms);

-- Track F: Learned per-model prompt-guidance profiles.
-- One row per (prompt_profile_key, variant_id). Only `Confirmed` variants are
-- injected into the system prompt; others are in the autonomic pipeline.
CREATE TABLE IF NOT EXISTS model_prompt_profiles (
    prompt_profile_key  TEXT    NOT NULL,
    variant_id          TEXT    NOT NULL,
    preamble_text       TEXT    NOT NULL DEFAULT '',
    confidence          TEXT    NOT NULL DEFAULT 'provisional',
    quality_delta       REAL    NOT NULL DEFAULT 0.0,
    applications        INTEGER NOT NULL DEFAULT 0,
    created_at_ms       INTEGER NOT NULL,
    approved_by         TEXT,
    PRIMARY KEY (prompt_profile_key, variant_id)
);

CREATE INDEX IF NOT EXISTS idx_model_prompt_profiles_key
    ON model_prompt_profiles(prompt_profile_key, confidence);

-- Harness issue discovery (Phase 1): repeated-correction patterns detected
-- during live chat/agent turns, plus static staleness findings from golden-
-- corpus scans. Distinct from scientia_discovery_inbox/scientia_review_decisions,
-- which are tightly bound to publication_id/claim_id (research findings).
-- No SQL CHECK/TRIGGER (Turso/libSQL does not support them); validated in Rust.
CREATE TABLE IF NOT EXISTS scientia_harness_issues (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    source         TEXT    NOT NULL,          -- chat_session|corpus_scan (validated in Rust)
    session_key    TEXT,                      -- null for corpus_scan
    target_path    TEXT,                      -- repo-relative path when this issue is tied to a
                                               -- specific corpus file (always set for corpus_scan;
                                               -- null for chat_session in v1 — see spec Out of scope)
    detected_at_ms INTEGER NOT NULL,
    category       TEXT    NOT NULL,
    severity       TEXT    NOT NULL,           -- low|medium|high (validated in Rust)
    summary        TEXT    NOT NULL,
    evidence_json  TEXT    NOT NULL,           -- redacted via vox_redact before storage
    status         TEXT    NOT NULL            -- pending|confirmed|dismissed (validated in Rust)
);
CREATE INDEX IF NOT EXISTS idx_scientia_harness_issues_status
    ON scientia_harness_issues(status);
CREATE INDEX IF NOT EXISTS idx_scientia_harness_issues_session
    ON scientia_harness_issues(session_key);
-- Enforces the chat_session dedup rule at the database level (PR review: the
-- app-level has_pending_harness_issue_for_session check-then-insert has a
-- race window between two concurrently-spawned judge tasks). Scoped to
-- chat_session/pending only — corpus_scan dedups on target_path instead of
-- session_key, via has_pending_harness_issue's separate check.
CREATE UNIQUE INDEX IF NOT EXISTS idx_scientia_harness_issues_pending_session_category
    ON scientia_harness_issues(session_key, category)
    WHERE status = 'pending' AND source = 'chat_session';

-- Append-only decision ledger for scientia_harness_issues (mirrors
-- scientia_review_decisions: only INSERT + SELECT ops exist, no UPDATE/DELETE).
CREATE TABLE IF NOT EXISTS scientia_harness_decisions (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    issue_id       INTEGER NOT NULL REFERENCES scientia_harness_issues(id) ON DELETE CASCADE,
    decision       TEXT    NOT NULL,           -- confirmed|dismissed (validated in Rust)
    actor          TEXT    NOT NULL,
    reason         TEXT,
    decided_at_ms  INTEGER NOT NULL
);

-- Dispatch-to-fix proposals for corpus-fixable confirmed issues (v1: those
-- with a non-null target_path). proposed_content is the full replacement
-- file content — the actual apply source of truth. proposed_diff is a
-- unified diff computed ONLY for human display; it is never parsed back
-- into content (a diff with context lines cannot be losslessly
-- reconstructed by filtering `+` lines, which is what an earlier draft of
-- this plan did — that truncated the applied file to just the changed
-- lines. Storing the real content directly avoids that class of bug).
CREATE TABLE IF NOT EXISTS scientia_harness_fix_proposals (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    issue_id         INTEGER NOT NULL REFERENCES scientia_harness_issues(id) ON DELETE CASCADE,
    target_path      TEXT    NOT NULL,
    proposed_content TEXT    NOT NULL,
    proposed_diff    TEXT    NOT NULL,          -- display-only, see above
    status           TEXT    NOT NULL,          -- pending_approval|applied|rejected (validated in Rust)
    proposed_at_ms   INTEGER NOT NULL,
    resolved_at_ms   INTEGER
);
"#;
