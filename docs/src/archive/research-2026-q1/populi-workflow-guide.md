---
title: "Mens Coordination Workflow Guide"
description: "Official documentation for Mens Coordination Workflow Guide for the Vox language. Detailed technical reference, architecture guides, and "
category: "Language Reference"
last_updated: "2026-03-24"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Mens Coordination Workflow Guide

Practical how-to for common multi-node scenarios using the Vox mens coordination layer.

---

## Workflow 1: Two Agents Editing the Same File

**Problem:** Agent A on Device 1 and Agent B on Device 2 both want to edit `src/parser.rs`.

**How it works:**

1. Both agents call `FileLockManager::try_acquire(path, Exclusive)` locally.
2. The orchestrator also calls `try_acquire_distributed(conn, "file:src/parser.rs", node_id, agent_id, 30)`.
3. The first node to `INSERT OR IGNORE` into `distributed_locks` wins.
4. The losing node receives `LockConflict::ExclusivelyHeld` → queues via `queue_agent_for_lock`.
5. When Agent A finishes: `release_distributed(conn, lock_key, fence_token)` deletes the row.
6. Agent B is notified (poll-based, ≤5s check) → acquires lock → proceeds.

**Stale lock safety:** if Node A crashes mid-edit, the TTL (`expires_at`) causes the row
to expire. Node B's next poll after TTL will succeed. Default TTL: 30 seconds for file
edits, extended by heartbeat pings on long-running tasks.

```
Node A                              Turso                          Node B
  │                                   │                              │
  ├── INSERT distributed_locks ──────▶│                              │
  │   lock_key="file:src/parser.rs"   │                              │
  │   (succeeds)                      │                              │
  │                                   │                              │
  │                                   │◀── INSERT distributed_locks ─┤
  │                                   │    (ON CONFLICT DO NOTHING)  │
  │                                   │    0 rows affected           │
  │                                   │                              │
  │                                   │──── SELECT fence_token ─────▶│
  │                                   │     (returns NULL = no win)  │
  │                                   │                              │
  │                                   │              LockConflict ◀──┤
  │                                   │              (queue & wait)  │
  │                                   │                              │
  ├── DELETE distributed_locks ──────▶│                              │
  │   (edit complete)                 │                              │
  │                                   │◀── poll: lock available? ───┤
  │                                   │    yes → INSERT wins        │
  │                                   │                              ├── Edit proceeds
```

archived_date: 2026-04-18
---

## Workflow 2: Agent Memory Write Conflict

**Problem:** Two agents update the same memory key (`agent_id="planner"`, `key="current_plan"`) simultaneously.

**How it works:**

1. Before writing, each agent reads `written_at` for the target row.
2. `occ_guarded_write("memories/planner/current_plan", remote_ts, local_ts, ctx, &mut conflict_mgr, write_fn)` is called.
3. If `remote_ts > local_ts` (remote is newer): default strategy `TakeRight` → skip local write.
4. The skipped agent re-reads the remote value and merges its changes into a new write.
5. If the agent needs manual review: use `ConflictResolution::DeferToAgent(AgentId)`.

---

## Workflow 3: Cross-Node Agent-to-Agent Message

**Problem:** Agent A on Device 1 needs to alert Agent B on Device 2 about a conflict.

**Two delivery paths:**

**Path 1 — HTTP relay (low latency <100ms):**
```
MessageBus::send_routed(sender, receiver, ConflictDetected, payload,
    A2ARoute::Remote { node_url: "http://device2:9847" }, Some(conn))
  → writes row to local a2a_messages (DB)
  → POST http://device2:9847/v1/a2a/deliver  (JSON)
  → Device 2 inserts into its a2a_messages table
  → Device 2's MessageBus::poll_inbox_from_db wakes up
```

**Path 2 — DB polling fallback (eventual, ≤60s):**
```
MessageBus::send_routed(sender, receiver, ..., A2ARoute::Local, Some(conn))
  → writes row to shared Turso a2a_messages table
  → Device 2's next poll_inbox_from_db heartbeat finds the row
```

