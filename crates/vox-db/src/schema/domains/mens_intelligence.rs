//! Arca SQL: Machine Execution Neural System (MENS) training, corpus, and GRPO telemetry.
pub const SCHEMA_MENS_INTELLIGENCE: &str = "
-- Observer events recorded during agent execution.
CREATE TABLE IF NOT EXISTS observer_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    task_id TEXT NOT NULL,
    observed_at_ms INTEGER NOT NULL,
    file_path TEXT NOT NULL,
    lsp_errors INTEGER NOT NULL DEFAULT 0,
    parse_rate REAL NOT NULL DEFAULT 0.0,
    construct_coverage REAL NOT NULL DEFAULT 0.0,
    action TEXT NOT NULL,
    raw_json TEXT, -- Full ObservationReport
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_observer_events_task ON observer_events(task_id);
CREATE INDEX IF NOT EXISTS idx_observer_events_session ON observer_events(session_id);

-- test_decisions: quarantined (DORMANT, Task 4) — see domains/quarantine.rs.

-- Multi-tier victory verdicts for task completion.
CREATE TABLE IF NOT EXISTS victory_verdicts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id TEXT NOT NULL,
    tier TEXT NOT NULL,
    passed INTEGER NOT NULL DEFAULT 0,
    error_count INTEGER NOT NULL DEFAULT 0,
    report TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_victory_verdicts_task ON victory_verdicts(task_id);

-- MENS training corpus quality metrics.
CREATE TABLE IF NOT EXISTS mens_corpus_quality (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    pair_hash TEXT NOT NULL UNIQUE,
    source TEXT NOT NULL,
    parse_valid INTEGER NOT NULL DEFAULT 0,
    ast_depth INTEGER NOT NULL DEFAULT 0,
    construct_count INTEGER NOT NULL DEFAULT 0,
    reward_score REAL NOT NULL DEFAULT 0.0,
    split TEXT NOT NULL DEFAULT 'training',
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_mens_corpus_quality_split ON mens_corpus_quality(split);

-- MENS training corpus pairs.
CREATE TABLE IF NOT EXISTS corpus_pairs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source TEXT NOT NULL,
    errors_json TEXT NOT NULL,
    origin TEXT NOT NULL,
    reward_signal REAL NOT NULL DEFAULT 0.0,
    label TEXT NOT NULL DEFAULT 'negative',
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- GRPO training run telemetry and reward tracking.
CREATE TABLE IF NOT EXISTS grpo_training_run (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id TEXT NOT NULL,
    step INTEGER NOT NULL,
    mean_reward REAL NOT NULL DEFAULT 0.0,
    policy_loss REAL NOT NULL DEFAULT 0.0,
    clip_fraction REAL NOT NULL DEFAULT 0.0,
    parse_rate REAL NOT NULL DEFAULT 0.0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_grpo_training_run_id_step ON grpo_training_run(run_id, step);
";
