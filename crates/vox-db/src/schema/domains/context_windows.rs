//! Arca schema fragment for ContextWindow spine (design 2026-06-20 §4.1).

pub const SCHEMA_CONTEXT_WINDOWS: &str = r#"
CREATE TABLE IF NOT EXISTS context_windows (
    id                TEXT PRIMARY KEY,
    repo_id           TEXT NOT NULL,
    title             TEXT,
    kind              TEXT NOT NULL,             -- 'chat'|'task'|'agent'|'a2a'|'archived'
    tier              TEXT NOT NULL DEFAULT 'hot', -- 'hot'|'warm'|'cold'|'frozen'
    parent_window_id  TEXT,
    root_window_id    TEXT NOT NULL,
    agent_id          TEXT,
    thread_id         TEXT,
    trace_id          TEXT,
    model_route       TEXT,
    git_sha_at_open   TEXT,
    git_sha_at_close  TEXT,
    token_estimate    INTEGER NOT NULL DEFAULT 0,
    pinned            INTEGER NOT NULL DEFAULT 0,
    created_at        INTEGER NOT NULL,
    updated_at        INTEGER NOT NULL,
    deleted_at        INTEGER
);
CREATE INDEX IF NOT EXISTS idx_ctxwin_repo_tier ON context_windows(repo_id, tier, updated_at);
CREATE INDEX IF NOT EXISTS idx_ctxwin_tree      ON context_windows(root_window_id, parent_window_id);

CREATE TABLE IF NOT EXISTS context_window_items (
    id             TEXT PRIMARY KEY,
    window_id      TEXT NOT NULL,
    ordinal        INTEGER NOT NULL,
    role           TEXT NOT NULL,                -- 'user'|'assistant'|'system'|'tool'
    item_kind      TEXT NOT NULL,                -- 'message'|'pin'|'attachment'|'summary'|'tool_call'
    content_hash   TEXT NOT NULL,                -- references objects(hash) in CAS
    byte_len       INTEGER NOT NULL DEFAULT 0,   -- exact, model-invariant size (design §6.1)
    token_estimate INTEGER NOT NULL DEFAULT 0,   -- heuristic; exact per-model count is computed at the API boundary, NOT here
    pinned         INTEGER NOT NULL DEFAULT 0,
    committed      INTEGER NOT NULL DEFAULT 0,
    redacted       INTEGER NOT NULL DEFAULT 0,
    created_at     INTEGER NOT NULL,
    trimmed_at     INTEGER
);
CREATE INDEX IF NOT EXISTS idx_ctxitem_window  ON context_window_items(window_id, ordinal);
CREATE INDEX IF NOT EXISTS idx_ctxitem_hash    ON context_window_items(content_hash);
"#;

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn context_windows_schema_round_trip() {
        let db = crate::VoxDb::connect(crate::DbConfig::Memory)
            .await
            .expect("db");
        db.connection().execute(
            "INSERT INTO context_windows (id, repo_id, kind, root_window_id, token_estimate, created_at, updated_at)
             VALUES ('w1','r1','chat','w1',0,1000,1000)", ()).await.expect("insert window");
        db.connection().execute(
            "INSERT INTO context_window_items (id, window_id, ordinal, role, item_kind, content_hash, created_at)
             VALUES ('i1','w1',0,'user','message','deadbeef',1000)", ()).await.expect("insert item");
        let mut q = db
            .connection()
            .query(
                "SELECT content_hash FROM context_window_items WHERE window_id='w1'",
                (),
            )
            .await
            .expect("q");
        let row = q.next().await.expect("r").expect("row");
        assert_eq!(row.get::<String>(0).expect("hash"), "deadbeef");
    }
}
