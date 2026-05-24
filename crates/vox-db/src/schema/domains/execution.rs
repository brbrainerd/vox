//! Arca SQL: Workflow and activity execution logging.
pub const SCHEMA_EXECUTION: &str = "
CREATE TABLE IF NOT EXISTS execution_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    workflow_id TEXT NOT NULL,
    agent_id TEXT,
    skill_id TEXT,
    activity_name TEXT NOT NULL,
    status TEXT NOT NULL,
    attempt INTEGER NOT NULL DEFAULT 1,
    duration_ms INTEGER NOT NULL DEFAULT 0,
    output_size INTEGER NOT NULL DEFAULT 0,
    input BLOB,
    output BLOB,
    error TEXT,
    options TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

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

CREATE TABLE IF NOT EXISTS scheduled (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    function_hash TEXT NOT NULL,
    args BLOB,
    run_at TEXT NOT NULL,
    cron_expr TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_exec_log_workflow ON execution_log(workflow_id);
CREATE INDEX IF NOT EXISTS idx_exec_log_status ON execution_log(status);
CREATE INDEX IF NOT EXISTS idx_exec_log_agent ON execution_log(agent_id);
CREATE INDEX IF NOT EXISTS idx_exec_log_skill ON execution_log(skill_id);
CREATE INDEX IF NOT EXISTS idx_workflow_executions_agent ON workflow_executions(agent_id);
CREATE INDEX IF NOT EXISTS idx_workflow_executions_status ON workflow_executions(status);
CREATE INDEX IF NOT EXISTS idx_workflow_executions_skill ON workflow_executions(skill_id);
CREATE INDEX IF NOT EXISTS idx_scheduled_run_at ON scheduled(run_at);
CREATE INDEX IF NOT EXISTS idx_scheduled_status ON scheduled(status);

-- Durable execution journal for crash-safe workflow resume.
CREATE TABLE IF NOT EXISTS workflow_activity_log (
    run_id          TEXT NOT NULL,
    workflow_name   TEXT NOT NULL,
    activity_name   TEXT NOT NULL,
    activity_id     TEXT NOT NULL,
    status          TEXT NOT NULL,
    result_json     TEXT,
    recorded_at_ms  INTEGER NOT NULL,
    PRIMARY KEY (run_id, workflow_name, activity_id, status)
);

CREATE INDEX IF NOT EXISTS idx_workflow_activity_run ON workflow_activity_log(run_id);
CREATE INDEX IF NOT EXISTS idx_workflow_activity_workflow ON workflow_activity_log(workflow_name);

CREATE TABLE IF NOT EXISTS workflow_run_log (
    run_id           TEXT PRIMARY KEY,
    workflow_name    TEXT NOT NULL,
    status           TEXT NOT NULL,
    planned_steps    INTEGER NOT NULL DEFAULT 0,
    completed_steps  INTEGER NOT NULL DEFAULT 0,
    plan_session_id  TEXT,
    plan_node_id     TEXT,
    plan_version     INTEGER,
    lease_owner      TEXT,
    lease_until_ms   INTEGER,
    started_at_ms    INTEGER NOT NULL,
    updated_at_ms    INTEGER NOT NULL,
    completed_at_ms  INTEGER,
    last_error       TEXT
);

CREATE INDEX IF NOT EXISTS idx_workflow_run_status ON workflow_run_log(status);
CREATE INDEX IF NOT EXISTS idx_workflow_run_workflow ON workflow_run_log(workflow_name);

CREATE TABLE IF NOT EXISTS workflow_signal_log (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id          TEXT NOT NULL,
    signal_key      TEXT NOT NULL,
    payload_json    TEXT,
    recorded_at_ms  INTEGER NOT NULL,
    consumed_at_ms  INTEGER
);

CREATE INDEX IF NOT EXISTS idx_workflow_signal_run_key ON workflow_signal_log(run_id, signal_key, consumed_at_ms);

CREATE TABLE IF NOT EXISTS workflow_activity_attempt_log (
    run_id          TEXT NOT NULL,
    workflow_name   TEXT NOT NULL,
    activity_id     TEXT NOT NULL,
    attempt_no      INTEGER NOT NULL,
    status          TEXT NOT NULL,
    worker_owner    TEXT,
    lease_until_ms  INTEGER,
    error           TEXT,
    recorded_at_ms  INTEGER NOT NULL,
    PRIMARY KEY (run_id, workflow_name, activity_id, attempt_no, status)
);

CREATE INDEX IF NOT EXISTS idx_workflow_attempt_run_activity ON workflow_activity_attempt_log(run_id, workflow_name, activity_id);

CREATE TABLE IF NOT EXISTS actor_state (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Dynamic planning sessions and branching versions.
CREATE TABLE IF NOT EXISTS plan_sessions (
    plan_session_id TEXT PRIMARY KEY,
    origin_session_id TEXT,
    goal_text TEXT NOT NULL,
    strategy TEXT NOT NULL,
    current_version INTEGER NOT NULL DEFAULT 1,
    status TEXT NOT NULL DEFAULT 'pending',
    question_session_id TEXT,
    iterative_loop_round INTEGER NOT NULL DEFAULT 0,
    iterative_stop_reason TEXT,
    iterative_loop_metadata_json TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS plan_versions (
    plan_session_id TEXT NOT NULL,
    version INTEGER NOT NULL,
    parent_version INTEGER,
    trigger_event TEXT,
    trigger_payload_json TEXT,
    quality_score REAL NOT NULL DEFAULT 0.0,
    reviewer_verdict TEXT NOT NULL DEFAULT 'pending',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (plan_session_id, version)
);

CREATE TABLE IF NOT EXISTS plan_nodes (
    plan_session_id TEXT NOT NULL,
    version INTEGER NOT NULL,
    node_id TEXT NOT NULL,
    description TEXT NOT NULL,
    dependencies_json TEXT NOT NULL DEFAULT '[]',
    execution_policy_json TEXT NOT NULL DEFAULT '{}',
    status TEXT NOT NULL DEFAULT 'pending',
    workflow_invocation TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (plan_session_id, version, node_id)
);

CREATE TABLE IF NOT EXISTS plan_node_attempts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    plan_session_id TEXT NOT NULL,
    version INTEGER NOT NULL,
    node_id TEXT NOT NULL,
    attempt_no INTEGER NOT NULL,
    task_id TEXT,
    outcome TEXT NOT NULL,
    error_text TEXT,
    latency_ms INTEGER,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_plan_sessions_status ON plan_sessions(status);
CREATE INDEX IF NOT EXISTS idx_plan_nodes_status ON plan_nodes(status);
CREATE INDEX IF NOT EXISTS idx_plan_attempts_node ON plan_node_attempts(plan_session_id, version, node_id);

-- Repository reconstruction campaign spec (compact prompt-expanded contract).
CREATE TABLE IF NOT EXISTS reconstruction_campaign_spec (
    campaign_id TEXT PRIMARY KEY,
    benchmark_tier TEXT NOT NULL,
    objective TEXT NOT NULL,
    spec_json TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL
);

-- Retrieval-first artifact state graph for reconstruction runs.
CREATE TABLE IF NOT EXISTS reconstruction_artifacts (
    campaign_id TEXT NOT NULL,
    artifact_id TEXT NOT NULL,
    artifact_kind TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    tags_json TEXT NOT NULL DEFAULT '[]',
    source TEXT,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    PRIMARY KEY (campaign_id, artifact_id)
);

CREATE INDEX IF NOT EXISTS idx_recon_artifacts_kind ON reconstruction_artifacts(artifact_kind);
CREATE INDEX IF NOT EXISTS idx_recon_artifacts_campaign_kind ON reconstruction_artifacts(campaign_id, artifact_kind);

-- Ladder KPIs by benchmark tier snapshot.
CREATE TABLE IF NOT EXISTS reconstruction_benchmark_kpis (
    campaign_id TEXT NOT NULL,
    benchmark_tier TEXT NOT NULL,
    elapsed_ms INTEGER NOT NULL DEFAULT 0,
    autonomous_recovery_rate REAL NOT NULL DEFAULT 0.0,
    regenerated_file_success_rate REAL NOT NULL DEFAULT 0.0,
    cost_per_success_step REAL NOT NULL DEFAULT 0.0,
    recorded_at_ms INTEGER NOT NULL,
    PRIMARY KEY (campaign_id, benchmark_tier)
);

-- MENS Telemetry secondary channel for non-code OAPV observation feedback
CREATE TABLE IF NOT EXISTS mens_observer_observations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    agent_id TEXT,
    observation_type TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    recorded_at_ms INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_mens_obs_agent ON mens_observer_observations(agent_id);

-- Test decisions from Orient Phase policy
CREATE TABLE IF NOT EXISTS plan_test_decisions (
    task_id TEXT PRIMARY KEY,
    decision TEXT NOT NULL,
    rationale TEXT NOT NULL,
    recorded_at_ms INTEGER NOT NULL
);

-- Phase 4.1: @scheduled function durable state.
-- One row per scheduled function. interval_ms captured from
-- HirFn::schedule_interval at boot (Phase 5 main_boot); next_due_at_ms is
-- the timer state the scheduler runner (Task 4.2) consults.
-- last_run_id/started/completed track the most recent invocation for
-- crash-recovery visibility.
CREATE TABLE IF NOT EXISTS scheduled_runs (
    function_name TEXT NOT NULL PRIMARY KEY,
    interval_ms INTEGER NOT NULL,
    next_due_at_ms INTEGER NOT NULL,
    last_run_id TEXT,
    last_started_at_ms INTEGER,
    last_completed_at_ms INTEGER
);
CREATE INDEX IF NOT EXISTS idx_scheduled_runs_next_due ON scheduled_runs(next_due_at_ms);
";
