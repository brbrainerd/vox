/// SQL schema for the content-addressed code store.
/// V1: Original schema — objects, names, causal.
pub const SCHEMA_V1: &str = "
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

CREATE INDEX IF NOT EXISTS idx_names_hash ON names(hash);
CREATE INDEX IF NOT EXISTS idx_causal_parent ON causal(parent_hash);
";

/// V2: Extended schema — metadata, packages, execution log,
/// scheduled functions, and component registry.
pub const SCHEMA_V2: &str = "
CREATE TABLE IF NOT EXISTS schema_version (
    version INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Typed metadata for any object (key-value with JSON values)
CREATE TABLE IF NOT EXISTS metadata (
    hash TEXT NOT NULL REFERENCES objects(hash),
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (hash, key)
);

-- Package registry
CREATE TABLE IF NOT EXISTS packages (
    name TEXT NOT NULL,
    version TEXT NOT NULL,
    hash TEXT NOT NULL REFERENCES objects(hash),
    description TEXT,
    author TEXT,
    license TEXT,
    published_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (name, version)
);

-- Package dependency graph
CREATE TABLE IF NOT EXISTS package_deps (
    package_name TEXT NOT NULL,
    package_version TEXT NOT NULL,
    dep_name TEXT NOT NULL,
    dep_version_req TEXT NOT NULL,
    PRIMARY KEY (package_name, package_version, dep_name),
    FOREIGN KEY (package_name, package_version) REFERENCES packages(name, version)
);

-- Workflow/activity execution log (append-only)
CREATE TABLE IF NOT EXISTS execution_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    workflow_id TEXT NOT NULL,
    activity_name TEXT NOT NULL,
    status TEXT NOT NULL,
    attempt INTEGER NOT NULL DEFAULT 1,
    input BLOB,
    output BLOB,
    error TEXT,
    options TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Scheduled functions (durable scheduling)
CREATE TABLE IF NOT EXISTS scheduled (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    function_hash TEXT NOT NULL REFERENCES objects(hash),
    args BLOB,
    run_at TEXT NOT NULL,
    cron_expr TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Component registry (skills, workflows, packages)
CREATE TABLE IF NOT EXISTS components (
    name TEXT PRIMARY KEY,
    namespace TEXT NOT NULL,
    schema_hash TEXT REFERENCES objects(hash),
    description TEXT,
    version TEXT NOT NULL DEFAULT '0.1.0',
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_metadata_hash ON metadata(hash);
CREATE INDEX IF NOT EXISTS idx_packages_hash ON packages(hash);
CREATE INDEX IF NOT EXISTS idx_exec_log_workflow ON execution_log(workflow_id);
CREATE INDEX IF NOT EXISTS idx_exec_log_status ON execution_log(status);
CREATE INDEX IF NOT EXISTS idx_scheduled_run_at ON scheduled(run_at);
CREATE INDEX IF NOT EXISTS idx_scheduled_status ON scheduled(status);
CREATE INDEX IF NOT EXISTS idx_components_namespace ON components(namespace);
";

/// All migrations in order. Each entry is (version, sql).
pub const MIGRATIONS: &[(i64, &str)] = &[
    (1, SCHEMA_V1),
    (2, SCHEMA_V2),
];

/// SQLite pragmas for performance and concurrency.
pub const PRAGMAS: &str = "
PRAGMA journal_mode=WAL;
PRAGMA busy_timeout=5000;
PRAGMA synchronous=NORMAL;
PRAGMA foreign_keys=ON;
PRAGMA cache_size=-8000;
";
