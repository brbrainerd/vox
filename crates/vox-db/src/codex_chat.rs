//! Codex **user chat**, **tool calls**, **usage counters**, and **topics** (manifest slices `v11`–`v14`).
//!
//! **S3 content plane:** conversation text, tool arguments, and transcript rows are user/workspace content —
//! not “usage telemetry”. Do not fold into `research_metrics` without explicit consent and classification
//! (`docs/src/architecture/telemetry-retention-sensitivity-ssot.md`).
//!
//! Callers must use a store opened through [`crate::VoxDb::connect`] so the baseline DDL has been applied.

use turso::params;

use crate::VoxDb;
use crate::store::StoreError;

/// One row from structured `conversation_messages` for workspace transcript hydration.
#[derive(Debug, Clone)]
pub struct WorkspaceTranscriptTurnRow {
    pub role: String,
    pub content_text: String,
    pub external_turn_id: String,
    pub model_used: Option<String>,
    pub token_count: Option<i64>,
    pub context_files_json: String,
    pub created_unix: u64,
}

impl VoxDb {
    /// Locate a workspace-scoped MCP transcript conversation (`repository_id` + `external_session_id`).
    pub async fn chat_find_workspace_conversation_id(
        &self,
        repository_id: &str,
        external_session_id: &str,
    ) -> Result<Option<i64>, StoreError> {
        let rid = repository_id.to_string();
        let sid = external_session_id.to_string();
        let mut rows = self
            .connection()
            .query(
                "SELECT id FROM conversations
                 WHERE repository_id = ?1 AND external_session_id = ?2
                 LIMIT 1",
                params![rid.as_str(), sid.as_str()],
            )
            .await?;
        let row = rows.next().await?;
        Ok(match row {
            Some(r) => Some(r.get(0).map_err(|e| StoreError::Db(e.to_string()))?),
            None => None,
        })
    }

