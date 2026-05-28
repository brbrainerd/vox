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
