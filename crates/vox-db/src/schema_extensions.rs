//! Ad-hoc schema extensions (e.g. FTS5 virtual tables) that depend on libSQL compile options
//! and cannot be included in the monolithic baseline DDL safely.

use crate::store::types::StoreError;
use turso::Connection;

pub async fn apply_schema_extensions(conn: &Connection) -> Result<(), StoreError> {
    for sql in [
        "ALTER TABLE llm_interactions ADD COLUMN task_category TEXT NOT NULL DEFAULT 'general'",
        "ALTER TABLE llm_interactions ADD COLUMN strength_tag TEXT NOT NULL DEFAULT 'generalist'",
        "ALTER TABLE llm_interactions ADD COLUMN trace_id TEXT",
        "ALTER TABLE llm_interactions ADD COLUMN success INTEGER NOT NULL DEFAULT 1",
        "ALTER TABLE llm_interactions ADD COLUMN latency_ms INTEGER",
        "ALTER TABLE llm_interactions ADD COLUMN input_tokens INTEGER",
        "ALTER TABLE llm_interactions ADD COLUMN output_tokens INTEGER",
        "ALTER TABLE llm_interactions ADD COLUMN cost_usd REAL",
        "ALTER TABLE llm_interactions ADD COLUMN context_utilization_pct REAL",
        "ALTER TABLE llm_interactions ADD COLUMN cache_read_tokens INTEGER",
        "ALTER TABLE model_scoreboard ADD COLUMN success_count INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE model_scoreboard ADD COLUMN cumulative_cost_usd REAL NOT NULL DEFAULT 0.0",
    ] {
        exec_optional(conn, sql).await;
    }

    apply_knowledge_fts_cutover(conn).await?;
    apply_search_document_chunks_fts_cutover(conn).await?;
    exec_optional_batch(
        conn,
        "UPDATE agent_trust_scores SET _data = json_set(_data, '$.variance', 1.0) WHERE json_type(json_extract(_data, '$.variance')) IS NULL;",
    )
    .await;
    Ok(())
}

async fn exec_optional(conn: &Connection, sql: &str) {
    if let Err(e) = conn.execute(sql, ()).await {
        tracing::warn!(statement = %sql, error = %e, "schema extension statement skipped");
    }
}

async fn exec_optional_batch(conn: &Connection, sql: &str) {
    if let Err(e) = conn.execute_batch(sql).await {
        tracing::warn!(statement = %sql, error = %e, "schema extension batch skipped");
    }
}

async fn apply_knowledge_fts_cutover(conn: &Connection) -> Result<(), StoreError> {
    let mut has_fts5 = false;
    if let Ok(mut rows) = conn.query("PRAGMA compile_options", ()).await {
        while let Some(row) = rows.next().await? {
            let opt: String = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
            if opt.contains("FTS5") || opt == "ENABLE_FTS5" {
                has_fts5 = true;
                break;
            }
        }
    }
    if !has_fts5 {
        return Ok(());
    }

    let batch = r#"
CREATE VIRTUAL TABLE IF NOT EXISTS knowledge_nodes_fts
USING fts5(
    id UNINDEXED,
    label,
    content,
    content='knowledge_nodes',
    content_rowid='rowid'
);
CREATE TRIGGER IF NOT EXISTS knowledge_nodes_fts_ai
AFTER INSERT ON knowledge_nodes BEGIN
    INSERT INTO knowledge_nodes_fts(rowid, id, label, content)
    VALUES (new.rowid, new.id, new.label, COALESCE(new.content, ''));
END;
CREATE TRIGGER IF NOT EXISTS knowledge_nodes_fts_ad
AFTER DELETE ON knowledge_nodes BEGIN
    INSERT INTO knowledge_nodes_fts(knowledge_nodes_fts, rowid, id, label, content)
    VALUES ('delete', old.rowid, old.id, old.label, COALESCE(old.content, ''));
END;
CREATE TRIGGER IF NOT EXISTS knowledge_nodes_fts_au
AFTER UPDATE ON knowledge_nodes BEGIN
    INSERT INTO knowledge_nodes_fts(knowledge_nodes_fts, rowid, id, label, content)
    VALUES ('delete', old.rowid, old.id, old.label, COALESCE(old.content, ''));
    INSERT INTO knowledge_nodes_fts(rowid, id, label, content)
    VALUES (new.rowid, new.id, new.label, COALESCE(new.content, ''));
END;
INSERT INTO knowledge_nodes_fts(rowid, id, label, content)
SELECT rowid, id, label, COALESCE(content, '')
FROM knowledge_nodes
WHERE rowid NOT IN (SELECT rowid FROM knowledge_nodes_fts);
"#;
    exec_optional_batch(conn, batch).await;
    Ok(())
}

/// FTS5 over `search_document_chunks.body_text` for RAG-style chunk retrieval (mirrors knowledge_nodes_fts).
async fn apply_search_document_chunks_fts_cutover(conn: &Connection) -> Result<(), StoreError> {
    let mut has_fts5 = false;
    if let Ok(mut rows) = conn.query("PRAGMA compile_options", ()).await {
        while let Some(row) = rows.next().await? {
            let opt: String = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
            if opt.contains("FTS5") || opt == "ENABLE_FTS5" {
                has_fts5 = true;
                break;
            }
        }
    }
    if !has_fts5 {
        return Ok(());
    }

    let batch = r#"
CREATE VIRTUAL TABLE IF NOT EXISTS search_document_chunks_fts
USING fts5(
    body_text,
    content='search_document_chunks',
    content_rowid='rowid'
);
CREATE TRIGGER IF NOT EXISTS search_document_chunks_fts_ai
AFTER INSERT ON search_document_chunks BEGIN
    INSERT INTO search_document_chunks_fts(rowid, body_text)
    VALUES (new.rowid, new.body_text);
END;
CREATE TRIGGER IF NOT EXISTS search_document_chunks_fts_ad
AFTER DELETE ON search_document_chunks BEGIN
    INSERT INTO search_document_chunks_fts(search_document_chunks_fts, rowid, body_text)
    VALUES ('delete', old.rowid, old.body_text);
END;
CREATE TRIGGER IF NOT EXISTS search_document_chunks_fts_au
AFTER UPDATE ON search_document_chunks BEGIN
    INSERT INTO search_document_chunks_fts(search_document_chunks_fts, rowid, body_text)
    VALUES ('delete', old.rowid, old.body_text);
    INSERT INTO search_document_chunks_fts(rowid, body_text)
    VALUES (new.rowid, new.body_text);
END;
INSERT INTO search_document_chunks_fts(rowid, body_text)
SELECT rowid, body_text FROM search_document_chunks
WHERE rowid NOT IN (SELECT rowid FROM search_document_chunks_fts);
"#;
    exec_optional_batch(conn, batch).await;
    Ok(())
}
