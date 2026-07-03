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
    let rows = store
        .list_oplog_entries(
            agent_id.map(|id| id.0.to_string()).as_deref(),
            repository_id,
            limit,
        )
        .await
        .map_err(|e| e.to_string())?;
    rows_to_entries(rows)
}

/// [`list_from_db`], but replay-from-offset shaped (T1.3): every entry whose
/// `op_id` is strictly greater than `since_op_id`, oldest-first, unbounded by
/// `limit` (a caller reconnecting after a long outage may legitimately need
/// more than any fixed page size). Used by `orch.subscribe`/
/// `orch.subscribe_events`'s replay phase to catch a client up on durable
/// Tier-A history before it starts tailing the live broadcast bus. A sibling
/// function rather than an extension of `list_from_db`'s signature because the
/// two have different result ordering (`list_from_db` is newest-first/limited,
/// this is oldest-first/unbounded) and different callers.
pub async fn list_from_db_since(
    store: &vox_db::VoxDb,
    repository_id: &str,
    since_op_id: u64,
) -> Result<Vec<OperationEntry>, String> {
    let rows = store
        .list_oplog_entries_since(repository_id, since_op_id)
        .await
        .map_err(|e| e.to_string())?;
    rows_to_entries(rows)
}

/// [`list_from_db`], but bounded by an inclusive upper `operation_id`
/// (`op_id <= up_to`), oldest-first, unbounded by `limit` — a full-history
/// view of everything currently present in `agent_oplog` at or below `up_to`,
/// as opposed to [`list_from_db_since`]'s tail-only view starting after some
/// offset. Used by T1.6's `compact_now` (Bug 2 follow-up) to scan for
/// unresolved HITL `*Requested` entries before pruning: an approval requested
/// in an *earlier* checkpoint interval and still unresolved would be invisible
/// to a tail-only scan (it only still exists in the table because the earlier
/// compaction already excluded it from that prune), so the scan must cover
/// everything still present up to `up_to`, not just what changed since the
/// last checkpoint.
pub async fn list_from_db_up_to(
    store: &vox_db::VoxDb,
    repository_id: &str,
    up_to: u64,
) -> Result<Vec<OperationEntry>, String> {
    let rows = store
        .list_oplog_entries_up_to(repository_id, up_to)
        .await
        .map_err(|e| e.to_string())?;
    rows_to_entries(rows)
}

fn rows_to_entries(rows: Vec<Vec<Option<String>>>) -> Result<Vec<OperationEntry>, String> {
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
            let expected = hex::encode(sha3::Digest::finalize(hasher));
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
