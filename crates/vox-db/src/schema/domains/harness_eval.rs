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

-- Task M2: bookkeeping for the retroactive "user re-asked within 2 turns" rescore of
-- `triggered_by = 'live_chat'` rows only (see `VoxDb::record_live_chat_turn` and
-- `VoxDb::rescore_pending_live_chat_reask`). Deliberately a separate table rather than new
-- columns on `harness_eval_task_result`/`harness_eval_run` -- those are shared with the real
-- `vox harness eval --live` path (9+ existing construction sites across report/publish/live_eval),
-- and this bookkeeping is meaningless for eval rows.
CREATE TABLE IF NOT EXISTS live_chat_completeness_pending (
    id                    INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id                TEXT    NOT NULL UNIQUE,
    task_id               TEXT    NOT NULL,
    session_run_prefix    TEXT    NOT NULL,
    user_prompt           TEXT    NOT NULL,
    -- Counts down 2 -> 1 -> 0 as later same-session turns arrive without matching the
    -- re-ask heuristic. A row is deleted once fully resolved (matched, or reached 0).
    checks_remaining      INTEGER NOT NULL,
    recorded_at_ms        INTEGER NOT NULL,
    FOREIGN KEY(run_id) REFERENCES harness_eval_run(run_id)
);

CREATE INDEX IF NOT EXISTS idx_live_chat_completeness_pending_session
    ON live_chat_completeness_pending(session_run_prefix, recorded_at_ms);
"#;
