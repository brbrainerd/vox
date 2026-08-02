//! Quarantined DDL for tables classified DORMANT or DEAD by the automated
//! two-pass census (`scripts/db-table-census.vox`; report at
//! `graphify-out/table_usage_report.json`) as of Task 1b / Task 4 of the
//! VoxDB audit condensation plan
//! (`docs/src/architecture/2026-08-01-voxdb-audit-condensation-plan.md`,
//! see the "Task 2 — SUPERSEDED" section for the reconciliation history).
//!
//! This DDL only compiles into the schema baseline when the `quarantine`
//! feature is enabled (see `crates/vox-db/Cargo.toml` and
//! `schema::manifest::baseline_sql`); it is OFF by default. Each table below
//! carries a status (DEAD = zero references anywhere in current source;
//! DORMANT = internal `vox-db` wrapper functions exist but have no caller
//! outside `vox-db`/`vox-db-types`), the referencing-file trail from the
//! census report (or "none" for DEAD), and evidence type (all entries here
//! are `literal_string` evidence — a literal `CREATE TABLE`/table-name match,
//! as opposed to a promoted wrapper-call finding).
//!
//! `handoff_payloads` (DEAD) is declared only via a `CollectionInfo` entry in
//! `schema::spec::orchestrator_schema_digest()`, not literal DDL, so it is not
//! listed here — see that function for its quarantine handling.
//!
//! Five quarantine-candidate tables have NO declaration anywhere in current
//! source (not even here): `archive_membership`, `chunk_members`,
//! `context_window_items`, `context_windows`, `zstd_dictionaries`. They need
//! no schema-file change and are handled only by Task 5 (`DROP TABLE` against
//! existing databases, out of this task's scope).

pub const SCHEMA_QUARANTINE: &str = "
-- Table: usage_counter_snapshots
-- Status: DORMANT (graphify-out/table_usage_report.json)
-- Evidence: literal_string
-- Referencing files: crates/vox-db/src/codex_chat.rs
-- Origin: crates/vox-db/src/schema/domains/foundation.rs
CREATE TABLE IF NOT EXISTS usage_counter_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    metric_key TEXT NOT NULL,
    scope_kind TEXT NOT NULL,
    scope_id TEXT NOT NULL DEFAULT '',
    period_start TEXT NOT NULL,
    amount INTEGER NOT NULL DEFAULT 0,
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(metric_key, scope_kind, scope_id, period_start)
);
CREATE INDEX IF NOT EXISTS idx_usage_counters_lookup
    ON usage_counter_snapshots(metric_key, scope_kind, scope_id, period_start);

-- Table: usage_limit_definitions
-- Status: DORMANT (graphify-out/table_usage_report.json)
-- Evidence: literal_string
-- Referencing files: crates/vox-db/src/codex_chat.rs
-- Origin: crates/vox-db/src/schema/domains/foundation.rs
CREATE TABLE IF NOT EXISTS usage_limit_definitions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    metric_key TEXT NOT NULL,
    scope_kind TEXT NOT NULL,
    scope_id TEXT NOT NULL DEFAULT '',
    period_kind TEXT NOT NULL,
    limit_value INTEGER NOT NULL,
    enforcement TEXT NOT NULL DEFAULT 'hard',
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(metric_key, scope_kind, scope_id, period_kind)
);
CREATE INDEX IF NOT EXISTS idx_usage_limit_defs_lookup
    ON usage_limit_definitions(metric_key, scope_kind, scope_id);

-- Table: codex_change_log
-- Status: DORMANT (graphify-out/table_usage_report.json)
-- Evidence: literal_string
-- Referencing files: crates/vox-db-types/src/store_types/rows_core.rs, crates/vox-db/src/store/ops_codex/codex_graph.rs, crates/vox-db/src/types/store_types/rows_core.rs
-- Origin: crates/vox-db/src/schema/domains/cas_codex.rs
CREATE TABLE IF NOT EXISTS codex_change_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    topic TEXT NOT NULL,
    entity_kind TEXT,
    entity_id TEXT,
    change_kind TEXT NOT NULL,
    payload_json TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_codex_change_log_topic ON codex_change_log(topic);
