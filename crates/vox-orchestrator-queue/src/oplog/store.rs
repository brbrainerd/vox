use std::sync::atomic::Ordering;
use std::time::{SystemTime, UNIX_EPOCH};

use sha3::{Digest, Sha3_256};

use vox_orchestrator_types::AgentId;
use vox_orchestrator_types::ChangeId;
use vox_orchestrator_types::SnapshotId;

use super::{OpLog, OperationEntry, OperationId, OperationKind};

/// Append an operation entry to the database with circuit breaker protection.
pub async fn append_to_db_with_breaker(
    db: &vox_db::VoxDb,
    agent_id: AgentId,
    op_id_str: &str,
    kind: &OperationKind,
    description: &str,
    predecessor_hash: Option<&str>,
    model_id: Option<&str>,
    change_id: Option<ChangeId>,
    timestamp_ms: u64,
    repository_id: &str,
) -> Result<(), String> {
    db.breaker()
        .call(|| async {
            append_to_db(
                db,
                agent_id,
                op_id_str,
                kind,
                description,
                predecessor_hash,
                model_id,
                change_id,
                timestamp_ms,
                repository_id,
            )
            .await
        })
        .await
}

/// Append an operation entry to the database.
pub async fn append_to_db(
    store: &vox_db::VoxDb,
    agent_id: AgentId,
    op_id_str: &str,
    kind: &OperationKind,
    description: &str,
    predecessor_hash: Option<&str>,
    model_id: Option<&str>,
    change_id: Option<ChangeId>,
    timestamp_ms: u64,
    repository_id: &str,
) -> Result<(), String> {
    let kind_json = serde_json::to_string(kind).map_err(|e| e.to_string())?;

    store
        .append_oplog_entry(
            &agent_id.0.to_string(),
            op_id_str,
            &kind_json,
            description,
            predecessor_hash,
            model_id,
            change_id.map(|c| c.0 as i64),
            timestamp_ms as i64,
            repository_id,
        )
        .await
        .map_err(|e| e.to_string())
}

/// Mark an operation as undone in the database.
pub async fn mark_undone_in_db(
    store: &vox_db::VoxDb,
    operation_id: &str,
    undone: bool,
) -> Result<(), String> {
    store
        .set_oplog_undone(operation_id, undone)
        .await
        .map_err(|e| e.to_string())
}

impl OpLog {
    /// Create a new log with the given capacity.
    pub fn new(max_entries: usize) -> Self {
        Self {
            id_gen: super::OperationIdGenerator::new(),
            db_snap_id_gen: std::sync::atomic::AtomicU64::new(1),
            entries: std::collections::VecDeque::new(),
            max_entries,
            persist: None,
        }
    }

    /// Produce the next unique DB snapshot ID.
    pub fn next_db_snapshot_id(&self) -> u64 {
        self.db_snap_id_gen.fetch_add(1, Ordering::Relaxed)
    }

    /// Record a new operation.
    #[allow(clippy::too_many_arguments)]
    pub fn record(
        &mut self,
        agent_id: AgentId,
        kind: OperationKind,
        description: impl Into<String>,
        snapshot_before: Option<SnapshotId>,
        snapshot_after: Option<SnapshotId>,
        db_snapshot_before: Option<u64>,
        db_snapshot_after: Option<u64>,
        context_snapshot_before: Option<u64>,
        context_snapshot_after: Option<u64>,
    ) -> OperationId {
        self.record_with_provenance(
            agent_id,
            kind,
            description,
            snapshot_before,
            snapshot_after,
            db_snapshot_before,
            db_snapshot_after,
            context_snapshot_before,
            context_snapshot_after,
            None,
            None,
        )
    }

    /// Record an operation with full provenance (change ID + model ID).
    #[allow(clippy::too_many_arguments)]
    pub fn record_with_provenance(
        &mut self,
        agent_id: AgentId,
        kind: OperationKind,
        description: impl Into<String>,
        snapshot_before: Option<SnapshotId>,
        snapshot_after: Option<SnapshotId>,
        db_snapshot_before: Option<u64>,
        db_snapshot_after: Option<u64>,
        context_snapshot_before: Option<u64>,
        context_snapshot_after: Option<u64>,
        change_id: Option<ChangeId>,
        model_id: Option<String>,
    ) -> OperationId {
        let id = self.id_gen.next();
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        // Compute cryptographic chain: hash(prev_id || prev_timestamp)
        let predecessor_hash = self.entries.back().map(|prev| {
            let mut hasher = Sha3_256::new();
            hasher.update(prev.id.0.to_le_bytes());
            hasher.update(prev.timestamp_ms.to_le_bytes());
            hasher.update(prev.predecessor_hash.as_deref().unwrap_or("").as_bytes());
            format!("{:x}", hasher.finalize())
        });

        let entry = OperationEntry {
            id,
            agent_id,
            timestamp_ms,
            kind,
            description: description.into(),
            snapshot_before,
            snapshot_after,
            db_snapshot_before,
            db_snapshot_after,
            context_snapshot_before,
            context_snapshot_after,
            undone: false,
            change_id,
            model_id,
            predecessor_hash,
            signature: None,
            signing_key_id: None,
            daemon_id: [0u8; 16],
            parent_op_ids: Vec::new(),
        };

        self.entries.push_back(entry);

        // Evict oldest if over limit.
        while self.entries.len() > self.max_entries {
            self.entries.pop_front();
        }

        id
    }

    /// Mark an operation as undone.
    ///
    /// Returns the entry's `snapshot_before` and `db_snapshot_before` so the caller can restore state.
    pub fn undo(&mut self, op_id: OperationId) -> Option<(Option<SnapshotId>, Option<u64>)> {
        for entry in self.entries.iter_mut().rev() {
            if entry.id == op_id && !entry.undone {
                entry.undone = true;
                return Some((entry.snapshot_before, entry.db_snapshot_before));
            }
        }
        None
    }

    /// Mark a previously undone operation as active again.
    ///
    /// Returns the entry's `snapshot_after` and `db_snapshot_after` so the caller can re-apply state.
    pub fn redo(&mut self, op_id: OperationId) -> Option<(Option<SnapshotId>, Option<u64>)> {
        for entry in self.entries.iter_mut().rev() {
            if entry.id == op_id && entry.undone {
                entry.undone = false;
                return Some((entry.snapshot_after, entry.db_snapshot_after));
            }
        }
        None
    }

    /// Record an AI model call in the log (convenience wrapper).
    pub fn record_ai_call(
        &mut self,
        agent_id: AgentId,
        provider: impl Into<String>,
        model: impl Into<String>,
        input_tokens: u32,
        output_tokens: u32,
        cost_usd: f64,
    ) -> OperationId {
        let cost_usd_micro = (cost_usd * 1_000_000.0).round() as u64;
        let provider = provider.into();
        let model = model.into();
        let desc = format!(
            "AI call: {}/{} ({} in / {} out tokens, ${:.6})",
            provider, model, input_tokens, output_tokens, cost_usd
        );
        self.record(
            agent_id,
            OperationKind::AiCall {
                provider,
                model,
                input_tokens,
                output_tokens,
                cost_usd_micro,
            },
            desc,
            None,
            None,
            None,
            None,
            None,
            None,
        )
    }
}

impl Default for OpLog {
    fn default() -> Self {
        Self::new(1000)
    }
}
