//! P2-T5: SQL constants for querying the activity result cache.

/// Insert-or-replace SQL. Idempotent: re-running an activity within its TTL
/// updates `produced_at_unix_ms`, refreshing the window.
pub const UPSERT_SQL: &str = r#"
INSERT INTO activity_result_cache
    (activity_id, arg_hash, result_json, produced_at_unix_ms,
     dedup_window_ms, dedup_window_until)
VALUES (?, ?, ?, ?, ?, ?)
ON CONFLICT(activity_id, arg_hash) DO UPDATE SET
    result_json         = excluded.result_json,
    produced_at_unix_ms = excluded.produced_at_unix_ms,
    dedup_window_ms     = excluded.dedup_window_ms,
    dedup_window_until  = excluded.dedup_window_until
"#;

/// Lookup SQL. Returns rows still inside their TTL window.
pub const LOOKUP_SQL: &str = r#"
SELECT result_json, produced_at_unix_ms, dedup_window_until
FROM activity_result_cache
WHERE activity_id = ? AND arg_hash = ?
  AND dedup_window_until > ?
LIMIT 1
"#;

/// Sweep SQL. Run on a background timer (cadence: every 60 seconds when
/// the orchestrator daemon is running; on-demand via `vox db prune` otherwise).
pub const SWEEP_SQL: &str = r#"
DELETE FROM activity_result_cache
WHERE dedup_window_until <= ?
"#;
