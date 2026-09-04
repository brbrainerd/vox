//! Arca SQL: Agent orchestration, reliability, and behavioral learning.
//!
//! ## Plane vs `research_metrics`
//!
//! - **`memories`**, **`behavior_events`**, **`llm_interactions`**, **`conversation_messages`** (see related domains):
//!   mix **S2–S3** (content, prompts, behavioral context). They are **not** interchangeable with coarse usage counters.
//! - **Usage-style counters** for product analytics belong under explicit `research_metrics` producers with their own
//!   sensitivity class — see `docs/src/architecture/telemetry-retention-sensitivity-ssot.md`.
pub const SCHEMA_AGENTS: &str = "
CREATE TABLE IF NOT EXISTS memories (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    agent_id TEXT NOT NULL,
    session_id TEXT NOT NULL,
    memory_type TEXT NOT NULL,
    content TEXT NOT NULL,
    metadata TEXT,
    importance REAL NOT NULL DEFAULT 1.0,
    vcs_snapshot_id TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS behavior_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    context TEXT,
    metadata TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS learned_patterns (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id TEXT NOT NULL,
    pattern_type TEXT NOT NULL,
    category TEXT NOT NULL,
    description TEXT NOT NULL,
    confidence REAL NOT NULL,
    vcs_snapshot_id TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS llm_interactions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    user_id TEXT,
    tenant_id TEXT,
    prompt TEXT NOT NULL,
    response TEXT NOT NULL,
    model_version TEXT NOT NULL,
    task_category TEXT NOT NULL DEFAULT 'general',
    strength_tag TEXT NOT NULL DEFAULT 'generalist',
    trace_id TEXT,
    context_utilization_pct REAL,
    cache_read_tokens INTEGER,
    success INTEGER NOT NULL DEFAULT 1,
    latency_ms INTEGER,
    input_tokens INTEGER,
    output_tokens INTEGER,
    cost_usd REAL,
    -- Task M3: time to first token / time per output token, ms. NULL for calls that
    -- didn't measure them (most callers as of this column's introduction -- see
    -- crates/vox-db-types/src/store_types/params.rs's ModelOutcome doc comment).
    ttft_ms INTEGER,
    tpot_ms REAL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS llm_feedback (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    interaction_id INTEGER NOT NULL,
    user_id TEXT,
    rating INTEGER,
    feedback_type TEXT NOT NULL,
    correction_text TEXT,
    preferred_response TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS llm_attempts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    trace_id TEXT NOT NULL,
    attempt_number INTEGER NOT NULL,
    model_id TEXT NOT NULL,
    provider TEXT NOT NULL,
    outcome TEXT NOT NULL,
    latency_ms INTEGER,
    error_class TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS artifacts (
    id TEXT PRIMARY KEY,
    artifact_type TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    author_id TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    version TEXT NOT NULL,
    tags TEXT,
    status TEXT NOT NULL DEFAULT 'public',
    downloads INTEGER NOT NULL DEFAULT 0,
    avg_rating REAL NOT NULL DEFAULT 0.0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- artifact_reviews: quarantined (DEAD, Task 4) — see domains/quarantine.rs.

CREATE TABLE IF NOT EXISTS agents (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    system_prompt TEXT,
    tools TEXT,
    model_config TEXT,
    owner_id TEXT,
    version TEXT NOT NULL,
    is_public INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- invocation_count/success_count drive skill reliability scoring returned by corpus_export.
CREATE TABLE IF NOT EXISTS skill_manifests (
    id TEXT NOT NULL,
    version TEXT NOT NULL,
    manifest_json TEXT NOT NULL,
    skill_md TEXT NOT NULL,
    invocation_count INTEGER NOT NULL DEFAULT 0,
    success_count INTEGER NOT NULL DEFAULT 0,
    last_used_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (id, version)
);

-- skill_executions: quarantined (DORMANT, Task 4) — see domains/quarantine.rs.

CREATE TABLE IF NOT EXISTS db_snapshots (
    id INTEGER PRIMARY KEY,
    agent_id TEXT NOT NULL,
    description TEXT NOT NULL,
    payload TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS research_metrics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    metric_type TEXT NOT NULL,
    metric_value REAL,
    metadata_json TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS eval_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id TEXT NOT NULL UNIQUE,
    model_path TEXT,
    format_validity REAL,
    safety_rejection_rate REAL,
    quality_proxy REAL,
    skills_discovered INTEGER,
    workflows_discovered INTEGER,
    metadata_json TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- builder_sessions, session_turns, typed_stream_events: quarantined (DEAD, Task 4) — see domains/quarantine.rs.

CREATE TABLE IF NOT EXISTS question_sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    repository_id TEXT NOT NULL DEFAULT '',
    task_id TEXT,
    policy_version TEXT NOT NULL DEFAULT 'v1',
    started_at_ms INTEGER NOT NULL,
    ended_at_ms INTEGER,
    resolution_status TEXT NOT NULL DEFAULT 'open',
    belief_state_json TEXT
);

CREATE TABLE IF NOT EXISTS question_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    question_session_id INTEGER NOT NULL REFERENCES question_sessions(id) ON DELETE CASCADE,
    question_id TEXT NOT NULL,
    turn_index INTEGER NOT NULL,
    actor TEXT NOT NULL DEFAULT 'assistant',
    question_kind TEXT NOT NULL,
    prompt TEXT NOT NULL,
    expected_information_gain_bits REAL NOT NULL DEFAULT 0.0,
    expected_user_cost REAL NOT NULL DEFAULT 0.0,
    utility_bits_per_cost REAL NOT NULL DEFAULT 0.0,
    answer_text TEXT,
    answer_type TEXT,
    answered_at_ms INTEGER,
    created_at_ms INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS question_options (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    question_event_id INTEGER NOT NULL REFERENCES question_events(id) ON DELETE CASCADE,
    option_id TEXT NOT NULL,
    label TEXT NOT NULL,
    prior_probability REAL,
    posterior_probability REAL,
    is_other INTEGER NOT NULL DEFAULT 0,
    UNIQUE(question_event_id, option_id)
);

-- question_option_outcomes: quarantined (DORMANT, Task 4) — see domains/quarantine.rs.

CREATE TABLE IF NOT EXISTS question_stop_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    question_session_id INTEGER NOT NULL REFERENCES question_sessions(id) ON DELETE CASCADE,
    stop_reason TEXT NOT NULL,
    confidence_at_stop REAL,
    marginal_gain_bits REAL,
    expected_user_cost REAL,
    turn_index INTEGER,
    created_at_ms INTEGER NOT NULL
);

-- populi_reviews: quarantined (DEAD, Task 4) — see domains/quarantine.rs.

CREATE TABLE IF NOT EXISTS agent_sessions (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    tenant_id TEXT,
    agent_name TEXT,
    started_at TEXT NOT NULL DEFAULT (datetime('now')),
    ended_at TEXT,
    status TEXT NOT NULL DEFAULT 'active',
    task_snapshot TEXT,
    context_summary TEXT
);

CREATE TABLE IF NOT EXISTS agent_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    agent_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    payload_json TEXT,
    cli_version TEXT,
    timestamp TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Orchestrator MCP session replay (SSOT when DB attached); JSONL is optional export only.
CREATE TABLE IF NOT EXISTS agent_session_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_agent_session_events_session ON agent_session_events(session_id);

CREATE TABLE IF NOT EXISTS agent_operations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    ts_ms INTEGER NOT NULL,
    session_id TEXT,
    agent_id TEXT,
    tool_name TEXT NOT NULL,
    args_redacted TEXT NOT NULL,
    result_redacted TEXT,
    duration_ms INTEGER,
    is_error INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_agent_operations_session ON agent_operations(session_id, ts_ms);
CREATE INDEX IF NOT EXISTS idx_agent_operations_tool ON agent_operations(tool_name);

-- Candidate skills discovered by the skill-mining pipeline (code_miner /
-- op_miner in vox-skill-discovery), persisted so a mining run is no longer
-- fire-and-forget stdout output. `status` is the coarse review outcome
-- (pending/reviewed/promoted/rejected, see ops_skill_candidates.rs); once a
-- candidate reaches `promoted` its `lifecycle_state` (Task 3.3) tracks the
-- provisional -> confirmed -> deprecated shadow-period state machine,
-- mirroring `vox_orchestrator::models::autonomic::ModelConfidence`.
-- `source_hash` binds a promoted skill's identity to the trajectory it was
-- derived from (blake3 of `raw_json`), so a later mining run that
-- produces a materially different trajectory under the same
-- `candidate_name` can be detected and forced back through verification
-- instead of silently overwriting the confirmed skill (Task 3.3 gate 8).
CREATE TABLE IF NOT EXISTS skill_candidates (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    candidate_name TEXT NOT NULL,
    source TEXT NOT NULL,
    raw_json TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    lifecycle_state TEXT NOT NULL DEFAULT 'provisional',
    source_hash TEXT,
    created_at_ms INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_skill_candidates_status ON skill_candidates(status, created_at_ms);
CREATE INDEX IF NOT EXISTS idx_skill_candidates_name ON skill_candidates(candidate_name);

CREATE TABLE IF NOT EXISTS cost_records (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    agent_id TEXT NOT NULL,
    session_id TEXT,
    tenant_id TEXT,
    provider TEXT NOT NULL,
    model TEXT,
    input_tokens INTEGER NOT NULL DEFAULT 0,
    output_tokens INTEGER NOT NULL DEFAULT 0,
    cost_usd REAL NOT NULL DEFAULT 0.0,
    timestamp TEXT NOT NULL DEFAULT (datetime('now'))
);


CREATE TABLE IF NOT EXISTS agent_metrics (
    agent_id TEXT NOT NULL,
    metric_name TEXT NOT NULL,
    metric_value REAL NOT NULL DEFAULT 0.0,
    period TEXT NOT NULL DEFAULT 'session',
    timestamp TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (agent_id, metric_name, period)
);

-- TEXT PK matches agents.id TEXT; success_count/failure_count power Laplace-smoothed reliability.
CREATE TABLE IF NOT EXISTS agent_reliability (
    agent_id TEXT NOT NULL PRIMARY KEY,
    reliability REAL NOT NULL DEFAULT 0.5,
    success_count INTEGER NOT NULL DEFAULT 0,
    failure_count INTEGER NOT NULL DEFAULT 0,
    updated_at_ms INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS research_sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_key TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'active',
    repository_id TEXT NOT NULL DEFAULT '',
    config_json TEXT,
    summary_json TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS endpoint_reliability (
    id                        INTEGER PRIMARY KEY AUTOINCREMENT,
    endpoint_url              TEXT NOT NULL,
    model_id                  TEXT NOT NULL,
    total_requests            INTEGER NOT NULL DEFAULT 0,
    hallucination_proxy_ewma  REAL    NOT NULL DEFAULT 0.0,
    contradiction_ratio_ewma  REAL    NOT NULL DEFAULT 0.0,
    infra_failure_ewma        REAL    NOT NULL DEFAULT 0.0,
    rate_limit_hits           INTEGER NOT NULL DEFAULT 0,
    timeout_hits              INTEGER NOT NULL DEFAULT 0,
    updated_at_ms             INTEGER NOT NULL DEFAULT 0,
    UNIQUE(endpoint_url, model_id)
);

-- skill_reliability: quarantined (DORMANT, Task 4) — see domains/quarantine.rs
-- (still superseded by `reliability_scores`, see that file's header comment).

CREATE TABLE IF NOT EXISTS workflow_reliability (
    workflow_name      TEXT NOT NULL PRIMARY KEY,
    reliability        REAL NOT NULL DEFAULT 0.5,
    success_count      INTEGER NOT NULL DEFAULT 0,
    failure_count      INTEGER NOT NULL DEFAULT 0,
    updated_at_ms      INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS repository_reliability (
    repository_id      TEXT NOT NULL PRIMARY KEY,
    reliability        REAL NOT NULL DEFAULT 0.5,
    success_count      INTEGER NOT NULL DEFAULT 0,
    failure_count      INTEGER NOT NULL DEFAULT 0,
    updated_at_ms      INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS trust_observations (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    entity_type         TEXT    NOT NULL,
    entity_id           TEXT    NOT NULL,
    dimension           TEXT    NOT NULL,
    domain              TEXT    NOT NULL DEFAULT '',
    task_class          TEXT    NOT NULL DEFAULT '',
    provider            TEXT    NOT NULL DEFAULT '',
    model_id            TEXT    NOT NULL DEFAULT '',
    repository_id       TEXT    NOT NULL DEFAULT '',
    source_kind         TEXT    NOT NULL DEFAULT '',
    observation_value   REAL    NOT NULL,
    confidence_weight   REAL    NOT NULL DEFAULT 1.0,
    sample_size         INTEGER NOT NULL DEFAULT 1,
    artifact_ref        TEXT,
    metadata_json       TEXT,
    created_at_ms       INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS trust_rollups (
    entity_type         TEXT    NOT NULL,
    entity_id           TEXT    NOT NULL,
    dimension           TEXT    NOT NULL,
    domain              TEXT    NOT NULL DEFAULT '',
    task_class          TEXT    NOT NULL DEFAULT '',
    provider            TEXT    NOT NULL DEFAULT '',
    model_id            TEXT    NOT NULL DEFAULT '',
    repository_id       TEXT    NOT NULL DEFAULT '',
    score               REAL    NOT NULL DEFAULT 0.5,
    sample_size         INTEGER NOT NULL DEFAULT 0,
    ewma_alpha          REAL    NOT NULL DEFAULT 0.10,
    updated_at_ms       INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (entity_type, entity_id, dimension, domain, task_class, provider, model_id, repository_id)
);

-- trusted_evidence_bundles: quarantined (DORMANT, Task 4) — see domains/quarantine.rs.

CREATE INDEX IF NOT EXISTS idx_memories_agent ON memories(agent_id);
CREATE INDEX IF NOT EXISTS idx_memories_type ON memories(memory_type);
CREATE INDEX IF NOT EXISTS idx_memories_created ON memories(created_at);
CREATE INDEX IF NOT EXISTS idx_memories_agent_created ON memories(agent_id, created_at);
CREATE INDEX IF NOT EXISTS idx_behavior_user ON behavior_events(user_id);
CREATE INDEX IF NOT EXISTS idx_behavior_type ON behavior_events(event_type);
CREATE INDEX IF NOT EXISTS idx_behavior_user_created ON behavior_events(user_id, created_at);
CREATE INDEX IF NOT EXISTS idx_learned_patterns_user ON learned_patterns(user_id);
CREATE INDEX IF NOT EXISTS idx_learned_patterns_category ON learned_patterns(user_id, category);
CREATE INDEX IF NOT EXISTS idx_llm_interactions_session ON llm_interactions(session_id);
CREATE INDEX IF NOT EXISTS idx_llm_feedback_interaction ON llm_feedback(interaction_id);
CREATE INDEX IF NOT EXISTS idx_artifacts_type ON artifacts(artifact_type);
CREATE INDEX IF NOT EXISTS idx_artifacts_name ON artifacts(name);
CREATE INDEX IF NOT EXISTS idx_agents_name ON agents(name);
CREATE INDEX IF NOT EXISTS idx_skill_manifests_id ON skill_manifests(id);

-- First-come-first-served namespace/identity claims for skills (Task 3.4,
-- harness parity plan). identity is the canonical <namespace>/<name>
-- string parsed/validated by vox_plugin_types::skill_identity::SkillIdentity
-- (e.g. local/my-skill or io.github.alice/my-skill). The PRIMARY KEY is
-- the uniqueness constraint: once an identity is claimed, a second distinct
-- owner cannot claim it (see claim_skill_identity in
-- store/ops_skill_identity.rs), preventing namespace squatting. owner is
-- presently the skill manifest's free-text author field, NOT a
-- cryptographically verified identity -- this codebase has no GitHub
-- OAuth/OIDC or other identity-proof mechanism (documented deferred gap;
-- see skill_identity.rs module docs).
CREATE TABLE IF NOT EXISTS skill_identities (
    identity TEXT PRIMARY KEY,
    owner TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_agent_reliability_score ON agent_reliability(reliability);
CREATE INDEX IF NOT EXISTS idx_research_metrics_session ON research_metrics(session_id, metric_type);
CREATE INDEX IF NOT EXISTS idx_question_sessions_session ON question_sessions(session_id, started_at_ms);
CREATE INDEX IF NOT EXISTS idx_question_sessions_repo ON question_sessions(repository_id, started_at_ms);
CREATE INDEX IF NOT EXISTS idx_question_events_session ON question_events(question_session_id, turn_index);
CREATE INDEX IF NOT EXISTS idx_question_events_qid ON question_events(question_id);
CREATE INDEX IF NOT EXISTS idx_question_options_event ON question_options(question_event_id);
CREATE INDEX IF NOT EXISTS idx_question_stop_events_session ON question_stop_events(question_session_id, created_at_ms);
CREATE INDEX IF NOT EXISTS idx_agent_sessions_agent ON agent_sessions(agent_id);
CREATE INDEX IF NOT EXISTS idx_agent_sessions_status ON agent_sessions(status);
CREATE INDEX IF NOT EXISTS idx_agent_events_agent ON agent_events(agent_id);
CREATE INDEX IF NOT EXISTS idx_agent_events_type ON agent_events(event_type);
CREATE INDEX IF NOT EXISTS idx_agent_events_ts ON agent_events(timestamp);
CREATE INDEX IF NOT EXISTS idx_cost_records_agent ON cost_records(agent_id);
CREATE INDEX IF NOT EXISTS idx_cost_records_session ON cost_records(session_id);
CREATE INDEX IF NOT EXISTS idx_cost_records_ts ON cost_records(timestamp);

CREATE INDEX IF NOT EXISTS idx_research_sessions_repo_created ON research_sessions(repository_id, created_at);
CREATE INDEX IF NOT EXISTS idx_research_sessions_status ON research_sessions(status);
CREATE INDEX IF NOT EXISTS idx_endpoint_reliability_degraded ON endpoint_reliability(hallucination_proxy_ewma, endpoint_url);
CREATE INDEX IF NOT EXISTS idx_workflow_reliability_score ON workflow_reliability(reliability);
CREATE INDEX IF NOT EXISTS idx_repository_reliability_score ON repository_reliability(reliability);
CREATE INDEX IF NOT EXISTS idx_trust_observations_entity_dim ON trust_observations(entity_type, entity_id, dimension, created_at_ms);
CREATE INDEX IF NOT EXISTS idx_trust_observations_scope ON trust_observations(domain, task_class, provider, model_id, repository_id);
CREATE INDEX IF NOT EXISTS idx_trust_rollups_entity_dim ON trust_rollups(entity_type, entity_id, dimension, score);

-- Bounded routing decision log (journeys, surface, model selection).
-- No prompt/body content — coarse metadata only.
CREATE TABLE IF NOT EXISTS routing_decisions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    journey_id TEXT,
    repository_id TEXT,
    session_id TEXT,
    surface TEXT NOT NULL DEFAULT '',
    model_id TEXT,
    reason_json TEXT
);
CREATE INDEX IF NOT EXISTS idx_routing_decisions_created ON routing_decisions(created_at);
CREATE INDEX IF NOT EXISTS idx_routing_decisions_journey ON routing_decisions(journey_id);

CREATE TABLE IF NOT EXISTS hopper_inbox (
    item_id TEXT PRIMARY KEY,
    intent TEXT NOT NULL,
    affinity_json TEXT NOT NULL,
    priority INTEGER NOT NULL,
    source TEXT NOT NULL,
    session_id TEXT,
    state TEXT NOT NULL,
    submitted_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_hopper_inbox_state ON hopper_inbox(state);
";
