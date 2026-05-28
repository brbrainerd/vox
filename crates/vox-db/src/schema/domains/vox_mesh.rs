//! Arca SQL: Vox Mesh reputation ledger, compute donation tracking, and A2A durable store.
pub const SCHEMA_VOX_MESH: &str = "
-- Contributor reputation ledger for decentralized GPU mesh.
-- Unifies compute donation (Kudos) and code contribution rewards.
CREATE TABLE IF NOT EXISTS vox_kudos (
    vox_user_id TEXT NOT NULL,
    node_id TEXT NOT NULL,
    primitive TEXT NOT NULL, -- 'gpu_compute', 'code_review', 'bug_fix'
    amount INTEGER NOT NULL,
    task_id TEXT,
    created_unix_ms INTEGER NOT NULL,
    metadata_json TEXT,
    PRIMARY KEY (vox_user_id, node_id, created_unix_ms)
);

CREATE INDEX IF NOT EXISTS idx_vox_kudos_user ON vox_kudos(vox_user_id);
CREATE INDEX IF NOT EXISTS idx_vox_kudos_node ON vox_kudos(node_id);

-- Sybil mitigation and peer routing tracker.
CREATE TABLE IF NOT EXISTS vox_peer_reputation (
    node_id TEXT PRIMARY KEY,
    success_count INTEGER NOT NULL DEFAULT 0,
    fail_count INTEGER NOT NULL DEFAULT 0,
    timeout_count INTEGER NOT NULL DEFAULT 0,
    invalid_output_count INTEGER NOT NULL DEFAULT 0,
    last_updated_unix_ms INTEGER NOT NULL
);

-- ── Populi mesh durable store (schema v1, 2026-05-01) ──────────────────────────

-- A2A inbox: persisted delivery envelopes for the mesh control plane.
CREATE TABLE IF NOT EXISTS mesh_a2a_messages (
    id                      INTEGER PRIMARY KEY AUTOINCREMENT,
    sender_agent_id         TEXT NOT NULL,
    receiver_agent_id       TEXT NOT NULL,
    message_type            TEXT NOT NULL,
    payload                 TEXT NOT NULL,
    idempotency_key         TEXT,
    idempotency_dedupe_key  TEXT,
    privacy_class           TEXT,
    payload_blake3_hex      TEXT,
    worker_ed25519_sig_b64  TEXT,
    jwe_payload             TEXT,
    priority                INTEGER NOT NULL DEFAULT 128,
    task_kind               TEXT,
    model_id                TEXT,
    sender_node_id          TEXT,
    traceparent             TEXT,
    created_at              INTEGER NOT NULL,
    acked_at                INTEGER,
    acknowledged            INTEGER NOT NULL DEFAULT 0,
    lease_holder_node_id    TEXT,
    lease_expires_unix_ms   INTEGER
);

CREATE INDEX IF NOT EXISTS idx_mesh_a2a_receiver
    ON mesh_a2a_messages(receiver_agent_id, acknowledged);
CREATE INDEX IF NOT EXISTS idx_mesh_a2a_idempotency
    ON mesh_a2a_messages(idempotency_key)
    WHERE idempotency_key IS NOT NULL;

-- Remote execution leases: serialised grant/renew/revoke lifecycle.
CREATE TABLE IF NOT EXISTS mesh_exec_leases (
    lease_id        TEXT PRIMARY KEY,
    task_id         TEXT NOT NULL,
    scope_key       TEXT NOT NULL,
    holder_node_id  TEXT NOT NULL,
    granted_at      INTEGER NOT NULL,
    expires_at      INTEGER NOT NULL,
    state           TEXT NOT NULL DEFAULT 'granted'
);

CREATE INDEX IF NOT EXISTS idx_mesh_lease_state
    ON mesh_exec_leases(state, expires_at);

-- Detached dispatch results (Wave 5 async execution).
CREATE TABLE IF NOT EXISTS mesh_dispatch_results (
    key         TEXT PRIMARY KEY,
    value_json  TEXT NOT NULL,
    created_at  INTEGER NOT NULL
);

-- ── Phase 0: persisted VCS file lock map (P0-T1) ──────────────────────────

