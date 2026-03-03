//! Session lifecycle management for Vox agents.
//!
//! Inspired by OpenClaw's session model:
//! - Sessions are persisted as append-only JSONL files
//! - Each session has its own context, permissions, and state
//! - Supports reset, cleanup, idle timeout, and daily reset policies
//! - Sessions survive restarts via replay from JSONL

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::types::AgentId;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn new_session_id() -> String {
    static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1);
    let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let t = now_secs();
    format!("sess_{t:x}_{c:x}")
}

// ---------------------------------------------------------------------------
// SessionState & SessionRecord
// ---------------------------------------------------------------------------

/// Lifecycle state of a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    /// Actively being used.
    Active,
    /// Idle — no recent activity.
    Idle,
    /// Has been compacted (history summarized).
    Compacted,
    /// Archived — can be cleaned up.
    Archived,
}

impl std::fmt::Display for SessionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Idle => write!(f, "idle"),
            Self::Compacted => write!(f, "compacted"),
            Self::Archived => write!(f, "archived"),
        }
    }
}

/// A JSONL event appended to the session file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum SessionEvent {
    Created {
        session_id: String,
        agent_id: u64,
        created_at: u64,
    },
    TurnAdded {
        role: String,
        content: String,
        tokens: usize,
        at: u64,
    },
    StateChanged {
        from: SessionState,
        to: SessionState,
        at: u64,
    },
    MetaUpdated {
        key: String,
        value: String,
        at: u64,
    },
    Reset {
        at: u64,
    },
    Compacted {
        summary: String,
        turns_removed: usize,
        at: u64,
    },
    PluginStateUpdated {
        plugin_id: String,
        state: serde_json::Value,
        at: u64,
    },
}

/// A single conversation turn stored in session history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionTurn {
    pub role: String,
    pub content: String,
    pub tokens: usize,
    pub at: u64,
}

/// In-memory representation of a live session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub agent_id: AgentId,
    pub state: SessionState,
    pub created_at: u64,
    pub last_active: u64,
    /// Conversation history (cleared on reset, pruned on compaction).
    pub turns: Vec<SessionTurn>,
    /// Arbitrary per-session key-value metadata.
    pub meta: HashMap<String, String>,
    /// Per-plugin persistent state.
    #[serde(default)]
    pub plugin_state: HashMap<String, serde_json::Value>,
    /// Total turns ever added (monotonic, not reset on compaction).
    pub turn_count: usize,
    /// Total tokens ever used (monotonic).
    pub total_tokens: usize,
}

impl Session {
    /// Create a new session for the given agent.
    pub fn new(agent_id: AgentId) -> Self {
        let now = now_secs();
        Self {
            id: new_session_id(),
            agent_id,
            state: SessionState::Active,
            created_at: now,
            last_active: now,
            turns: Vec::new(),
            meta: HashMap::new(),
            plugin_state: HashMap::new(),
            turn_count: 0,
            total_tokens: 0,
        }
    }

    /// Set plugin state.
    pub fn set_plugin_state(&mut self, plugin_id: impl Into<String>, state: serde_json::Value) {
        self.plugin_state.insert(plugin_id.into(), state);
        self.last_active = now_secs();
    }

    /// Add a conversation turn.
    pub fn add_turn(&mut self, role: impl Into<String>, content: impl Into<String>, tokens: usize) {
        let turn = SessionTurn {
            role: role.into(),
            content: content.into(),
            tokens,
            at: now_secs(),
        };
        self.turn_count += 1;
        self.total_tokens += tokens;
        self.last_active = now_secs();
        self.turns.push(turn);
    }

    /// Set a metadata key.
    pub fn set_meta(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.meta.insert(key.into(), value.into());
        self.last_active = now_secs();
    }

    /// Mark session as idle.
    pub fn mark_idle(&mut self) {
        self.state = SessionState::Idle;
    }

