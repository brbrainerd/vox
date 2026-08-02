use crate::types::AgentId;

use super::super::errors::SessionError;
use super::super::state::{Session, SessionEvent, now_secs};
use super::SessionManager;
use super::db_io::run_session_db_io;

impl SessionManager {
    /// Create a new `SessionManager` (file-only mode).
    pub fn new(config: super::super::config::SessionConfig) -> Result<Self, SessionError> {
        if config.persist {
            std::fs::create_dir_all(&config.sessions_dir)?;
        }
        Ok(Self {
            config,
            sessions: std::collections::HashMap::new(),
            db: None,
        })
    }

    /// Attach a VoxDb for dual-write session persistence (SSOT mode).
    pub fn with_db(mut self, db: std::sync::Arc<vox_db::VoxDb>) -> Self {
        self.db = Some(db);
        self
    }

    /// Set the db reference after construction.
    pub fn set_db(&mut self, db: std::sync::Arc<vox_db::VoxDb>) {
        self.db = Some(db);
    }

    /// Create a new session for the given agent and optional tenant. Persists immediately.
    pub fn create(
        &mut self,
        agent_id: AgentId,
        tenant_id: Option<String>,
    ) -> Result<String, SessionError> {
        if self.sessions.len() >= self.config.max_sessions {
            return Err(SessionError::MaxSessions(self.config.max_sessions));
        }
        let session = Session::new(agent_id, tenant_id.clone());
        let id = session.id.clone();

        if let Some(db) = &self.db {
            let db = db.clone();
            let sid = id.clone();
            let aid_str = agent_id.0.to_string();
            let tid = tenant_id.clone();
            let created_at = session.created_at;
            let event = SessionEvent::Created {
                session_id: id.clone(),
                agent_id: agent_id.0,
                tenant_id: tenant_id.clone(),
                created_at,
            };
            let payload = serde_json::to_string(&event).map_err(SessionError::Serialize)?;
            let meta = format!("{{\"agent_id\":\"{aid_str}\",\"state\":\"active\"}}");
            run_session_db_io(async move {
                db.create_session(&sid, &aid_str, tid.as_deref(), Some(meta.as_str()))
                    .await?;
                db.append_session_event(&sid, "created", &payload).await?;
                Ok(())
            })?;
        }

        self.sessions.insert(id.clone(), session);
        Ok(id)
    }

    /// Get a reference to a session by ID.
    pub fn get(&self, id: &str) -> Option<&super::super::state::Session> {
        self.sessions.get(id)
    }

