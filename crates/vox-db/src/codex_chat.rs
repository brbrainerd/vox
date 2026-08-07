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

    /// Returns the `created_at` timestamp SQLite assigned to a message row,
    /// immediately after insert — used by callers (like `chat_send_message`)
    /// that need to return a DTO with a real, not-yet-reloaded timestamp.
    pub async fn chat_message_created_at(&self, message_id: i64) -> Result<String, StoreError> {
        let mut rows = self
            .connection()
            .query(
                "SELECT created_at FROM conversation_messages WHERE id = ?1",
                params![message_id],
            )
            .await?;
        match rows.next().await? {
            Some(row) => row.get(0).map_err(|e| StoreError::Db(e.to_string())),
            None => Err(StoreError::Db(format!(
                "conversation_messages row {message_id} not found"
            ))),
        }
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

    /// Locate a GUI chat session by its external session id (`origin_surface = gui`).
    /// Archived conversations are excluded — a resumed/deep-linked `external_session_id`
    /// pointing at an archived row must not silently reuse it (see
    /// `archived_session_is_not_found_by_external_session_id_lookup`).
    pub async fn chat_find_gui_conversation_id(
        &self,
        external_session_id: &str,
    ) -> Result<Option<i64>, StoreError> {
        let sid = external_session_id.to_string();
        let mut rows = self
            .connection()
            .query(
                "SELECT id FROM conversations
                 WHERE origin_surface = 'gui' AND external_session_id = ?1 AND archived_at IS NULL
                 ORDER BY id DESC LIMIT 1",
                params![sid.as_str()],
            )
            .await?;
        let row = rows.next().await?;
        Ok(match row {
            Some(r) => Some(r.get(0).map_err(|e| StoreError::Db(e.to_string()))?),
            None => None,
        })
    }

    /// Same as [`Self::chat_find_gui_conversation_id`] but also finds archived rows — used only
    /// by the unarchive path, which needs to locate a conversation precisely because it's
    /// archived.
    pub async fn chat_find_gui_conversation_id_including_archived(
        &self,
        external_session_id: &str,
    ) -> Result<Option<i64>, StoreError> {
        let sid = external_session_id.to_string();
        let mut rows = self
            .connection()
            .query(
                "SELECT id FROM conversations
                 WHERE origin_surface = 'gui' AND external_session_id = ?1
                 ORDER BY id DESC LIMIT 1",
                params![sid.as_str()],
            )
            .await?;
        let row = rows.next().await?;
        Ok(match row {
            Some(r) => Some(r.get(0).map_err(|e| StoreError::Db(e.to_string()))?),
            None => None,
        })
    }

    /// Ensure a GUI-scoped conversation row exists; returns SQLite id.
    pub async fn chat_ensure_gui_session(
        &self,
        external_session_id: &str,
        title: &str,
    ) -> Result<i64, StoreError> {
        self.chat_ensure_gui_session_with_repo(external_session_id, title, None)
            .await
    }

    /// Same as [`Self::chat_ensure_gui_session`], additionally recording which
    /// repository this session targets (see `vox_repository::RepositoryContext::repository_id`
    /// for how callers derive `repository_id`). If a conversation with this
    /// `external_session_id` already exists, `repository_id` is ignored on this call — the
    /// existing row's value is left as-is (find-or-create semantics; this method never updates
    /// an existing row's repository tag).
    pub async fn chat_ensure_gui_session_with_repo(
        &self,
        external_session_id: &str,
        title: &str,
        repository_id: Option<&str>,
    ) -> Result<i64, StoreError> {
        if let Some(id) = self
            .chat_find_gui_conversation_id(external_session_id)
            .await?
        {
            return Ok(id);
        }
        let sid = external_session_id.to_string();
        let title = title.to_string();
        let repo = repository_id.map(str::to_string);
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO conversations (title, external_session_id, origin_surface, repository_id)
                     VALUES (?1, ?2, 'gui', ?3)",
                    params![title.as_str(), sid.as_str(), repo.as_deref()],
                )
                .await?;
                Ok::<i64, StoreError>(conn.last_insert_rowid())
            })
            .await
    }

    /// List recent GUI chat sessions for the tab strip.
    ///
    /// Excludes `bg-task-*` session ids: `/spawn`, "Deploy skill", and the
    /// composer's "Background task" send mode all give their
    /// `submit_orchestrator_task` dispatch a `bg-task-*` session id
    /// specifically so it never gets folded into a real chat session's
    /// history (see `newBackgroundSessionId` in the GUI). But
    /// `chat_append_message` still unconditionally persists the description
    /// and any eventual reply under that id (it has no category awareness),
    /// which would otherwise mint a permanent, one-off, mistitled "Chat"
    /// sidebar entry per background dispatch. The task itself remains fully
    /// visible via the Tasks surface regardless of this filter.
    ///
    /// When `include_archived` is false, also excludes rows with `archived_at IS NOT NULL`.
    pub async fn chat_list_gui_sessions(
        &self,
        limit: usize,
        include_archived: bool,
    ) -> Result<Vec<(i64, String, String, String, i64, Option<String>)>, StoreError> {
        let lim = limit.max(1) as i64;
        let archive_clause = if include_archived {
            ""
        } else {
            "AND c.archived_at IS NULL"
        };
        let sql = format!(
            "SELECT c.id, c.title, c.external_session_id, c.updated_at,
                    (SELECT COUNT(*) FROM conversation_messages m WHERE m.conversation_id = c.id),
                    c.repository_id
             FROM conversations c
             WHERE c.origin_surface = 'gui'
               AND c.external_session_id NOT LIKE 'bg-task-%'
               {archive_clause}
             ORDER BY c.updated_at DESC
             LIMIT ?1"
        );
        let mut rows = self.connection().query(&sql, params![lim]).await?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await? {
            let id: i64 = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
            let title: String = row.get(1).map_err(|e| StoreError::Db(e.to_string()))?;
            let ext: String = row.get(2).map_err(|e| StoreError::Db(e.to_string()))?;
            let updated: String = row.get(3).map_err(|e| StoreError::Db(e.to_string()))?;
            let count: i64 = row.get(4).map_err(|e| StoreError::Db(e.to_string()))?;
            let repository_id: Option<String> =
                row.get(5).map_err(|e| StoreError::Db(e.to_string()))?;
            out.push((id, title, ext, updated, count, repository_id));
        }
        Ok(out)
    }

    /// Rename a conversation title.
    pub async fn chat_rename_conversation(
        &self,
        conversation_id: i64,
        title: &str,
    ) -> Result<(), StoreError> {
        let title = title.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "UPDATE conversations SET title = ?2, updated_at = datetime('now') WHERE id = ?1",
                    params![conversation_id, title.as_str()],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Soft-archive a conversation (recoverable — see `chat_unarchive_conversation`).
    pub async fn chat_archive_conversation(&self, conversation_id: i64) -> Result<(), StoreError> {
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "UPDATE conversations SET archived_at = datetime('now') WHERE id = ?1",
                    params![conversation_id],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Reverse `chat_archive_conversation`.
    pub async fn chat_unarchive_conversation(
        &self,
        conversation_id: i64,
    ) -> Result<(), StoreError> {
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "UPDATE conversations SET archived_at = NULL WHERE id = ?1",
                    params![conversation_id],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Load messages for a GUI session (by external session id).
    pub async fn chat_get_gui_messages(
        &self,
        external_session_id: &str,
        limit: usize,
    ) -> Result<Vec<(i64, String, String, String, Option<String>)>, StoreError> {
        let conv_id = self
            .chat_find_gui_conversation_id(external_session_id)
            .await?
            .unwrap_or(-1);
        if conv_id < 0 {
            return Ok(Vec::new());
        }
        let lim = limit.max(1) as i64;
        let mut rows = self
            .connection()
            .query(
                "SELECT m.id, m.role, m.content_text, m.created_at, m.payload_json
                 FROM conversation_messages m
                 WHERE m.conversation_id = ?1
                 ORDER BY m.id ASC
                 LIMIT ?2",
                params![conv_id, lim],
            )
            .await?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await? {
            let id: i64 = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
            let role: String = row.get(1).map_err(|e| StoreError::Db(e.to_string()))?;
            let text: String = row.get(2).map_err(|e| StoreError::Db(e.to_string()))?;
            let created: String = row.get(3).map_err(|e| StoreError::Db(e.to_string()))?;
            let payload: Option<String> = row.get(4).map_err(|e| StoreError::Db(e.to_string()))?;
            out.push((id, role, text, created, payload));
        }
        Ok(out)
    }

    /// Simple LIKE search over GUI chat messages for the chats corpus.
    pub async fn chat_search_gui_messages(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<(i64, i64, String, String, String)>, StoreError> {
        let pattern = format!("%{}%", query.replace('%', ""));
        let lim = limit.max(1) as i64;
        let mut rows = self
            .connection()
            .query(
                "SELECT m.id, m.conversation_id, c.external_session_id, m.role,
                        substr(m.content_text, 1, 240)
                 FROM conversation_messages m
                 INNER JOIN conversations c ON c.id = m.conversation_id
                 WHERE c.origin_surface = 'gui' AND m.content_text LIKE ?1
                 ORDER BY m.id DESC
                 LIMIT ?2",
                params![pattern.as_str(), lim],
            )
            .await?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await? {
            let msg_id: i64 = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
            let conv_id: i64 = row.get(1).map_err(|e| StoreError::Db(e.to_string()))?;
            let session_id: String = row.get(2).map_err(|e| StoreError::Db(e.to_string()))?;
            let role: String = row.get(3).map_err(|e| StoreError::Db(e.to_string()))?;
            let snippet: String = row.get(4).map_err(|e| StoreError::Db(e.to_string()))?;
            out.push((msg_id, conv_id, session_id, role, snippet));
        }
        Ok(out)
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

    /// Count open (pending/queued/in_progress) plan nodes at each plan session's *current*
    /// version, summed per originating chat session, across every `plan_sessions` row that
    /// chat session has ever produced (one per dispatched goal — see `goal.rs`). Sessions with
    /// no dispatched goals, or whose nodes are all resolved, are absent from the returned map
    /// (not present with a `0` entry) — callers should treat a missing key as zero.
    pub async fn open_task_counts_for_sessions(
        &self,
        chat_external_session_ids: &[String],
    ) -> Result<std::collections::HashMap<String, i64>, StoreError> {
        let mut out = std::collections::HashMap::new();
        if chat_external_session_ids.is_empty() {
            return Ok(out);
        }
        let placeholders = (0..chat_external_session_ids.len())
            .map(|i| format!("?{}", i + 1))
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT ps.origin_session_id, COUNT(*)
             FROM plan_sessions ps
             JOIN plan_nodes pn
               ON pn.plan_session_id = ps.plan_session_id AND pn.version = ps.current_version
             WHERE ps.origin_session_id IN ({placeholders})
               AND pn.status IN ('pending', 'queued', 'in_progress')
             GROUP BY ps.origin_session_id"
        );
        let bound: Vec<turso::Value> = chat_external_session_ids
            .iter()
            .map(|id| turso::Value::from(id.as_str()))
            .collect();
        let mut rows = self.conn.query(&sql, bound).await?;
        while let Some(row) = rows.next().await? {
            let session_id: String = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
            let count: i64 = row.get(1).map_err(|e| StoreError::Db(e.to_string()))?;
            out.insert(session_id, count);
        }
        Ok(out)
    }
}

#[cfg(all(test, feature = "local"))]
mod tests {
    use crate::{DbConfig, VoxDb};

    // Exercises conversation_tool_calls, usage_limit_definitions,
    // usage_counter_snapshots, conversation_topics, and
    // conversation_message_topics, all quarantined (Task 4, VoxDB audit
    // condensation plan) — off by default, see domains/quarantine.rs.
    #[tokio::test]
    #[cfg(feature = "quarantine")]
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

    // Code-review fix: `/spawn`, "Deploy skill", and the composer's
    // "Background task" send mode all give their submit_orchestrator_task
    // dispatch a `bg-task-*` session id specifically so it's never folded
    // into a real chat session -- but chat_append_message still persists
    // under that id regardless, which would otherwise mint a permanent,
    // one-off "Chat" sidebar entry per background dispatch. Confirms the
    // sidebar listing filters those out while a real GUI session still shows.
    #[tokio::test]
    async fn chat_list_gui_sessions_excludes_bg_task_ids() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("db");
        db.chat_ensure_gui_session("gui-real-session", "Chat")
            .await
            .expect("ensure real session");
        db.chat_ensure_gui_session("bg-task-gui-run-1", "Chat")
            .await
            .expect("ensure bg-task session");

        let sessions = db
            .chat_list_gui_sessions(40, false)
            .await
            .expect("list sessions");
        let ids: Vec<&str> = sessions
            .iter()
            .map(|(_, _, ext, _, _, _)| ext.as_str())
            .collect();
        assert!(
            ids.contains(&"gui-real-session"),
            "a real GUI session must still be listed: {ids:?}"
        );
        assert!(
            !ids.contains(&"bg-task-gui-run-1"),
            "a bg-task-* session must not appear in the sidebar list: {ids:?}"
        );
    }

    #[tokio::test]
    async fn ensure_gui_session_persists_repository_id() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("db");
        let conv_id = db
            .chat_ensure_gui_session_with_repo("sess-repo-1", "Session 1", Some("abc123"))
            .await
            .expect("ensure session with repo");
        let mut rows = db
            .connection()
            .query(
                "SELECT repository_id FROM conversations WHERE id = ?1",
                turso::params![conv_id],
            )
            .await
            .expect("query");
        let row = rows.next().await.expect("row").expect("row present");
        let repo: Option<String> = row.get(0).expect("get repository_id");
        assert_eq!(repo.as_deref(), Some("abc123"));
    }

    #[tokio::test]
    async fn archive_conversation_is_recoverable_not_deleted() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("db");
        let conv_id = db
            .chat_ensure_gui_session("sess-1", "Session 1")
            .await
            .expect("ensure session");

        db.chat_archive_conversation(conv_id)
            .await
            .expect("archive");

        // Row must still exist.
        let mut rows = db
            .connection()
            .query(
                "SELECT archived_at FROM conversations WHERE id = ?1",
                turso::params![conv_id],
            )
            .await
            .expect("query");
        let row = rows
            .next()
            .await
            .expect("next")
            .expect("row survives archive");
        let archived_at: Option<String> = row.get(0).expect("get");
        assert!(archived_at.is_some(), "archived_at must be set");

        // Excluded from the default (non-archived) listing.
        let active = db
            .chat_list_gui_sessions(40, false)
            .await
            .expect("list active");
        assert!(!active.iter().any(|s| s.0 == conv_id));

        // Included when include_archived=true.
        let all = db.chat_list_gui_sessions(40, true).await.expect("list all");
        assert!(all.iter().any(|s| s.0 == conv_id));

        db.chat_unarchive_conversation(conv_id)
            .await
            .expect("unarchive");
        let active_again = db
            .chat_list_gui_sessions(40, false)
            .await
            .expect("list active again");
        assert!(active_again.iter().any(|s| s.0 == conv_id));
    }

    #[tokio::test]
    async fn archived_session_is_not_found_by_external_session_id_lookup() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("db");
        let conv_id = db
            .chat_ensure_gui_session("sess-resume-1", "Session 1")
            .await
            .expect("ensure session");
        db.chat_archive_conversation(conv_id)
            .await
            .expect("archive");

        // A resumed/deep-linked external_session_id must not find the archived row...
        let found = db
            .chat_find_gui_conversation_id("sess-resume-1")
            .await
            .expect("find");
        assert_eq!(
            found, None,
            "archived conversations must not be resurrected by find-or-create lookups"
        );

        // ...so calling chat_ensure_gui_session again creates a fresh row instead of reusing the archived one.
        let new_conv_id = db
            .chat_ensure_gui_session("sess-resume-1", "Session 1")
            .await
            .expect("re-ensure session");
        assert_ne!(
            new_conv_id, conv_id,
            "must create a new conversation, not resurrect the archived one"
        );
    }

    #[tokio::test]
    async fn unarchive_finds_an_archived_conversation_by_id() {
        let db = VoxDb::connect(DbConfig::Memory).await.unwrap();
        let conv_id = db
            .chat_ensure_gui_session("sess-unarchive-1", "S")
            .await
            .unwrap();
        db.chat_archive_conversation(conv_id).await.unwrap();

        let found = db
            .chat_find_gui_conversation_id_including_archived("sess-unarchive-1")
            .await
            .unwrap();
        assert_eq!(found, Some(conv_id));
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

    #[tokio::test]
    async fn latest_plan_session_id_for_origin_picks_the_most_recently_updated_row() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("db");
        db.create_plan_session("plan-old", Some("chat-z"), "goal one", "sequential")
            .await
            .unwrap();
        db.create_plan_session("plan-new", Some("chat-z"), "goal two", "sequential")
            .await
            .unwrap();
        // Touch plan-new again so its updated_at is later than plan-old's.
        db.update_plan_session_goal_text("plan-new", "goal two, revised")
            .await
            .unwrap();

        let latest = db
            .latest_plan_session_id_for_origin("chat-z")
            .await
            .unwrap();
        assert_eq!(latest.as_deref(), Some("plan-new"));
    }

    #[tokio::test]
    async fn latest_plan_session_id_for_origin_is_none_for_a_session_with_no_dispatched_goals() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("db");
        let latest = db
            .latest_plan_session_id_for_origin("chat-with-no-tasks")
            .await
            .unwrap();
        assert_eq!(latest, None);
    }

    #[tokio::test]
    async fn open_task_counts_join_on_origin_session_id_and_current_version() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("db");

        // Session "chat-a" has dispatched two goals -> two plan_sessions rows.
        db.create_plan_session("plan-a1", Some("chat-a"), "goal one", "sequential")
            .await
            .unwrap();
        db.append_plan_version("plan-a1", 1, None, None, None)
            .await
            .unwrap();
        db.upsert_plan_node("plan-a1", 1, "n1", "step", "[]", "{}", "pending", None)
            .await
            .unwrap();

        db.create_plan_session("plan-a2", Some("chat-a"), "goal two", "sequential")
            .await
            .unwrap();
        db.append_plan_version("plan-a2", 1, None, None, None)
            .await
            .unwrap();
        db.upsert_plan_node("plan-a2", 1, "n1", "step", "[]", "{}", "in_progress", None)
            .await
            .unwrap();

        // Session "chat-b" has one dispatched goal, already completed.
        db.create_plan_session("plan-b1", Some("chat-b"), "goal three", "sequential")
            .await
            .unwrap();
        db.append_plan_version("plan-b1", 1, None, None, None)
            .await
            .unwrap();
        db.upsert_plan_node("plan-b1", 1, "n1", "step", "[]", "{}", "completed", None)
            .await
            .unwrap();

        // Session "chat-c" has never dispatched anything.
        let counts = db
            .open_task_counts_for_sessions(&[
                "chat-a".to_string(),
                "chat-b".to_string(),
                "chat-c".to_string(),
            ])
            .await
            .unwrap();

        assert_eq!(
            counts.get("chat-a").copied(),
            Some(2),
            "sums across both of chat-a's plan_sessions rows"
        );
        assert_eq!(
            counts.get("chat-b"),
            None,
            "zero-count sessions are absent from the map, not present with 0"
        );
        assert_eq!(counts.get("chat-c"), None);
    }

    #[tokio::test]
    async fn open_task_counts_exclude_superseded_plan_versions() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("db");

        db.create_plan_session("plan-v1", Some("chat-v"), "goal", "sequential")
            .await
            .unwrap();
        db.append_plan_version("plan-v1", 1, None, None, None)
            .await
            .unwrap();
        db.upsert_plan_node("plan-v1", 1, "n1", "step", "[]", "{}", "pending", None)
            .await
            .unwrap();

        // Bump to version 2 — current_version moves to 2, so version 1's pending node must no
        // longer count (it belongs to a superseded version).
        db.append_plan_version("plan-v1", 2, Some(1), None, None)
            .await
            .unwrap();
        db.upsert_plan_node("plan-v1", 2, "n1", "step", "[]", "{}", "completed", None)
            .await
            .unwrap();

        let counts = db
            .open_task_counts_for_sessions(&["chat-v".to_string()])
            .await
            .unwrap();
        assert_eq!(
            counts.get("chat-v"),
            None,
            "version 1's pending node must not count once version 2 is current"
        );
    }
}
