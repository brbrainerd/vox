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

-- conversation_topics, conversation_message_topics: quarantined (Task 4) —
-- see domains/quarantine.rs.

-- conversation_versions, conversation_edges, topic_evolution_events:
-- UN-quarantined 2026-08-02 — Task 4's classification for these three was
-- wrong. They're written via VoxDb::conversation_version_append /
-- conversation_edge_insert / topic_evolution_event_append
-- (crates/vox-db/src/codex_conversation_graph.rs), which are pure
-- delegation aliases over append_conversation_version /
-- insert_conversation_edge / append_topic_evolution_event
-- (store/ops_codex/codex_graph.rs, the functions the census tool actually
-- checked for external callers). vox-orchestrator-mcp's codex_tools.rs
-- calls the alias names, not the underlying ones, so the single-hop
-- wrapper-call detection never saw it — found only by a manual sweep before
-- running the existing-DB drop migration. See
-- docs/src/architecture/2026-08-01-voxdb-audit-condensation-plan.md's
-- Task 9 addendum for the full writeup.
CREATE TABLE IF NOT EXISTS conversation_versions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    conversation_id INTEGER NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    version_index INTEGER NOT NULL,
    label TEXT NOT NULL DEFAULT '',
    snapshot_json TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(conversation_id, version_index)
);
CREATE INDEX IF NOT EXISTS idx_conversation_versions_conv ON conversation_versions(conversation_id);

CREATE TABLE IF NOT EXISTS conversation_edges (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    from_conversation_id INTEGER NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    to_conversation_id INTEGER NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    edge_kind TEXT NOT NULL DEFAULT 'related',
    weight REAL NOT NULL DEFAULT 1.0,
    metadata_json TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_conversation_edges_from ON conversation_edges(from_conversation_id);
CREATE INDEX IF NOT EXISTS idx_conversation_edges_to ON conversation_edges(to_conversation_id);

CREATE TABLE IF NOT EXISTS topic_evolution_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    topic_id INTEGER NOT NULL REFERENCES topics(id) ON DELETE CASCADE,
    event_kind TEXT NOT NULL,
    prior_label TEXT,
    new_label TEXT,
    detail_json TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_topic_evolution_topic_created ON topic_evolution_events(topic_id, created_at);

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