Retry on HTTP failure: 3 attempts at 500ms / 1000ms / 2000ms with ±250ms jitter.

archived_date: 2026-04-18
---

## Workflow 4: Node Failure & Recovery

**Problem:** Node A dies mid-task. How does Node B detect this and take over?

1. Node A stops sending heartbeats. `mesh_heartbeats.last_seen_ms` stops updating.
2. Node B's `HeartbeatMonitor::check_stale()` polls `live_nodes_from_db(stale_threshold_ms=60000)`.
3. After `warn_after_misses=1` missed window → `StalenessLevel::Warn`.
4. After `dead_after_misses=10` → `StalenessLevel::Dead`.
5. Dead nodes are excluded from `RoutingService` for new task dispatch.
6. Distributed locks held by the dead node expire via TTL → unblock waiting agents.
7. Node A's `agent_oplog` entries survive in Turso → crash recovery via `load_recent`.

---

## Workflow 5: Crash Recovery via OpLog

**Problem:** Node A's orchestrator crashes. How does it restore state on restart?

```rust
// At orchestrator startup when DB is present:
let recent_ops = OpLog::load_recent(&conn, 200, &repository_id).await?;
// Replay: restore in-progress task state, re-acquire distributed locks,
// re-queue pending tasks from AgentQueue serialised state.
```

The op-log chain hash is verified via `verify_chain()`. If the chain is broken
(e.g. partial write before crash), the last verified entry is used as the recovery point.

archived_date: 2026-04-18
---

## Workflow 6: Enabling Mens Mode

Minimal environment for a two-node mens with shared Turso:

**Node A:**
```env
VOX_MESH_ENABLED=1
VOX_MESH_NODE_ID=desktop-488
VOX_MESH_CONTROL_ADDR=http://0.0.0.0:9847   # bind; clients use the external IP
VOX_MESH_SCOPE_ID=my-vox-cluster
VOX_DB_URL=libsql://my-vox.turso.io
VOX_DB_TOKEN=<token>
VOX_DB_PATH=/home/user/.vox/cache/db/local.db
VOX_DB_CIRCUIT_BREAKER=1
```

**Node B:**
```env
VOX_MESH_ENABLED=1
VOX_MESH_NODE_ID=laptop-192
VOX_MESH_CONTROL_ADDR=http://192.168.1.100:9847   # Node A's external IP
VOX_MESH_SCOPE_ID=my-vox-cluster
VOX_DB_URL=libsql://my-vox.turso.io
VOX_DB_TOKEN=<token>
VOX_DB_PATH=/home/user/.vox/cache/db/local.db
VOX_DB_CIRCUIT_BREAKER=1
```

**Start the mens control plane on Node A:**
```bash
vox populi serve --bind 0.0.0.0:9847
```

**Node B joins:**
```bash
vox populi join
```

**Verify both nodes are visible:**
```bash
vox populi status          # shows local registry
vox populi status --remote # queries the control plane HTTP API
```

---

## Workflow 7: Verifying Database Coordination

```bash
# Check distributed locks (should be empty when no agents running)
vox db query "SELECT * FROM distributed_locks"

# Check cross-node heartbeats
vox db query "SELECT node_id, agent_id, datetime(last_seen_ms/1000,'unixepoch') as last_seen FROM mesh_heartbeats ORDER BY last_seen DESC"

# Check pending A2A messages (unacknowledged)
vox db query "SELECT sender_agent, receiver_agent, msg_type, payload FROM a2a_messages WHERE acknowledged = 0"

# Check recent op-log
vox db query "SELECT agent_id, operation_id, kind, description FROM agent_oplog ORDER BY timestamp_ms DESC LIMIT 20"
```

archived_date: 2026-04-18
---

## See Also

- `docs/src/reference/mens-coordination.md` — Architecture SSOT  
- `docs/src/adr/004-codex-arca-turso.md` — Turso/Arca naming  
- `docs/src/reference/orchestration-unified.md` — Orchestrator internals  


