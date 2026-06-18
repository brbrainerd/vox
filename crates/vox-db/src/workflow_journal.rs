//! Interpreted workflow journal rows in `research_metrics` (session `workflow:<repository_id>`).
//! The embedded journal entries come from the interpreted workflow contract and currently carry
//! `journal_version = 1`.
//!
//! This is **durable-run telemetry**, not generic product “usage” analytics: classify **S1–S2** depending on
//! whether `entry` includes mesh or workspace-identifying payloads (see telemetry retention SSOT in-repo).

use serde::Serialize;
use serde_json::Value;

use crate::research_metrics_contract::{METRIC_TYPE_WORKFLOW_JOURNAL_ENTRY, TelemetryWriteOptions};
use crate::{StoreError, VoxDb};

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
struct WorkflowJournalMeta {
    /// Workflow name executed.
    pub workflow: String,
    /// One versioned journal object (e.g. `WorkflowStarted`, `ActivityReplayed`, `MeshActivity`).
    pub entry: Value,
}

impl VoxDb {
    /// Append one interpreted-workflow journal row under `workflow:<repository_id>`, metric `workflow_journal_entry`.
    pub async fn record_workflow_journal_entry(
        &self,
        repository_id: &str,
        workflow_name: &str,
        entry: &Value,
    ) -> Result<i64, StoreError> {
        let meta = WorkflowJournalMeta {
            workflow: workflow_name.to_string(),
            entry: entry.clone(),
        };
        let json =
            serde_json::to_string(&meta).map_err(|e| StoreError::Serialization(e.to_string()))?;
        let tw = TelemetryWriteOptions::new(repository_id);
        self.append_research_metric(
            &tw.session_workflow(),
            METRIC_TYPE_WORKFLOW_JOURNAL_ENTRY,
            None,
            Some(&json),
        )
        .await
    }

    /// Load all journal entries for a given repository and workflow, ordered by creation (id).
    /// Returns the raw `entry` payloads from the journal.
    pub async fn load_workflow_journal(
        &self,
        repository_id: &str,
        workflow_name: &str,
    ) -> Result<Vec<Value>, StoreError> {
        let tw = TelemetryWriteOptions::new(repository_id);
        let session_id = tw.session_workflow();

        let mut entries = Vec::new();
        let mut rows = self
            .conn
            .query(
                "SELECT metadata_json FROM research_metrics
                 WHERE session_id = ?1 AND metric_type = ?2
                 ORDER BY id ASC",
                (session_id, METRIC_TYPE_WORKFLOW_JOURNAL_ENTRY),
            )
            .await?;

        while let Some(row) = rows.next().await? {
            let json: String = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
            if let Ok(meta) = serde_json::from_str::<WorkflowJournalMeta>(&json)
                && meta.workflow == workflow_name {
                    entries.push(meta.entry);
                }
        }

        Ok(entries)
    }
}
