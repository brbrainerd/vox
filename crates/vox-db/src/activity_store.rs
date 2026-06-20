//! Typed accessors for the `activity_log` table.
//!
//! Centralises all SQL for the activity log so that GUI commands and the
//! orchestrator core do not embed raw SQL or import `turso` directly.

use crate::VoxDb;
use crate::store::StoreError;
use turso::params;

/// A single row from the `activity_log` table.
#[derive(Debug, Clone)]
pub struct ActivityLogRow {
    pub id: i64,
    pub ts_ms: i64,
    pub agent_id: Option<String>,
    pub session_id: Option<String>,
    pub kind: String,
    pub summary: String,
    pub detail_json: String,
}

/// Optional filter parameters for `query_activity`.
#[derive(Debug, Default, Clone)]
pub struct ActivityFilter {
    pub agent_id: Option<String>,
    pub kind: Option<String>,
    pub before_id: Option<i64>,
    pub limit: u32,
}

/// Query the `activity_log` table with optional filters, ordered by `id DESC`.
pub async fn query_activity(
    db: &VoxDb,
    filter: &ActivityFilter,
) -> Result<Vec<ActivityLogRow>, StoreError> {
    let mut clauses: Vec<String> = Vec::new();
    let mut vals: Vec<turso::Value> = Vec::new();

    if let Some(agent_id) = &filter.agent_id {
        clauses.push("agent_id = ?".to_string());
        vals.push(turso::Value::from(agent_id.clone()));
    }
    if let Some(kind) = &filter.kind {
        clauses.push("kind = ?".to_string());
        vals.push(turso::Value::from(kind.clone()));
    }
    if let Some(before_id) = filter.before_id {
        clauses.push("id < ?".to_string());
        vals.push(turso::Value::from(before_id));
    }
    vals.push(turso::Value::from(filter.limit as i64));

    let where_clause = if clauses.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", clauses.join(" AND "))
    };

    let sql = format!(
        "SELECT id, ts_ms, agent_id, session_id, kind, summary, detail_json \
         FROM activity_log \
         {} \
         ORDER BY id DESC LIMIT ?",
        where_clause
    );

    let mut rows = db.connection().query(&sql, vals).await?;

    let mut out = Vec::new();
    while let Some(row) = rows.next().await? {
        let id: i64 = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
        let ts_ms: i64 = row.get(1).map_err(|e| StoreError::Db(e.to_string()))?;
        let agent_id: Option<String> = row.get(2).map_err(|e| StoreError::Db(e.to_string()))?;
        let session_id: Option<String> = row.get(3).map_err(|e| StoreError::Db(e.to_string()))?;
        let kind: String = row.get(4).map_err(|e| StoreError::Db(e.to_string()))?;
        let summary: String = row.get(5).map_err(|e| StoreError::Db(e.to_string()))?;
        let detail_json: String = row.get(6).map_err(|e| StoreError::Db(e.to_string()))?;
        out.push(ActivityLogRow {
            id,
            ts_ms,
            agent_id,
            session_id,
            kind,
            summary,
            detail_json,
        });
    }
    Ok(out)
}

/// Insert a single row into `activity_log`.
pub async fn log_activity(
    db: &VoxDb,
    ts_ms: i64,
    agent_id: Option<&str>,
    session_id: Option<&str>,
    kind: &str,
    summary: &str,
    detail_json: &str,
) -> Result<(), StoreError> {
    let breaker = db.breaker.clone();
    let conn = db.conn.clone();
    let agent_id = agent_id.map(str::to_string);
    let session_id = session_id.map(str::to_string);
    let kind = kind.to_string();
    let summary = summary.to_string();
    let detail_json = detail_json.to_string();

    breaker
        .call(|| async move {
            conn.execute(
                "INSERT INTO activity_log (ts_ms, agent_id, session_id, kind, summary, detail_json) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    ts_ms,
                    agent_id,
                    session_id,
                    kind.as_str(),
                    summary.as_str(),
                    detail_json.as_str()
                ],
            )
            .await?;
            Ok::<(), StoreError>(())
        })
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DbConfig;

    #[tokio::test]
    async fn log_and_query_activity_round_trip() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("db");
        log_activity(
            &db,
            1000,
            Some("a1"),
            Some("s1"),
            "AgentSpawned",
            "Agent spawned: foo",
            "{}",
        )
        .await
        .expect("log");
        log_activity(
            &db,
            1001,
            Some("a2"),
            None,
            "TaskCompleted",
            "Task done",
            "{\"ok\":true}",
        )
        .await
        .expect("log");

        let all = query_activity(
            &db,
            &ActivityFilter {
                limit: 50,
                ..Default::default()
            },
        )
        .await
        .expect("query");
        assert_eq!(all.len(), 2);
        // Ordered by id DESC — newest first
        assert_eq!(all[0].kind, "TaskCompleted");
        assert_eq!(all[1].kind, "AgentSpawned");
    }

    #[tokio::test]
    async fn query_activity_filters_by_kind() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("db");
        log_activity(&db, 1000, None, None, "AgentSpawned", "s", "{}")
            .await
            .expect("log");
        log_activity(&db, 1001, None, None, "TaskCompleted", "t", "{}")
            .await
            .expect("log");

        let filter = ActivityFilter {
            kind: Some("AgentSpawned".to_string()),
            limit: 50,
            ..Default::default()
        };
        let rows = query_activity(&db, &filter).await.expect("query");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].kind, "AgentSpawned");
    }
}
