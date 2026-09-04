use crate::StoreError;
use serde_json::Value;

fn workflow_now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

impl crate::VoxDb {
    /// Register the current machine directory as a known Vox project (`components` + path key).
    ///
    /// The `user_preferences` path write is **best-effort**: failures are ignored so component
    /// registration still succeeds (check logs if paths do not persist).
    ///
    /// Needs `crate::paths::local_user_id`, gated behind `host-integration` —
    /// this function is exercised only by `vox-cli`, which already enables
    /// that feature.
    #[cfg(feature = "host-integration")]
    pub async fn register_local_project(
        &self,
        name: &str,
        path: &std::path::Path,
    ) -> Result<(), StoreError> {
        let abs_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let path_str = abs_path.to_string_lossy();

        self.register_component(
            name,
            "local", // namespace for local projects
            None,    // schema_hash not needed for projects
            Some(&format!("Local project at {}", path_str)),
            "1.0.0",
        )
        .await?;

        // Also store the path in user_preferences as a 'known_project'
        let user_id = crate::paths::local_user_id();
        let key = format!("project.{}.path", name);
        let value = path_str.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        let _ = breaker
            .call(|| async move {
                conn.execute(
                    "INSERT OR REPLACE INTO user_preferences (user_id, key, value) VALUES (?1, ?2, ?3)",
                    (user_id, key, value),
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await;

        Ok(())
    }

    /// Return true if the given activity was completed in the specified workflow run.
    pub async fn is_workflow_activity_completed(
        &self,
        run_id: &str,
        workflow_name: &str,
        activity_id: &str,
    ) -> Result<bool, StoreError> {
        let row = self.query_all(
            "SELECT 1 FROM workflow_activity_log WHERE run_id = ?1 AND workflow_name = ?2 AND activity_id = ?3 AND status = 'completed'",
            (run_id.to_string(), workflow_name.to_string(), activity_id.to_string())
        ).await?;
        Ok(!row.is_empty())
    }

    /// Load the stored result payload for a completed activity in the specified workflow run.
    pub async fn load_workflow_activity_result(
        &self,
        run_id: &str,
        workflow_name: &str,
        activity_id: &str,
    ) -> Result<Option<Value>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT result_json FROM workflow_activity_log
                 WHERE run_id = ?1 AND workflow_name = ?2 AND activity_id = ?3 AND status = 'completed'
                 ORDER BY recorded_at_ms DESC LIMIT 1",
                (
                    run_id.to_string(),
                    workflow_name.to_string(),
                    activity_id.to_string(),
                ),
            )
            .await?;
        let Some(row) = rows.next().await? else {
            return Ok(None);
        };
        let result_json: Option<String> = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
        match result_json {
            Some(json) => {
                let value = serde_json::from_str(&json)
                    .map_err(|e| StoreError::Serialization(e.to_string()))?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    /// Mark a workflow run as started (or resumed) with the current planned step count.
    pub async fn record_workflow_run_started(
        &self,
        run_id: &str,
        workflow_name: &str,
        planned_steps: usize,
    ) -> Result<(), StoreError> {
        let now = workflow_now_ms();
        let run_id = run_id.to_string();
        let workflow_name = workflow_name.to_string();
        let planned_steps = planned_steps as i64;
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO workflow_run_log (run_id, workflow_name, status, planned_steps, completed_steps, plan_session_id, plan_node_id, plan_version, lease_owner, lease_until_ms, started_at_ms, updated_at_ms, completed_at_ms, last_error)
                     VALUES (?1, ?2, 'running', ?3, 0, NULL, NULL, NULL, NULL, NULL, ?4, ?4, NULL, NULL)
                     ON CONFLICT(run_id) DO UPDATE SET
                       workflow_name=excluded.workflow_name,
                       status='running',
                       planned_steps=MAX(workflow_run_log.planned_steps, excluded.planned_steps),
                       updated_at_ms=excluded.updated_at_ms,
                       completed_at_ms=NULL",
                    (run_id, workflow_name, planned_steps, now),
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Attach orchestration planning context to a workflow run when available.
    pub async fn record_workflow_run_plan_context(
        &self,
        run_id: &str,
        plan_session_id: &str,
        plan_node_id: &str,
        plan_version: i64,
    ) -> Result<(), StoreError> {
        let run_id = run_id.to_string();
        let plan_session_id = plan_session_id.to_string();
        let plan_node_id = plan_node_id.to_string();
        let now = workflow_now_ms();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "UPDATE workflow_run_log
                     SET plan_session_id = ?2,
                         plan_node_id = ?3,
                         plan_version = ?4,
                         updated_at_ms = ?5
                     WHERE run_id = ?1",
                    (run_id, plan_session_id, plan_node_id, plan_version, now),
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Attempt to claim or renew single-owner lease for a workflow run.
    pub async fn try_claim_workflow_run_lease(
        &self,
        run_id: &str,
        owner_id: &str,
        lease_ttl_ms: u64,
    ) -> Result<bool, StoreError> {
        let now = workflow_now_ms();
        let lease_until_ms = now.saturating_add(lease_ttl_ms.max(1) as i64);
        let run_id = run_id.to_string();
        let owner_id = owner_id.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "UPDATE workflow_run_log
                     SET lease_owner = ?2,
                         lease_until_ms = ?3,
                         updated_at_ms = ?4
                     WHERE run_id = ?1
                       AND status = 'running'
                       AND (lease_owner IS NULL OR lease_until_ms IS NULL OR lease_until_ms < ?4 OR lease_owner = ?2)",
                    (run_id, owner_id, lease_until_ms, now),
                )
                .await?;
                let mut rows = conn.query("SELECT changes()", ()).await?;
                let Some(row) = rows.next().await? else {
                    return Ok::<bool, StoreError>(false);
                };
                let changes: i64 = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
                Ok::<bool, StoreError>(changes > 0)
            })
            .await
    }

    /// Return true when a workflow run has been cancelled.
    pub async fn is_workflow_run_cancelled(&self, run_id: &str) -> Result<bool, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT 1 FROM workflow_run_log WHERE run_id = ?1 AND status = 'cancelled' LIMIT 1",
                (run_id.to_string(),),
            )
            .await?;
        Ok(rows.next().await?.is_some())
    }

    /// Record that an activity has started in the durable journal.
    pub async fn record_workflow_activity_started(
        &self,
        run_id: &str,
        workflow_name: &str,
        activity_name: &str,
        activity_id: &str,
    ) -> Result<(), StoreError> {
        let now = workflow_now_ms();
        let run_id = run_id.to_string();
        let workflow_name = workflow_name.to_string();
        let activity_name = activity_name.to_string();
        let activity_id = activity_id.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
            "INSERT OR IGNORE INTO workflow_activity_log (run_id, workflow_name, activity_name, activity_id, status, recorded_at_ms) VALUES (?1, ?2, ?3, ?4, 'started', ?5)",
            (run_id, workflow_name, activity_name, activity_id, now)
        ).await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Append a workflow signal for one run.
    pub async fn record_workflow_signal(
        &self,
        run_id: &str,
        signal_key: &str,
        payload: Option<&Value>,
    ) -> Result<(), StoreError> {
        let now = workflow_now_ms();
        let run_id = run_id.to_string();
        let signal_key = signal_key.to_string();
        let payload_json = payload
            .map(serde_json::to_string)
            .transpose()
            .map_err(|e| StoreError::Serialization(e.to_string()))?;
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO workflow_signal_log (run_id, signal_key, payload_json, recorded_at_ms, consumed_at_ms)
                     VALUES (?1, ?2, ?3, ?4, NULL)",
                    (run_id, signal_key, payload_json, now),
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Record one attempt start for an activity execution boundary.
    pub async fn record_workflow_activity_attempt_started(
        &self,
        run_id: &str,
        workflow_name: &str,
        activity_id: &str,
        attempt_no: u32,
        worker_owner: &str,
        lease_ttl_ms: u64,
    ) -> Result<(), StoreError> {
        let now = workflow_now_ms();
        let lease_until_ms = now.saturating_add(lease_ttl_ms.max(1) as i64);
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT OR REPLACE INTO workflow_activity_attempt_log
                     (run_id, workflow_name, activity_id, attempt_no, status, worker_owner, lease_until_ms, error, recorded_at_ms)
                     VALUES (?1, ?2, ?3, ?4, 'started', ?5, ?6, NULL, ?7)",
                    (
                        run_id.to_string(),
                        workflow_name.to_string(),
                        activity_id.to_string(),
                        i64::from(attempt_no),
                        worker_owner.to_string(),
                        lease_until_ms,
                        now,
                    ),
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Record one attempt completion for an activity execution boundary.
    pub async fn record_workflow_activity_attempt_completed(
        &self,
        run_id: &str,
        workflow_name: &str,
        activity_id: &str,
        attempt_no: u32,
        worker_owner: &str,
    ) -> Result<(), StoreError> {
        let now = workflow_now_ms();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT OR REPLACE INTO workflow_activity_attempt_log
                     (run_id, workflow_name, activity_id, attempt_no, status, worker_owner, lease_until_ms, error, recorded_at_ms)
                     VALUES (?1, ?2, ?3, ?4, 'completed', ?5, NULL, NULL, ?6)",
                    (
                        run_id.to_string(),
                        workflow_name.to_string(),
                        activity_id.to_string(),
                        i64::from(attempt_no),
                        worker_owner.to_string(),
                        now,
                    ),
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Record one failed attempt for an activity execution boundary.
    pub async fn record_workflow_activity_attempt_failed(
        &self,
        run_id: &str,
        workflow_name: &str,
        activity_id: &str,
        attempt_no: u32,
        worker_owner: &str,
        error: &str,
    ) -> Result<(), StoreError> {
        let now = workflow_now_ms();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT OR REPLACE INTO workflow_activity_attempt_log
                     (run_id, workflow_name, activity_id, attempt_no, status, worker_owner, lease_until_ms, error, recorded_at_ms)
                     VALUES (?1, ?2, ?3, ?4, 'failed', ?5, NULL, ?6, ?7)",
                    (
                        run_id.to_string(),
                        workflow_name.to_string(),
                        activity_id.to_string(),
                        i64::from(attempt_no),
                        worker_owner.to_string(),
                        error.to_string(),
                        now,
                    ),
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Load the latest recorded attempt row for one activity.
    pub async fn load_latest_workflow_activity_attempt(
        &self,
        run_id: &str,
        workflow_name: &str,
        activity_id: &str,
    ) -> Result<Option<(u32, String, Option<String>, Option<i64>)>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT attempt_no, status, worker_owner, lease_until_ms
                 FROM workflow_activity_attempt_log
                 WHERE run_id = ?1 AND workflow_name = ?2 AND activity_id = ?3
                 ORDER BY attempt_no DESC, recorded_at_ms DESC
                 LIMIT 1",
                (
                    run_id.to_string(),
                    workflow_name.to_string(),
                    activity_id.to_string(),
                ),
            )
            .await?;
        let Some(row) = rows.next().await? else {
            return Ok(None);
        };
        let attempt_no: i64 = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
        let status: String = row.get(1).map_err(|e| StoreError::Db(e.to_string()))?;
        let worker_owner: Option<String> = row.get(2).map_err(|e| StoreError::Db(e.to_string()))?;
        let lease_until_ms: Option<i64> = row.get(3).map_err(|e| StoreError::Db(e.to_string()))?;
        let attempt_no = u32::try_from(attempt_no).map_err(|e| StoreError::Db(e.to_string()))?;
        Ok(Some((attempt_no, status, worker_owner, lease_until_ms)))
    }

    /// Consume one pending signal instance for this run/key.
    pub async fn consume_workflow_signal(
        &self,
        run_id: &str,
        signal_key: &str,
    ) -> Result<bool, StoreError> {
        let now = workflow_now_ms();
        let run_id = run_id.to_string();
        let signal_key = signal_key.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                let mut rows = conn
                    .query(
                        "SELECT id FROM workflow_signal_log
                         WHERE run_id = ?1 AND signal_key = ?2 AND consumed_at_ms IS NULL
                         ORDER BY recorded_at_ms ASC, id ASC
                         LIMIT 1",
                        (run_id.clone(), signal_key.clone()),
                    )
                    .await?;
                let Some(row) = rows.next().await? else {
                    return Ok::<bool, StoreError>(false);
                };
                let id: i64 = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
                conn.execute(
                    "UPDATE workflow_signal_log SET consumed_at_ms = ?2
                     WHERE id = ?1 AND consumed_at_ms IS NULL",
                    (id, now),
                )
                .await?;
                let mut ch = conn.query("SELECT changes()", ()).await?;
                let Some(ch_row) = ch.next().await? else {
                    return Ok::<bool, StoreError>(false);
                };
                let changes: i64 = ch_row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
                Ok::<bool, StoreError>(changes > 0)
            })
            .await
    }

    /// Record that an activity has successfully completed in the durable journal.
    pub async fn record_workflow_activity_completed(
        &self,
        run_id: &str,
        workflow_name: &str,
        activity_name: &str,
        activity_id: &str,
        result: &Value,
    ) -> Result<(), StoreError> {
        let now = workflow_now_ms();
        let run_id = run_id.to_string();
        let workflow_name = workflow_name.to_string();
        let activity_name = activity_name.to_string();
        let activity_id = activity_id.to_string();
        let result_json =
            serde_json::to_string(result).map_err(|e| StoreError::Serialization(e.to_string()))?;
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                let run_id_for_activity = run_id.clone();
                let run_id_for_progress = run_id.clone();
                conn.execute(
            "INSERT OR REPLACE INTO workflow_activity_log (run_id, workflow_name, activity_name, activity_id, status, result_json, recorded_at_ms) VALUES (?1, ?2, ?3, ?4, 'completed', ?5, ?6)",
            (
                run_id_for_activity,
                workflow_name,
                activity_name,
                activity_id,
                result_json,
                now,
            )
        ).await?;
                conn.execute(
                    "UPDATE workflow_run_log
                     SET completed_steps = CASE
                           WHEN planned_steps > completed_steps THEN completed_steps + 1
                           ELSE completed_steps
                         END,
                         updated_at_ms = ?2
                     WHERE run_id = ?1",
                    (run_id_for_progress, now),
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Mark a workflow run as completed.
    pub async fn record_workflow_run_completed(
        &self,
        run_id: &str,
        workflow_name: &str,
    ) -> Result<(), StoreError> {
        let now = workflow_now_ms();
        let run_id = run_id.to_string();
        let workflow_name = workflow_name.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "UPDATE workflow_run_log
                     SET status = 'completed',
                         workflow_name = ?2,
                         updated_at_ms = ?3,
                         completed_at_ms = ?3,
                         lease_owner = NULL,
                         lease_until_ms = NULL
                     WHERE run_id = ?1",
                    (run_id, workflow_name, now),
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Mark a workflow run as cancelled and release any active lease.
    pub async fn record_workflow_run_cancelled(
        &self,
        run_id: &str,
        reason: &str,
    ) -> Result<(), StoreError> {
        let now = workflow_now_ms();
        let run_id = run_id.to_string();
        let reason = reason.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "UPDATE workflow_run_log
                     SET status = 'cancelled',
                         last_error = ?2,
                         lease_owner = NULL,
                         lease_until_ms = NULL,
                         updated_at_ms = ?3
                     WHERE run_id = ?1",
                    (run_id, reason, now),
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Upsert the durable spec JSON for a reconstruction campaign.
    pub async fn upsert_reconstruction_campaign_spec(
        &self,
        campaign_id: &str,
        benchmark_tier: &str,
        objective: &str,
        spec_json: &Value,
    ) -> Result<(), StoreError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let campaign_id = campaign_id.trim().to_string();
        let benchmark_tier = benchmark_tier.trim().to_string();
        let objective = objective.trim().to_string();
        if campaign_id.is_empty() || benchmark_tier.is_empty() || objective.is_empty() {
            return Err(StoreError::Db(
                "reconstruction spec requires campaign_id, benchmark_tier, and objective".into(),
            ));
        }
        let spec_json = serde_json::to_string(spec_json)
            .map_err(|e| StoreError::Serialization(e.to_string()))?;
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO reconstruction_campaign_spec (campaign_id, benchmark_tier, objective, spec_json, created_at_ms, updated_at_ms)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?5)
                     ON CONFLICT(campaign_id) DO UPDATE SET
                       benchmark_tier=excluded.benchmark_tier,
                       objective=excluded.objective,
                       spec_json=excluded.spec_json,
                       updated_at_ms=excluded.updated_at_ms",
                    (campaign_id, benchmark_tier, objective, spec_json, now),
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Upsert one reconstruction artifact row.
    pub async fn upsert_reconstruction_artifact(
        &self,
        campaign_id: &str,
        artifact_id: &str,
        artifact_kind: &str,
        payload_json: &Value,
        tags: &[String],
        source: Option<&str>,
    ) -> Result<(), StoreError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let campaign_id = campaign_id.trim().to_string();
        let artifact_id = artifact_id.trim().to_string();
        let artifact_kind = artifact_kind.trim().to_string();
        if campaign_id.is_empty() || artifact_id.is_empty() || artifact_kind.is_empty() {
            return Err(StoreError::Db(
                "reconstruction artifact requires campaign_id, artifact_id, artifact_kind".into(),
            ));
        }
        let payload_json = serde_json::to_string(payload_json)
            .map_err(|e| StoreError::Serialization(e.to_string()))?;
        let tags_json =
            serde_json::to_string(tags).map_err(|e| StoreError::Serialization(e.to_string()))?;
        let source = source
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO reconstruction_artifacts (campaign_id, artifact_id, artifact_kind, payload_json, tags_json, source, created_at_ms, updated_at_ms)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
                     ON CONFLICT(campaign_id, artifact_id) DO UPDATE SET
                       artifact_kind=excluded.artifact_kind,
                       payload_json=excluded.payload_json,
                       tags_json=excluded.tags_json,
                       source=excluded.source,
                       updated_at_ms=excluded.updated_at_ms",
                    (campaign_id, artifact_id, artifact_kind, payload_json, tags_json, source, now),
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Record benchmark ladder KPIs for one campaign tier.
    pub async fn upsert_reconstruction_benchmark_kpis(
        &self,
        campaign_id: &str,
        benchmark_tier: &str,
        elapsed_ms: u64,
        autonomous_recovery_rate: f32,
        regenerated_file_success_rate: f32,
        cost_per_success_step: f64,
    ) -> Result<(), StoreError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let campaign_id = campaign_id.trim().to_string();
        let benchmark_tier = benchmark_tier.trim().to_string();
        if campaign_id.is_empty() || benchmark_tier.is_empty() {
            return Err(StoreError::Db(
                "reconstruction KPI requires campaign_id and benchmark_tier".into(),
            ));
        }
        let elapsed_ms = elapsed_ms as i64;
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO reconstruction_benchmark_kpis
                     (campaign_id, benchmark_tier, elapsed_ms, autonomous_recovery_rate, regenerated_file_success_rate, cost_per_success_step, recorded_at_ms)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                     ON CONFLICT(campaign_id, benchmark_tier) DO UPDATE SET
                       elapsed_ms=excluded.elapsed_ms,
                       autonomous_recovery_rate=excluded.autonomous_recovery_rate,
                       regenerated_file_success_rate=excluded.regenerated_file_success_rate,
                       cost_per_success_step=excluded.cost_per_success_step,
                       recorded_at_ms=excluded.recorded_at_ms",
                    (
                        campaign_id,
                        benchmark_tier,
                        elapsed_ms,
                        autonomous_recovery_rate as f64,
                        regenerated_file_success_rate as f64,
                        cost_per_success_step,
                        now,
                    ),
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }
}
