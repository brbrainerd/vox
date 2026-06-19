//! Arca schema fragment for the history entries (Ditto-style).

pub const SCHEMA_HISTORY: &str = r#"
CREATE TABLE IF NOT EXISTS history_entries (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id        TEXT NOT NULL,
    kind           TEXT NOT NULL,            -- 'clip' | 'command' | 'chat'
    text           TEXT NOT NULL,
    redacted_text  TEXT NOT NULL,
    created_at     INTEGER NOT NULL,
    pinned         INTEGER NOT NULL DEFAULT 0,
    source         TEXT,                      -- 'cli' | 'gui' | 'osc633' | 'agent' | 'chat'
    token_estimate INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_history_repo_kind ON history_entries(repo_id, kind, created_at);
CREATE INDEX IF NOT EXISTS idx_history_pinned    ON history_entries(repo_id, pinned);
"#;

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn history_entries_schema_round_trip() {
        let db = crate::VoxDb::connect(crate::DbConfig::Memory)
            .await
            .expect("db");
        db.connection()
            .execute(
                "INSERT INTO history_entries (repo_id, kind, text, redacted_text, created_at, pinned, source, token_estimate)
                 VALUES ('r1','clip','hello','hello',1000,0,'cli',1)",
                (),
            )
            .await
            .expect("insert");
        let mut q = db
            .connection()
            .query("SELECT kind FROM history_entries WHERE repo_id='r1'", ())
            .await
            .expect("q");
        let row = q.next().await.expect("r").expect("row");
        assert_eq!(row.get::<String>(0).expect("kind"), "clip");
    }
}
