use sha3::Digest;

use vox_orchestrator_types::AgentId;
use vox_orchestrator_types::ChangeId;

use super::{OpLog, OperationEntry, OperationId, OperationKind};

/// List operations from the database for a repository/agent.
pub async fn list_from_db(
    store: &vox_db::VoxDb,
    agent_id: Option<AgentId>,
    repository_id: &str,
    limit: u32,
) -> Result<Vec<OperationEntry>, String> {
    let aid_str = agent_id.map(|id| id.0.to_string());
    let rows = store
        .list_oplog_entries(aid_str.as_deref(), repository_id, limit)
        .await
        .map_err(|e| e.to_string())?;

    let mut entries = Vec::new();
    for row in rows {
        let op_id_str = row[0].clone().unwrap_or_default();
        let agent_id_str = row[1].clone().unwrap_or_default();
        let kind_json = row[2].clone().unwrap_or_default();
        let description = row[3].clone().unwrap_or_default();
        let predecessor_hash = row[4].clone();
        let model_id = row[5].clone();
        let change_id = row[6].as_ref().and_then(|s| s.parse::<i64>().ok());
        let timestamp_ms = row[7]
            .as_ref()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0);
        let undone = row[8]
            .as_ref()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0);

        let op_id = OperationId(
            op_id_str
                .strip_prefix("OP-")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0),
        );
        let agent_id = AgentId(agent_id_str.parse().unwrap_or(0));
        let kind = serde_json::from_str(&kind_json).map_err(|e| e.to_string())?;

        entries.push(OperationEntry {
            id: op_id,
            agent_id,
            kind,
            description,
            predecessor_hash,
            model_id,
            change_id: change_id.map(|c| ChangeId(c as u64)),
            timestamp_ms: timestamp_ms as u64,
            undone: undone != 0,
            snapshot_before: None,
            snapshot_after: None,
            db_snapshot_before: None,
            db_snapshot_after: None,
            context_snapshot_before: None,
            context_snapshot_after: None,
            signature: None,
            signing_key_id: None,
            daemon_id: [0u8; 16],
            parent_op_ids: Vec::new(),
        });
    }

    Ok(entries)
}

impl OpLog {
    /// Access the full history of operations (oldest first).
    pub fn history(&self) -> Vec<&OperationEntry> {
        self.entries.iter().collect()
    }

    /// Access the stack of operations that can be redone (most recently undone last).
    pub fn redo_stack(&self) -> Vec<&OperationEntry> {
        self.entries.iter().filter(|e| e.undone).collect()
    }

    /// List recent operations (newest first), optionally filtered by agent.
    pub fn list(&self, agent_id: Option<AgentId>, limit: usize) -> Vec<&OperationEntry> {
        self.entries
            .iter()
            .rev()
            .filter(|e| agent_id.is_none_or(|a| e.agent_id == a))
            .take(limit)
            .collect()
    }

    /// Find the most recent non-undone operation for an agent.
    pub fn last_for_agent(&self, agent_id: AgentId) -> Option<&OperationEntry> {
        self.entries
            .iter()
            .rev()
            .find(|e| e.agent_id == agent_id && !e.undone)
    }

    /// Get a specific operation by ID.
    pub fn get(&self, op_id: OperationId) -> Option<&OperationEntry> {
        self.entries.iter().find(|e| e.id == op_id)
    }

    /// Alias for [`Self::get`] — looks up an entry by its [`OperationId`].
    pub fn lookup(&self, op_id: OperationId) -> Option<&OperationEntry> {
        self.get(op_id)
    }

    /// Total number of entries.
    pub fn count(&self) -> usize {
        self.entries.len()
    }

    /// Find the snapshots associated with a task's submission.
    pub fn find_task_snapshots(
        &self,
        task_id: u64,
    ) -> (Option<vox_orchestrator_types::SnapshotId>, Option<u64>) {
        for entry in self.entries.iter().rev() {
            if let OperationKind::TaskSubmit { task_id: id } = entry.kind
                && id == task_id
            {
                return (entry.snapshot_before, entry.db_snapshot_before);
            }
        }
        (None, None)
    }

    /// Find all operations belonging to a logical change.
    /// Returns entries in chronological order (oldest first).
    pub fn find_by_change_id(&self, change_id: ChangeId) -> Vec<&OperationEntry> {
        self.entries
            .iter()
            .filter(|e| e.change_id == Some(change_id))
            .collect()
    }

    /// Find all operations produced by a specific model (e.g. "gemini-2.5-pro").
    pub fn find_by_model(&self, model: &str) -> Vec<&OperationEntry> {
        self.entries
            .iter()
            .rev()
            .filter(|e| e.model_id.as_deref() == Some(model))
            .collect()
    }

    /// Verify the cryptographic chain integrity of the log.
    /// Returns the index of the first broken link, or `Ok(())` if intact.
    pub fn verify_chain(&self) -> Result<(), usize> {
        for (i, entry) in self.entries.iter().enumerate().skip(1) {
            let prev = &self.entries[i - 1];
            let mut hasher = sha3::Sha3_256::new();
            sha3::Digest::update(&mut hasher, prev.id.0.to_le_bytes());
            sha3::Digest::update(&mut hasher, prev.timestamp_ms.to_le_bytes());
            sha3::Digest::update(
                &mut hasher,
                prev.predecessor_hash.as_deref().unwrap_or("").as_bytes(),
            );
            let expected = format!("{:x}", sha3::Digest::finalize(hasher));
            if entry.predecessor_hash.as_deref() != Some(&expected) {
                return Err(i);
            }
        }
        Ok(())
    }

    /// Total cost across all recorded AI calls (in USD).
    pub fn total_cost_usd(&self) -> f64 {
        self.entries
            .iter()
            .filter_map(|e| {
                if let OperationKind::AiCall { cost_usd_micro, .. } = &e.kind {
                    Some(*cost_usd_micro as f64 * 1e-6)
                } else {
                    None
                }
            })
            .sum()
    }

    /// Total tokens consumed across all AI calls.
    pub fn total_tokens(&self) -> (u64, u64) {
        self.entries
            .iter()
            .filter_map(|e| {
                if let OperationKind::AiCall {
                    input_tokens,
                    output_tokens,
                    ..
                } = &e.kind
                {
                    Some((*input_tokens as u64, *output_tokens as u64))
                } else {
                    None
                }
            })
            .fold((0u64, 0u64), |(ai, ao), (i, o)| (ai + i, ao + o))
    }
}
