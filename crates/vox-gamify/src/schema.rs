//! V5 database schema: Gamification tables.
//!
//! Extends the existing vox-pm schema with tables for player profiles,
//! companions, quests, and battles.

/// V5 migration SQL — gamification tables.
///
/// All tables reference `users(id)` from V3, but use TEXT foreign keys
/// rather than enforcing FK constraints (user may be a local-only profile).
pub const SCHEMA_V5: &str = "
-- ── Gamification: Player Profiles ────────────────────────

CREATE TABLE IF NOT EXISTS gamify_profiles (
    user_id TEXT PRIMARY KEY,
    level INTEGER NOT NULL DEFAULT 1,
    xp INTEGER NOT NULL DEFAULT 0,
    crystals INTEGER NOT NULL DEFAULT 100,
    energy INTEGER NOT NULL DEFAULT 100,
    max_energy INTEGER NOT NULL DEFAULT 100,
    last_energy_regen TEXT NOT NULL DEFAULT (datetime('now')),
    last_active TEXT NOT NULL DEFAULT (datetime('now')),
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- ── Gamification: Companions ─────────────────────────────

CREATE TABLE IF NOT EXISTS gamify_companions (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    code_hash TEXT,
    language TEXT NOT NULL DEFAULT 'vox',
    ascii_sprite TEXT,
    mood TEXT NOT NULL DEFAULT 'neutral',
    health INTEGER NOT NULL DEFAULT 100,
    max_health INTEGER NOT NULL DEFAULT 100,
    energy INTEGER NOT NULL DEFAULT 100,
    max_energy INTEGER NOT NULL DEFAULT 100,
    code_quality INTEGER NOT NULL DEFAULT 50,
    last_active TEXT NOT NULL DEFAULT (datetime('now')),
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_gamify_companions_user ON gamify_companions(user_id);

-- ── Gamification: Daily Quests ───────────────────────────

CREATE TABLE IF NOT EXISTS gamify_quests (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    quest_type TEXT NOT NULL,
    description TEXT NOT NULL,
    target INTEGER NOT NULL DEFAULT 1,
    progress INTEGER NOT NULL DEFAULT 0,
    crystal_reward INTEGER NOT NULL DEFAULT 10,
    xp_reward INTEGER NOT NULL DEFAULT 15,
    completed INTEGER NOT NULL DEFAULT 0,
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_gamify_quests_user ON gamify_quests(user_id);
CREATE INDEX IF NOT EXISTS idx_gamify_quests_expires ON gamify_quests(expires_at);

-- ── Gamification: Bug Battles ────────────────────────────

CREATE TABLE IF NOT EXISTS gamify_battles (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    companion_id TEXT NOT NULL,
    bug_type TEXT NOT NULL,
    bug_description TEXT NOT NULL,
    bug_code TEXT,
    submitted_code TEXT,
    success INTEGER NOT NULL DEFAULT 0,
    crystals_earned INTEGER NOT NULL DEFAULT 0,
    xp_earned INTEGER NOT NULL DEFAULT 0,
    duration_secs INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_gamify_battles_user ON gamify_battles(user_id);
CREATE INDEX IF NOT EXISTS idx_gamify_battles_companion ON gamify_battles(companion_id);
";

/// V6 migration SQL — multi-agent orchestration tables.
///
/// Adds tables for tracking agent sessions, events, costs,
/// inter-agent messages, and aggregated metrics.
pub const SCHEMA_V6: &str = "
-- ── Multi-Agent: Sessions ────────────────────────────────

CREATE TABLE IF NOT EXISTS agent_sessions (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    agent_name TEXT,
    started_at TEXT NOT NULL DEFAULT (datetime('now')),
    ended_at TEXT,
    status TEXT NOT NULL DEFAULT 'active',
    task_snapshot TEXT,
    context_summary TEXT
);

CREATE INDEX IF NOT EXISTS idx_agent_sessions_agent ON agent_sessions(agent_id);
CREATE INDEX IF NOT EXISTS idx_agent_sessions_status ON agent_sessions(status);

-- ── Multi-Agent: Events (timeline) ───────────────────────

CREATE TABLE IF NOT EXISTS agent_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    agent_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    payload TEXT,
    timestamp TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_agent_events_agent ON agent_events(agent_id);
CREATE INDEX IF NOT EXISTS idx_agent_events_type ON agent_events(event_type);
CREATE INDEX IF NOT EXISTS idx_agent_events_ts ON agent_events(timestamp);

-- ── Multi-Agent: Cost Records ────────────────────────────

CREATE TABLE IF NOT EXISTS cost_records (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    agent_id TEXT NOT NULL,
    session_id TEXT,
    provider TEXT NOT NULL,
    model TEXT,
    input_tokens INTEGER NOT NULL DEFAULT 0,
    output_tokens INTEGER NOT NULL DEFAULT 0,
    cost_usd REAL NOT NULL DEFAULT 0.0,
    timestamp TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_cost_records_agent ON cost_records(agent_id);
CREATE INDEX IF NOT EXISTS idx_cost_records_session ON cost_records(session_id);
CREATE INDEX IF NOT EXISTS idx_cost_records_ts ON cost_records(timestamp);

-- ── Multi-Agent: A2A Messages ────────────────────────────

CREATE TABLE IF NOT EXISTS a2a_messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    sender TEXT NOT NULL,
    receiver TEXT,
    msg_type TEXT NOT NULL,
    payload TEXT,
    correlation_id TEXT,
    acknowledged INTEGER NOT NULL DEFAULT 0,
    timestamp TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_a2a_messages_sender ON a2a_messages(sender);
CREATE INDEX IF NOT EXISTS idx_a2a_messages_receiver ON a2a_messages(receiver);
CREATE INDEX IF NOT EXISTS idx_a2a_messages_type ON a2a_messages(msg_type);

-- ── Multi-Agent: Aggregated Metrics ──────────────────────

CREATE TABLE IF NOT EXISTS agent_metrics (
    agent_id TEXT NOT NULL,
    metric_name TEXT NOT NULL,
    metric_value REAL NOT NULL DEFAULT 0.0,
    period TEXT NOT NULL DEFAULT 'session',
    timestamp TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (agent_id, metric_name, period)
);
";
