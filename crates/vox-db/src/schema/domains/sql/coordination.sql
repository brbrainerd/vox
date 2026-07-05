CREATE TABLE IF NOT EXISTS distributed_locks (
    lock_key      TEXT    NOT NULL,
    holder_node   TEXT    NOT NULL,
    holder_agent  TEXT    NOT NULL,
    fence_token   INTEGER NOT NULL,
    acquired_at   TEXT    NOT NULL DEFAULT (datetime('now')),
    expires_at    TEXT    NOT NULL,
    repository_id TEXT    NOT NULL DEFAULT '',
    PRIMARY KEY (lock_key, repository_id)
);

CREATE INDEX IF NOT EXISTS idx_distributed_locks_expires ON distributed_locks(expires_at);
CREATE INDEX IF NOT EXISTS idx_distributed_locks_key_repo_exp
    ON distributed_locks(lock_key, repository_id, expires_at);

CREATE TABLE IF NOT EXISTS agent_oplog (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    agent_id         TEXT    NOT NULL,
    operation_id     TEXT    NOT NULL UNIQUE,
    kind             TEXT    NOT NULL,
    description      TEXT    NOT NULL,
    predecessor_hash TEXT,
    model_id         TEXT,
    change_id        INTEGER,
    timestamp_ms     INTEGER NOT NULL,
    undone           INTEGER NOT NULL DEFAULT 0,
    repository_id    TEXT    NOT NULL DEFAULT ''
);

CREATE INDEX IF NOT EXISTS idx_agent_oplog_agent ON agent_oplog(agent_id);
CREATE INDEX IF NOT EXISTS idx_agent_oplog_ts ON agent_oplog(timestamp_ms);
CREATE INDEX IF NOT EXISTS idx_agent_oplog_repo ON agent_oplog(repository_id);
CREATE INDEX IF NOT EXISTS idx_agent_oplog_repo_ts ON agent_oplog(repository_id, timestamp_ms);

-- T1.6: cold-tier checkpoint blobs. Each row is a `Projection::snapshot()`-derived
-- payload for the ops in (op_id_lo..=op_id_hi] of `agent_oplog`, referenced from an
-- `OperationKind::Checkpoint { payload_blob_id, .. }` entry in that same table.
-- `id` is what `payload_blob_id` stores (a plain autoincrement row id, not a CAS
-- content hash) so `Checkpoint.payload_blob_id: u64` round-trips exactly.
CREATE TABLE IF NOT EXISTS checkpoint_blobs (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    repository_id TEXT    NOT NULL DEFAULT '',
    op_id_lo      INTEGER NOT NULL,
    op_id_hi      INTEGER NOT NULL,
    blake3        TEXT    NOT NULL,   -- hex blake3 of `payload`, matches Checkpoint.projection_blake3
    payload       BLOB    NOT NULL,   -- concatenated deterministic Projection snapshots
    created_at_ms INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_checkpoint_blobs_repo_hi ON checkpoint_blobs(repository_id, op_id_hi);

CREATE TABLE IF NOT EXISTS a2a_messages (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    message_uuid   TEXT    NOT NULL UNIQUE,
    sender_agent   TEXT    NOT NULL,
    receiver_agent TEXT    NOT NULL,
    msg_type       TEXT    NOT NULL,
    payload        TEXT    NOT NULL,
    priority       INTEGER NOT NULL DEFAULT 1,
    thread_id      TEXT,
    acknowledged   INTEGER NOT NULL DEFAULT 0,
    created_at     TEXT    NOT NULL DEFAULT (datetime('now')),
    repository_id  TEXT    NOT NULL DEFAULT '',
    claim_owner       TEXT,
    claim_until_ms    INTEGER,
    delivery_attempts INTEGER NOT NULL DEFAULT 0,
    last_claim_error  TEXT,
    processed_at_ms   INTEGER
);

CREATE INDEX IF NOT EXISTS idx_a2a_receiver ON a2a_messages (receiver_agent);
CREATE INDEX IF NOT EXISTS idx_a2a_acknowledged ON a2a_messages (acknowledged);
CREATE INDEX IF NOT EXISTS idx_a2a_thread ON a2a_messages (thread_id);
CREATE INDEX IF NOT EXISTS idx_a2a_inbox_claim
    ON a2a_messages(receiver_agent, repository_id, acknowledged, claim_until_ms);
CREATE INDEX IF NOT EXISTS idx_a2a_ack_created ON a2a_messages(acknowledged, created_at);

CREATE TABLE IF NOT EXISTS mesh_heartbeats (
    node_id       TEXT    NOT NULL,
    agent_id      TEXT    NOT NULL,
    last_seen_ms  INTEGER NOT NULL,
    activity      TEXT    NOT NULL DEFAULT 'idle',
    repository_id TEXT    NOT NULL DEFAULT '',
    PRIMARY KEY (node_id, repository_id)
);

CREATE INDEX IF NOT EXISTS idx_mesh_heartbeats_seen ON mesh_heartbeats(last_seen_ms);

CREATE TABLE IF NOT EXISTS orchestration_lineage_events (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    repository_id     TEXT    NOT NULL DEFAULT '',
    kind              TEXT    NOT NULL,
    task_id           INTEGER NOT NULL,
    agent_id          INTEGER,
    session_id        TEXT,
    workflow_id       TEXT,
    plan_session_id   TEXT,
    plan_node_id      TEXT,
    payload_json      TEXT,
    created_at_ms     INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_orch_lineage_repo_task
    ON orchestration_lineage_events(repository_id, task_id);
CREATE INDEX IF NOT EXISTS idx_orch_lineage_repo_ts
    ON orchestration_lineage_events(repository_id, created_at_ms);
CREATE INDEX IF NOT EXISTS idx_orch_lineage_repo_kind
    ON orchestration_lineage_events(repository_id, kind);
