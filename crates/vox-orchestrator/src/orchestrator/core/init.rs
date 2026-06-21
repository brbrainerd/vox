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
    pub fn attach_db(&self, db: std::sync::Arc<vox_db::VoxDb>) {
        crate::sync_lock::rw_write(&*self.db).replace(db);
    }
}
