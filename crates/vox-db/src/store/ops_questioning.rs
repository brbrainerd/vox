//! Information-theoretic questioning telemetry persistence for [`crate::VoxDb`].

use std::collections::HashMap;

use turso::params;
use vox_db_types::{DbSessionId, DbTaskId};

use crate::store::types::{
    A2aClarificationMessageParams, QuestionEventParams, QuestionEventRow,
    QuestionOptionOutcomeParams, QuestionOptionOutcomeRow, QuestionOptionParams, QuestionOptionRow,
    QuestionSessionCreateParams, QuestionSessionRow, QuestionStopEventParams, QuestionStopEventRow,
    StoreError,
};

impl crate::VoxDb {
    /// Insert one `question_sessions` row and return its id.
    pub async fn create_question_session(
        &self,
        p: QuestionSessionCreateParams<'_>,
    ) -> Result<i64, StoreError> {
        let session_id = p.session_id.to_string();
        let repository_id = p.repository_id.to_string();
        let task_id = p.task_id.map(str::to_string);
        let policy_version = p.policy_version.to_string();
        let started_at_ms = p.started_at_ms;
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO question_sessions
                 (session_id, repository_id, task_id, policy_version, started_at_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        session_id.as_str(),
                        repository_id.as_str(),
                        task_id.as_deref(),
                        policy_version.as_str(),
                        started_at_ms
                    ],
                )
                .await?;
                Ok::<_, StoreError>(conn.last_insert_rowid())
            })
            .await
    }

    /// Mark a `question_sessions` row as closed.
    pub async fn close_question_session(
        &self,
        question_session_id: i64,
        resolution_status: &str,
        ended_at_ms: i64,
    ) -> Result<(), StoreError> {
        let resolution_status = resolution_status.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "UPDATE question_sessions
                 SET ended_at_ms = ?2, resolution_status = ?3
                 WHERE id = ?1",
                    params![question_session_id, ended_at_ms, resolution_status.as_str()],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Insert one `question_events` row and return its id.
    pub async fn insert_question_event(
        &self,
        p: QuestionEventParams<'_>,
    ) -> Result<i64, StoreError> {
        let question_id = p.question_id.to_string();
        let actor = p.actor.to_string();
        let question_kind = p.question_kind.to_string();
        let prompt = p.prompt.to_string();
        let answer_text = p.answer_text.map(str::to_string);
        let answer_type = p.answer_type.map(str::to_string);
        let question_session_id = p.question_session_id;
        let turn_index = p.turn_index;
        let expected_information_gain_bits = p.expected_information_gain_bits;
        let expected_user_cost = p.expected_user_cost;
        let utility_bits_per_cost = p.utility_bits_per_cost;
        let answered_at_ms = p.answered_at_ms;
        let created_at_ms = p.created_at_ms;
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO question_events
                 (question_session_id, question_id, turn_index, actor, question_kind, prompt,
                  expected_information_gain_bits, expected_user_cost, utility_bits_per_cost,
                  answer_text, answer_type, answered_at_ms, created_at_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                    params![
                        question_session_id,
                        question_id.as_str(),
                        turn_index,
                        actor.as_str(),
                        question_kind.as_str(),
                        prompt.as_str(),
                        expected_information_gain_bits,
                        expected_user_cost,
                        utility_bits_per_cost,
                        answer_text.as_deref(),
                        answer_type.as_deref(),
                        answered_at_ms,
                        created_at_ms
                    ],
                )
                .await?;
                Ok::<_, StoreError>(conn.last_insert_rowid())
            })
            .await
    }

    /// Upsert one option row tied to a question event.
    pub async fn upsert_question_option(
        &self,
        p: QuestionOptionParams<'_>,
    ) -> Result<(), StoreError> {
        let option_id = p.option_id.to_string();
        let label = p.label.to_string();
        let question_event_id = p.question_event_id;
        let prior_probability = p.prior_probability;
        let posterior_probability = p.posterior_probability;
        let is_other_flag = if p.is_other { 1i64 } else { 0i64 };
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO question_options
                 (question_event_id, option_id, label, prior_probability, posterior_probability, is_other)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(question_event_id, option_id) DO UPDATE SET
                    label = excluded.label,
                    prior_probability = excluded.prior_probability,
                    posterior_probability = excluded.posterior_probability,
                    is_other = excluded.is_other",
                    params![
                        question_event_id,
                        option_id.as_str(),
                        label.as_str(),
                        prior_probability,
                        posterior_probability,
                        is_other_flag
                    ],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Insert one option outcome row.
    pub async fn insert_question_option_outcome(
        &self,
        p: QuestionOptionOutcomeParams<'_>,
    ) -> Result<i64, StoreError> {
        let option_id = p.option_id.to_string();
        let question_event_id = p.question_event_id;
        let selected = if p.selected { 1i64 } else { 0i64 };
        let diagnostic_weight = p.diagnostic_weight;
        let information_contribution_bits = p.information_contribution_bits;
        let created_at_ms = p.created_at_ms;
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO question_option_outcomes
                 (question_event_id, option_id, selected, diagnostic_weight, information_contribution_bits, created_at_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        question_event_id,
                        option_id.as_str(),
                        selected,
                        diagnostic_weight,
                        information_contribution_bits,
                        created_at_ms
                    ],
                )
                .await?;
                Ok::<_, StoreError>(conn.last_insert_rowid())
            })
            .await
    }

    /// Insert one stop event for a question session.
    pub async fn insert_question_stop_event(
        &self,
        p: QuestionStopEventParams<'_>,
    ) -> Result<i64, StoreError> {
        let stop_reason = p.stop_reason.to_string();
        let question_session_id = p.question_session_id;
        let confidence_at_stop = p.confidence_at_stop;
        let marginal_gain_bits = p.marginal_gain_bits;
        let expected_user_cost = p.expected_user_cost;
        let turn_index = p.turn_index;
        let created_at_ms = p.created_at_ms;
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO question_stop_events
                 (question_session_id, stop_reason, confidence_at_stop, marginal_gain_bits,
                  expected_user_cost, turn_index, created_at_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        question_session_id,
                        stop_reason.as_str(),
                        confidence_at_stop,
                        marginal_gain_bits,
                        expected_user_cost,
                        turn_index,
                        created_at_ms
                    ],
                )
                .await?;
                Ok::<_, StoreError>(conn.last_insert_rowid())
            })
            .await
    }

    /// Open `question_sessions` row for this MCP logical session + repo (`ended_at_ms` unset).
    pub async fn find_open_question_session_for_repo(
        &self,
        session_id: &str,
        repository_id: &str,
    ) -> Result<Option<QuestionSessionRow>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT id, session_id, repository_id, task_id, policy_version,
                        started_at_ms, ended_at_ms, resolution_status, belief_state_json
                 FROM question_sessions
                 WHERE session_id = ?1 AND repository_id = ?2 AND ended_at_ms IS NULL
                 ORDER BY started_at_ms DESC
                 LIMIT 1",
                params![session_id, repository_id],
            )
            .await?;
        let Some(row) = rows.next().await? else {
            return Ok(None);
        };
        Ok(Some(QuestionSessionRow {
            id: row.get(0).map_err(|e| StoreError::Db(e.to_string()))?,
            session_id: DbSessionId::new(
                row.get::<String>(1)
                    .map_err(|e| StoreError::Db(e.to_string()))?,
            ),
            repository_id: row.get(2).map_err(|e| StoreError::Db(e.to_string()))?,
            task_id: row
                .get::<Option<String>>(3)
                .map_err(|e| StoreError::Db(e.to_string()))?
                .map(DbTaskId::new),
            policy_version: row.get(4).map_err(|e| StoreError::Db(e.to_string()))?,
            started_at_ms: row.get(5).map_err(|e| StoreError::Db(e.to_string()))?,
            ended_at_ms: row.get(6).map_err(|e| StoreError::Db(e.to_string()))?,
            resolution_status: row.get(7).map_err(|e| StoreError::Db(e.to_string()))?,
            belief_state_json: row.get(8).map_err(|e| StoreError::Db(e.to_string()))?,
        }))
    }

    /// Next `turn_index` for `question_events` under this session (0-based contiguous).
    pub async fn next_question_event_turn_index(
        &self,
        question_session_id: i64,
    ) -> Result<i32, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT COALESCE(MAX(turn_index), -1) + 1 FROM question_events
                 WHERE question_session_id = ?1",
                params![question_session_id],
            )
            .await?;
        let row = rows
            .next()
            .await?
            .ok_or_else(|| StoreError::Db("expected aggregate row".to_string()))?;
        let v: i32 = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
        Ok(v)
    }

    /// Count assistant-asked questions in the current open session (for Socrates `clarification_turn_index`).
    pub async fn count_assistant_questions_in_open_session(
        &self,
        session_id: &str,
        repository_id: &str,
    ) -> Result<u32, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT COUNT(*) FROM question_events qe
                 INNER JOIN question_sessions qs ON qs.id = qe.question_session_id
                 WHERE qs.session_id = ?1 AND qs.repository_id = ?2
                   AND qs.ended_at_ms IS NULL AND qe.actor = 'assistant'",
                params![session_id, repository_id],
            )
            .await?;
        let row = rows
            .next()
            .await?
            .ok_or_else(|| StoreError::Db("expected count row".to_string()))?;
        let c: i64 = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
        Ok(c.max(0) as u32)
    }

    /// `true` when an assistant question in the open session has no `answer_text` yet.
    pub async fn has_pending_clarification_for_mcp_session(
        &self,
        session_id: &str,
        repository_id: &str,
    ) -> Result<bool, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT 1 FROM question_events qe
                 INNER JOIN question_sessions qs ON qs.id = qe.question_session_id
                 WHERE qs.session_id = ?1 AND qs.repository_id = ?2
                   AND qs.ended_at_ms IS NULL
                   AND qe.actor = 'assistant'
                   AND qe.answer_text IS NULL
                 LIMIT 1",
                params![session_id, repository_id],
            )
            .await?;
        Ok(rows.next().await?.is_some())
    }

    /// Count unanswered assistant clarification prompts for this MCP session + repository.
    pub async fn count_pending_clarifications_for_mcp_session(
        &self,
        session_id: &str,
        repository_id: &str,
    ) -> Result<u32, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT COUNT(*) FROM question_events qe
                 INNER JOIN question_sessions qs ON qs.id = qe.question_session_id
                 WHERE qs.session_id = ?1 AND qs.repository_id = ?2
                   AND qs.ended_at_ms IS NULL
                   AND qe.actor = 'assistant'
                   AND qe.answer_text IS NULL",
                params![session_id, repository_id],
            )
            .await?;
        let row = rows
            .next()
            .await?
            .ok_or_else(|| StoreError::Db("expected count row".to_string()))?;
        let c: i64 = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
        Ok(c.max(0) as u32)
    }

    async fn resolve_question_event_id_for_answer(
        &self,
        question_session_id: i64,
        question_id: Option<&str>,
    ) -> Result<Option<i64>, StoreError> {
        let mut rows = if let Some(qid) = question_id {
            self.conn
                .query(
                    "SELECT id FROM question_events
                     WHERE question_session_id = ?1 AND question_id = ?2
                     ORDER BY id DESC LIMIT 1",
                    params![question_session_id, qid],
                )
                .await?
        } else {
            self.conn
                .query(
                    "SELECT id FROM question_events
                     WHERE question_session_id = ?1 AND actor = 'assistant' AND answer_text IS NULL
                     ORDER BY id DESC LIMIT 1",
                    params![question_session_id],
                )
                .await?
        };
        let Some(row) = rows.next().await? else {
            return Ok(None);
        };
        let id: i64 = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
        Ok(Some(id))
    }

    async fn latest_question_event_id_and_eig(
        &self,
        question_session_id: i64,
        question_id: &str,
    ) -> Result<Option<(i64, f64)>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT id, expected_information_gain_bits FROM question_events
                 WHERE question_session_id = ?1 AND question_id = ?2
                 ORDER BY id DESC LIMIT 1",
                params![question_session_id, question_id],
            )
            .await?;
        let Some(row) = rows.next().await? else {
            return Ok(None);
        };
        let id: i64 = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
        let eig: f64 = row.get(1).map_err(|e| StoreError::Db(e.to_string()))?;
        Ok(Some((id, eig)))
    }

    /// Fill in `answer_*` columns for one question row.
    pub async fn update_question_event_answer(
        &self,
        question_event_id: i64,
        answer_text: &str,
        answer_type: &str,
        answered_at_ms: i64,
    ) -> Result<(), StoreError> {
        let answer_text = answer_text.to_string();
        let answer_type = answer_type.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                let n = conn
                    .execute(
                        "UPDATE question_events
             SET answer_text = ?2, answer_type = ?3, answered_at_ms = ?4
             WHERE id = ?1",
                        params![
                            question_event_id,
                            answer_text.as_str(),
                            answer_type.as_str(),
                            answered_at_ms
                        ],
                    )
                    .await?;
                if n == 0 {
                    return Err(StoreError::Db("no question_event row updated".into()));
                }
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Persist a user reply to a pending clarification; optional MC outcome row.
    pub async fn record_questioning_user_answer(
        &self,
        question_session_id: i64,
        question_id: Option<&str>,
        answer_text: &str,
        answer_type: &str,
        selected_option_id: Option<&str>,
        information_contribution_bits: f64,
        answered_at_ms: i64,
    ) -> Result<String, StoreError> {
        let Some(qeid) = self
            .resolve_question_event_id_for_answer(question_session_id, question_id)
            .await?
        else {
            return Err(StoreError::Db(
                "no unanswered question_event for this session/question_id".into(),
            ));
        };
        let resolved_qid: String = {
            let mut rows = self
                .conn
                .query(
                    "SELECT question_id FROM question_events WHERE id = ?1",
                    params![qeid],
                )
                .await?;
            let Some(row) = rows.next().await? else {
                return Err(StoreError::Db(
                    "question_event row missing after resolve".into(),
                ));
            };
            row.get(0).map_err(|e| StoreError::Db(e.to_string()))?
        };
        self.update_question_event_answer(qeid, answer_text, answer_type, answered_at_ms)
            .await?;
        if let Some(oid) = selected_option_id {
            self.insert_question_option_outcome(QuestionOptionOutcomeParams {
                question_event_id: qeid,
                option_id: oid,
                selected: true,
                diagnostic_weight: 1.0,
                information_contribution_bits,
                created_at_ms: answered_at_ms,
            })
            .await?;
        }
        Ok(resolved_qid)
    }

    /// Append-only merge into `belief_state_json` for posterior bookkeeping.
    ///
    /// For multiple-choice answers, pass `selected_option_id` to update `hypothesis_mass.by_question`
    /// and row `posterior_probability` values (proportional rescaling from priors with likelihood
    /// `1 + min(EIG bits, 8)` on the selected option).
    pub async fn merge_question_session_belief_answer(
        &self,
        question_session_id: i64,
        question_id: &str,
        answer_text: &str,
        answered_at_ms: i64,
        selected_option_id: Option<&str>,
    ) -> Result<(), StoreError> {
        let mut pending_option_updates: Vec<(i64, String, f64)> = Vec::new();
        let prev: Option<String> = {
            let mut rows = self
                .conn
                .query(
                    "SELECT belief_state_json FROM question_sessions WHERE id = ?1",
                    params![question_session_id],
                )
                .await?;
            let Some(row) = rows.next().await? else {
                return Err(StoreError::Db("question_session not found".into()));
            };
            row.get(0).map_err(|e| StoreError::Db(e.to_string()))?
        };
        let mut v = prev
            .as_deref()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
            .unwrap_or_else(|| serde_json::json!({}));
        let obj = v
            .as_object_mut()
            .ok_or_else(|| StoreError::Db("belief_state_json root".into()))?;
        let arr_slot = obj
            .entry("answers".to_string())
            .or_insert_with(|| serde_json::json!([]));
        let arr = arr_slot
            .as_array_mut()
            .ok_or_else(|| StoreError::Db("belief answers array".into()))?;

        let mut ans_obj = serde_json::Map::new();
        ans_obj.insert("question_id".to_string(), serde_json::json!(question_id));
        ans_obj.insert("answer_text".to_string(), serde_json::json!(answer_text));
        ans_obj.insert(
            "answered_at_ms".to_string(),
            serde_json::json!(answered_at_ms),
        );
        if let Some(oid) = selected_option_id {
            ans_obj.insert("selected_option_id".to_string(), serde_json::json!(oid));
        }
        arr.push(serde_json::Value::Object(ans_obj));

        if let Some(sel) = selected_option_id {
            if let Some((qeid, eig_bits)) = self
                .latest_question_event_id_and_eig(question_session_id, question_id)
                .await?
            {
                let opts = self.list_question_options(qeid).await?;
                if !opts.is_empty() && opts.iter().any(|o| o.option_id == sel) {
                    let n = opts.len() as f64;
                    let uniform = if n > 0.0 { 1.0 / n } else { 1.0 };
                    fn is_good_prob(x: f64) -> bool {
                        x.is_finite() && x > 0.0
                    }
                    let mut mass: Vec<(String, f64)> = Vec::with_capacity(opts.len());
                    for o in &opts {
                        let p = if let Some(pp) = o.prior_probability {
                            if is_good_prob(pp) {
                                pp
                            } else if let Some(pq) = o.posterior_probability {
                                if is_good_prob(pq) { pq } else { uniform }
                            } else {
                                uniform
                            }
                        } else if let Some(pq) = o.posterior_probability {
                            if is_good_prob(pq) { pq } else { uniform }
                        } else {
                            uniform
                        };
                        mass.push((o.option_id.clone(), p));
                    }
                    let sum: f64 = mass.iter().map(|(_, p)| p).sum();
                    if sum > 0.0 && sum.is_finite() {
                        for (_, p) in mass.iter_mut() {
                            *p /= sum;
                        }
                    } else {
                        mass = opts
                            .iter()
                            .map(|o| (o.option_id.clone(), uniform))
                            .collect();
                    }
                    let lik_mul = 1.0_f64 + eig_bits.clamp(0.0, 8.0);
                    let mut updated: HashMap<String, f64> = HashMap::new();
                    for (oid, p) in mass {
                        let l = if oid == sel { lik_mul } else { 1.0 };
                        updated.insert(oid, p * l);
                    }
                    let s: f64 = updated.values().copied().sum();
                    if s > 0.0 && s.is_finite() {
                        for p in updated.values_mut() {
                            *p /= s;
                        }
                    }
                    let by_q = obj
                        .entry("hypothesis_mass".to_string())
                        .or_insert_with(|| serde_json::json!({}));
                    let by_q_obj = by_q
                        .as_object_mut()
                        .ok_or_else(|| StoreError::Db("belief hypothesis_mass object".into()))?;
                    let per_q = by_q_obj
                        .entry("by_question".to_string())
                        .or_insert_with(|| serde_json::json!({}));
                    let per_q_obj = per_q
                        .as_object_mut()
                        .ok_or_else(|| StoreError::Db("belief by_question object".into()))?;
                    let prob_map: serde_json::Map<String, serde_json::Value> = updated
                        .iter()
                        .map(|(k, v)| (k.clone(), serde_json::json!(v)))
                        .collect();
                    per_q_obj.insert(question_id.to_string(), serde_json::Value::Object(prob_map));
                    for (oid, prob) in &updated {
                        pending_option_updates.push((qeid, oid.clone(), *prob));
                    }
                }
            }
        }

        obj.insert(
            "last_updated_ms".to_string(),
            serde_json::json!(answered_at_ms),
        );
        let out = serde_json::to_string(&v).map_err(|e| StoreError::Db(e.to_string()))?;
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                for (qeid, oid, prob) in pending_option_updates {
                    conn.execute(
                        "UPDATE question_options SET posterior_probability = ?3
                                 WHERE question_event_id = ?1 AND option_id = ?2",
                        params![qeid, oid.as_str(), prob],
                    )
                    .await?;
                }
                conn.execute(
                    "UPDATE question_sessions SET belief_state_json = ?2 WHERE id = ?1",
                    params![question_session_id, out],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Pending assistant clarifications for a repo-bound MCP session (for UI / `vox_questioning_pending`).
    pub async fn pending_clarifications_json_for_repo(
        &self,
        session_id: &str,
        repository_id: &str,
    ) -> Result<serde_json::Value, StoreError> {
        let Some(sess) = self
            .find_open_question_session_for_repo(session_id, repository_id)
            .await?
        else {
            return Ok(serde_json::json!({
                "open": false,
                "pending": [],
            }));
        };
        let belief_parsed = sess
            .belief_state_json
            .as_deref()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
            .unwrap_or(serde_json::Value::Null);

        let events = self.list_question_events(sess.id).await?;
        let mut pending_items = Vec::new();
        for e in events {
            if e.actor != "assistant" || e.answer_text.is_some() {
                continue;
            }
            let opts = self.list_question_options(e.id).await?;
            let opt_json: Vec<serde_json::Value> = opts
                .iter()
                .map(|o| {
                    serde_json::json!({
                        "option_id": o.option_id,
                        "label": o.label,
                        "prior_probability": o.prior_probability,
                        "posterior_probability": o.posterior_probability,
                        "is_other": o.is_other,
                    })
                })
                .collect();
            pending_items.push(serde_json::json!({
                "question_event_id": e.id,
                "question_id": e.question_id,
                "question_kind": e.question_kind,
                "prompt": e.prompt,
                "expected_information_gain_bits": e.expected_information_gain_bits,
                "options": opt_json,
            }));
        }
        Ok(serde_json::json!({
            "open": true,
            "question_session_id": sess.id,
            "belief_state_json": belief_parsed,
            "pending": pending_items,
        }))
    }

    /// List question sessions for one MCP session id.
    pub async fn list_question_sessions(
        &self,
        session_id: &str,
        limit: i64,
    ) -> Result<Vec<QuestionSessionRow>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT id, session_id, repository_id, task_id, policy_version,
                        started_at_ms, ended_at_ms, resolution_status, belief_state_json
                 FROM question_sessions
                 WHERE session_id = ?1
                 ORDER BY started_at_ms DESC
                 LIMIT ?2",
                params![session_id, limit.clamp(1, 10_000)],
            )
            .await?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await? {
            out.push(QuestionSessionRow {
                id: row.get(0).map_err(|e| StoreError::Db(e.to_string()))?,
                session_id: DbSessionId::new(
                    row.get::<String>(1)
                        .map_err(|e| StoreError::Db(e.to_string()))?,
                ),
                repository_id: row.get(2).map_err(|e| StoreError::Db(e.to_string()))?,
                task_id: row
                    .get::<Option<String>>(3)
                    .map_err(|e| StoreError::Db(e.to_string()))?
                    .map(DbTaskId::new),
                policy_version: row.get(4).map_err(|e| StoreError::Db(e.to_string()))?,
                started_at_ms: row.get(5).map_err(|e| StoreError::Db(e.to_string()))?,
                ended_at_ms: row.get(6).map_err(|e| StoreError::Db(e.to_string()))?,
                resolution_status: row.get(7).map_err(|e| StoreError::Db(e.to_string()))?,
                belief_state_json: row.get(8).map_err(|e| StoreError::Db(e.to_string()))?,
            });
        }
        Ok(out)
    }

    /// List ordered question events for a question session id.
    pub async fn list_question_events(
        &self,
        question_session_id: i64,
    ) -> Result<Vec<QuestionEventRow>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT id, question_session_id, question_id, turn_index, actor, question_kind, prompt,
                        expected_information_gain_bits, expected_user_cost, utility_bits_per_cost,
                        answer_text, answer_type, answered_at_ms, created_at_ms
                 FROM question_events
                 WHERE question_session_id = ?1
                 ORDER BY turn_index ASC, id ASC",
                params![question_session_id],
            )
            .await?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await? {
            out.push(QuestionEventRow {
                id: row.get(0).map_err(|e| StoreError::Db(e.to_string()))?,
                question_session_id: row.get(1).map_err(|e| StoreError::Db(e.to_string()))?,
                question_id: row.get(2).map_err(|e| StoreError::Db(e.to_string()))?,
                turn_index: row.get(3).map_err(|e| StoreError::Db(e.to_string()))?,
                actor: row.get(4).map_err(|e| StoreError::Db(e.to_string()))?,
                question_kind: row.get(5).map_err(|e| StoreError::Db(e.to_string()))?,
                prompt: row.get(6).map_err(|e| StoreError::Db(e.to_string()))?,
                expected_information_gain_bits: row
                    .get(7)
                    .map_err(|e| StoreError::Db(e.to_string()))?,
                expected_user_cost: row.get(8).map_err(|e| StoreError::Db(e.to_string()))?,
                utility_bits_per_cost: row.get(9).map_err(|e| StoreError::Db(e.to_string()))?,
                answer_text: row.get(10).map_err(|e| StoreError::Db(e.to_string()))?,
                answer_type: row.get(11).map_err(|e| StoreError::Db(e.to_string()))?,
                answered_at_ms: row.get(12).map_err(|e| StoreError::Db(e.to_string()))?,
                created_at_ms: row.get(13).map_err(|e| StoreError::Db(e.to_string()))?,
            });
        }
        Ok(out)
    }

    /// List options for a question event.
    pub async fn list_question_options(
        &self,
        question_event_id: i64,
    ) -> Result<Vec<QuestionOptionRow>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT id, question_event_id, option_id, label, prior_probability,
                        posterior_probability, is_other
                 FROM question_options
                 WHERE question_event_id = ?1
                 ORDER BY id ASC",
                params![question_event_id],
            )
            .await?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await? {
            let is_other_raw: i64 = row.get(6).map_err(|e| StoreError::Db(e.to_string()))?;
            out.push(QuestionOptionRow {
                id: row.get(0).map_err(|e| StoreError::Db(e.to_string()))?,
                question_event_id: row.get(1).map_err(|e| StoreError::Db(e.to_string()))?,
                option_id: row.get(2).map_err(|e| StoreError::Db(e.to_string()))?,
                label: row.get(3).map_err(|e| StoreError::Db(e.to_string()))?,
                prior_probability: row.get(4).map_err(|e| StoreError::Db(e.to_string()))?,
                posterior_probability: row.get(5).map_err(|e| StoreError::Db(e.to_string()))?,
                is_other: is_other_raw != 0,
            });
        }
        Ok(out)
    }

    /// List option outcomes for a question event.
    pub async fn list_question_option_outcomes(
        &self,
        question_event_id: i64,
    ) -> Result<Vec<QuestionOptionOutcomeRow>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT id, question_event_id, option_id, selected, diagnostic_weight,
                        information_contribution_bits, created_at_ms
                 FROM question_option_outcomes
                 WHERE question_event_id = ?1
                 ORDER BY id ASC",
                params![question_event_id],
            )
            .await?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await? {
            let selected_raw: i64 = row.get(3).map_err(|e| StoreError::Db(e.to_string()))?;
            out.push(QuestionOptionOutcomeRow {
                id: row.get(0).map_err(|e| StoreError::Db(e.to_string()))?,
                question_event_id: row.get(1).map_err(|e| StoreError::Db(e.to_string()))?,
                option_id: row.get(2).map_err(|e| StoreError::Db(e.to_string()))?,
                selected: selected_raw != 0,
                diagnostic_weight: row.get(4).map_err(|e| StoreError::Db(e.to_string()))?,
                information_contribution_bits: row
                    .get(5)
                    .map_err(|e| StoreError::Db(e.to_string()))?,
                created_at_ms: row.get(6).map_err(|e| StoreError::Db(e.to_string()))?,
            });
        }
        Ok(out)
    }

    /// List stop events for a session.
    pub async fn list_question_stop_events(
        &self,
        question_session_id: i64,
    ) -> Result<Vec<QuestionStopEventRow>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT id, question_session_id, stop_reason, confidence_at_stop, marginal_gain_bits,
                        expected_user_cost, turn_index, created_at_ms
                 FROM question_stop_events
                 WHERE question_session_id = ?1
                 ORDER BY id ASC",
                params![question_session_id],
            )
            .await?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await? {
            out.push(QuestionStopEventRow {
                id: row.get(0).map_err(|e| StoreError::Db(e.to_string()))?,
                question_session_id: row.get(1).map_err(|e| StoreError::Db(e.to_string()))?,
                stop_reason: row.get(2).map_err(|e| StoreError::Db(e.to_string()))?,
                confidence_at_stop: row.get(3).map_err(|e| StoreError::Db(e.to_string()))?,
                marginal_gain_bits: row.get(4).map_err(|e| StoreError::Db(e.to_string()))?,
                expected_user_cost: row.get(5).map_err(|e| StoreError::Db(e.to_string()))?,
                turn_index: row.get(6).map_err(|e| StoreError::Db(e.to_string()))?,
                created_at_ms: row.get(7).map_err(|e| StoreError::Db(e.to_string()))?,
            });
        }
        Ok(out)
    }

    /// Send a normalized A2A clarification message using `a2a_messages`.
    pub async fn send_a2a_clarification_message(
        &self,
        p: A2aClarificationMessageParams<'_>,
    ) -> Result<(), StoreError> {
        let payload = serde_json::json!({
            "clarification_intent": p.clarification_intent,
            "hypothesis_set_id": p.hypothesis_set_id,
            "question_kind": p.question_kind,
            "expected_information_gain_bits": p.expected_information_gain_bits,
            "expected_user_cost": p.expected_user_cost,
            "requested_evidence_dimensions_json": p.requested_evidence_dimensions_json,
            "urgency": p.urgency,
            "stop_policy_json": p.stop_policy_json
        });
        self.send_a2a_message(
            p.message_uuid,
            p.sender_agent,
            p.receiver_agent,
            p.msg_type,
            &payload.to_string(),
            p.priority,
            p.thread_id,
            p.repository_id,
        )
        .await
    }
}
