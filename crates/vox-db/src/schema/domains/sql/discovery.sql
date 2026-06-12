-- Per-user command/concept exposure ledger for the Vox Console discovery engine.
-- One row per (user, action-manifest id). Created lazily on first sight; rows for
-- action ids absent from the current manifest are simply never resurfaced.
CREATE TABLE IF NOT EXISTS discovery_state (
    user_id         TEXT    NOT NULL,
    action_id       TEXT    NOT NULL,
    seen_count      INTEGER NOT NULL DEFAULT 0,
    used_count      INTEGER NOT NULL DEFAULT 0,
    last_seen_ms    INTEGER NOT NULL DEFAULT 0,
    last_used_ms    INTEGER NOT NULL DEFAULT 0,
    dwell_ms_total  INTEGER NOT NULL DEFAULT 0,
    fsrs_stability  REAL    NOT NULL DEFAULT 0.0,
    fsrs_difficulty REAL    NOT NULL DEFAULT 0.0,
    fsrs_due_ms     INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (user_id, action_id)
);

CREATE INDEX IF NOT EXISTS idx_discovery_state_due
    ON discovery_state(user_id, fsrs_due_ms);
