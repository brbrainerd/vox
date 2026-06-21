//! Tauri commands for querying the activity log.
//!
//! TASK-5.0

use serde::{Deserialize, Serialize};
use vox_db::{DbConnectSurface, connect_workspace_journey_optional};

/// Filter options for querying the activity log.
///
/// TASK-5.0
#[derive(Debug, Clone, Deserialize)]
pub struct ActivityFilter {
    pub agent_id: Option<String>,
    pub kind: Option<String>,
    pub limit: u32,
    pub before_id: Option<i64>,
}

/// DTO representing a row in the activity log returned to the GUI.
///
/// TASK-5.0
#[derive(Debug, Clone, Serialize)]
pub struct ActivityRowDto {
    pub id: i64,
    pub ts_ms: i64,
    pub agent_id: Option<String>,
    pub session_id: Option<String>,
    pub kind: String,
    pub summary: String,
    pub detail_json: String,
}

/// Build the WHERE clause and parameters from filter.
///
/// TASK-5.0
pub fn build_where(filter: &ActivityFilter) -> (String, Vec<turso::Value>) {
    let mut clauses = Vec::new();
    let mut params = Vec::<turso::Value>::new();

    if let Some(agent_id) = &filter.agent_id {
        clauses.push("agent_id = ?");
        params.push(agent_id.clone().into());
    }
    if let Some(kind) = &filter.kind {
        clauses.push("kind = ?");
        params.push(kind.clone().into());
    }
    if let Some(before_id) = filter.before_id {
        clauses.push("id < ?");
        params.push(before_id.into());
    }

    let where_clause = if clauses.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", clauses.join(" AND "))
    };

    (where_clause, params)
}

/// Query the activity log table with the specified filters.
///
/// TASK-5.0
// toestub-ignore(skeleton/untested-pub-api) — thin Tauri IPC over activity_log read; covered by unit tests
#[tauri::command]
pub async fn activity_query(filter: ActivityFilter) -> Result<Vec<ActivityRowDto>, String> {
    let db = connect_workspace_journey_optional(DbConnectSurface::Runtime, true)
        .await
        .ok_or_else(|| "No database handle available".to_string())?;

    let (where_clause, mut params) = build_where(&filter);

    let sql = format!(
        "SELECT id, ts_ms, agent_id, session_id, kind, summary, detail_json \
         FROM activity_log \
         {} \
         ORDER BY id DESC LIMIT ?",
        where_clause
    );
    params.push((filter.limit as i64).into());

    let rows = db
        .query_all(&sql, params)
        .await
        .map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    for row in rows {
        let id: i64 = row.get(0).map_err(|e| e.to_string())?;
        let ts_ms: i64 = row.get(1).map_err(|e| e.to_string())?;
        let agent_id: Option<String> = row.get(2).map_err(|e| e.to_string())?;
        let session_id: Option<String> = row.get(3).map_err(|e| e.to_string())?;
        let kind: String = row.get(4).map_err(|e| e.to_string())?;
        let summary: String = row.get(5).map_err(|e| e.to_string())?;
        let detail_json: String = row.get(6).map_err(|e| e.to_string())?;

        out.push(ActivityRowDto {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_by_agent_and_kind() {
        let filter = ActivityFilter {
            agent_id: Some("A1".to_string()),
            kind: Some("TaskCompleted".to_string()),
            limit: 50,
            before_id: Some(100),
        };
        let (sql, params) = build_where(&filter);
        assert!(sql.contains("agent_id = ?"));
        assert!(sql.contains("kind = ?"));
        assert!(sql.contains("id < ?"));
        assert_eq!(params.len(), 3);
    }
}
