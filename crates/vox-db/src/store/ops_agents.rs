//! Agent session management and LLM interaction logging for [`VoxDb`].
//!
//! Covers two V3/agents-domain table groups:
//! - **`agent_sessions`** — lifecycle tracking: create, close, query.
//! - **`llm_interactions`** + **`llm_feedback`** — RLHF data pipeline used by `vox-package/feedback.rs`.

use turso::params;

use crate::store::types::{StoreError, TrainingPair};

impl crate::VoxDb {
    // ── Agent Events (agent_events) ──────────────────────────────────────────

    /// Insert a row into `agent_events` for telemetry tracking.
    /// Prefers the dedicated writer actor for high-concurrency safety.
    pub async fn record_agent_event(
        &self,
        agent_id: &str,
        event_type: &str,
        payload_json: &str,
        cli_version: &str,
    ) -> Result<i64, StoreError> {
        if let Some(writer) = &self.writer {
            return writer
                .insert_agent_event(
                    agent_id.to_string(),
                    event_type.to_string(),
                    Some(payload_json.to_string()),
                    Some(cli_version.to_string()),
                )
                .await;
        }

        let agent_id = agent_id.to_string();
        let event_type = event_type.to_string();
        let payload_json = payload_json.to_string();
        let cli_version = cli_version.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO agent_events (agent_id, event_type, payload_json, cli_version, timestamp)
             VALUES (?1, ?2, ?3, ?4, datetime('now'))",
                    params![
                        agent_id.as_str(),
                        event_type.as_str(),
                        payload_json.as_str(),
                        cli_version.as_str(),
                    ],
                )
                .await?;
                Ok::<i64, StoreError>(conn.last_insert_rowid())
            })
            .await
    }

    // ── Agent Sessions (agent_sessions) ──────────────────────────────────────

    /// Insert or activate an `agent_sessions` row.
    ///
    /// On conflict the row's `status` is set back to `'active'` and `task_snapshot` updated.
    /// Called from `vox-orchestrator/src/session.rs`.
    pub async fn create_session(
        &self,
        session_id: &str,
        agent_id: &str,
        tenant_id: Option<&str>,
        task_snapshot: Option<&str>,
    ) -> Result<(), StoreError> {
        let session_id = session_id.to_string();
        let agent_id = agent_id.to_string();
        let tenant_id = tenant_id.map(str::to_string);
        let task_snapshot = task_snapshot.map(str::to_string);
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO agent_sessions (id, agent_id, tenant_id, task_snapshot, status, started_at)
                     VALUES (?1, ?2, ?3, ?4, 'active', datetime('now'))
                     ON CONFLICT(id) DO UPDATE SET
                         task_snapshot = excluded.task_snapshot,
                         status        = 'active'",
                    params![
                        session_id.as_str(),
                        agent_id.as_str(),
                        tenant_id.as_deref(),
                        task_snapshot.as_deref(),
                    ],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Mark an `agent_sessions` row as the given `status` and set `ended_at`.
    pub async fn close_session(&self, session_id: &str, status: &str) -> Result<(), StoreError> {
        let session_id = session_id.to_string();
        let status = status.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "UPDATE agent_sessions
                     SET status = ?2, ended_at = datetime('now')
                     WHERE id = ?1",
                    params![session_id.as_str(), status.as_str()],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    // ── LLM Interactions (llm_interactions) ──────────────────────────────────

    /// Append a row to `llm_interactions`. Returns the inserted `rowid`.
    ///
    /// Called from `vox-package/src/feedback.rs` `FeedbackCollector::persist_to_store`.
    pub async fn log_interaction(
        &self,
        session_id: &str,
        user_id: Option<&str>,
        prompt: &str,
        response: &str,
        model_version: &str,
        latency_ms: Option<i64>,
        input_tokens: Option<i64>,
        output_tokens: Option<i64>,
    ) -> Result<i64, StoreError> {
        let session_id = session_id.to_string();
        let user_id = user_id.map(str::to_string);
        let prompt = prompt.to_string();
        let response = response.to_string();
        let model_version = model_version.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO llm_interactions
                         (session_id, user_id, prompt, response, model_version, latency_ms, input_tokens, output_tokens)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        session_id.as_str(),
                        user_id.as_deref(),
                        prompt.as_str(),
                        response.as_str(),
                        model_version.as_str(),
                        latency_ms,
                        input_tokens,
                        output_tokens
                    ],
                )
                .await?;
                Ok::<_, StoreError>(conn.last_insert_rowid())
            })
            .await
    }

    /// Record a complete LLM outcome, writing to both the `llm_interactions` table for full-text
    /// retention and the `model_scoreboard` aggregation buffer for intelligent routing.
    pub async fn record_llm_outcome(
        &self,
        outcome: crate::store::types::ModelOutcome<'_>,
    ) -> Result<i64, StoreError> {
        let session_id = outcome.session_id.to_string();
        let user_id = outcome.user_id.map(str::to_string);
        let prompt = outcome.prompt.to_string();
        let response = outcome.response.to_string();
        let model_id = outcome.model_id.to_string();
        let task_category = outcome.task_category.to_string();
        let strength_tag = outcome.strength_tag.to_string();

        let latency_ms = outcome.latency_ms;
        let input_tokens = outcome.input_tokens;
        let output_tokens = outcome.output_tokens;
        let cache_read_tokens = outcome.cache_read_tokens;
        let trace_id = outcome.trace_id.map(str::to_string);
        let context_utilization_pct = outcome.context_utilization_pct;
        let success = outcome.success;
        let cost_usd = outcome.cost_usd;
        let quality_score = outcome.quality_score.unwrap_or(1.0);

        let breaker = self.breaker.clone();
        let conn = self.conn.clone();

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        breaker
            .call(|| async move {
                // 1. Insert detailed interaction
                conn.execute(
                    "INSERT INTO llm_interactions
                         (session_id, user_id, tenant_id, prompt, response, model_version, task_category, strength_tag, trace_id, context_utilization_pct, cache_read_tokens, success, latency_ms, input_tokens, output_tokens, cost_usd)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
                    params![
                        session_id.as_str(),
                        user_id.as_deref(),
                        outcome.tenant_id,
                        prompt.as_str(),
                        response.as_str(),
                        model_id.as_str(),
                        task_category.as_str(),
                        strength_tag.as_str(),
                        trace_id.as_deref(),
                        context_utilization_pct,
                        cache_read_tokens,
                        if success { 1 } else { 0 },
                        latency_ms,
                        input_tokens,
                        output_tokens,
                        cost_usd
                    ],
                )
                .await?;

                let rowid = conn.last_insert_rowid();

                // 2. Upsert to model_scoreboard (7-day window)
                let window_days = 7;
                let cost_to_add = cost_usd.unwrap_or(0.0);

                // Note: p50/p99 approximations require separate compute batches.
                // We do a simple exponential moving average for quality/cost here for now,
                // or just increment the counters and let batch jobs recalculate p50.
                conn.execute(
                    "INSERT INTO model_scoreboard
                        (model_id, task_category, strength_tag, window_days, n_calls, success_rate, cost_per_success_usd, quality_score, updated_at_ms)
                     VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6, ?7, ?8)
                     ON CONFLICT(model_id, task_category, strength_tag, window_days) DO UPDATE SET
                        n_calls = n_calls + 1,
                        success_rate = ((success_rate * n_calls) + ?5) / (n_calls + 1),
                        cost_per_success_usd = ((cost_per_success_usd * (success_rate * n_calls)) + ?6) / MAX(1.0, (success_rate * n_calls) + ?5),
                        quality_score = ((quality_score * n_calls) + ?7) / (n_calls + 1),
                        updated_at_ms = ?8",
                    params![
                        model_id.as_str(),
                        task_category.as_str(),
                        strength_tag.as_str(),
                        window_days,
                        if success { 1.0 } else { 0.0 },
                        cost_to_add,
                        quality_score,
                        now_ms
                    ]
                ).await?;

                Ok::<_, StoreError>(rowid)
            })
            .await
    }

    // ── LLM Feedback (llm_feedback) ───────────────────────────────────────────

    /// Append a `llm_feedback` row linked to an `llm_interactions` rowid.
    ///
    /// Called from `vox-package/src/feedback.rs` `FeedbackCollector::persist_to_store`.
    pub async fn submit_feedback(
        &self,
        interaction_id: i64,
        user_id: Option<&str>,
        rating: Option<i64>,
        feedback_type: &str,
        correction_text: Option<&str>,
        preferred_response: Option<&str>,
    ) -> Result<i64, StoreError> {
        let user_id = user_id.map(str::to_string);
        let feedback_type = feedback_type.to_string();
        let correction_text = correction_text.map(str::to_string);
        let preferred_response = preferred_response.map(str::to_string);
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO llm_feedback
                         (interaction_id, user_id, rating, feedback_type, correction_text, preferred_response)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        interaction_id,
                        user_id.as_deref(),
                        rating,
                        feedback_type.as_str(),
                        correction_text.as_deref(),
                        preferred_response.as_deref(),
                    ],
                )
                .await?;
                Ok::<_, StoreError>(conn.last_insert_rowid())
            })
            .await
    }

    // ── Agent Reliability (agent_reliability) ─────────────────────────────────

    /// Return all `(agent_id, reliability)` pairs from `agent_reliability`, highest first.
    ///
    /// Used by `vox-orchestrator` `RoutingService::route` when Socrates reputation routing
    /// is enabled (`OrchestratorConfig::socrates_reputation_routing = true`).
    pub async fn list_agent_reliability(&self) -> Result<Vec<(String, f64)>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT entity_id, reliability FROM reliability_scores WHERE entity_type = 'agent' ORDER BY reliability DESC",
                (),
            )
            .await?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await? {
            let id: String = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
            let r: f64 = row.get(1).map_err(|e| StoreError::Db(e.to_string()))?;
            out.push((id, r));
        }
        Ok(out)
    }

    /// Read `reliability` for one `agent_id`, or `None` if no row exists.
    pub async fn get_agent_reliability(&self, agent_id: &str) -> Result<Option<f64>, StoreError> {
        let agent_id = agent_id.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                let mut rows = conn
                    .query(
                        "SELECT reliability FROM reliability_scores WHERE entity_type = 'agent' AND entity_id = ?1 LIMIT 1",
                        params![agent_id.as_str()],
                    )
                    .await?;
                match rows.next().await? {
                    Some(row) => {
                        let r: f64 = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
                        Ok(Some(r))
                    }
                    None => Ok(None),
                }
            })
            .await
    }

    /// Upsert a Laplace-smoothed reliability score for `agent_id` in `agent_reliability`.
    ///
    /// On first insert the row starts at `(success=1, failure=0)` or `(success=0, failure=1)`.
    /// Subsequent calls increment the relevant counter and recompute
    /// `reliability = (success_count + 1) / (success_count + failure_count + 2)`.
    ///
    /// Called from `vox-orchestrator` `Orchestrator::complete_task` and `fail_task`.
    pub async fn record_task_reliability_observation(
        &self,
        agent_id: &str,
        success: bool,
    ) -> Result<(), StoreError> {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let agent_id = agent_id.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                if success {
                    conn.execute(
                        "INSERT INTO reliability_scores (entity_type, entity_id, success_count, failure_count,
                             reliability, updated_at_ms)
                         VALUES ('agent', ?1, 1, 0,
                             CAST(2 AS REAL) / CAST(3 AS REAL),
                             ?2)
                         ON CONFLICT(entity_type, entity_id) DO UPDATE SET
                             success_count  = success_count + 1,
                             reliability    = CAST(success_count + 2 AS REAL)
                                            / CAST(success_count + failure_count + 3 AS REAL),
                             updated_at_ms  = ?2",
                        params![agent_id.as_str(), now_ms],
                    )
                    .await?;
                } else {
                    conn.execute(
                        "INSERT INTO reliability_scores (entity_type, entity_id, success_count, failure_count,
                             reliability, updated_at_ms)
                         VALUES ('agent', ?1, 0, 1,
                             CAST(1 AS REAL) / CAST(3 AS REAL),
                             ?2)
                         ON CONFLICT(entity_type, entity_id) DO UPDATE SET
                             failure_count  = failure_count + 1,
                             reliability    = CAST(success_count + 1 AS REAL)
                                            / CAST(success_count + failure_count + 3 AS REAL),
                             updated_at_ms  = ?2",
                        params![agent_id.as_str(), now_ms],
                    )
                    .await?;
                }
                Ok::<(), StoreError>(())
            })
            .await
    }

    // ── Object / Workspace Metadata (user_preferences) ───────────────────────

    /// Read a metadata value keyed by `namespace` and `key` from `user_preferences`.
    ///
    /// The look-up key is `"{namespace}.{key}"` and returns the `value` column,
    /// or `StoreError::NotFound` when absent. Used by `vox doctor` to detect
    /// registered project workspaces.
    pub async fn get_object_metadata(
        &self,
        namespace: &str,
        key: &str,
    ) -> Result<String, StoreError> {
        let lookup = format!("{namespace}.{key}");
        let mut rows = self
            .conn
            .query(
                "SELECT value FROM user_preferences WHERE key = ?1 LIMIT 1",
                params![lookup],
            )
            .await?;
        match rows.next().await? {
            Some(row) => {
                let val: String = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
                Ok(val)
            }
            None => Err(StoreError::NotFound(format!("{namespace}.{key}"))),
        }
    }

    /// Read `user_preferences.value` for an exact `key` (any `user_id`), or `None` if missing.
    ///
    /// Used by `vox doctor` for legacy rows keyed by dotted paths (e.g. `project.vox-workspace.path`).
    pub async fn get_user_preference_value_by_key(
        &self,
        key: &str,
    ) -> Result<Option<String>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT value FROM user_preferences WHERE key = ?1 LIMIT 1",
                params![key],
            )
            .await?;
        match rows.next().await? {
            Some(row) => Ok(Some(row.get(0).map_err(|e| StoreError::Db(e.to_string()))?)),
            None => Ok(None),
        }
    }

    /// `agent_reliability` rows with `reliability >= min_reliability`, highest first.
    pub async fn list_agent_reliability_above(
        &self,
        min_reliability: f64,
        limit: i64,
    ) -> Result<Vec<(String, f64, i64, i64)>, StoreError> {
        let lim = limit.clamp(1, 10_000);
        let mut rows = self
            .conn
            .query(
                "SELECT entity_id, reliability, success_count, failure_count
             FROM reliability_scores WHERE entity_type = 'agent' AND reliability >= ?1 ORDER BY reliability DESC LIMIT ?2",
                params![min_reliability, lim],
            )
            .await?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await? {
            out.push((
                row.get(0).map_err(|e| StoreError::Db(e.to_string()))?,
                row.get(1).map_err(|e| StoreError::Db(e.to_string()))?,
                row.get(2).map_err(|e| StoreError::Db(e.to_string()))?,
                row.get(3).map_err(|e| StoreError::Db(e.to_string()))?,
            ));
        }
        Ok(out)
    }

    /// Fetch one `agent_sessions` row by id (any `status`), for replay without scanning actives.
    pub async fn get_agent_session_row(
        &self,
        session_id: &str,
    ) -> Result<Option<(String, String, Option<String>)>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT id, agent_id, task_snapshot FROM agent_sessions WHERE id = ?1 LIMIT 1",
                params![session_id],
            )
            .await?;
        Ok(if let Some(row) = rows.next().await? {
            Some((
                row.get(0).map_err(|e| StoreError::Db(e.to_string()))?,
                row.get(1).map_err(|e| StoreError::Db(e.to_string()))?,
                row.get(2).ok(),
            ))
        } else {
            None
        })
    }

    /// List all `agent_sessions` rows with status = 'active'.
    /// Returns (session_id, agent_id, task_snapshot) triples.
    pub async fn list_active_sessions(
        &self,
    ) -> Result<Vec<(String, String, Option<String>)>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT id, agent_id, task_snapshot FROM agent_sessions WHERE status = 'active'",
                (),
            )
            .await?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await? {
            let sid: String = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
            let aid: String = row.get(1).map_err(|e| StoreError::Db(e.to_string()))?;
            let snap: Option<String> = row.get(2).ok();
            out.push((sid, aid, snap));
        }
        Ok(out)
    }

    /// Export training pairs for RLHF fine-tuning.
    pub async fn export_training_pairs(&self, limit: i64) -> Result<Vec<TrainingPair>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT i.prompt, i.response, f.rating, f.correction_text, f.feedback_type
             FROM llm_interactions i
             LEFT JOIN llm_feedback f ON f.interaction_id = i.rowid
             ORDER BY i.rowid DESC LIMIT ?1",
                params![limit],
            )
            .await?;

        let mut out = Vec::new();
        while let Some(row) = rows.next().await? {
            out.push(TrainingPair {
                prompt: row.get(0)?,
                response: row.get(1)?,
                rating: row.get::<Option<i64>>(2)?,
                correction: row.get::<Option<String>>(3)?,
                feedback_type: row
                    .get::<Option<String>>(4)?
                    .unwrap_or_else(|| "none".to_string()),
            });
        }
        Ok(out)
    }

    /// Load all events for a given session for replay.
    pub async fn load_session_events(
        &self,
        session_id: &str,
    ) -> Result<Vec<(String, String)>, StoreError> {
        let mut rows = self.conn.query(
            "SELECT event_type, payload_json FROM agent_session_events WHERE session_id = ?1 ORDER BY id ASC",
            params![session_id],
        ).await?;

        let mut out = Vec::new();
        while let Some(row) = rows.next().await? {
            out.push((row.get(0)?, row.get(1)?));
        }
        Ok(out)
    }

    /// Append a single event to a session's history in the DB.
    pub async fn append_session_event(
        &self,
        session_id: &str,
        event_type: &str,
        payload_json: &str,
    ) -> Result<(), StoreError> {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let session_id = session_id.to_string();
        let event_type = event_type.to_string();
        let payload_json = payload_json.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO agent_session_events (session_id, event_type, payload_json, created_at_ms)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![
                        session_id.as_str(),
                        event_type.as_str(),
                        payload_json.as_str(),
                        now_ms,
                    ],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Record a single LLM request attempt (success or failure).
    pub async fn record_llm_attempt(
        &self,
        attempt: crate::store::types::ModelAttempt<'_>,
    ) -> Result<i64, StoreError> {
        let trace_id = attempt.trace_id.to_string();
        let attempt_number = attempt.attempt_number;
        let model_id = attempt.model_id.to_string();
        let provider = attempt.provider.to_string();
        let outcome = attempt.outcome.to_string();
        let latency_ms = attempt.latency_ms;
        let error_class = attempt.error_class.map(|s: &str| s.to_string());

        let breaker = self.breaker.clone();
        let conn = self.conn.clone();

        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO llm_attempts
                         (trace_id, attempt_number, model_id, provider, outcome, latency_ms, error_class)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        trace_id.as_str(),
                        attempt_number,
                        model_id.as_str(),
                        provider.as_str(),
                        outcome.as_str(),
                        latency_ms,
                        error_class.as_deref(),
                    ],
                )
                .await?;
                Ok(conn.last_insert_rowid())
            })
            .await
    }
}
