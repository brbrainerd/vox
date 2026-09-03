//! Agent session management and LLM interaction logging for [`VoxDb`].
//!
//! Covers two V3/agents-domain table groups:
//! - **`agent_sessions`** — lifecycle tracking: create, close, query.
//! - **`llm_interactions`** + **`llm_feedback`** — RLHF data pipeline used by `vox-package/feedback.rs`.

use turso::params;

use crate::store::types::{StoreError, TrainingPair};

/// One captured operation row (subset used by sequence mining).
#[derive(Debug, Clone)]
pub struct OperationRow {
    pub ts_ms: i64,
    pub session_id: Option<String>,
    pub agent_id: Option<String>,
    pub tool_name: String,
    pub args_redacted: String,
}

impl crate::VoxDb {
    /// Most-recent `limit` captured operations, newest first. Mining regroups by session.
    pub async fn list_recent_operations(
        &self,
        limit: i64,
    ) -> Result<Vec<OperationRow>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT ts_ms, session_id, agent_id, tool_name, args_redacted
                 FROM agent_operations ORDER BY ts_ms DESC, id DESC LIMIT ?1",
                params![limit],
            )
            .await?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await? {
            out.push(OperationRow {
                ts_ms: row.get(0).map_err(|e| StoreError::Db(e.to_string()))?,
                session_id: row.get(1).ok(),
                agent_id: row.get(2).ok(),
                tool_name: row.get(3).map_err(|e| StoreError::Db(e.to_string()))?,
                args_redacted: row.get(4).map_err(|e| StoreError::Db(e.to_string()))?,
            });
        }
        Ok(out)
    }

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

    // ── Agent Operations (agent_operations) ──────────────────────────────────

    /// Record one (already-redacted) tool-call operation. Best-effort capture
    /// signal for the skill-suggestion pipeline. Returns the new row id.
    #[allow(clippy::too_many_arguments)]
    pub async fn record_operation(
        &self,
        session_id: Option<&str>,
        agent_id: Option<&str>,
        tool_name: &str,
        args_redacted: &str,
        result_redacted: Option<&str>,
        duration_ms: i64,
        is_error: bool,
    ) -> Result<i64, StoreError> {
        let ts_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        // Own everything before the `move` closure (mirrors record_agent_event).
        let session_id = session_id.map(str::to_string);
        let agent_id = agent_id.map(str::to_string);
        let tool_name = tool_name.to_string();
        let args_redacted = args_redacted.to_string();
        let result_redacted = result_redacted.map(str::to_string);
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO agent_operations
                       (ts_ms, session_id, agent_id, tool_name, args_redacted, result_redacted, duration_ms, is_error)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        ts_ms,
                        session_id.as_deref(),
                        agent_id.as_deref(),
                        tool_name.as_str(),
                        args_redacted.as_str(),
                        result_redacted.as_deref(),
                        duration_ms,
                        is_error as i64,
                    ],
                )
                .await?;
                Ok::<i64, StoreError>(conn.last_insert_rowid())
            })
            .await
    }

    /// Bound `agent_operations` growth: drop rows older than 30 days, then trim to
    /// the newest 50k. Cheap; called opportunistically after writes.
    pub async fn prune_operations(&self) -> Result<(), StoreError> {
        let cutoff_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0)
            - 30 * 24 * 60 * 60 * 1000;
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "DELETE FROM agent_operations WHERE ts_ms < ?1",
                    params![cutoff_ms],
                )
                .await?;
                conn.execute(
                    "DELETE FROM agent_operations WHERE id NOT IN
                       (SELECT id FROM agent_operations ORDER BY id DESC LIMIT 50000)",
                    (),
                )
                .await?;
                Ok::<(), StoreError>(())
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
        let ttft_ms = outcome.ttft_ms;
        let tpot_ms = outcome.tpot_ms;

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
                         (session_id, user_id, tenant_id, prompt, response, model_version, task_category, strength_tag, trace_id, context_utilization_pct, cache_read_tokens, success, latency_ms, input_tokens, output_tokens, cost_usd, ttft_ms, tpot_ms)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
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
                        cost_usd,
                        ttft_ms,
                        tpot_ms
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

    /// Aggregate LLM spend (USD) from recorded `llm_interactions.cost_usd` — the single
    /// recorded cost source. Returns total, today, and (optionally) the given session.
    /// This is the SSOT for "actual spend" surfaced to the GUI / cost displays.
    pub async fn llm_spend_summary(
        &self,
        session_id: Option<&str>,
    ) -> Result<LlmSpendSummary, StoreError> {
        let session = session_id.unwrap_or("");
        let mut rows = self
            .conn
            .query(
                "SELECT
                    COALESCE(SUM(cost_usd), 0.0)                                            AS total,
                    COALESCE(SUM(cost_usd) FILTER (WHERE created_at >= date('now')), 0.0)   AS day,
                    COALESCE(SUM(cost_usd) FILTER (WHERE session_id = ?1), 0.0)             AS session
                 FROM llm_interactions",
                params![session],
            )
            .await?;
        if let Some(r) = rows.next().await? {
            Ok(LlmSpendSummary {
                total_usd: r.get::<f64>(0).unwrap_or(0.0),
                day_usd: r.get::<f64>(1).unwrap_or(0.0),
                session_usd: r.get::<f64>(2).unwrap_or(0.0),
            })
        } else {
            Ok(LlmSpendSummary::default())
        }
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

    /// Test-only: like [`Self::record_llm_attempt`], but lets the caller backdate
    /// `created_at` via a SQLite modifier string (e.g. `"-1 hour"`), to exercise
    /// staleness-window logic in downstream crates that can't reach vox-db's raw
    /// Turso connection directly (see `crates/vox-cli`'s doctor rate-limit test).
    /// Gated behind the `test-support` feature so it never ships as part of
    /// vox-db's normal public API surface.
    #[cfg(feature = "test-support")]
    pub async fn record_llm_attempt_with_created_at_offset(
        &self,
        attempt: crate::store::types::ModelAttempt<'_>,
        created_at_modifier: &str,
    ) -> Result<i64, StoreError> {
        let trace_id = attempt.trace_id.to_string();
        let attempt_number = attempt.attempt_number;
        let model_id = attempt.model_id.to_string();
        let provider = attempt.provider.to_string();
        let outcome = attempt.outcome.to_string();
        let latency_ms = attempt.latency_ms;
        let error_class = attempt.error_class.map(|s: &str| s.to_string());
        let created_at_modifier = created_at_modifier.to_string();

        let breaker = self.breaker.clone();
        let conn = self.conn.clone();

        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO llm_attempts
                         (trace_id, attempt_number, model_id, provider, outcome, latency_ms, error_class, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now', ?8))",
                    params![
                        trace_id.as_str(),
                        attempt_number,
                        model_id.as_str(),
                        provider.as_str(),
                        outcome.as_str(),
                        latency_ms,
                        error_class.as_deref(),
                        created_at_modifier.as_str(),
                    ],
                )
                .await?;
                Ok(conn.last_insert_rowid())
            })
            .await
    }

    /// The most recently recorded `llm_attempts` row (across all providers/models),
    /// with its age in seconds computed in SQL. `None` when no attempt has ever been
    /// recorded. Read-only counterpart to [`Self::record_llm_attempt`] — added for
    /// `vox doctor`'s LLM routing check, which needs to distinguish "no credential
    /// configured" from "a credential is configured but the most recent real dispatch
    /// was rate-limited," without doctor itself making a live provider call (see
    /// `crates/vox-cli/.../doctor/checks_standard/llm_routing.rs`'s doc comment on
    /// `rate_limit_check` for why doctor avoids live network I/O).
    ///
    /// Callers doing staleness filtering should treat `age_seconds` as authoritative
    /// rather than re-deriving it from `created_at` — SQLite's `datetime('now')` and
    /// `julianday()` are computed inside the same query, avoiding clock-skew or
    /// timezone-parsing mismatches between the DB and the caller's process.
    pub async fn get_last_llm_attempt(
        &self,
    ) -> Result<Option<crate::store::types::LastLlmAttemptRow>, StoreError> {
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();

        breaker
            .call(move || {
                let conn = conn.clone();
                async move {
                    let mut rows = conn
                        .query(
                            "SELECT provider, model_id, outcome, error_class,
                                    (julianday('now') - julianday(created_at)) * 86400.0 AS age_seconds
                             FROM llm_attempts
                             ORDER BY created_at DESC, id DESC
                             LIMIT 1",
                            (),
                        )
                        .await?;

                    if let Some(row) = rows.next().await? {
                        Ok::<_, StoreError>(Some(crate::store::types::LastLlmAttemptRow {
                            provider: row.get(0)?,
                            model_id: row.get(1)?,
                            outcome: row.get(2)?,
                            error_class: row.get(3)?,
                            age_seconds: row.get(4)?,
                        }))
                    } else {
                        Ok::<_, StoreError>(None)
                    }
                }
            })
            .await
    }
}