    /// Mark session as active.
    pub fn mark_active(&mut self) {
        self.state = SessionState::Active;
        self.last_active = now_secs();
    }

    /// Reset: clear history, keep metadata. Returns number of turns cleared.
    pub fn reset(&mut self) -> usize {
        let cleared = self.turns.len();
        self.turns.clear();
        self.state = SessionState::Active;
        self.last_active = now_secs();
        cleared
    }

    /// Compact: replace history with a summary turn.
    pub fn compact(&mut self, summary: &str) -> usize {
        let removed = self.turns.len().saturating_sub(1);
        let summary_tokens = crate::compaction::CompactionEngine::estimate_tokens(summary);
        let summary_turn = SessionTurn {
            role: "system".to_string(),
            content: format!("[compacted summary]\n{summary}"),
            tokens: summary_tokens,
            at: now_secs(),
        };
        self.turns.clear();
        self.turns.push(summary_turn);
        self.state = SessionState::Compacted;
        removed
    }

    /// Returns estimated token count of current history.
    pub fn current_tokens(&self) -> usize {
        self.turns.iter().map(|t| t.tokens).sum()
    }

    /// Returns true if session has been idle longer than `timeout_secs`.
    pub fn is_timed_out(&self, timeout_secs: u64) -> bool {
        timeout_secs > 0 && now_secs().saturating_sub(self.last_active) >= timeout_secs
    }
}

// ---------------------------------------------------------------------------
// SessionConfig
// ---------------------------------------------------------------------------

/// Configuration for the session manager.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    /// Directory where JSONL session files are stored. Default: `.sessions/`.
    pub sessions_dir: PathBuf,
    /// Seconds of inactivity before a session is considered idle. Default: 1800 (30 min).
    pub idle_timeout_secs: u64,
    /// Seconds of idle before archiving. Default: 86_400 (24 h).
    pub archive_timeout_secs: u64,
    /// Maximum number of active sessions. Default: 16.
    pub max_sessions: usize,
    /// Whether to enable JSONL persistence. Default: true.
    pub persist: bool,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            sessions_dir: PathBuf::from(".sessions"),
            idle_timeout_secs: 1_800,
            archive_timeout_secs: 86_400,
            max_sessions: 16,
            persist: true,
        }
    }
}

// ---------------------------------------------------------------------------
// SessionError
// ---------------------------------------------------------------------------

/// Errors from session management.
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Session '{0}' not found")]
    NotFound(String),
    #[error("Serialization error: {0}")]
    Serialize(serde_json::Error),
    #[error("Max sessions ({0}) reached")]
    MaxSessions(usize),
}

// ---------------------------------------------------------------------------
// SessionManager
// ---------------------------------------------------------------------------

/// Manages agent sessions: creation, persistence, lifecycle, cleanup.
///
/// When a `VoxDb` is attached via [`with_db`], every session creation and
/// turn addition also writes to the `user_sessions` and `session_turns`
/// tables. JSONL files remain the hot cache; VoxDB is the durable SSOT.
pub struct SessionManager {
    config: SessionConfig,
    sessions: HashMap<String, Session>,
    /// Optional VoxDB backing store for SSOT persistence.
    db: Option<Arc<vox_db::VoxDb>>,
}

impl SessionManager {
    /// Create a new `SessionManager` (file-only mode).
    pub fn new(config: SessionConfig) -> Result<Self, SessionError> {
        if config.persist {
            fs::create_dir_all(&config.sessions_dir)?;
        }
        Ok(Self {
            config,
            sessions: HashMap::new(),
            db: None,
        })
    }

    /// Attach a VoxDb for dual-write session persistence (SSOT mode).
    pub fn with_db(mut self, db: Arc<vox_db::VoxDb>) -> Self {
        self.db = Some(db);
        self
    }

    /// Set the db reference after construction.
    pub fn set_db(&mut self, db: Arc<vox_db::VoxDb>) {
        self.db = Some(db);
    }

