//! Schema for `vox harness eval --live` persistence (chat harness continuous eval design,
//! 2026-08-02). See `crates/vox-db/src/harness_eval.rs` for the `VoxDb` methods that write/read
//! these tables, and `crates/vox-db-types/src/store_types/harness_eval.rs` for the Rust record
//! shapes.

pub const SCHEMA_HARNESS_EVAL: &str = r#"
CREATE TABLE IF NOT EXISTS harness_eval_run (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id              TEXT    NOT NULL UNIQUE,
    triggered_by        TEXT    NOT NULL,
    git_sha             TEXT    NOT NULL,
    git_branch          TEXT    NOT NULL,
    changed_files_json  TEXT,
    config_version      TEXT,
    samples_per_task    INTEGER NOT NULL,
    task_count          INTEGER NOT NULL,
    pass_count          INTEGER NOT NULL,
    fail_count          INTEGER NOT NULL,
    skip_count          INTEGER NOT NULL,
    total_cost_usd      REAL    NOT NULL DEFAULT 0.0,
    started_at_ms       INTEGER NOT NULL,
    finished_at_ms      INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_harness_eval_run_time
    ON harness_eval_run(started_at_ms);

CREATE TABLE IF NOT EXISTS harness_eval_task_result (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id              TEXT    NOT NULL,
    task_id             TEXT    NOT NULL,
    category             TEXT    NOT NULL,
    checker_kind         TEXT    NOT NULL,
    status               TEXT    NOT NULL,
    pass_samples         INTEGER NOT NULL,
    total_samples        INTEGER NOT NULL,
    latency_p50_ms       INTEGER,
    cost_usd             REAL,
    failure_detail        TEXT,
    recorded_at_ms       INTEGER NOT NULL,
    FOREIGN KEY(run_id) REFERENCES harness_eval_run(run_id)
);

CREATE INDEX IF NOT EXISTS idx_harness_eval_task_result_run
    ON harness_eval_task_result(run_id, task_id);

CREATE TABLE IF NOT EXISTS model_selection_event (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id              TEXT    NOT NULL,
    task_id             TEXT    NOT NULL,
    model_id            TEXT    NOT NULL,
    cost_tier            TEXT    NOT NULL,
    selection_reason      TEXT    NOT NULL,
    was_privacy_gated     INTEGER NOT NULL,
    recorded_at_ms       INTEGER NOT NULL,
    FOREIGN KEY(run_id) REFERENCES harness_eval_run(run_id)
);

CREATE INDEX IF NOT EXISTS idx_model_selection_event_run
    ON model_selection_event(run_id, model_id);
"#;
