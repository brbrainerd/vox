//! Arca SQL: CAS objects/names + Codex reactivity and processing (cas + codex fragments).
pub const SCHEMA_CAS_CODEX: &str = "
CREATE TABLE IF NOT EXISTS objects (
    hash TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    data BLOB,
    codec TEXT NOT NULL DEFAULT 'none',
    dict_id INTEGER,
    uncompressed_len INTEGER NOT NULL DEFAULT 0,
    storage TEXT NOT NULL DEFAULT 'inline',
    file_path TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS names (
    namespace TEXT NOT NULL,
    name TEXT NOT NULL,
    hash TEXT NOT NULL REFERENCES objects(hash),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (namespace, name)
);

CREATE TABLE IF NOT EXISTS causal (
    hash TEXT NOT NULL REFERENCES objects(hash),
    parent_hash TEXT NOT NULL REFERENCES objects(hash),
    PRIMARY KEY (hash, parent_hash)
);

CREATE TABLE IF NOT EXISTS schema_version (
    version INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS metadata (
    hash TEXT NOT NULL REFERENCES objects(hash),
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (hash, key)
);

CREATE INDEX IF NOT EXISTS idx_names_hash ON names(hash);
CREATE INDEX IF NOT EXISTS idx_causal_parent ON causal(parent_hash);
CREATE INDEX IF NOT EXISTS idx_metadata_hash ON metadata(hash);

CREATE TABLE IF NOT EXISTS codex_schema_lineage (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    baseline_id TEXT NOT NULL,
    schema_digest TEXT NOT NULL,
    provenance TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS codex_change_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    topic TEXT NOT NULL,
    entity_kind TEXT,
    entity_id TEXT,
    change_kind TEXT NOT NULL,
    payload_json TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS codex_subscriptions (
    id TEXT PRIMARY KEY,
    topic TEXT NOT NULL,
    filter_json TEXT,
    client_hint TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS codex_query_snapshots (
    id TEXT PRIMARY KEY,
    query_name TEXT NOT NULL,
    snapshot_json TEXT NOT NULL,
    digest TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS codex_projection_versions (
    projection_name TEXT NOT NULL,
    version INTEGER NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (projection_name, version)
);

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

CREATE TABLE IF NOT EXISTS audit_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    actor_kind TEXT NOT NULL,
    actor_id TEXT NOT NULL DEFAULT '',
    action TEXT NOT NULL,
    resource_kind TEXT NOT NULL DEFAULT '',
    resource_id TEXT NOT NULL DEFAULT '',
    scope_kind TEXT NOT NULL DEFAULT '',
    scope_id TEXT NOT NULL DEFAULT '',
    payload_json TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_codex_schema_lineage_baseline ON codex_schema_lineage(baseline_id);
CREATE INDEX IF NOT EXISTS idx_codex_change_log_topic ON codex_change_log(topic);
CREATE INDEX IF NOT EXISTS idx_codex_change_log_topic_id ON codex_change_log(topic, id);
CREATE INDEX IF NOT EXISTS idx_codex_change_log_created ON codex_change_log(created_at);
CREATE INDEX IF NOT EXISTS idx_codex_subscriptions_topic ON codex_subscriptions(topic);
CREATE INDEX IF NOT EXISTS idx_codex_query_snapshots_name ON codex_query_snapshots(query_name);
CREATE INDEX IF NOT EXISTS idx_processing_runs_status_created ON processing_runs(status, created_at);
CREATE INDEX IF NOT EXISTS idx_processing_runs_scope ON processing_runs(scope_kind, scope_id);
CREATE INDEX IF NOT EXISTS idx_processing_runs_kind ON processing_runs(run_kind);
CREATE INDEX IF NOT EXISTS idx_processing_run_steps_run ON processing_run_steps(processing_run_id);
CREATE INDEX IF NOT EXISTS idx_audit_log_scope_created ON audit_log(scope_kind, scope_id, created_at);
CREATE INDEX IF NOT EXISTS idx_audit_log_resource_created ON audit_log(resource_kind, resource_id, created_at);
";
