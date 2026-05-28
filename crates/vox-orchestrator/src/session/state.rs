//! Session record types and in-memory session behavior.

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::types::AgentId;

pub(crate) fn now_secs() -> u64 {
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

/// An event appended to the session db row.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum SessionEvent {
    Created {
        session_id: String,
        agent_id: u64,
        tenant_id: Option<String>,
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
    ExpensiveOpRecorded {
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
    #[serde(default)]
    pub tenant_id: Option<String>,
    pub state: SessionState,
    pub created_at: u64,
    pub last_active: u64,
    #[serde(default)]
    pub last_expensive_op_at: Option<u64>,
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
    /// Create a new session for the given agent and optional tenant.
    pub fn new(agent_id: AgentId, tenant_id: Option<String>) -> Self {
        let now = now_secs();
        Self {
            id: new_session_id(),
            agent_id,
            tenant_id,
            state: SessionState::Active,
            created_at: now,
            last_active: now,
            last_expensive_op_at: None,
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
        self.add_turn_at(role, content, tokens, now_secs());
    }

    /// Add a turn with an explicit timestamp (matches persisted event ordering).
    pub(crate) fn add_turn_at(
        &mut self,
        role: impl Into<String>,
        content: impl Into<String>,
        tokens: usize,
        at: u64,
    ) {
        let turn = SessionTurn {
            role: role.into(),
            content: content.into(),
            tokens,
            at,
        };
        self.turn_count += 1;
        self.total_tokens += tokens;
        self.last_active = at;
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

    /// Record that an expensive operation occurred during this session.
    pub fn record_expensive_op(&mut self) {
        self.last_expensive_op_at = Some(now_secs());
        self.last_active = now_secs();
    }

    /// Seconds since the last expensive operation in this session, if any.
    pub fn expensive_op_age_secs(&self) -> Option<u64> {
        self.last_expensive_op_at
            .map(|t| now_secs().saturating_sub(t))
    }

    /// Produces a summary string summarizing temporal freshness of the session.
    pub fn temporal_summary(&self) -> String {
        let idle = now_secs().saturating_sub(self.last_active);
        let exp = self
            .expensive_op_age_secs()
            .map(|s| format!("{}s ago", s))
            .unwrap_or_else(|| "never".to_string());
        format!(
            "Session active. Last user interaction: {}s ago. Last expensive operation (compile/index): {}.",
            idle, exp
        )
    }
}