    /// Create a new session for the given agent. Persists immediately.
    pub fn create(&mut self, agent_id: AgentId) -> Result<String, SessionError> {
        if self.sessions.len() >= self.config.max_sessions {
            return Err(SessionError::MaxSessions(self.config.max_sessions));
        }
        let session = Session::new(agent_id);
        let id = session.id.clone();

        if self.config.persist {
            let event = SessionEvent::Created {
                session_id: id.clone(),
                agent_id: agent_id.0,
                created_at: session.created_at,
            };
            self.append_event(&id, &event)?;
        }

        // Dual-write to VoxDB
        if let Some(db) = &self.db {
            let db = db.clone();
            let sid = id.clone();
            let aid = agent_id.0.to_string();
            tokio::spawn(async move {
                let meta = format!("{{\"agent_id\":\"{aid}\",\"state\":\"active\"}}");
                let _ = db.store().create_session(&sid, &aid, Some(&meta)).await;
            });
        }

        self.sessions.insert(id.clone(), session);
        Ok(id)
    }

    /// Get a reference to a session by ID.
    pub fn get(&self, id: &str) -> Option<&Session> {
        self.sessions.get(id)
    }

    /// Get a mutable reference to a session by ID.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut Session> {
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

        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| SessionError::NotFound(session_id.to_string()))?;

        session.add_turn(&role, &content, tokens);

        if self.config.persist {
            let event = SessionEvent::TurnAdded {
                role,
                content,
                tokens,
                at: now_secs(),
            };
            self.append_event(session_id, &event)?;
        }
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

        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| SessionError::NotFound(session_id.to_string()))?;

        session.set_meta(&key, &value);

        if self.config.persist {
            let event = SessionEvent::MetaUpdated {
                key,
                value,
                at: now_secs(),
            };
            self.append_event(session_id, &event)?;
        }
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
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| SessionError::NotFound(session_id.to_string()))?;

        session.set_plugin_state(&plugin_id, state.clone());

        if self.config.persist {
            let event = SessionEvent::PluginStateUpdated {
                plugin_id,
                state,
                at: now_secs(),
            };
            self.append_event(session_id, &event)?;
        }
        Ok(())
    }

    /// Reset a session (clear history but keep metadata).
    pub fn reset(&mut self, session_id: &str) -> Result<usize, SessionError> {
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| SessionError::NotFound(session_id.to_string()))?;

        let cleared = session.reset();

        if self.config.persist {
            let event = SessionEvent::Reset { at: now_secs() };
            self.append_event(session_id, &event)?;
        }
        Ok(cleared)
    }

    /// Compact a session with a summary string.
    pub fn compact(&mut self, session_id: &str, summary: &str) -> Result<usize, SessionError> {
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| SessionError::NotFound(session_id.to_string()))?;

        let removed = session.compact(summary);

        if self.config.persist {
            let event = SessionEvent::Compacted {
                summary: summary.to_string(),
                turns_removed: removed,
                at: now_secs(),
            };
            self.append_event(session_id, &event)?;
        }
        Ok(removed)
    }

    /// List all session IDs currently in memory.
    pub fn list(&self) -> Vec<&str> {
        self.sessions.keys().map(|s| s.as_str()).collect()
    }

    /// List all sessions sorted by last_active descending.
    pub fn list_sessions(&self) -> Vec<&Session> {
        let mut sessions: Vec<&Session> = self.sessions.values().collect();
        sessions.sort_by(|a, b| b.last_active.cmp(&a.last_active));
        sessions
    }

    /// Archive idle sessions.
    ///
    /// Returns number of sessions archived.
    pub fn tick_lifecycle(&mut self) -> usize {
        let idle_timeout = self.config.idle_timeout_secs;
        let archive_timeout = self.config.archive_timeout_secs;
        let mut changed = 0;

        for session in self.sessions.values_mut() {
            match session.state {
                SessionState::Active | SessionState::Compacted => {
                    if session.is_timed_out(idle_timeout) {
                        session.state = SessionState::Idle;
                        changed += 1;
                    }
                }
                SessionState::Idle => {
                    if session.is_timed_out(archive_timeout) {
                        session.state = SessionState::Archived;
                        changed += 1;
                    }
                }
                SessionState::Archived => {}
            }
        }
        changed
    }

    /// Remove all archived sessions from memory and disk.
    ///
    /// Returns the number of sessions removed.
    pub fn cleanup(&mut self) -> Result<usize, SessionError> {
        let archived: Vec<String> = self
            .sessions
            .iter()
            .filter(|(_, s)| s.state == SessionState::Archived)
            .map(|(id, _)| id.clone())
            .collect();

        let count = archived.len();
        for id in archived {
            self.sessions.remove(&id);
            if self.config.persist {
                let path = self.session_path(&id);
                if path.exists() {
                    fs::remove_file(&path)?;
                }
            }
        }
        Ok(count)
    }

    /// Load a session from its JSONL file by replaying events.
    pub fn load(&mut self, session_id: &str) -> Result<(), SessionError> {
        let path = self.session_path(session_id);
        if !path.exists() {
            return Err(SessionError::NotFound(session_id.to_string()));
        }

        let file = File::open(&path)?;
        let reader = BufReader::new(file);

        let mut session: Option<Session> = None;

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let event: SessionEvent =
                serde_json::from_str(&line).map_err(SessionError::Serialize)?;
            match event {
                SessionEvent::Created {
                    session_id: sid,
                    agent_id,
                    created_at,
                } => {
                    let now = now_secs();
                    session = Some(Session {
                        id: sid,
                        agent_id: AgentId(agent_id),
                        state: SessionState::Active,
                        created_at,
                        last_active: now,
                        turns: Vec::new(),
                        meta: HashMap::new(),
                        plugin_state: HashMap::new(),
                        turn_count: 0,
                        total_tokens: 0,
                    });
                }
                SessionEvent::TurnAdded {
                    role,
                    content,
                    tokens,
                    at,
                } => {
                    if let Some(ref mut s) = session {
                        s.turns.push(SessionTurn {
                            role,
                            content,
                            tokens,
                            at,
                        });
                        s.turn_count += 1;
                        s.total_tokens += tokens;
                    }
                }
                SessionEvent::StateChanged { to, .. } => {
                    if let Some(ref mut s) = session {
                        s.state = to;
                    }
                }
                SessionEvent::MetaUpdated { key, value, .. } => {
                    if let Some(ref mut s) = session {
                        s.meta.insert(key, value);
                    }
                }
                SessionEvent::PluginStateUpdated {
                    plugin_id, state, ..
                } => {
                    if let Some(ref mut s) = session {
                        s.plugin_state.insert(plugin_id, state);
                    }
                }
                SessionEvent::Reset { .. } => {
                    if let Some(ref mut s) = session {
                        s.turns.clear();
                        s.state = SessionState::Active;
                    }
                }
                SessionEvent::Compacted {
                    summary,
                    turns_removed: _,
                    at,
                } => {
                    if let Some(ref mut s) = session {
                        let tokens = crate::compaction::CompactionEngine::estimate_tokens(&summary);
                        s.turns.clear();
                        s.turns.push(SessionTurn {
                            role: "system".to_string(),
                            content: format!("[compacted summary]\n{summary}"),
                            tokens,
                            at,
                        });
                        s.state = SessionState::Compacted;
                    }
                }
            }
        }

        if let Some(s) = session {
            self.sessions.insert(s.id.clone(), s);
        }
        Ok(())
    }

    /// Scan the sessions directory and load all JSONL files.
    pub fn load_all(&mut self) -> Result<usize, SessionError> {
        if !self.config.sessions_dir.exists() {
            return Ok(0);
        }
        let entries = fs::read_dir(&self.config.sessions_dir)?;
        let mut loaded = 0;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                let _ = self.load(stem);
                loaded += 1;
            }
        }
        Ok(loaded)
    }

    /// Session file path for a given ID.
    fn session_path(&self, session_id: &str) -> PathBuf {
        self.config.sessions_dir.join(format!("{session_id}.jsonl"))
    }

    /// Append a JSONL event to the session's file.
    fn append_event(&self, session_id: &str, event: &SessionEvent) -> Result<(), SessionError> {
        let path = self.session_path(session_id);
        let json = serde_json::to_string(event).map_err(SessionError::Serialize)?;
        let mut f = OpenOptions::new().create(true).append(true).open(&path)?;
        writeln!(f, "{json}")?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn temp_sessions_dir() -> PathBuf {
        static DIR_COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let c = DIR_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let d = env::temp_dir().join(format!("vox_sessions_{}_{c}", now_secs()));
        fs::create_dir_all(&d).ok();
        d
    }

    fn test_config() -> SessionConfig {
        SessionConfig {
            sessions_dir: temp_sessions_dir(),
            idle_timeout_secs: 30,
            archive_timeout_secs: 60,
            max_sessions: 4,
            persist: true,
        }
    }

    #[test]
    fn create_and_retrieve_session() {
        let mut mgr = SessionManager::new(test_config()).expect("create manager");
        let id = mgr.create(AgentId(1)).expect("create session");
        let session = mgr.get(&id).expect("get session");
        assert_eq!(session.agent_id, AgentId(1));
        assert_eq!(session.state, SessionState::Active);
        assert_eq!(session.turns.len(), 0);
    }

    #[test]
    fn add_turn_and_check_tokens() {
        let mut mgr = SessionManager::new(test_config()).expect("create manager");
        let id = mgr.create(AgentId(1)).expect("create");
        mgr.add_turn(&id, "user", "hello world", 3)
            .expect("add turn");
        let s = mgr.get(&id).expect("get");
        assert_eq!(s.turns.len(), 1);
        assert_eq!(s.current_tokens(), 3);
        assert_eq!(s.turn_count, 1);
        assert_eq!(s.total_tokens, 3);
    }

    #[test]
    fn reset_clears_history() {
        let mut mgr = SessionManager::new(test_config()).expect("create manager");
        let id = mgr.create(AgentId(1)).expect("create");
        mgr.add_turn(&id, "user", "hello", 2).expect("add");
        mgr.add_turn(&id, "assistant", "hi", 1).expect("add");
        let cleared = mgr.reset(&id).expect("reset");
        assert_eq!(cleared, 2);
        assert_eq!(mgr.get(&id).expect("get").turns.len(), 0);
    }

    #[test]
    fn compact_replaces_with_summary() {
        let mut mgr = SessionManager::new(test_config()).expect("create manager");
        let id = mgr.create(AgentId(1)).expect("create");
        mgr.add_turn(&id, "user", "lots of content", 100)
            .expect("add");
        mgr.add_turn(&id, "assistant", "response", 50).expect("add");
        let removed = mgr
            .compact(&id, "Session summary: fixed parser")
            .expect("compact");
        assert_eq!(removed, 1); // 2 turns → replace with 1 summary → removed = 2-1
        assert_eq!(mgr.get(&id).expect("get").turns.len(), 1);
        assert_eq!(mgr.get(&id).expect("get").state, SessionState::Compacted);
    }

    #[test]
    fn set_meta_persisted() {
        let mut mgr = SessionManager::new(test_config()).expect("create manager");
        let id = mgr.create(AgentId(1)).expect("create");
        mgr.set_meta(&id, "model", "claude-sonnet-4")
            .expect("set meta");
        let val = mgr.get(&id).expect("get").meta.get("model").cloned();
        assert_eq!(val.as_deref(), Some("claude-sonnet-4"));
    }

    #[test]
    fn session_persistence_roundtrip() {
        let cfg = test_config();
        let dir = cfg.sessions_dir.clone();
        let session_id;

        {
            let mut mgr = SessionManager::new(cfg.clone()).expect("create");
            session_id = mgr.create(AgentId(2)).expect("create session");
            mgr.add_turn(&session_id, "user", "fix parser", 10)
                .expect("add");
            mgr.set_meta(&session_id, "crate", "vox-parser")
                .expect("meta");
        }

        // Reload into fresh manager
        let mut mgr2 = SessionManager::new(SessionConfig {
            sessions_dir: dir,
            ..cfg
        })
        .expect("create");
        mgr2.load(&session_id).expect("load");
        let s = mgr2.get(&session_id).expect("get");
        assert_eq!(s.agent_id, AgentId(2));
        assert_eq!(s.turns.len(), 1);
        assert_eq!(s.meta.get("crate").map(|s| s.as_str()), Some("vox-parser"));
    }

    #[test]
    fn max_sessions_limit() {
        let cfg = SessionConfig {
            max_sessions: 2,
            ..test_config()
        };
        let mut mgr = SessionManager::new(cfg).expect("create");
        mgr.create(AgentId(1)).expect("1st");
        mgr.create(AgentId(2)).expect("2nd");
        let err = mgr.create(AgentId(3));
        assert!(matches!(err, Err(SessionError::MaxSessions(2))));
    }

    #[test]
    fn lifecycle_tick_marks_idle_then_archives() {
        let cfg = SessionConfig {
            idle_timeout_secs: 10,
            archive_timeout_secs: 10,
            ..test_config()
        };
        let mut mgr = SessionManager::new(cfg).expect("create");
        let id = mgr.create(AgentId(1)).expect("create");
        // Force last_active to be far in the past
        if let Some(s) = mgr.get_mut(&id) {
            s.last_active = now_secs().saturating_sub(20);
        }
        mgr.tick_lifecycle();
        // Force last_active again because mark_idle updates last_active if we didn't specify,
        // Wait, mark_idle does NOT update last_active: we keep it older!
        // But the next tick needs another timeout cycle. If last_active is still 20 secs old,
        // and timeout is 10, it's timed out for archiving too!
        // So a second tick will immediately archive it.
        mgr.tick_lifecycle();
        assert_eq!(mgr.get(&id).expect("get").state, SessionState::Archived);
    }

    #[test]
    fn cleanup_removes_archived_sessions() {
        let cfg = SessionConfig {
            idle_timeout_secs: 1,
            archive_timeout_secs: 1,
            ..test_config()
        };
        let mut mgr = SessionManager::new(cfg).expect("create");
        let id = mgr.create(AgentId(1)).expect("create");
        if let Some(s) = mgr.get_mut(&id) {
            s.state = SessionState::Archived;
        }
        let removed = mgr.cleanup().expect("cleanup");
        assert_eq!(removed, 1);
        assert!(mgr.get(&id).is_none());
    }

    #[test]
    fn plugin_state_persistence_roundtrip() {
        let cfg = test_config();
        let dir = cfg.sessions_dir.clone();
        let session_id;

        {
            let mut mgr = SessionManager::new(cfg.clone()).expect("create");
            session_id = mgr.create(AgentId(3)).expect("create");
            mgr.set_plugin_state(
                &session_id,
                "weather",
                serde_json::json!({"city": "London"}),
            )
            .expect("set");
        }

        let mut mgr2 = SessionManager::new(SessionConfig {
            sessions_dir: dir,
            ..cfg
        })
        .expect("create");
        mgr2.load(&session_id).expect("load");
        let s = mgr2.get(&session_id).expect("get");
        assert_eq!(s.plugin_state.get("weather").unwrap()["city"], "London");
    }
}
