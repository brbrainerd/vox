use std::collections::HashMap;

use std::path::PathBuf;

use crate::types::AgentId;

use super::super::errors::SessionError;
use super::super::state::{Session, SessionEvent, SessionState, SessionTurn, now_secs};
use super::SessionManager;

impl SessionManager {
    /// Load a session by ID from **Codex only** (`agent_sessions` + `agent_session_events`).
    ///
    /// When [`SessionManager`](super::SessionManager) has no [`VoxDb`](vox_db::VoxDb) handle, this
    /// returns [`SessionError::NotFound`]. There is no JSONL read path here; [`SessionConfig::sessions_dir`](super::super::config::SessionConfig::sessions_dir) is retained for legacy cleanup (e.g. removing stale files on archive).
    pub async fn load(&mut self, session_id: &str) -> Result<(), SessionError> {
        if let Some(db) = &self.db {
            let db = db.clone();
            let sid_owned = session_id.to_string();
            let row =
                db.get_agent_session_row(&sid_owned)
                    .await
                    .map_err(|e: vox_db::StoreError| {
                        SessionError::Io(std::io::Error::other(e.to_string()))
                    })?;

            if let Some((_, agent_id_str, _)) = row {
                let aid = agent_id_str.parse::<AgentId>().unwrap_or(AgentId(0));
                let mut session = Session {
                    id: session_id.to_string(),
                    agent_id: aid,
                    tenant_id: None,
                    state: SessionState::Active,
                    created_at: now_secs(), // Replaced by Create event if found
                    last_active: now_secs(),
                    last_expensive_op_at: None,
                    turns: Vec::new(),
                    meta: HashMap::new(),
                    plugin_state: HashMap::new(),
                    turn_count: 0,
                    total_tokens: 0,
                };

                let events =
                    db.load_session_events(session_id)
                        .await
                        .map_err(|e: vox_db::StoreError| {
                            SessionError::Io(std::io::Error::other(e.to_string()))
                        })?;

                for (_etype, payload_json) in events {
                    let event: SessionEvent =
                        serde_json::from_str(&payload_json).map_err(SessionError::Serialize)?;
                    self.apply_event_to_session(&mut session, event);
                }

                self.sessions.insert(session_id.to_string(), session);
                return Ok(());
            }
        }

        Err(SessionError::NotFound(session_id.to_string()))
    }

    /// Helper to apply an event to a session object.
    fn apply_event_to_session(&self, s: &mut Session, event: SessionEvent) {
        match event {
            SessionEvent::Created {
                created_at,
                agent_id,
                tenant_id,
                ..
            } => {
                s.created_at = created_at;
                s.agent_id = AgentId(agent_id);
                s.tenant_id = tenant_id;
            }
            SessionEvent::TurnAdded {
                role,
                content,
                tokens,
                at,
            } => {
                s.turns.push(SessionTurn {
                    role,
                    content,
                    tokens,
                    at,
                });
                s.turn_count += 1;
                s.total_tokens += tokens;
                s.last_active = at;
            }
            SessionEvent::StateChanged { to, at, .. } => {
                s.state = to;
                s.last_active = at;
            }
            SessionEvent::MetaUpdated { key, value, at } => {
                s.meta.insert(key, value);
                s.last_active = at;
            }
            SessionEvent::PluginStateUpdated {
                plugin_id,
                state,
                at,
            } => {
                s.plugin_state.insert(plugin_id, state);
                s.last_active = at;
            }
            SessionEvent::Reset { at } => {
                s.turns.clear();
                s.state = SessionState::Active;
                s.last_active = at;
            }
            SessionEvent::Compacted { summary, at, .. } => {
                let tokens = crate::compaction::CompactionEngine::estimate_tokens(&summary);
                s.turns.clear();
                s.turns.push(SessionTurn {
                    role: "system".to_string(),
                    content: format!("[compacted summary]\n{summary}"),
                    tokens,
                    at,
                });
                s.state = SessionState::Compacted;
                s.last_active = at;
            }
            SessionEvent::ExpensiveOpRecorded { at } => {
                s.last_expensive_op_at = Some(at);
                s.last_active = at;
            }
        }
    }

    pub async fn load_all(&mut self) -> Result<usize, SessionError> {
        let _ = std::hint::black_box(self as *mut _ as usize);
        Ok(0)
    }

    /// Path under [`SessionConfig::sessions_dir`](super::super::config::SessionConfig::sessions_dir) used for legacy `.jsonl` cleanup (e.g. [`super::lifecycle::SessionManager::cleanup`]).
    pub(super) fn session_path(&self, session_id: &str) -> PathBuf {
        self.config.sessions_dir.join(format!("{session_id}.jsonl"))
    }
}
