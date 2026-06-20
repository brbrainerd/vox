//! Tauri commands for querying the activity log.
//!
//! TASK-5.0

use serde::{Deserialize, Serialize};
use vox_db::{DbConnectSurface, activity_store, connect_workspace_journey_optional};

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

/// Query the activity log table with the specified filters.
///
/// TASK-5.0
// toestub-ignore(skeleton/untested-pub-api) — thin Tauri IPC over activity_log read; covered by unit tests
#[tauri::command]
pub async fn activity_query(filter: ActivityFilter) -> Result<Vec<ActivityRowDto>, String> {
    let db = connect_workspace_journey_optional(DbConnectSurface::Runtime, true)
        .await
        .ok_or_else(|| "No database handle available".to_string())?;

    let store_filter = activity_store::ActivityFilter {
        agent_id: filter.agent_id,
        kind: filter.kind,
        before_id: filter.before_id,
        limit: filter.limit,
    };

    let rows = activity_store::query_activity(&db, &store_filter)
        .await
        .map_err(|e| e.to_string())?;

    Ok(rows
        .into_iter()
        .map(|r| ActivityRowDto {
            id: r.id,
            ts_ms: r.ts_ms,
            agent_id: r.agent_id,
            session_id: r.session_id,
            kind: r.kind,
            summary: r.summary,
            detail_json: r.detail_json,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_mapping_preserves_fields() {
        let filter = ActivityFilter {
            agent_id: Some("A1".to_string()),
            kind: Some("TaskCompleted".to_string()),
            limit: 50,
            before_id: Some(100),
        };
        let store_filter = activity_store::ActivityFilter {
            agent_id: filter.agent_id.clone(),
            kind: filter.kind.clone(),
            before_id: filter.before_id,
            limit: filter.limit,
        };
        assert_eq!(store_filter.agent_id.as_deref(), Some("A1"));
        assert_eq!(store_filter.kind.as_deref(), Some("TaskCompleted"));
        assert_eq!(store_filter.before_id, Some(100));
        assert_eq!(store_filter.limit, 50);
    }
}