/// Aggregated LLM spend (USD), the SSOT for actual-cost displays. Budgets (caps) are
/// read separately from `VoxConfig`; this is the recorded actuals from `llm_interactions`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct LlmSpendSummary {
    /// Total recorded spend across all sessions.
    pub total_usd: f64,
    /// Spend recorded today (since local midnight).
    pub day_usd: f64,
    /// Spend recorded for the queried session (0 when no session was given).
    pub session_usd: f64,
}

#[cfg(test)]
mod operation_tests {
    use crate::{DbConfig, VoxDb};

    #[tokio::test]
    async fn record_and_prune_operations_roundtrip() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");

        let id = db
            .record_operation(
                Some("sess-1"),
                None, // agent_id NULL
                "vox_skill_list",
                r#"{"q":"[REDACTED]"}"#,
                Some("ok"),
                12,
                false,
            )
            .await
            .expect("record");
        assert!(id > 0);

        // prune must not error on a small table and must keep the fresh row.
        db.prune_operations().await.expect("prune");
    }

    #[tokio::test]
    async fn list_recent_operations_orders_and_limits() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
        for (i, tool) in ["a", "b", "c"].iter().enumerate() {
            db.record_operation(Some("s1"), None, tool, "{}", Some("ok"), i as i64, false)
                .await
                .expect("record");
        }
        let rows = db.list_recent_operations(2).await.expect("list");
        assert_eq!(rows.len(), 2, "respects limit");
        assert!(rows.iter().all(|r| r.session_id.as_deref() == Some("s1")));
        assert!(
            rows.iter().any(|r| r.tool_name == "c"),
            "includes most recent"
        );
    }
}

