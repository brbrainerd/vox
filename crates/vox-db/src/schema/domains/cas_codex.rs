//! Arca SQL: CAS objects/names + Codex reactivity and processing (cas + codex fragments).
pub const SCHEMA_CAS_CODEX: &str = "
CREATE TABLE IF NOT EXISTS objects (
    hash TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    data BLOB NOT NULL,
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

-- codex_schema_lineage, codex_change_log, codex_subscriptions,
-- codex_query_snapshots, codex_projection_versions, processing_runs,
-- processing_run_steps: quarantined (Task 4) — see domains/quarantine.rs.

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

CREATE INDEX IF NOT EXISTS idx_audit_log_scope_created ON audit_log(scope_kind, scope_id, created_at);
CREATE INDEX IF NOT EXISTS idx_audit_log_resource_created ON audit_log(resource_kind, resource_id, created_at);
";
