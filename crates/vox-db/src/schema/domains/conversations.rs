//! Arca SQL: Conversations, topics, and versions.
pub const SCHEMA_CONVERSATIONS: &str = "
CREATE TABLE IF NOT EXISTS conversations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
    title TEXT NOT NULL DEFAULT '',
    code_version TEXT,
    repository_id TEXT,
    external_session_id TEXT,
    thread_id TEXT,
    origin_surface TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS conversation_messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    conversation_id INTEGER NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    role TEXT NOT NULL,
    content_text TEXT NOT NULL DEFAULT '',
    payload_json TEXT,
    external_turn_id TEXT,
    model_used TEXT,
    token_count INTEGER,
    context_files_json TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- conversation_tool_calls: quarantined (Task 4) — see domains/quarantine.rs.

CREATE TABLE IF NOT EXISTS topics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    slug TEXT NOT NULL UNIQUE,
    label TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- conversation_topics, conversation_message_topics, conversation_versions,
-- conversation_edges, topic_evolution_events: quarantined (Task 4) — see
-- domains/quarantine.rs.

CREATE INDEX IF NOT EXISTS idx_conversations_user ON conversations(user_id);
CREATE INDEX IF NOT EXISTS idx_conversations_updated ON conversations(updated_at);
CREATE INDEX IF NOT EXISTS idx_conversations_repository ON conversations(repository_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_conversations_repo_ext_session ON conversations(repository_id, external_session_id)
    WHERE repository_id IS NOT NULL AND external_session_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_conversation_messages_conv ON conversation_messages(conversation_id);
CREATE INDEX IF NOT EXISTS idx_conversation_messages_created ON conversation_messages(conversation_id, created_at);
CREATE INDEX IF NOT EXISTS idx_conversation_messages_external_turn ON conversation_messages(external_turn_id);
CREATE INDEX IF NOT EXISTS idx_topics_label ON topics(label);
";
