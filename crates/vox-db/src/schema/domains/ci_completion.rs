//! Arca SQL: CI completion audit telemetry (runs, findings, detector rollups, suppressions).
//!
//! Column sensitivity: **S2** on repo-identifying and path-bearing fields — see module rustdoc on `crate::store::ops_completion`.

pub const SCHEMA_CI_COMPLETION: &str = "
CREATE TABLE IF NOT EXISTS ci_completion_run (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    repository_id    TEXT    NOT NULL, -- S2: stable repo label
    branch           TEXT,              -- S2: branch name
    commit_sha       TEXT,              -- S2: commit id
    workflow         TEXT    NOT NULL DEFAULT 'local',
    run_kind         TEXT    NOT NULL DEFAULT 'audit',
    started_at       TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    finished_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    tool_versions_json TEXT              -- S1: versions / tool ids
);

CREATE TABLE IF NOT EXISTS ci_completion_finding (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id           INTEGER NOT NULL REFERENCES ci_completion_run(id) ON DELETE CASCADE,
    detector_id      TEXT    NOT NULL, -- S1
    tier             TEXT    NOT NULL,
    severity         TEXT    NOT NULL DEFAULT 'warning',
    confidence       TEXT,
    file_path        TEXT,              -- S2: relative path may reveal layout
    symbol           TEXT,              -- S2: may reveal API surface
    line_start       INTEGER,
    line_end         INTEGER,
    fingerprint      TEXT    NOT NULL, -- S2: stable id derived from location/symbol
    status           TEXT    NOT NULL DEFAULT 'open',
    suppressed       INTEGER NOT NULL DEFAULT 0,
    suppression_id   INTEGER,
    meta_json        TEXT               -- S1–S2: keep free-form payload small; no secrets
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_ci_completion_finding_run_fp
    ON ci_completion_finding(run_id, fingerprint);

CREATE INDEX IF NOT EXISTS idx_ci_completion_run_repo_time
    ON ci_completion_run(repository_id, finished_at);

CREATE INDEX IF NOT EXISTS idx_ci_completion_finding_detector
    ON ci_completion_finding(detector_id, tier, run_id);

CREATE TABLE IF NOT EXISTS ci_completion_detector_snapshot (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id           INTEGER NOT NULL REFERENCES ci_completion_run(id) ON DELETE CASCADE,
    detector_id      TEXT    NOT NULL,
    tier             TEXT    NOT NULL,
    finding_count    INTEGER NOT NULL DEFAULT 0,
    new_count        INTEGER NOT NULL DEFAULT 0,
    resolved_count   INTEGER NOT NULL DEFAULT 0,
    precision_estimate REAL,
    block_state      TEXT,
    UNIQUE(run_id, detector_id)
);

CREATE TABLE IF NOT EXISTS ci_completion_suppression (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    detector_id      TEXT    NOT NULL,
    scope            TEXT    NOT NULL,
    reason           TEXT    NOT NULL,
    owner            TEXT    NOT NULL,
    created_at       TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    expires_at       TEXT,
    approved_by      TEXT
);

CREATE INDEX IF NOT EXISTS idx_ci_completion_suppression_detector
    ON ci_completion_suppression(detector_id, expires_at);
";