-- One row per locked path (canonical absolute form, NFC-normalised).
-- `kind` is 'exclusive' | 'shared_read'; `holder` is the AgentId.0 string.
-- `expires_at` is the UNIX-ms TTL deadline; the leader prunes expired rows.
-- `lease_id` references mesh_exec_leases.lease_id when the lock is being
-- proxied to a remote node; NULL for purely local locks.
CREATE TABLE IF NOT EXISTS vcs_lock (
    path             TEXT NOT NULL PRIMARY KEY,
    kind             TEXT NOT NULL, -- 'exclusive' | 'shared_read'; enforced in Rust (Turso does not support CHECK)
    holder           TEXT NOT NULL,
    holder_node_id   TEXT NOT NULL,
    repository_id    TEXT NOT NULL,
    acquired_at      INTEGER NOT NULL,
    expires_at       INTEGER NOT NULL,
    lease_id         TEXT,
    fence_token      INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_vcs_lock_holder
    ON vcs_lock(holder_node_id, repository_id);
CREATE INDEX IF NOT EXISTS idx_vcs_lock_expires
    ON vcs_lock(expires_at);

-- ── Phase 0: lock-leader election (P0-T2) ─────────────────────────────────

-- Singleton row per repository: who is currently the lock leader.
-- Followers proxy lock-mutation requests via A2A to leader_node_id.
CREATE TABLE IF NOT EXISTS lock_leader (
    repository_id    TEXT NOT NULL PRIMARY KEY,
    leader_node_id   TEXT NOT NULL,
    elected_at       INTEGER NOT NULL,
    heartbeat_at     INTEGER NOT NULL,
    expires_at       INTEGER NOT NULL,
    epoch            INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_lock_leader_expires
    ON lock_leader(expires_at);

-- ── Phase 2: activity result cache (P2-T5) ────────────────────────────────

-- Per-activity dedup cache. Result rows are pruned by the background sweep;
-- rows are append-only otherwise.
CREATE TABLE IF NOT EXISTS activity_result_cache (
    activity_id           TEXT    NOT NULL,
    arg_hash              TEXT    NOT NULL,        -- hex SHA3-512 of canonicalized args
    result_json           TEXT    NOT NULL,        -- serialized activity result value
    produced_at_unix_ms   INTEGER NOT NULL,
    dedup_window_ms       INTEGER NOT NULL,        -- TTL window in ms, e.g. 86_400_000 for 24h
    dedup_window_until    INTEGER NOT NULL,        -- produced_at_unix_ms + dedup_window_ms

    PRIMARY KEY (activity_id, arg_hash)
);

-- Cheap range scan for the background sweep (cadence: every 60s when daemon
-- is running; on-demand via `vox db prune` otherwise).
CREATE INDEX IF NOT EXISTS idx_activity_result_cache_until
    ON activity_result_cache (dedup_window_until);

-- ── Phase 3: durable convergence op-log (P3-T1) ──────────────────────────

-- Durable convergence op-log (P3-T1). Warm tier for gossiped op-fragments.
-- Hot tier lives in OpLog::entries VecDeque; cold tier is Checkpoint-encoded blobs.
CREATE TABLE IF NOT EXISTS convergence_op_log (
    op_id            INTEGER PRIMARY KEY,            -- monotonic OperationId.0
    set_id           BLOB    NOT NULL,                -- 16-byte ULID for the convergence set
    parent_op_ids    TEXT    NOT NULL DEFAULT '[]',  -- JSON array of u64 parents (DAG)
    kind_json        TEXT    NOT NULL,                -- serde_json of OperationKind
    payload          BLOB    NOT NULL DEFAULT X'',    -- opaque op-fragment payload bytes
    payload_blake3   BLOB    NOT NULL,                -- 32-byte blake3 of payload
    predecessor_hash TEXT,                            -- hex sha3-256 / blake3 chain hash
    signature        TEXT,                            -- hex-encoded 64-byte Ed25519 sig
    signing_key_id   TEXT,                            -- hex-encoded 32-byte daemon pubkey id
    agent_id         INTEGER NOT NULL,
    daemon_id        TEXT    NOT NULL,                -- hex-encoded 16-byte daemon UUID
    produced_at      INTEGER NOT NULL,                -- ms since epoch
    description      TEXT    NOT NULL DEFAULT '',
    change_id        INTEGER,
    model_id         TEXT,
    undone           INTEGER NOT NULL DEFAULT 0       -- 0=false / 1=true
);

CREATE INDEX IF NOT EXISTS convergence_op_log_set_id_produced_at
    ON convergence_op_log(set_id, produced_at);
CREATE INDEX IF NOT EXISTS convergence_op_log_daemon_produced
    ON convergence_op_log(daemon_id, produced_at);
CREATE INDEX IF NOT EXISTS convergence_op_log_change_id
    ON convergence_op_log(change_id) WHERE change_id IS NOT NULL;

-- Backfill DLQ: fragments whose parents we have not yet seen.
CREATE TABLE IF NOT EXISTS convergence_op_log_backfill_dlq (
    op_id            INTEGER PRIMARY KEY,
    payload          BLOB    NOT NULL,
    parent_op_ids    TEXT    NOT NULL,
    first_seen_at    INTEGER NOT NULL,
    retry_count      INTEGER NOT NULL DEFAULT 0,
    last_error       TEXT
);
";
