use turso::params;
use vox_db_types::{DbPlanSessionId, DbTaskId};

use crate::store::types::{PlanNodeAttemptRow, PlanNodeRow, PlanSessionRow, StoreError};

impl crate::VoxDb {
    pub async fn create_plan_session(
        &self,
        plan_session_id: &str,
        origin_session_id: Option<&str>,
        goal_text: &str,
        strategy: &str,
    ) -> Result<(), StoreError> {
        let plan_session_id = plan_session_id.to_string();
        let origin_session_id = origin_session_id.map(str::to_string);
        let goal_text = goal_text.to_string();
        let strategy = strategy.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO plan_sessions (
                    plan_session_id, origin_session_id, goal_text, strategy, current_version, status
                ) VALUES (?1, ?2, ?3, ?4, 1, 'pending')
                ON CONFLICT(plan_session_id) DO UPDATE SET
                    origin_session_id = excluded.origin_session_id,
                    goal_text = excluded.goal_text,
                    strategy = excluded.strategy,
                    updated_at = datetime('now')",
                    params![
                        plan_session_id.as_str(),
                        origin_session_id.as_deref(),
                        goal_text.as_str(),
                        strategy.as_str(),
                    ],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    pub async fn update_plan_session_goal_text(
        &self,
        plan_session_id: &str,
        goal_text: &str,
    ) -> Result<(), StoreError> {
        let plan_session_id = plan_session_id.to_string();
        let goal_text = goal_text.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "UPDATE plan_sessions SET goal_text = ?2, updated_at = datetime('now') WHERE plan_session_id = ?1",
                    params![plan_session_id.as_str(), goal_text.as_str()],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    pub async fn append_plan_version(
        &self,
        plan_session_id: &str,
        version: i64,
        parent_version: Option<i64>,
        trigger_event: Option<&str>,
        trigger_payload_json: Option<&str>,
    ) -> Result<(), StoreError> {
        let plan_session_id = plan_session_id.to_string();
        let trigger_event = trigger_event.map(str::to_string);
        let trigger_payload_json = trigger_payload_json.map(str::to_string);
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT OR REPLACE INTO plan_versions (
                    plan_session_id, version, parent_version, trigger_event, trigger_payload_json, quality_score, reviewer_verdict
                ) VALUES (?1, ?2, ?3, ?4, ?5, 0.0, 'pending')",
                    params![
                        plan_session_id.as_str(),
                        version,
                        parent_version,
                        trigger_event.as_deref(),
                        trigger_payload_json.as_deref(),
                    ],
                )
                .await?;
                conn.execute(
                    "UPDATE plan_sessions SET current_version = ?2, updated_at = datetime('now') WHERE plan_session_id = ?1",
                    params![plan_session_id.as_str(), version],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    pub async fn update_plan_version_quality(
        &self,
        plan_session_id: &str,
        version: i64,
        quality_score: f64,
        reviewer_verdict: &str,
    ) -> Result<(), StoreError> {
        let plan_session_id = plan_session_id.to_string();
        let reviewer_verdict = reviewer_verdict.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "UPDATE plan_versions SET quality_score = ?1, reviewer_verdict = ?2 
                     WHERE plan_session_id = ?3 AND version = ?4",
                    params![
                        quality_score,
                        reviewer_verdict.as_str(),
                        plan_session_id.as_str(),
                        version,
                    ],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn upsert_plan_node(
        &self,
        plan_session_id: &str,
        version: i64,
        node_id: &str,
        description: &str,
        dependencies_json: &str,
        execution_policy_json: &str,
        status: &str,
        workflow_invocation: Option<&str>,
    ) -> Result<(), StoreError> {
        let plan_session_id = plan_session_id.to_string();
        let node_id = node_id.to_string();
        let description = description.to_string();
        let dependencies_json = dependencies_json.to_string();
        let execution_policy_json = execution_policy_json.to_string();
        let status = status.to_string();
        let workflow_invocation = workflow_invocation.map(str::to_string);
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO plan_nodes (
                    plan_session_id, version, node_id, description, dependencies_json, execution_policy_json, status, workflow_invocation
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                ON CONFLICT(plan_session_id, version, node_id) DO UPDATE SET
                    description = excluded.description,
                    dependencies_json = excluded.dependencies_json,
                    execution_policy_json = excluded.execution_policy_json,
                    status = excluded.status,
                    workflow_invocation = excluded.workflow_invocation,
                    updated_at = datetime('now')",
                    params![
                        plan_session_id.as_str(),
                        version,
                        node_id.as_str(),
                        description.as_str(),
                        dependencies_json.as_str(),
                        execution_policy_json.as_str(),
                        status.as_str(),
                        workflow_invocation.as_deref(),
                    ],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    pub async fn record_plan_node_attempt(
        &self,
        plan_session_id: &str,
        version: i64,
        node_id: &str,
        attempt_no: i64,
        task_id: Option<&str>,
        outcome: &str,
        error_text: Option<&str>,
        latency_ms: Option<i64>,
    ) -> Result<(), StoreError> {
        let plan_session_id = plan_session_id.to_string();
        let node_id = node_id.to_string();
        let task_id = task_id.map(str::to_string);
        let outcome = outcome.to_string();
        let error_text = error_text.map(str::to_string);
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO plan_node_attempts (
                    plan_session_id, version, node_id, attempt_no, task_id, outcome, error_text, latency_ms
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        plan_session_id.as_str(),
                        version,
                        node_id.as_str(),
                        attempt_no,
                        task_id.as_deref(),
                        outcome.as_str(),
                        error_text.as_deref(),
                        latency_ms,
                    ],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    pub async fn set_plan_head(
        &self,
        plan_session_id: &str,
        version: i64,
        status: Option<&str>,
    ) -> Result<(), StoreError> {
        let plan_session_id = plan_session_id.to_string();
        let status = status.map(str::to_string);
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "UPDATE plan_sessions
                 SET current_version = ?2,
                     status = COALESCE(?3, status),
                     updated_at = datetime('now')
                 WHERE plan_session_id = ?1",
                    params![plan_session_id.as_str(), version, status.as_deref(),],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    pub async fn load_plan_head(&self, plan_session_id: &str) -> Result<Option<i64>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT current_version FROM plan_sessions WHERE plan_session_id = ?1",
                params![plan_session_id],
            )
            .await?;
        if let Some(r) = rows.next().await? {
            return Ok(Some(r.get::<i64>(0)?));
        }
        Ok(None)
    }

    pub async fn list_runnable_nodes(
        &self,
        plan_session_id: &str,
        version: i64,
    ) -> Result<Vec<(String, String)>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT node_id, description
                 FROM plan_nodes
                 WHERE plan_session_id = ?1
                   AND version = ?2
                   AND status IN ('pending','queued')",
                params![plan_session_id, version],
            )
            .await?;
        let mut out = Vec::new();
        while let Some(r) = rows.next().await? {
            out.push((r.get::<String>(0)?, r.get::<String>(1)?));
        }
        Ok(out)
    }

    /// Full node rows for DAG scheduling (`plan_nodes.status` drives readiness).
    pub async fn load_plan_nodes_with_status(
        &self,
        plan_session_id: &str,
        version: i64,
    ) -> Result<Vec<PlanNodeRow>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT node_id, description, dependencies_json, execution_policy_json, status, workflow_invocation
                 FROM plan_nodes
                 WHERE plan_session_id = ?1 AND version = ?2
                 ORDER BY node_id",
                params![plan_session_id, version],
            )
            .await?;
        let mut out = Vec::new();
        while let Some(r) = rows.next().await? {
            out.push(PlanNodeRow {
                plan_session_id: DbPlanSessionId::new(plan_session_id),
                version,
                node_id: r.get::<String>(0)?,
                description: r.get::<String>(1)?,
                dependencies_json: r.get::<String>(2)?,
                execution_policy_json: r.get::<String>(3)?,
                status: r.get::<String>(4)?,
                workflow_invocation: r.get::<Option<String>>(5)?,
            });
        }
        Ok(out)
    }

    pub async fn set_plan_node_status(
        &self,
        plan_session_id: &str,
        version: i64,
        node_id: &str,
        status: &str,
    ) -> Result<(), StoreError> {
        let plan_session_id = plan_session_id.to_string();
        let node_id = node_id.to_string();
        let status = status.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "UPDATE plan_nodes SET status = ?4, updated_at = datetime('now')
                 WHERE plan_session_id = ?1 AND version = ?2 AND node_id = ?3",
                    params![
                        plan_session_id.as_str(),
                        version,
                        node_id.as_str(),
                        status.as_str(),
                    ],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    pub async fn approve_all_blocked_plan_nodes(
        &self,
        plan_session_id: &str,
    ) -> Result<u64, StoreError> {
        let plan_session_id = plan_session_id.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                let affected = conn
                    .execute(
                        "UPDATE plan_nodes SET status = 'pending', updated_at = datetime('now')
                 WHERE plan_session_id = ?1 AND status = 'blocked_on_approval'",
                        params![plan_session_id.as_str()],
                    )
                    .await?;
                Ok::<u64, StoreError>(affected)
            })
            .await
    }

    pub async fn update_plan_session_iterative_fields(
        &self,
        plan_session_id: &str,
        question_session_id: Option<&str>,
        loop_round: i64,
        stop_reason: Option<&str>,
        metadata_json: Option<&str>,
    ) -> Result<(), StoreError> {
        let plan_session_id = plan_session_id.to_string();
        let question_session_id = question_session_id.map(str::to_string);
        let stop_reason = stop_reason.map(str::to_string);
        let metadata_json = metadata_json.map(str::to_string);
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "UPDATE plan_sessions SET
                    question_session_id = COALESCE(?2, question_session_id),
                    iterative_loop_round = ?3,
                    iterative_stop_reason = COALESCE(?4, iterative_stop_reason),
                    iterative_loop_metadata_json = COALESCE(?5, iterative_loop_metadata_json),
                    updated_at = datetime('now')
                 WHERE plan_session_id = ?1",
                    params![
                        plan_session_id.as_str(),
                        question_session_id.as_deref(),
                        loop_round,
                        stop_reason.as_deref(),
                        metadata_json.as_deref(),
                    ],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    pub async fn record_test_decision(
        &self,
        task_id: &str,
        decision: &str,
        rationale: &str,
        recorded_at_ms: i64,
    ) -> Result<(), StoreError> {
        let task_id = task_id.to_string();
        let decision = decision.to_string();
        let rationale = rationale.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO plan_test_decisions (
                    task_id, decision, rationale, recorded_at_ms
                ) VALUES (?1, ?2, ?3, ?4)
                ON CONFLICT(task_id) DO UPDATE SET
                    decision = excluded.decision,
                    rationale = excluded.rationale,
                    recorded_at_ms = excluded.recorded_at_ms",
                    params![
                        task_id.as_str(),
                        decision.as_str(),
                        rationale.as_str(),
                        recorded_at_ms
                    ],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    pub async fn load_test_decision(
        &self,
        task_id: &str,
    ) -> Result<Option<(String, String)>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT decision, rationale FROM plan_test_decisions WHERE task_id = ?1",
                params![task_id],
            )
            .await?;
        if let Some(r) = rows.next().await? {
            return Ok(Some((r.get::<String>(0)?, r.get::<String>(1)?)));
        }
        Ok(None)
    }

    /// Load one plan session row by primary key.
    pub async fn get_plan_session_by_id(
        &self,
        plan_session_id: &str,
    ) -> Result<Option<PlanSessionRow>, StoreError> {
        let plan_session_id = plan_session_id.to_string();
        let mut rows = self
            .conn
            .query(
                "SELECT plan_session_id, origin_session_id, goal_text, strategy, current_version, status 
                 FROM plan_sessions WHERE plan_session_id = ?1",
                params![plan_session_id.as_str()],
            )
            .await?;
        if let Some(r) = rows.next().await? {
            return Ok(Some(PlanSessionRow {
                plan_session_id: DbPlanSessionId::new(r.get::<String>(0)?),
                origin_session_id: r.get::<Option<String>>(1)?,
                goal_text: r.get::<String>(2)?,
                strategy: r.get::<String>(3)?,
                current_version: r.get::<i64>(4)?,
                status: r.get::<String>(5)?,
            }));
        }
        Ok(None)
    }

    pub async fn list_plan_sessions(
        &self,
        limit: i64,
        status_filter: Option<&str>,
    ) -> Result<Vec<PlanSessionRow>, StoreError> {
        let limit = limit.clamp(1, 200);
        let mut rows = if let Some(status) = status_filter {
            self.conn
                .query(
                    "SELECT plan_session_id, origin_session_id, goal_text, strategy, current_version, status 
                     FROM plan_sessions WHERE status = ?1 ORDER BY updated_at DESC LIMIT ?2",
                    params![status, limit],
                )
                .await?
        } else {
            self.conn
                .query(
                    "SELECT plan_session_id, origin_session_id, goal_text, strategy, current_version, status 
                     FROM plan_sessions ORDER BY updated_at DESC LIMIT ?1",
                    params![limit],
                )
                .await?
        };

        let mut out = Vec::new();
        while let Some(r) = rows.next().await? {
            out.push(PlanSessionRow {
                plan_session_id: DbPlanSessionId::new(r.get::<String>(0)?),
                origin_session_id: r.get::<Option<String>>(1)?,
                goal_text: r.get::<String>(2)?,
                strategy: r.get::<String>(3)?,
                current_version: r.get::<i64>(4)?,
                status: r.get::<String>(5)?,
            });
        }
        Ok(out)
    }

    pub async fn list_plan_node_attempts(
        &self,
        plan_session_id: &str,
        node_id: &str,
    ) -> Result<Vec<PlanNodeAttemptRow>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT plan_session_id, version, node_id, attempt_no, task_id, outcome, error_text, latency_ms, created_at
                 FROM plan_node_attempts WHERE plan_session_id = ?1 AND node_id = ?2
                 ORDER BY attempt_no ASC",
                params![plan_session_id, node_id],
            )
            .await?;
        let mut out = Vec::new();
        while let Some(r) = rows.next().await? {
            out.push(PlanNodeAttemptRow {
                plan_session_id: DbPlanSessionId::new(r.get::<String>(0)?),
                version: r.get::<i64>(1)?,
                node_id: r.get::<String>(2)?,
                attempt_no: r.get::<i64>(3)?,
                task_id: r.get::<Option<String>>(4)?.map(DbTaskId::new),
                outcome: r.get::<String>(5)?,
                error_text: r.get::<Option<String>>(6)?,
                latency_ms: r.get::<Option<i64>>(7)?,
                created_at: r.get::<String>(8)?,
            });
        }
        Ok(out)
    }
}
