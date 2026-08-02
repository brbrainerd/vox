//! Arca SQL: external review intelligence (CodeRabbit / GitHub review telemetry).
//!
//! Sensitivity: S2 for repository identifiers, paths, thread payload excerpts, and fingerprints.

pub const SCHEMA_EXTERNAL_REVIEW: &str = "
CREATE TABLE IF NOT EXISTS external_review_run (
    id                 INTEGER PRIMARY KEY AUTOINCREMENT,
    provider           TEXT    NOT NULL, -- coderabbit, github
    repository_id      TEXT    NOT NULL, -- S2
    owner              TEXT    NOT NULL, -- S2
    repo               TEXT    NOT NULL, -- S2
    pr_number          INTEGER NOT NULL,
    commit_sha         TEXT,             -- S2
    trigger_kind       TEXT    NOT NULL DEFAULT 'review', -- review|full_review|auto
    idempotency_key    TEXT,
    item_count         INTEGER NOT NULL DEFAULT 0,
    started_at         TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    finished_at        TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    metadata_json      TEXT
);

CREATE INDEX IF NOT EXISTS idx_external_review_run_repo_pr
    ON external_review_run(repository_id, pr_number, id);

CREATE UNIQUE INDEX IF NOT EXISTS idx_external_review_run_idempotency
    ON external_review_run(provider, repository_id, pr_number, idempotency_key);

CREATE TABLE IF NOT EXISTS external_review_comment_thread (
    id                 INTEGER PRIMARY KEY AUTOINCREMENT,
    provider           TEXT    NOT NULL,
    repository_id      TEXT    NOT NULL,
    pr_number          INTEGER NOT NULL,
    thread_identity    TEXT    NOT NULL,
    placement_kind     TEXT    NOT NULL, -- inline|review_summary|issue_comment|reply
    line_anchor_state  TEXT    NOT NULL DEFAULT 'missing', -- current|outdated|missing
    file_path          TEXT,
    line_start         INTEGER,
    line_end           INTEGER,
    source_comment_id  INTEGER,
    parent_comment_id  INTEGER,
    source_payload_hash TEXT   NOT NULL,
    raw_payload_json   TEXT    NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_external_review_thread_unique
    ON external_review_comment_thread(provider, repository_id, pr_number, thread_identity, source_payload_hash);

CREATE TABLE IF NOT EXISTS external_review_finding (
    id                    INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id                INTEGER NOT NULL REFERENCES external_review_run(id) ON DELETE CASCADE,
    provider              TEXT    NOT NULL,
    repository_id         TEXT    NOT NULL,
    pr_number             INTEGER NOT NULL,
    finding_identity      TEXT    NOT NULL,
    thread_identity       TEXT,
    source_comment_id     INTEGER,
    placement_kind        TEXT    NOT NULL,
    line_anchor_state     TEXT    NOT NULL DEFAULT 'missing',
    file_path             TEXT,
    line_start            INTEGER,
    line_end              INTEGER,
    category              TEXT    NOT NULL,
    anti_pattern_id       TEXT,
    severity              TEXT    NOT NULL,
    title                 TEXT    NOT NULL,
    details               TEXT    NOT NULL,
    suggested_fix         TEXT,
    extraction_confidence REAL,
    source_payload_hash   TEXT    NOT NULL,
    fingerprint           TEXT    NOT NULL,
    status                TEXT    NOT NULL DEFAULT 'open'
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_external_review_finding_unique
    ON external_review_finding(provider, repository_id, pr_number, fingerprint);

CREATE INDEX IF NOT EXISTS idx_external_review_finding_training
    ON external_review_finding(repository_id, category, severity, id);

CREATE TABLE IF NOT EXISTS external_review_finding_state_history (
    id                    INTEGER PRIMARY KEY AUTOINCREMENT,
    finding_id            INTEGER NOT NULL REFERENCES external_review_finding(id) ON DELETE CASCADE,
    previous_state        TEXT,
    new_state             TEXT    NOT NULL, -- unverified|likely_true|confirmed_true|likely_false|confirmed_false|superseded
    reason                TEXT,
    confidence            REAL,
    evidence_ref          TEXT,
    created_at            TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);

CREATE INDEX IF NOT EXISTS idx_external_review_finding_state_finding
    ON external_review_finding_state_history(finding_id, id);

-- external_review_outcome: quarantined (DORMANT, Task 4) — see domains/quarantine.rs.

CREATE TABLE IF NOT EXISTS external_review_deadletter (
    id                    INTEGER PRIMARY KEY AUTOINCREMENT,
    provider              TEXT    NOT NULL,
    repository_id         TEXT    NOT NULL,
    pr_number             INTEGER NOT NULL,
    source_kind           TEXT    NOT NULL, -- inline|review|issue|reply
    source_comment_id     INTEGER,
    source_payload_hash   TEXT    NOT NULL,
    error_class           TEXT    NOT NULL,
    error_message         TEXT,
    raw_payload_json      TEXT    NOT NULL,
    retry_state           TEXT    NOT NULL DEFAULT 'pending', -- pending|retried|ignored
    created_at            TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    retried_at            TEXT
);

CREATE INDEX IF NOT EXISTS idx_external_review_deadletter_lookup
    ON external_review_deadletter(provider, repository_id, pr_number, id);

CREATE TABLE IF NOT EXISTS external_review_kpi_snapshot (
    id                    INTEGER PRIMARY KEY AUTOINCREMENT,
    repository_id         TEXT    NOT NULL,
    period_start          TEXT    NOT NULL,
    period_end            TEXT    NOT NULL,
    coverage_ratio        REAL,
    ingest_to_fix_latency_ms REAL,
    repeated_finding_rate REAL,
    post_training_regression_rate REAL,
    auto_fix_acceptance_rate REAL,
    created_at            TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);

CREATE INDEX IF NOT EXISTS idx_external_review_kpi_repo_period
    ON external_review_kpi_snapshot(repository_id, period_end, id);
";