    /// Get a mutable reference to a session by ID.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut super::super::state::Session> {
        self.sessions.get_mut(id)
    }

    /// Add a turn to a session and persist the event.
    pub fn add_turn(
        &mut self,
        session_id: &str,
        role: impl Into<String>,
        content: impl Into<String>,
        tokens: usize,
    ) -> Result<(), SessionError> {
        let content = content.into();
        let role = role.into();
        let at = now_secs();
        let event = SessionEvent::TurnAdded {
            role: role.clone(),
            content: content.clone(),
            tokens,
            at,
        };

        if let Some(db) = &self.db {
            let db = db.clone();
            let sid = session_id.to_string();
            let payload = serde_json::to_string(&event).map_err(SessionError::Serialize)?;
            run_session_db_io(async move {
                db.append_session_event(&sid, "turn_added", &payload).await
            })?;
        }

        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| SessionError::NotFound(session_id.to_string()))?;

        session.add_turn_at(role, content, tokens, at);
        Ok(())
    }

    /// Set metadata on a session.
    pub fn set_meta(
        &mut self,
        session_id: &str,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<(), SessionError> {
        let key = key.into();
        let value = value.into();
        let at = now_secs();
        let event = SessionEvent::MetaUpdated {
            key: key.clone(),
            value: value.clone(),
            at,
        };

        if let Some(db) = &self.db {
            let db = db.clone();
            let sid = session_id.to_string();
            let payload = serde_json::to_string(&event).map_err(SessionError::Serialize)?;
            run_session_db_io(async move {
                db.append_session_event(&sid, "meta_updated", &payload)
                    .await
            })?;
        }

        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| SessionError::NotFound(session_id.to_string()))?;

        session.set_meta(&key, &value);
        Ok(())
    }

    /// Set plugin state on a session.
    pub fn set_plugin_state(
        &mut self,
        session_id: &str,
        plugin_id: impl Into<String>,
        state: serde_json::Value,
    ) -> Result<(), SessionError> {
        let plugin_id = plugin_id.into();
        let at = now_secs();
        let event = SessionEvent::PluginStateUpdated {
            plugin_id: plugin_id.clone(),
            state: state.clone(),
            at,
        };

        if let Some(db) = &self.db {
            let db = db.clone();
            let sid = session_id.to_string();
            let payload = serde_json::to_string(&event).map_err(SessionError::Serialize)?;
            run_session_db_io(async move {
                db.append_session_event(&sid, "plugin_state_updated", &payload)
                    .await
            })?;
        }

        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| SessionError::NotFound(session_id.to_string()))?;

        session.set_plugin_state(&plugin_id, state);
        Ok(())
    }

    /// Reset a session (clear history but keep metadata).
    pub fn reset(&mut self, session_id: &str) -> Result<usize, SessionError> {
        let at = now_secs();
        let event = SessionEvent::Reset { at };

        if let Some(db) = &self.db {
            let db = db.clone();
            let sid = session_id.to_string();
            let payload = serde_json::to_string(&event).map_err(SessionError::Serialize)?;
            run_session_db_io(
                async move { db.append_session_event(&sid, "reset", &payload).await },
            )?;
        }

        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| SessionError::NotFound(session_id.to_string()))?;

        let cleared = session.reset();
        Ok(cleared)
    }

    /// Compact a session with a summary string.
    pub fn compact(&mut self, session_id: &str, summary: &str) -> Result<usize, SessionError> {
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| SessionError::NotFound(session_id.to_string()))?;

        let removed = session.compact(summary);
        let at = now_secs();
        let event = SessionEvent::Compacted {
            summary: summary.to_string(),
            turns_removed: removed,
            at,
        };

        if let Some(db) = &self.db {
            let db = db.clone();
            let sid = session_id.to_string();
            let payload = serde_json::to_string(&event).map_err(SessionError::Serialize)?;
            run_session_db_io(async move {
                db.append_session_event(&sid, "compacted", &payload).await
            })?;
        }
        Ok(removed)
    }

    /// Assemble the message list for an `llm_chat`/`llm_stream` call from a
    /// session's live turns, running an automatic compaction pass first when
    /// the session is over `engine`'s threshold.
    ///
    /// This is the T4.2 wiring point: message assembly for the LLM call
    /// **always** goes through this method (or an equivalent that calls
    /// [`super::super::state::Session::compact_auto`]) rather than reading
    /// `session.turns` directly, so a conversation driven over the context
    /// limit is compacted — with dropped turns archived losslessly — before
    /// the oversized history ever reaches the wire.
    ///
    /// Returns the assembled messages plus the [`crate::compaction::CompactionResult`]
    /// when a compaction pass actually ran (`None` when the session was under
    /// threshold and no compaction was needed).
    ///
    /// ## T4.2 follow-up review (2026-07-03): no production caller today
    ///
    /// An adversarial re-review confirmed this method has exactly one caller
    /// in the workspace: [`context_compaction_wiring_test`](../../../tests/context_compaction_wiring_test.rs)
    /// (the T4.2 acceptance test). This was audited, not assumed:
    ///
    /// - Every real production `llm_chat`/`llm_stream` call site in the
    ///   workspace was audited (`orchestrator/task_dispatch/submit/goal.rs`'s
    ///   CRAG relevance evaluator + LLM plan synthesizer, the identical plan
    ///   synthesizer in `orchestrator/task_dispatch/submit/dei_plan_materialize.rs`,
    ///   `vox-scientia/src/evidence_assist.rs`, and
    ///   `vox-cli/src/commands/model/eval.rs`) and every one is genuinely
    ///   single-shot, stateless prompt/response calls (one query + one
    ///   retrieved document -> one relevance word; one goal -> synthesized
    ///   plan nodes; `eval.rs` loops over fixtures but builds a fresh
    ///   one-message vec per iteration, never an accumulating conversation).
    ///   None accumulates conversation turns, and `Orchestrator` does not
    ///   hold a `SessionManager` at all — its `session_id: Option<String>`
    ///   params are Codex/plan-persistence correlation IDs, unrelated to
    ///   `SessionManager`'s own session-id space. Wiring these through
    ///   `assemble_llm_messages` would be a no-op at best (no accumulated
    ///   turns to compact) and a false semantic link at worst (conflating
    ///   two unrelated "session_id" concepts). Out of scope by design, not
    ///   by oversight.
    /// - `SessionManager` itself is held by `vox-orchestrator-mcp`'s
    ///   `ServerState` (a real, turn-accumulating session store used by
    ///   GUI/CLI-facing chat surfaces), but that crate has **zero**
    ///   `llm_chat`/`llm_stream` call sites — it's an MCP tool-dispatch
    ///   layer, not an LLM chat driver. The genuine multi-turn conversation
    ///   loop that should call `assemble_llm_messages` before its `llm_chat`
    ///   call does not exist yet in this codebase.
    ///
    /// Net: the mechanism is real, tested end-to-end (including a real
    /// `llm_chat` call over the compacted message list), and its data
    /// structure/losslessness properties are sound. What's still open is a
    /// production integration point, which requires a real multi-turn chat
    /// surface to exist first — building one is out of scope for a T4.2
    /// follow-up. Tracked for a future task once such a surface lands.
    #[cfg(feature = "runtime")]
    pub fn assemble_llm_messages(
        &mut self,
        session_id: &str,
        engine: &crate::compaction::CompactionEngine,
    ) -> Result<
        (
            Vec<vox_actor_runtime::llm::LlmChatMessage>,
            Option<crate::compaction::CompactionResult>,
        ),
        SessionError,
    > {
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| SessionError::NotFound(session_id.to_string()))?;

        let compaction_result = session.compact_auto(engine);

        if let Some(result) = &compaction_result {
            let at = now_secs();
            let event = SessionEvent::Compacted {
                summary: format!(
                    "auto-compacted {} turn(s) ({} -> {} tokens)",
                    result.dropped_count, result.tokens_before, result.tokens_after
                ),
                turns_removed: result.dropped_count,
                at,
            };
            if let Some(db) = &self.db {
                let db = db.clone();
                let sid = session_id.to_string();
                let payload = serde_json::to_string(&event).map_err(SessionError::Serialize)?;
                run_session_db_io(async move {
                    db.append_session_event(&sid, "compacted", &payload).await
                })?;
            }
        }

        let messages = session
            .turns
            .iter()
            .map(|t| vox_actor_runtime::llm::LlmChatMessage {
                role: t.role.clone(),
                content: t.content.clone(),
                ..Default::default()
            })
            .collect();

        Ok((messages, compaction_result))
    }

    /// Record an expensive op for the session and persist the event.
    pub fn record_expensive_op(&mut self, session_id: &str) -> Result<(), SessionError> {
        let at = now_secs();
        let event = SessionEvent::ExpensiveOpRecorded { at };

        if let Some(db) = &self.db {
            let db = db.clone();
            let sid = session_id.to_string();
            let payload = serde_json::to_string(&event).map_err(SessionError::Serialize)?;
            run_session_db_io(async move {
                db.append_session_event(&sid, "expensive_op_recorded", &payload)
                    .await
            })?;
        }

        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| SessionError::NotFound(session_id.to_string()))?;

        session.last_expensive_op_at = Some(at);
        session.last_active = at;
        Ok(())
    }
}