    /// Ensure a `conversations` row exists for the MCP / workspace session (structured transcript SSOT).
    pub async fn chat_ensure_workspace_conversation(
        &self,
        repository_id: &str,
        external_session_id: &str,
        thread_id: Option<&str>,
        origin_surface: &str,
    ) -> Result<i64, StoreError> {
        if let Some(id) = self
            .chat_find_workspace_conversation_id(repository_id, external_session_id)
            .await?
        {
            return Ok(id);
        }
        let title = format!(
            "workspace {}…",
            external_session_id.chars().take(12).collect::<String>()
        );
        let rid = repository_id.to_string();
        let sid = external_session_id.to_string();
        let tid = thread_id.map(str::to_string);
        let origin = origin_surface.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO conversations
                    (user_id, title, repository_id, external_session_id, thread_id, origin_surface)
                 VALUES (NULL, ?1, ?2, ?3, ?4, ?5)",
                    params![
                        title.as_str(),
                        rid.as_str(),
                        sid.as_str(),
                        tid.as_deref(),
                        origin.as_str(),
                    ],
                )
                .await?;
                Ok::<i64, StoreError>(conn.last_insert_rowid())
            })
            .await
    }

    /// Append a transcript turn with workspace metadata (dual-write / structured SSOT path).
    #[allow(clippy::too_many_arguments)]
    pub async fn chat_append_workspace_message(
        &self,
        conversation_id: i64,
        external_turn_id: &str,
        role: &str,
        content_text: &str,
        model_used: Option<&str>,
        token_count: Option<i64>,
        context_files_json: Option<&str>,
        journey_payload_json: Option<&str>,
    ) -> Result<i64, StoreError> {
        let external_turn_id = external_turn_id.to_string();
        let role = role.to_string();
        let content_text = content_text.to_string();
        let model_used = model_used.map(str::to_string);
        let context_files_json = context_files_json.map(str::to_string);
        let journey_payload_json = journey_payload_json.map(str::to_string);
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO conversation_messages
                    (conversation_id, role, content_text, payload_json, external_turn_id,
                     model_used, token_count, context_files_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        conversation_id,
                        role.as_str(),
                        content_text.as_str(),
                        journey_payload_json.as_deref(),
                        external_turn_id.as_str(),
                        model_used.as_deref(),
                        token_count,
                        context_files_json.as_deref(),
                    ],
                )
                .await?;
                let id = conn.last_insert_rowid();
                conn.execute(
                    "UPDATE conversations SET updated_at = datetime('now') WHERE id = ?1",
                    params![conversation_id],
                )
                .await?;
                Ok::<i64, StoreError>(id)
            })
            .await
    }

    /// Load recent structured transcript turns for hydration (oldest → newest).
    pub async fn chat_load_workspace_transcript_turns(
        &self,
        repository_id: &str,
        external_session_id: &str,
        limit: i64,
    ) -> Result<Vec<WorkspaceTranscriptTurnRow>, StoreError> {
        let Some(conversation_id) = self
            .chat_find_workspace_conversation_id(repository_id, external_session_id)
            .await?
        else {
            return Ok(Vec::new());
        };
        let lim = limit.clamp(1, 500);
        let mut rows = self
            .connection()
            .query(
                "SELECT role, content_text, COALESCE(external_turn_id, ''),
                        model_used, token_count, COALESCE(context_files_json, ''),
                        COALESCE(unixepoch(created_at), 0)
                 FROM conversation_messages
                 WHERE conversation_id = ?1
                 ORDER BY id DESC
                 LIMIT ?2",
                params![conversation_id, lim],
            )
            .await?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await? {
            let role: String = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
            let content: String = row.get(1).map_err(|e| StoreError::Db(e.to_string()))?;
            let turn_id: String = row.get(2).map_err(|e| StoreError::Db(e.to_string()))?;
            let model_used: Option<String> = row
                .get::<Option<String>>(3)
                .map_err(|e| StoreError::Db(e.to_string()))?;
            let token_count: Option<i64> = row
                .get::<Option<i64>>(4)
                .map_err(|e| StoreError::Db(e.to_string()))?;
            let ctx_files: String = row.get(5).map_err(|e| StoreError::Db(e.to_string()))?;
            let ts: u64 = row
                .get::<i64>(6)
                .map(|u| u.max(0) as u64)
                .map_err(|e| StoreError::Db(e.to_string()))?;
            out.push(WorkspaceTranscriptTurnRow {
                role,
                content_text: content,
                external_turn_id: turn_id,
                model_used,
                token_count,
                context_files_json: ctx_files,
                created_unix: ts,
            });
        }
        out.reverse();
        Ok(out)
    }

    /// Insert a `conversations` row (V11+). Returns SQLite `rowid` / `id`.
    pub async fn chat_create_conversation(
        &self,
        user_id: Option<&str>,
        title: &str,
    ) -> Result<i64, StoreError> {
        let user_id = user_id.map(str::to_string);
        let title = title.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO conversations (user_id, title) VALUES (?1, ?2)",
                    params![user_id.as_deref(), title.as_str()],
                )
                .await?;
                Ok::<i64, StoreError>(conn.last_insert_rowid())
            })
            .await
    }

    /// Bump `conversations.updated_at` for listing recency (V11+).
    pub async fn chat_touch_conversation(&self, conversation_id: i64) -> Result<(), StoreError> {
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "UPDATE conversations SET updated_at = datetime('now') WHERE id = ?1",
                    params![conversation_id],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Append a `conversation_messages` row (V11+). Returns message `id`.
    pub async fn chat_append_message(
        &self,
        conversation_id: i64,
        role: &str,
        content_text: &str,
        payload_json: Option<&str>,
    ) -> Result<i64, StoreError> {
        let role = role.to_string();
        let content_text = content_text.to_string();
        let payload_json = payload_json.map(str::to_string);
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO conversation_messages (conversation_id, role, content_text, payload_json)
                 VALUES (?1, ?2, ?3, ?4)",
                    params![
                        conversation_id,
                        role.as_str(),
                        content_text.as_str(),
                        payload_json.as_deref(),
                    ],
                )
                .await?;
                let id = conn.last_insert_rowid();
                conn.execute(
                    "UPDATE conversations SET updated_at = datetime('now') WHERE id = ?1",
                    params![conversation_id],
                )
                .await?;
                Ok::<i64, StoreError>(id)
            })
            .await
    }

    /// Record a tool invocation for an assistant message (V12+). Returns tool-call row `id`.
    pub async fn chat_insert_tool_call(
        &self,
        conversation_message_id: i64,
        ordinal: i32,
        tool_name: &str,
        arguments_json: &str,
        status: &str,
    ) -> Result<i64, StoreError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let tool_name = tool_name.to_string();
        let arguments_json = arguments_json.to_string();
        let status = status.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO conversation_tool_calls
                    (conversation_message_id, ordinal, tool_name, arguments_json, status, started_at_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        conversation_message_id,
                        ordinal,
                        tool_name.as_str(),
                        arguments_json.as_str(),
                        status.as_str(),
                        now
                    ],
                )
                .await?;
                Ok::<i64, StoreError>(conn.last_insert_rowid())
            })
            .await
    }

    /// Update result / terminal state for a tool call (V12+).
    pub async fn chat_finish_tool_call(
        &self,
        tool_call_id: i64,
        status: &str,
        result_json: Option<&str>,
        error_text: Option<&str>,
    ) -> Result<(), StoreError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let status = status.to_string();
        let result_json = result_json.map(str::to_string);
        let error_text = error_text.map(str::to_string);
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "UPDATE conversation_tool_calls
                 SET status = ?2, result_json = ?3, error_text = ?4, finished_at_ms = ?5
                 WHERE id = ?1",
                    params![
                        tool_call_id,
                        status.as_str(),
                        result_json.as_deref(),
                        error_text.as_deref(),
                        now,
                    ],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Upsert a usage limit policy row (V13+).
    pub async fn chat_upsert_usage_limit(
        &self,
        metric_key: &str,
        scope_kind: &str,
        scope_id: &str,
        period_kind: &str,
        limit_value: i64,
        enforcement: &str,
    ) -> Result<(), StoreError> {
        let metric_key = metric_key.to_string();
        let scope_kind = scope_kind.to_string();
        let scope_id = scope_id.to_string();
        let period_kind = period_kind.to_string();
        let enforcement = enforcement.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO usage_limit_definitions
                    (metric_key, scope_kind, scope_id, period_kind, limit_value, enforcement, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))
                 ON CONFLICT(metric_key, scope_kind, scope_id, period_kind) DO UPDATE SET
                    limit_value = excluded.limit_value,
                    enforcement = excluded.enforcement,
                    updated_at = datetime('now')",
                    params![
                        metric_key.as_str(),
                        scope_kind.as_str(),
                        scope_id.as_str(),
                        period_kind.as_str(),
                        limit_value,
                        enforcement.as_str(),
                    ],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Add `delta` to a usage counter for the given window (V13+). Returns the new total `amount`.
    pub async fn chat_add_usage_amount(
        &self,
        metric_key: &str,
        scope_kind: &str,
        scope_id: &str,
        period_start: &str,
        delta: i64,
    ) -> Result<i64, StoreError> {
        let mk = metric_key.to_string();
        let sk = scope_kind.to_string();
        let sid = scope_id.to_string();
        let ps = period_start.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO usage_counter_snapshots
                    (metric_key, scope_kind, scope_id, period_start, amount, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'))
                 ON CONFLICT(metric_key, scope_kind, scope_id, period_start) DO UPDATE SET
                    amount = usage_counter_snapshots.amount + excluded.amount,
                    updated_at = datetime('now')",
                    params![mk.as_str(), sk.as_str(), sid.as_str(), ps.as_str(), delta],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await?;
        let mut rows = self
            .connection()
            .query(
                "SELECT amount FROM usage_counter_snapshots
                 WHERE metric_key = ?1 AND scope_kind = ?2 AND scope_id = ?3 AND period_start = ?4",
                params![metric_key, scope_kind, scope_id, period_start],
            )
            .await?;
        let row = rows
            .next()
            .await?
            .ok_or_else(|| StoreError::Db("usage_counter_snapshots readback".into()))?;
        let amount: i64 = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
        Ok(amount)
    }

    /// Current counted usage for a window, or `0` if missing (V13+).
    pub async fn chat_usage_amount(
        &self,
        metric_key: &str,
        scope_kind: &str,
        scope_id: &str,
        period_start: &str,
    ) -> Result<i64, StoreError> {
        let mut rows = self.connection()
            .query(
                "SELECT COALESCE(
                    (SELECT amount FROM usage_counter_snapshots
                     WHERE metric_key = ?1 AND scope_kind = ?2 AND scope_id = ?3 AND period_start = ?4),
                    0)",
                params![metric_key, scope_kind, scope_id, period_start],
            )
            .await?;
        let row = rows
            .next()
            .await?
            .ok_or_else(|| StoreError::Db("usage amount".into()))?;
        let v: i64 = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
        Ok(v)
    }

    /// Resolved limit for an exact scope match, if defined (V13+).
    pub async fn chat_usage_limit_value(
        &self,
        metric_key: &str,
        scope_kind: &str,
        scope_id: &str,
        period_kind: &str,
    ) -> Result<Option<i64>, StoreError> {
        let mut rows = self
            .connection()
            .query(
                "SELECT limit_value FROM usage_limit_definitions
                 WHERE metric_key = ?1 AND scope_kind = ?2 AND scope_id = ?3 AND period_kind = ?4
                 LIMIT 1",
                params![metric_key, scope_kind, scope_id, period_kind],
            )
            .await?;
        let row = match rows.next().await? {
            Some(r) => r,
            None => return Ok(None),
        };
        let v: i64 = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
        Ok(Some(v))
    }

    /// `INSERT OR IGNORE` then return `topics.id` for `slug` (V14+).
    pub async fn chat_ensure_topic(&self, slug: &str, label: &str) -> Result<i64, StoreError> {
        let slug_own = slug.to_string();
        let label_own = label.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT OR IGNORE INTO topics (slug, label) VALUES (?1, ?2)",
                    params![slug_own.as_str(), label_own.as_str()],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await?;
        let mut rows = self
            .connection()
            .query(
                "SELECT id FROM topics WHERE slug = ?1 LIMIT 1",
                params![slug],
            )
            .await?;
        let row = rows
            .next()
            .await?
            .ok_or_else(|| StoreError::Db("topics slug missing after insert".into()))?;
        let id: i64 = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
        Ok(id)
    }

    /// Link a conversation to a topic with optional weight (V14+).
    pub async fn chat_link_conversation_topic(
        &self,
        conversation_id: i64,
        topic_id: i64,
        weight: f64,
    ) -> Result<(), StoreError> {
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO conversation_topics (conversation_id, topic_id, weight)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(conversation_id, topic_id) DO UPDATE SET weight = excluded.weight",
                    params![conversation_id, topic_id, weight],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Link a single message to a topic (V14+).
    pub async fn chat_link_message_topic(
        &self,
        conversation_message_id: i64,
        topic_id: i64,
    ) -> Result<(), StoreError> {
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT OR IGNORE INTO conversation_message_topics (conversation_message_id, topic_id)
                 VALUES (?1, ?2)",
                    params![conversation_message_id, topic_id],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }
}

