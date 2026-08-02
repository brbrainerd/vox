//! Arca SQL: Knowledge graph, embeddings, and RAG search.
pub const SCHEMA_KNOWLEDGE: &str = "
CREATE TABLE IF NOT EXISTS knowledge_nodes (
    id TEXT PRIMARY KEY,
    label TEXT NOT NULL,
    content TEXT,
    node_type TEXT NOT NULL DEFAULT 'concept',
    media_url TEXT,
    media_type TEXT,
    metadata TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS knowledge_edges (
    src_id TEXT NOT NULL,
    dst_id TEXT NOT NULL,
    relation TEXT NOT NULL,
    weight REAL NOT NULL DEFAULT 1.0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (src_id, dst_id, relation)
);

CREATE TABLE IF NOT EXISTS snippets (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    language TEXT NOT NULL,
    title TEXT NOT NULL,
    code TEXT NOT NULL,
    description TEXT,
    tags TEXT,
    author_id TEXT,
    source_ref TEXT,
    embedding_ref TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS embeddings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source_type TEXT,
    source_id TEXT NOT NULL,
    dim INTEGER NOT NULL,
    vector BLOB NOT NULL,
    metadata TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS search_documents (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source_uri TEXT NOT NULL,
    title TEXT NOT NULL DEFAULT '',
    mime_type TEXT NOT NULL DEFAULT '',
    content_hash TEXT NOT NULL DEFAULT '',
    ingested_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_search_documents_uri ON search_documents(source_uri);
CREATE INDEX IF NOT EXISTS idx_search_documents_hash ON search_documents(content_hash);

CREATE TABLE IF NOT EXISTS search_document_chunks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    document_id INTEGER NOT NULL REFERENCES search_documents(id) ON DELETE CASCADE,
    chunk_index INTEGER NOT NULL,
    body_text TEXT NOT NULL,
    embedding_ref TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(document_id, chunk_index)
);

-- search_indexing_jobs: quarantined (DEAD, Task 4) — see domains/quarantine.rs.

CREATE INDEX IF NOT EXISTS idx_knowledge_edges_src ON knowledge_edges(src_id);
CREATE INDEX IF NOT EXISTS idx_knowledge_edges_dst ON knowledge_edges(dst_id);
CREATE INDEX IF NOT EXISTS idx_embeddings_source ON embeddings(source_type, source_id);
CREATE INDEX IF NOT EXISTS idx_embeddings_source_created ON embeddings(source_type, created_at);
CREATE INDEX IF NOT EXISTS idx_search_chunks_doc ON search_document_chunks(document_id);

-- Knowledge Base tables (VoxKB) ---------------------------------------------------
CREATE TABLE IF NOT EXISTS knowledge_bases (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    description TEXT NOT NULL DEFAULT '',
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    entry_count INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS kb_entries (
    id TEXT PRIMARY KEY,
    kb_id TEXT NOT NULL REFERENCES knowledge_bases(id) ON DELETE CASCADE,
    content TEXT NOT NULL,
    source_signal TEXT NOT NULL,
    source_ref TEXT,
    routing_confidence REAL NOT NULL DEFAULT 1.0,
    tags TEXT NOT NULL DEFAULT '[]',
    created_at_ms INTEGER NOT NULL,
    last_accessed_at_ms INTEGER,
    access_count INTEGER NOT NULL DEFAULT 0,
    accepted INTEGER NOT NULL DEFAULT 1,
    mens_queued INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS kb_routing_rules (
    id TEXT PRIMARY KEY,
    kb_id TEXT NOT NULL REFERENCES knowledge_bases(id) ON DELETE CASCADE,
    rule_type TEXT NOT NULL,
    pattern TEXT NOT NULL,
    priority INTEGER NOT NULL DEFAULT 0,
    created_at_ms INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_kb_entries_kb_id ON kb_entries(kb_id);
CREATE INDEX IF NOT EXISTS idx_kb_entries_source_signal ON kb_entries(source_signal);
CREATE INDEX IF NOT EXISTS idx_kb_entries_accepted ON kb_entries(accepted);
CREATE INDEX IF NOT EXISTS idx_kb_entries_mens_queued ON kb_entries(mens_queued, accepted);
CREATE INDEX IF NOT EXISTS idx_kb_routing_rules_kb_id ON kb_routing_rules(kb_id);
";
