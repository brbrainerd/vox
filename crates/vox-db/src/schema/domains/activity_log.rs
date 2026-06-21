//! Arca schema fragment for the activity log.

pub const SCHEMA_ACTIVITY_LOG: &str = r#"
CREATE TABLE IF NOT EXISTS activity_log (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    ts_ms       INTEGER NOT NULL,
    node_id     TEXT,                       -- mesh node origin (NULL = local); forward-compat for mesh-wide aggregation
    agent_id    TEXT,
    session_id  TEXT,
    kind        TEXT NOT NULL,
    summary     TEXT NOT NULL,
    detail_json TEXT NOT NULL DEFAULT '{}'
);
CREATE INDEX IF NOT EXISTS idx_activity_ts   ON activity_log(ts_ms);
CREATE INDEX IF NOT EXISTS idx_activity_agent ON activity_log(agent_id);
CREATE INDEX IF NOT EXISTS idx_activity_kind ON activity_log(kind);
CREATE INDEX IF NOT EXISTS idx_activity_node ON activity_log(node_id);
"#;

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn activity_log_round_trip() {
        let db = crate::VoxDb::connect(crate::DbConfig::Memory)
            .await
            .expect("db");
        db.connection()
            .execute(
                "INSERT INTO activity_log (ts_ms, agent_id, session_id, kind, summary, detail_json)
             VALUES (1000, 'A1', 's1', 'TaskCompleted', 'done', '{}')",
                (),
            )
            .await
            .expect("insert");
        let mut q = db
            .connection()
            .query("SELECT kind FROM activity_log WHERE agent_id='A1'", ())
            .await
            .expect("q");
        let row = q.next().await.expect("r").expect("row");
        assert_eq!(row.get::<String>(0).expect("kind"), "TaskCompleted");
    }
}