#[cfg(all(test, feature = "local"))]
mod tests {
    use crate::{DbConfig, VoxDb};

    #[tokio::test]
    async fn chat_tool_usage_topic_round_trip() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("db");
        assert_eq!(
            db.schema_version().await.expect("v"),
            crate::schema::BASELINE_VERSION
        );

        db.connection()
            .execute(
                "INSERT OR IGNORE INTO users (id, display_name, role) VALUES ('u1', 'u1', 'user')",
                (),
            )
            .await
            .expect("seed user");

        let conv = db
            .chat_create_conversation(Some("u1"), "hi")
            .await
            .expect("conv");
        let msg = db
            .chat_append_message(conv, "assistant", "calling tool", None)
            .await
            .expect("msg");
        let tc = db
            .chat_insert_tool_call(msg, 0, "search", "{}", "running")
            .await
            .expect("tc");
        db.chat_finish_tool_call(tc, "succeeded", Some("{\"ok\":true}"), None)
            .await
            .expect("fin");

        db.chat_upsert_usage_limit("tokens", "user", "u1", "daily", 1000, "hard")
            .await
            .expect("lim");
        let amt = db
            .chat_add_usage_amount("tokens", "user", "u1", "2026-03-21", 42)
            .await
            .expect("add");
        assert_eq!(amt, 42);
        let lim = db
            .chat_usage_limit_value("tokens", "user", "u1", "daily")
            .await
            .expect("q");
        assert_eq!(lim, Some(1000));

        let tid = db.chat_ensure_topic("rust", "Rust").await.expect("topic");
        db.chat_link_conversation_topic(conv, tid, 1.0)
            .await
            .expect("ct");
        db.chat_link_message_topic(msg, tid).await.expect("mt");
    }

    #[tokio::test]
    async fn workspace_conversation_dual_write_round_trip() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("db");
        let conv = db
            .chat_ensure_workspace_conversation("repo1", "sess-a", Some("thr-1"), "mcp")
            .await
            .expect("ensure");
        assert_eq!(
            db.chat_find_workspace_conversation_id("repo1", "sess-a")
                .await
                .expect("find"),
            Some(conv)
        );
        let _ = db
            .chat_append_workspace_message(
                conv,
                "t1",
                "user",
                "hello",
                None,
                None,
                Some("[]"),
                Some(r#"{"envelope_version":1,"journey_id":"j1"}"#),
            )
            .await
            .expect("append");
        let rows = db
            .chat_load_workspace_transcript_turns("repo1", "sess-a", 50)
            .await
            .expect("load");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].role, "user");
        assert_eq!(rows[0].content_text, "hello");
    }
}
