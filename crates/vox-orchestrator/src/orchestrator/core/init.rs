use crate::orchestrator::types::OrchestratorError;

impl crate::orchestrator::Orchestrator {
    /// Initialize the orchestrator database schema and set the DB handle.
    pub async fn init_db(
        &self,
        db: std::sync::Arc<vox_db::VoxDb>,
    ) -> Result<(), OrchestratorError> {
        db.sync_schema_from_digest(&crate::schema::orchestrator_schema())
            .await
            .map_err(|e| OrchestratorError::DatabaseError(format!("DB sync failed: {}", e)))?;

        crate::sync_lock::rw_write(&*self.db).replace(db.clone());

        self.reseed_oplog_id_gen(&db).await;

        let db_clone = db.clone();
        tokio::spawn(crate::activity::sink::run_sink(
            self.event_bus.subscribe(),
            move |row, ts_ms| {
                let db = db_clone.clone();
                async move {
                    // Typed vox-db ops method — keeps direct turso/SQL out of the
                    // orchestrator (turso-import / query-all SSOT boundary).
                    if let Err(e) = db
                        .insert_activity_log_row(
                            ts_ms as i64,
                            row.agent_id.as_deref(),
                            row.session_id.as_deref(),
                            &row.kind,
                            &row.summary,
                            &row.detail_json,
                        )
                        .await
                    {
                        tracing::warn!("Failed to persist activity log to database: {:?}", e);
                    }
                }
            },
            None,
        ));

        if let Some(swappable) = self
            .hopper
            .as_any()
            .downcast_ref::<crate::hopper::store::SwappableHopper>()
        {
            let sqlite_hopper =
                std::sync::Arc::new(crate::hopper::sqlite_store::SqliteHopper::with_bus(
                    db.clone(),
                    std::sync::Arc::new(self.event_bus.clone()),
                ));
            swappable.swap(sqlite_hopper).await;
        }

        // Rehydrate task hopper inbox items on boot using enqueue_dedup
        let inbox_items = self.hopper.inbox().await;
        for item in inbox_items {
            let task = crate::orchestrator::dispatch::intake_to_task(&item);
            let agents_lock = self.agents.read().unwrap();
            if let Some(queue_arc) = agents_lock.values().min_by_key(|q| q.read().unwrap().len()) {
                let mut queue = queue_arc.write().unwrap();
                queue.enqueue_dedup(task);
            } else {
                tracing::warn!(
                    "No active agents available to rehydrate task: {:?}",
                    item.item_id
                );
            }
        }

        // T1.4: reconstruct in-flight direct-submit tasks (submitted but never
        // reached TaskComplete/TaskFail as of the last durable oplog record)
        // that live ONLY in an agent's in-memory AgentQueue — the hopper-inbox
        // loop above only rehydrates hopper-sourced work. Must run after the
        // hopper loop so its HopperAssign exclusion reflects hopper state at
        // boot. See `orchestrator/core/rehydrate.rs` for fidelity/dedup notes.
        super::rehydrate::rehydrate_direct_submit_tasks(self, &db).await;

        match db.sqlite_capabilities_snapshot().await {
            Ok(p) => {
                tracing::debug!(
                    journal_mode = %p.journal_mode,
                    foreign_keys_on = p.foreign_keys_on,
                    fts5_reported = p.fts5_reported,
                    "sqlite capabilities (orchestrator init_db)"
                );
            }
            Err(e) => {
                tracing::debug!(error = %e, "sqlite capability probe failed during orchestrator init_db");
            }
        }

        // Resuscitate task transcripts from the workflow journal (cross-session recovery)
        let _ = self.hydrate_all_tasks_from_journal().await;

        // Perform initial scoreboard refresh to enable data-driven routing immediately
        self.refresh_model_scoreboard().await;

        Ok(())
    }

    /// Builder-style variant of [`Self::init_db`] (takes ownership, sets db, returns self).
    pub fn with_db(self, db: std::sync::Arc<vox_db::VoxDb>) -> Self {
        crate::sync_lock::rw_write(&*self.db).replace(db);
        self
    }

    /// Attach a database handle late (e.g. after async MCP connection).
    ///
    /// Used by the `vox mcp` stdio server (`ServerState::with_db_initialized`
    /// in vox-orchestrator-mcp), which — unlike `vox-orchestrator-d` — never
    /// calls the heavier [`Self::init_db`] (schema sync, hopper rehydration,
    /// journal resuscitation, etc.). It must still reseed the durable
    /// `OperationId` generator (T1.3), otherwise every `vox mcp` restart
    /// resets replay-offset ids back to 1 even though this process durably
    /// records Tier-A operations via `record_operation`.
    pub async fn attach_db(&self, db: std::sync::Arc<vox_db::VoxDb>) {
        crate::sync_lock::rw_write(&*self.db).replace(db.clone());
        self.reseed_oplog_id_gen(&db).await;
    }

    /// T1.3: reseed the in-process `OperationId` generator from durable
    /// history so it resumes strictly after the highest op_id already
    /// persisted in `agent_oplog`, instead of resetting to OP-000001
    /// whenever a process attaches a DB (daemon restart, or a fresh
    /// `vox mcp` stdio server attaching to an existing workspace DB). This
    /// is the prerequisite for using `OperationId` as a real
    /// replay-from-offset cursor in `orch.subscribe`/`orch.subscribe_events`
    /// — without it a restarted process could hand out ids that collide
    /// with (or shadow) history a client has already replayed.
    ///
    /// Queries [`vox_db::VoxDb::max_agent_oplog_id`] — the `agent_oplog`
    /// table, scoped to this process's `repository_id` — because that is
    /// the exact table [`crate::oplog::list_from_db_since`] reads for the
    /// replay phase (via `list_oplog_entries_since`). The unrelated
    /// `convergence_op_log` table (mesh-replication state, a different id
    /// sequence entirely) is deliberately not used here even though an
    /// earlier revision of this method queried it by mistake.
    ///
    /// Shared by [`Self::init_db`] and [`Self::attach_db`] so both real
    /// production DB-attach entry points (`vox-orchestrator-d` and
    /// `vox mcp`) get identical restart-durability guarantees.
    async fn reseed_oplog_id_gen(&self, db: &std::sync::Arc<vox_db::VoxDb>) {
        let repo = crate::lineage::repository_id();
        match db.max_agent_oplog_id(&repo).await {
            Ok(Some(highest)) => {
                crate::sync_lock::rw_write(&*self.oplog).reseed_id_gen_from_highest(highest);
            }
            Ok(None) => {} // fresh DB, no rows yet — generator already starts at 1
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "failed to query max agent_oplog id while attaching DB; \
                     OperationId generator NOT reseeded (may restart at 1, risking \
                     replay-offset collisions after a crash)"
                );
            }
        }
    }
}