CREATE INDEX IF NOT EXISTS idx_codex_change_log_topic_id ON codex_change_log(topic, id);
CREATE INDEX IF NOT EXISTS idx_codex_change_log_created ON codex_change_log(created_at);

-- Table: codex_projection_versions
-- Status: DEAD (graphify-out/table_usage_report.json)
-- Evidence: literal_string
-- Referencing files: none
-- Origin: crates/vox-db/src/schema/domains/cas_codex.rs
CREATE TABLE IF NOT EXISTS codex_projection_versions (
    projection_name TEXT NOT NULL,
    version INTEGER NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (projection_name, version)
);

-- Table: codex_query_snapshots
-- Status: DEAD (graphify-out/table_usage_report.json)
-- Evidence: literal_string
-- Referencing files: none
-- Origin: crates/vox-db/src/schema/domains/cas_codex.rs
CREATE TABLE IF NOT EXISTS codex_query_snapshots (
    id TEXT PRIMARY KEY,
    query_name TEXT NOT NULL,
    snapshot_json TEXT NOT NULL,
    digest TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_codex_query_snapshots_name ON codex_query_snapshots(query_name);

-- Table: codex_schema_lineage
-- Status: DORMANT (graphify-out/table_usage_report.json)
-- Evidence: literal_string
-- Referencing files: crates/vox-db/src/store/ops_codex/codex_graph.rs
-- Origin: crates/vox-db/src/schema/domains/cas_codex.rs
CREATE TABLE IF NOT EXISTS codex_schema_lineage (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    baseline_id TEXT NOT NULL,
    schema_digest TEXT NOT NULL,
    provenance TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_codex_schema_lineage_baseline ON codex_schema_lineage(baseline_id);

-- Table: codex_subscriptions
-- Status: DEAD (graphify-out/table_usage_report.json)
-- Evidence: literal_string
-- Referencing files: none
-- Origin: crates/vox-db/src/schema/domains/cas_codex.rs
CREATE TABLE IF NOT EXISTS codex_subscriptions (
    id TEXT PRIMARY KEY,
    topic TEXT NOT NULL,
    filter_json TEXT,
    client_hint TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_codex_subscriptions_topic ON codex_subscriptions(topic);

-- Table: processing_run_steps
-- Status: DEAD (graphify-out/table_usage_report.json)
-- Evidence: literal_string
-- Referencing files: none
-- Origin: crates/vox-db/src/schema/domains/cas_codex.rs
CREATE TABLE IF NOT EXISTS processing_run_steps (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    processing_run_id INTEGER NOT NULL REFERENCES processing_runs(id) ON DELETE CASCADE,
    step_index INTEGER NOT NULL,
    step_name TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    detail_json TEXT,
    started_at_ms INTEGER NOT NULL DEFAULT 0,
    finished_at_ms INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(processing_run_id, step_index)
);
CREATE INDEX IF NOT EXISTS idx_processing_run_steps_run ON processing_run_steps(processing_run_id);

-- Table: processing_runs
-- Status: DEAD (graphify-out/table_usage_report.json)
-- Evidence: literal_string
-- Referencing files: none
-- Origin: crates/vox-db/src/schema/domains/cas_codex.rs
CREATE TABLE IF NOT EXISTS processing_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_kind TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'queued',
    scope_kind TEXT NOT NULL DEFAULT '',
    scope_id TEXT NOT NULL DEFAULT '',
    correlation_id TEXT NOT NULL DEFAULT '',
    payload_json TEXT,
    error_text TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    started_at TEXT,
    completed_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_processing_runs_status_created ON processing_runs(status, created_at);
CREATE INDEX IF NOT EXISTS idx_processing_runs_scope ON processing_runs(scope_kind, scope_id);
CREATE INDEX IF NOT EXISTS idx_processing_runs_kind ON processing_runs(run_kind);

-- Table: conversation_edges
-- Status: DORMANT (graphify-out/table_usage_report.json)
-- Evidence: literal_string
-- Referencing files: crates/vox-db/src/codex_conversation_graph.rs, crates/vox-db/src/store/ops_codex/codex_graph.rs
-- Origin: crates/vox-db/src/schema/domains/conversations.rs
CREATE TABLE IF NOT EXISTS conversation_edges (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    from_conversation_id INTEGER NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    to_conversation_id INTEGER NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    edge_kind TEXT NOT NULL DEFAULT 'related',
    weight REAL NOT NULL DEFAULT 1.0,
    metadata_json TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_conversation_edges_from ON conversation_edges(from_conversation_id);
CREATE INDEX IF NOT EXISTS idx_conversation_edges_to ON conversation_edges(to_conversation_id);

-- Table: conversation_message_topics
-- Status: DORMANT (graphify-out/table_usage_report.json)
-- Evidence: literal_string
-- Referencing files: crates/vox-db/src/codex_chat.rs
-- Origin: crates/vox-db/src/schema/domains/conversations.rs
CREATE TABLE IF NOT EXISTS conversation_message_topics (
    conversation_message_id INTEGER NOT NULL REFERENCES conversation_messages(id) ON DELETE CASCADE,
    topic_id INTEGER NOT NULL REFERENCES topics(id) ON DELETE CASCADE,
    PRIMARY KEY (conversation_message_id, topic_id)
);
CREATE INDEX IF NOT EXISTS idx_conversation_message_topics_topic ON conversation_message_topics(topic_id);

-- Table: conversation_tool_calls
-- Status: DORMANT (graphify-out/table_usage_report.json)
-- Evidence: literal_string
-- Referencing files: crates/vox-db/src/codex_chat.rs
-- Origin: crates/vox-db/src/schema/domains/conversations.rs
CREATE TABLE IF NOT EXISTS conversation_tool_calls (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    conversation_message_id INTEGER NOT NULL REFERENCES conversation_messages(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL DEFAULT 0,
    tool_name TEXT NOT NULL,
    arguments_json TEXT NOT NULL DEFAULT '{}',
    result_json TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    error_text TEXT,
    started_at_ms INTEGER NOT NULL DEFAULT 0,
    finished_at_ms INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_conversation_tool_calls_msg_ord ON conversation_tool_calls(conversation_message_id, ordinal);
CREATE INDEX IF NOT EXISTS idx_conversation_tool_calls_tool ON conversation_tool_calls(tool_name);
CREATE INDEX IF NOT EXISTS idx_conversation_tool_calls_status ON conversation_tool_calls(status);

-- Table: conversation_topics
-- Status: DORMANT (graphify-out/table_usage_report.json)
-- Evidence: literal_string
-- Referencing files: crates/vox-db/src/codex_chat.rs
-- Origin: crates/vox-db/src/schema/domains/conversations.rs
CREATE TABLE IF NOT EXISTS conversation_topics (
    conversation_id INTEGER NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    topic_id INTEGER NOT NULL REFERENCES topics(id) ON DELETE CASCADE,
    weight REAL NOT NULL DEFAULT 1.0,
    PRIMARY KEY (conversation_id, topic_id)
);
CREATE INDEX IF NOT EXISTS idx_conversation_topics_topic ON conversation_topics(topic_id);

-- Table: conversation_versions
-- Status: DORMANT (graphify-out/table_usage_report.json)
-- Evidence: literal_string
-- Referencing files: crates/vox-db/src/codex_conversation_graph.rs, crates/vox-db/src/store/ops_codex/codex_graph.rs
-- Origin: crates/vox-db/src/schema/domains/conversations.rs
CREATE TABLE IF NOT EXISTS conversation_versions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    conversation_id INTEGER NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    version_index INTEGER NOT NULL,
    label TEXT NOT NULL DEFAULT '',
    snapshot_json TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(conversation_id, version_index)
);
CREATE INDEX IF NOT EXISTS idx_conversation_versions_conv ON conversation_versions(conversation_id);

-- Table: topic_evolution_events
-- Status: DORMANT (graphify-out/table_usage_report.json)
-- Evidence: literal_string
-- Referencing files: crates/vox-db/src/codex_conversation_graph.rs, crates/vox-db/src/store/ops_codex/codex_graph.rs
-- Origin: crates/vox-db/src/schema/domains/conversations.rs
CREATE TABLE IF NOT EXISTS topic_evolution_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    topic_id INTEGER NOT NULL REFERENCES topics(id) ON DELETE CASCADE,
    event_kind TEXT NOT NULL,
    prior_label TEXT,
    new_label TEXT,
    detail_json TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_topic_evolution_topic_created ON topic_evolution_events(topic_id, created_at);

-- Table: search_indexing_jobs
-- Status: DEAD (graphify-out/table_usage_report.json)
-- Evidence: literal_string
-- Referencing files: none
-- Origin: crates/vox-db/src/schema/domains/knowledge.rs
CREATE TABLE IF NOT EXISTS search_indexing_jobs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    job_kind TEXT NOT NULL,
    target_uri TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'queued',
    detail_json TEXT,
    error_text TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_search_jobs_status ON search_indexing_jobs(status);

-- Table: workflow_executions
-- Status: DORMANT (graphify-out/table_usage_report.json)
-- Evidence: literal_string
-- Referencing files: crates/vox-db-types/src/store_types/rows_extended.rs, crates/vox-db/src/store/ops_codex/codex_graph.rs, crates/vox-db/src/types/store_types/rows_extended.rs
-- Origin: crates/vox-db/src/schema/domains/execution.rs
-- Aggregate workflow-level record; links to execution_log rows via workflow_id.
CREATE TABLE IF NOT EXISTS workflow_executions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    workflow_id TEXT NOT NULL UNIQUE,
    agent_id TEXT,
    skill_id TEXT,
    status TEXT NOT NULL DEFAULT 'running',
    step_count INTEGER NOT NULL DEFAULT 0,
    steps_ok INTEGER NOT NULL DEFAULT 0,
    error_count INTEGER NOT NULL DEFAULT 0,
    total_duration_ms INTEGER NOT NULL DEFAULT 0,
    started_at TEXT NOT NULL DEFAULT (datetime('now')),
    finished_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_workflow_executions_agent ON workflow_executions(agent_id);
CREATE INDEX IF NOT EXISTS idx_workflow_executions_status ON workflow_executions(status);
CREATE INDEX IF NOT EXISTS idx_workflow_executions_skill ON workflow_executions(skill_id);

-- Table: external_review_outcome
-- Status: DORMANT (graphify-out/table_usage_report.json)
-- Evidence: literal_string
-- Referencing files: crates/vox-db/src/store/ops_external_review.rs
-- Origin: crates/vox-db/src/schema/domains/external_review.rs
CREATE TABLE IF NOT EXISTS external_review_outcome (
    id                    INTEGER PRIMARY KEY AUTOINCREMENT,
    finding_id            INTEGER NOT NULL REFERENCES external_review_finding(id) ON DELETE CASCADE,
    outcome_kind          TEXT    NOT NULL, -- task_generated|fix_merged|regression_observed|regression_resolved
    outcome_ref           TEXT,             -- task id / commit sha / eval run id
    outcome_json          TEXT,
    recorded_at           TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_external_review_outcome_finding
    ON external_review_outcome(finding_id, id);

-- Table: artifact_reviews
-- Status: DEAD (graphify-out/table_usage_report.json)
-- Evidence: literal_string
-- Referencing files: none
-- Origin: crates/vox-db/src/schema/domains/agents.rs
CREATE TABLE IF NOT EXISTS artifact_reviews (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    artifact_id TEXT NOT NULL,
    reviewer_id TEXT NOT NULL,
    status TEXT NOT NULL,
    comment TEXT,
    rating INTEGER,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_artifact_reviews_target ON artifact_reviews(artifact_id);

-- Table: builder_sessions
-- Status: DEAD (graphify-out/table_usage_report.json)
-- Evidence: literal_string
-- Referencing files: none
-- Origin: crates/vox-db/src/schema/domains/agents.rs
CREATE TABLE IF NOT EXISTS builder_sessions (
    id TEXT PRIMARY KEY,
    payload_json TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Table: populi_reviews
-- Status: DEAD (graphify-out/table_usage_report.json)
-- Evidence: literal_string
-- Referencing files: none
-- Origin: crates/vox-db/src/schema/domains/agents.rs
CREATE TABLE IF NOT EXISTS populi_reviews (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    target_id TEXT NOT NULL,
    review_kind TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_populi_reviews_target ON populi_reviews(target_id);

-- Table: question_option_outcomes
-- Status: DORMANT (graphify-out/table_usage_report.json)
-- Evidence: literal_string
-- Referencing files: crates/vox-db-types/src/store_types/rows_extended.rs, crates/vox-db/src/questioning_telemetry.rs, crates/vox-db/src/store/ops_questioning.rs, crates/vox-db/src/types/store_types/rows_extended.rs
-- Origin: crates/vox-db/src/schema/domains/agents.rs
CREATE TABLE IF NOT EXISTS question_option_outcomes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    question_event_id INTEGER NOT NULL REFERENCES question_events(id) ON DELETE CASCADE,
    option_id TEXT NOT NULL,
    selected INTEGER NOT NULL DEFAULT 0,
    diagnostic_weight REAL NOT NULL DEFAULT 0.0,
    information_contribution_bits REAL NOT NULL DEFAULT 0.0,
    created_at_ms INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_question_option_outcomes_event ON question_option_outcomes(question_event_id);

-- Table: session_turns
-- Status: DEAD (graphify-out/table_usage_report.json)
-- Evidence: literal_string
-- Referencing files: none
-- Origin: crates/vox-db/src/schema/domains/agents.rs
CREATE TABLE IF NOT EXISTS session_turns (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_session_turns_session ON session_turns(session_id);

-- Table: skill_executions
-- Status: DORMANT (graphify-out/table_usage_report.json)
-- Evidence: literal_string
-- Referencing files: crates/vox-db-types/src/store_types/rows_extended.rs, crates/vox-db/src/store/ops_codex/codex_graph.rs, crates/vox-db/src/types/store_types/rows_extended.rs, crates/vox-db/tests/ops_skill_tests.rs
-- Origin: crates/vox-db/src/schema/domains/agents.rs
-- Per-execution record for skills — SSOT for skill reliability scoring.
CREATE TABLE IF NOT EXISTS skill_executions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    skill_id TEXT NOT NULL,
    version TEXT NOT NULL DEFAULT '',
    session_id TEXT,
    workflow_id TEXT,
    agent_id TEXT,
    status TEXT NOT NULL,
    duration_ms INTEGER NOT NULL DEFAULT 0,
    input_hash TEXT,
    output_size INTEGER NOT NULL DEFAULT 0,
    error_kind TEXT,
    reflection_score REAL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_skill_executions_skill ON skill_executions(skill_id, version);
CREATE INDEX IF NOT EXISTS idx_skill_executions_status ON skill_executions(status, created_at);
CREATE INDEX IF NOT EXISTS idx_skill_executions_agent ON skill_executions(agent_id);

-- Table: skill_reliability
-- Status: DORMANT (graphify-out/table_usage_report.json)
-- Evidence: literal_string
-- Referencing files: crates/vox-db/src/store/open.rs, crates/vox-db/src/store/ops_codex/codex_metrics_packages.rs, crates/vox-db/src/store/ops_skills.rs
-- Origin: crates/vox-db/src/schema/domains/agents.rs
-- Superseded by `reliability_scores` (entity_type = 'skill') as of schema v51
-- (see `store/open.rs` migration notes and `store/ops_skills.rs`). Nothing
-- writes to this legacy table anymore; it is kept only so existing databases
-- don't lose historical rows on upgrade. Do NOT add new reads/writes against
-- this table — use `ops_skills::list_skill_reliability`/`get_skill_reliability`
-- (which read `reliability_scores`) instead.
CREATE TABLE IF NOT EXISTS skill_reliability (
    skill_id           TEXT NOT NULL PRIMARY KEY,
    reliability        REAL NOT NULL DEFAULT 0.5,
    success_count      INTEGER NOT NULL DEFAULT 0,
    failure_count      INTEGER NOT NULL DEFAULT 0,
    updated_at_ms      INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_skill_reliability_score ON skill_reliability(reliability);

-- Table: trusted_evidence_bundles
-- Status: DORMANT (graphify-out/table_usage_report.json)
-- Evidence: literal_string
-- Referencing files: crates/vox-db/src/rag_evidence.rs, crates/vox-db/src/store/ops_codex/codex_graph.rs, crates/vox-db/src/store/ops_codex/codex_metrics_packages.rs
-- Origin: crates/vox-db/src/schema/domains/agents.rs
CREATE TABLE IF NOT EXISTS trusted_evidence_bundles (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    bundle_key          TEXT    NOT NULL UNIQUE,
    repository_id       TEXT    NOT NULL DEFAULT '',
    session_key         TEXT    NOT NULL DEFAULT '',
    evidence_json       TEXT    NOT NULL,
    contradiction_count INTEGER NOT NULL DEFAULT 0,
    created_at          TEXT    NOT NULL DEFAULT (datetime('now')),
    expires_at          TEXT
);
CREATE INDEX IF NOT EXISTS idx_trusted_evidence_repo_session ON trusted_evidence_bundles(repository_id, session_key, created_at);

-- Table: typed_stream_events
-- Status: DEAD (graphify-out/table_usage_report.json)
-- Evidence: literal_string
-- Referencing files: none
-- Origin: crates/vox-db/src/schema/domains/agents.rs
CREATE TABLE IF NOT EXISTS typed_stream_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    stream_id TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_typed_stream_events_stream ON typed_stream_events(stream_id);

-- Table: package_deps
-- Status: DEAD (graphify-out/table_usage_report.json)
-- Evidence: literal_string
-- Referencing files: none
-- Origin: crates/vox-db/src/schema/domains/packages.rs
CREATE TABLE IF NOT EXISTS package_deps (
    package_name TEXT NOT NULL,
    package_version TEXT NOT NULL,
    dep_name TEXT NOT NULL,
    dep_version_req TEXT NOT NULL,
    PRIMARY KEY (package_name, package_version, dep_name),
    FOREIGN KEY (package_name, package_version) REFERENCES packages(name, version)
);

-- Table: news_publish_approvals
-- Status: DORMANT (graphify-out/table_usage_report.json)
-- Evidence: literal_string
-- Referencing files: crates/vox-db/src/store/ops_news.rs
-- Origin: crates/vox-db/src/schema/domains/publish_cloud.rs
-- Two-person approval: distinct approver identities per news id (filename stem).
CREATE TABLE IF NOT EXISTS news_publish_approvals (
    news_id TEXT NOT NULL,
    approver TEXT NOT NULL,
    approved_at_ms INTEGER NOT NULL,
    PRIMARY KEY (news_id, approver)
);

-- Table: publication_external_links
-- Status: DORMANT (graphify-out/table_usage_report.json)
-- Evidence: literal_string
-- Referencing files: crates/vox-db-types/src/store_types/rows_extended.rs, crates/vox-db/src/store/ops_publication/external_artifacts.rs, crates/vox-db/src/types/store_types/rows_extended.rs
-- Origin: crates/vox-db/src/schema/domains/publish_cloud.rs
CREATE TABLE IF NOT EXISTS publication_external_links (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    publication_id TEXT NOT NULL,
    content_sha3_256 TEXT NOT NULL,
    adapter TEXT NOT NULL,
    link_kind TEXT NOT NULL,
    link_value TEXT NOT NULL,
    metadata_json TEXT,
    created_at_ms INTEGER NOT NULL,
    UNIQUE(publication_id, content_sha3_256, adapter, link_kind)
);
CREATE INDEX IF NOT EXISTS idx_publication_external_links_pub
    ON publication_external_links(publication_id, content_sha3_256);

-- Table: publication_external_revisions
-- Status: DORMANT (graphify-out/table_usage_report.json)
-- Evidence: literal_string
-- Referencing files: crates/vox-db-types/src/store_types/rows_extended.rs, crates/vox-db/src/store/ops_publication/external_artifacts.rs, crates/vox-db/src/types/store_types/rows_extended.rs
-- Origin: crates/vox-db/src/schema/domains/publish_cloud.rs
-- Maps an immutable local content digest to the adapter's current revision/version identifier
-- (e.g. Zenodo deposition version, OpenReview revision tag) for idempotent updates.
CREATE TABLE IF NOT EXISTS publication_external_revisions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    publication_id TEXT NOT NULL,
    content_sha3_256 TEXT NOT NULL,
    adapter TEXT NOT NULL,
    external_revision TEXT NOT NULL,
    metadata_json TEXT,
    updated_at_ms INTEGER NOT NULL,
    UNIQUE(publication_id, content_sha3_256, adapter)
);
CREATE INDEX IF NOT EXISTS idx_publication_external_revisions_pub_digest
    ON publication_external_revisions(publication_id, content_sha3_256);

-- Table: scholarly_publication_records
-- Status: DEAD (graphify-out/table_usage_report.json)
-- Evidence: literal_string
-- Referencing files: none
-- Origin: crates/vox-db/src/schema/domains/publish_cloud.rs
CREATE TABLE IF NOT EXISTS scholarly_publication_records (
    id                    TEXT PRIMARY KEY,
    publication_id        TEXT NOT NULL UNIQUE,
    doi                   TEXT,
    zenodo_deposit_id     TEXT,
    zenodo_doi            TEXT,
    orcid_put_code        INTEGER,        -- returned integer from ORCID POST
    figshare_article_id   TEXT,
    arxiv_submission_id   TEXT,
    openreview_forum_id   TEXT,
    crossref_deposit_id   TEXT,
    researchgate_confirmed INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'draft',
    -- status: 'draft' | 'deposited' | 'published' | 'retracted'
    published_at          TEXT,
    created_at            TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at            TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX IF NOT EXISTS idx_scholarly_pub_doi
    ON scholarly_publication_records (doi) WHERE doi IS NOT NULL;

-- Table: syndication_events
-- Status: DEAD (graphify-out/table_usage_report.json)
-- Evidence: literal_string
-- Referencing files: none
-- Origin: crates/vox-db/src/schema/domains/publish_cloud.rs
CREATE TABLE IF NOT EXISTS syndication_events (
    id               TEXT    PRIMARY KEY,
    publication_id   TEXT    NOT NULL,
    channel          TEXT    NOT NULL,
    outcome          TEXT    NOT NULL,
    external_id      TEXT,
    attempt_number   INTEGER NOT NULL DEFAULT 1,
    retryable        INTEGER NOT NULL DEFAULT 0,
    attempted_at     TEXT    NOT NULL,
    created_at       TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX IF NOT EXISTS idx_syndication_events_pub
    ON syndication_events (publication_id);
CREATE INDEX IF NOT EXISTS idx_syndication_events_channel
    ON syndication_events (channel, attempted_at DESC);

-- Table: test_decisions
-- Status: DORMANT (graphify-out/table_usage_report.json)
-- Evidence: literal_string
-- Referencing files: crates/vox-db/src/store/ops_mens_intelligence.rs
-- Origin: crates/vox-db/src/schema/domains/mens_intelligence.rs
-- Testing decisions made by the TestDecisionPolicy.
CREATE TABLE IF NOT EXISTS test_decisions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id TEXT NOT NULL UNIQUE,
    decision TEXT NOT NULL,
    rationale TEXT,
    complexity_score INTEGER NOT NULL,
    file_count INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Table: scientia_citations
-- Status: DEAD (graphify-out/table_usage_report.json)
-- Evidence: literal_string
-- Referencing files: none
-- Origin: crates/vox-db/src/schema/domains/scientia.rs
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

-- Table: scientia_prereg
-- Status: DEAD (graphify-out/table_usage_report.json)
-- Evidence: literal_string
-- Referencing files: none
-- Origin: crates/vox-db/src/schema/domains/scientia.rs
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

-- Table: scientia_provider_runs
-- Status: DORMANT (graphify-out/table_usage_report.json)
-- Evidence: literal_string
-- Referencing files: crates/vox-db/src/research_pipeline.rs
-- Origin: crates/vox-db/src/schema/domains/scientia.rs
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

-- Table: scientia_publication_attempts
-- Status: DEAD (graphify-out/table_usage_report.json)
-- Evidence: literal_string
-- Referencing files: none
-- Origin: crates/vox-db/src/schema/domains/scientia.rs
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

-- Table: scientia_training_pairs
-- Status: DORMANT (graphify-out/table_usage_report.json)
-- Evidence: literal_string
-- Referencing files: crates/vox-db/src/research_pipeline.rs
-- Origin: crates/vox-db/src/schema/domains/scientia.rs
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

-- Table: developer_journey_definitions
-- Status: DEAD (graphify-out/table_usage_report.json)
-- Evidence: literal_string
-- Referencing files: none
-- Origin: crates/vox-db/src/schema/domains/developer_journeys.rs
CREATE TABLE IF NOT EXISTS developer_journey_definitions (
    journey_id TEXT PRIMARY KEY,
    version INTEGER NOT NULL,
    title TEXT NOT NULL,
    description TEXT,
    definition_json TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Deterministic-seed row (2.1's deterministic-seed exception) — moved here
-- verbatim from domains/developer_journeys.rs alongside the table DDL, since
-- it targets this now-quarantined table.
INSERT OR IGNORE INTO developer_journey_definitions (
    journey_id, version, title, description, definition_json
) VALUES (
    'canonical_journey.v1.greenfield_vox_mens_devloop',
    1,
    'Greenfield Vox + MENS dev loop',
    'Bootstrap repo → workspace store → author → plan → assist → research → corpus/train → verify.',
    '{\"journey_id\":\"canonical_journey.v1.greenfield_vox_mens_devloop\",\"version\":1,\"title\":\"Greenfield Vox + MENS dev loop\",\"documentation_contract\":\"contracts/journeys/canonical-journey-definition.v1.schema.json\"}'
);

-- Table: activity_result_cache
-- Status: DORMANT (graphify-out/table_usage_report.json)
-- Evidence: literal_string
-- Referencing files: crates/vox-db/src/ddl/activity_result_cache.rs, crates/vox-db/src/ddl/mod.rs
-- Origin: crates/vox-db/src/schema/domains/vox_mesh.rs
-- ── Phase 2: activity result cache (P2-T5) ────────────────────────────────
-- Per-activity dedup cache. Result rows are pruned by the background sweep;
-- rows are append-only otherwise.
CREATE TABLE IF NOT EXISTS activity_result_cache (
    activity_id           TEXT    NOT NULL,
    arg_hash              TEXT    NOT NULL,        -- hex SHA3-512 of canonicalized args
    result_json           TEXT    NOT NULL,        -- serialized activity result value
    produced_at_unix_ms   INTEGER NOT NULL,
    dedup_window_ms       INTEGER NOT NULL,        -- TTL window in ms, e.g. 86_400_000 for 24h
    dedup_window_until    INTEGER NOT NULL,        -- produced_at_unix_ms + dedup_window_ms
    PRIMARY KEY (activity_id, arg_hash)
);

-- Cheap range scan for the background sweep (cadence: every 60s when daemon
-- is running; on-demand via `vox db prune` otherwise).
CREATE INDEX IF NOT EXISTS idx_activity_result_cache_until
    ON activity_result_cache (dedup_window_until);

-- Table: toestub_file_cache
-- Status: DORMANT (graphify-out/table_usage_report.json)
-- Evidence: literal_string
-- Referencing files: crates/vox-db/src/toestub_store.rs
-- Origin: crates/vox-db/src/schema/domains/toestub_build.rs
CREATE TABLE IF NOT EXISTS toestub_file_cache (
    path TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    rules_version TEXT NOT NULL,
    findings_json TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (path, content_hash, rules_version)
);
";