#[cfg(test)]
mod spend_tests {
    use crate::store::types::ModelOutcome;
    use crate::{DbConfig, VoxDb};

    fn outcome<'a>(session: &'a str, cost: f64) -> ModelOutcome<'a> {
        ModelOutcome {
            session_id: session,
            user_id: None,
            tenant_id: None,
            prompt: "p",
            response: "r",
            model_id: "m",
            provider: "openrouter",
            task_category: "general",
            strength_tag: "generalist",
            latency_ms: Some(10),
            input_tokens: Some(5),
            output_tokens: Some(5),
            cache_read_tokens: Some(0),
            trace_id: None,
            context_utilization_pct: None,
            success: true,
            cost_usd: Some(cost),
            quality_score: Some(1.0),
            ttft_ms: None,
            tpot_ms: None,
        }
    }

    #[tokio::test]
    async fn llm_spend_summary_sums_recorded_costs() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
        db.record_llm_outcome(outcome("sess-1", 0.01))
            .await
            .expect("rec 1");
        db.record_llm_outcome(outcome("sess-1", 0.02))
            .await
            .expect("rec 2");
        db.record_llm_outcome(outcome("sess-2", 0.04))
            .await
            .expect("rec 3");

        let all = db.llm_spend_summary(None).await.expect("summary");
        assert!(
            (all.total_usd - 0.07).abs() < 1e-9,
            "total: {}",
            all.total_usd
        );
        assert!((all.day_usd - 0.07).abs() < 1e-9, "day: {}", all.day_usd);
        assert_eq!(all.session_usd, 0.0, "no session given");

        let s1 = db.llm_spend_summary(Some("sess-1")).await.expect("summary");
        assert!(
            (s1.session_usd - 0.03).abs() < 1e-9,
            "session: {}",
            s1.session_usd
        );
        assert!((s1.total_usd - 0.07).abs() < 1e-9, "total still all");
    }
}
