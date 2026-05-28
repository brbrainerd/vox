---
title: "Mens Coordination & Database Write Safety"
description: "Official documentation for Mens Coordination & Database Write Safety for the Vox language. Detailed technical reference, architecture gui"
category: "Language Reference"
last_updated: "2026-03-24"
training_eligible: true

schema_type: "TechArticle"
---

# Mens Coordination & Database Write Safety

Single Source of Truth for how Vox mens nodes coordinate on Turso/libSQL,
prevent simultaneous write conflicts, and deliver agent-to-agent messages
reliably across process and machine boundaries.

> [!IMPORTANT]
> All orchestrator coordination state (locks, op-log, A2A messages, heartbeats)
> persists to Turso when `VOX_MESH_ENABLED=1`. On a single machine without mens
> these remain in-process only for zero-overhead local development.

**Mental model:** “Distributed” here means **many orchestrator processes** (e.g. two `vox-mcp` hosts) sharing **durable Turso rows** and **HTTP A2A** — not a single long-lived orchestrator singleton in one OS process. File routing and per-process structures still exist in each process; cross-node arbitration uses coordination tables (`distributed_locks`, etc.). The shared bootstrap factory lives in [`vox_orchestrator::bootstrap`](../../../crates/vox-orchestrator/src/bootstrap.rs).

---

## 1. Architecture Overview

```text
┌────────────────────────────────────┐  ┌────────────────────────────────────┐
│       Mens Node A  (Device 1)      │  │       Mens Node B  (Device 2)      │
│                                    │  │                                    │
│  Orchestrator A                    │  │  Orchestrator B                    │
│  ├─ FileLockManager (in-process)   │  │  ├─ FileLockManager (in-process)   │
│  ├─ MessageBus → DB-backed         │  │  ├─ MessageBus → DB-backed         │
│  ├─ OpLog → persist to Turso       │  │  ├─ OpLog → persist to Turso       │
│  └─ HeartbeatMonitor → Turso       │  │  └─ HeartbeatMonitor → Turso       │
│                                    │  │                                    │
│  EmbeddedReplica (local.db)  ──────┼──┼──▶ Turso Cloud Primary             │
└────────────────────────────────────┘  └────────────────────────────────────┘
                         ▲                              ▲
                         └──────── A2A HTTP relay ──────┘
                                  /v1/a2a/deliver
```

---

## 2. Turso Coordination Tables (Codex schema domain: `coordination`)

All tables are added via the `coordination` Arca schema domain and created with
`IF NOT EXISTS` — safe for multi-node concurrent schema bootstrapping.

### `distributed_locks`

Per-resource advisory fencing lock. Uses SQLite row atomicity (`INSERT OR IGNORE`)
as the CAS primitive — no external lock manager required.

| Column | Type | Purpose |
|--------|------|---------|
| `lock_key` | TEXT PK | Logical resource path (e.g. `"file:src/lib.rs"`) |
| `holder_node` | TEXT | `VOX_MESH_NODE_ID` of lock owner |
| `holder_agent` | TEXT | Agent session or task ID |
| `fence_token` | INTEGER | Monotone counter; prevents ABA re-use |
| `acquired_at` | TEXT | ISO8601 timestamp |
| `expires_at` | TEXT | TTL-based expiry; `sweep_expired_distributed_locks` cleans stale rows |
| `repository_id` | TEXT | Scope to git repository |

**Lock acquisition protocol:**
```sql
-- Attempt atomic acquisition (no-op if row exists and not expired)
INSERT INTO distributed_locks
    (lock_key, holder_node, holder_agent, fence_token, expires_at, repository_id)
VALUES (?, ?, ?, ?, datetime('now', '+30 seconds'), ?)
ON CONFLICT(lock_key, repository_id) DO NOTHING;

-- Check if we won
SELECT fence_token FROM distributed_locks
WHERE lock_key = ? AND repository_id = ?
  AND holder_node = ? AND expires_at > datetime('now');
```

### `agent_oplog`

Persisted mirror of the in-memory `OpLog` SHA-3 chain. Enables crash recovery
and cross-node auditability. Append-only; no OCC guard needed.

### `a2a_messages`

Durable inbox for agent-to-agent messages. Cross-node delivery via the mens HTTP
relay endpoint `POST /v1/a2a/deliver`; fallback is DB polling.

### `mesh_heartbeats`

Cross-node heartbeat table. Updated by each node's background tick. Any node can
query `live_nodes_from_db(stale_threshold_ms)` to see the full mens membership.

---

## 3. Conflict Resolution Strategy

### Default: Last-Push-Wins (Turso sync)

Turso applies last-push-wins at the row level during embedded replica sync. This
is acceptable for **append-only** tables (`agent_oplog`, `a2a_messages`) where
the `AUTOINCREMENT` primary key ensures no row is ever overwritten.

### Opt-in: OCC for Contested Rows

For **mutating** tables (e.g. `memories`, `agent_sessions`) the `occ` module in
`vox-orchestrator` provides an application-layer guard:

1. `SELECT written_at` before writing.
2. Compare remote vs local ISO timestamp lexicographically.
3. If remote is newer: apply `ConflictResolution` strategy.
4. Default strategy: `TakeRight` (remote wins; local write skipped).
5. On `DeferToAgent`: creates a `ConflictManager` entry for human review.

### Not Used: Turso MVCC (`BEGIN CONCURRENT`)

Turso's experimental MVCC implementation has had acknowledged data-loss incidents
and is not stable as of 2026-03. We do **not** use `BEGIN CONCURRENT`.  
Revisit when Turso marks it stable.

---

## 4. EmbeddedReplica for Mens Nodes

When `VOX_MESH_ENABLED=1` + `VOX_DB_URL` + `VOX_DB_TOKEN` are all set, `VoxDb`
automatically opens an **EmbeddedReplica** instead of a plain local file:

```bash
VOX_MESH_ENABLED=1
VOX_DB_URL=libsql://my-db.turso.io
VOX_DB_TOKEN=<token>
VOX_DB_PATH=/path/to/local-replica.db  (optional; defaults to .vox/cache/db/local.db)
```

Reads are sub-millisecond from the local file. Writes go to the primary and
replicate back. After shared-table writes, `VoxDb::sync()` is called
asynchronously to flush.

---

## 5. A2A Cross-Node Message Delivery

```text
Node A: MessageBus::send_routed(receiver, route=Remote { node_url })
          │
          ├─▶ Writes row to local a2a_messages (DB)
          │
          └─▶ POST {node_url}/v1/a2a/deliver  (JSON A2AMessage)
                │
                ▼
              Node B: inserts into its local a2a_messages
              Node B: MessageBus::poll_inbox_from_db() returns message
```

Retry on HTTP failure: 3 attempts with exponential backoff (500ms, 1s, 2s).
After all retries fail: message remains in the DB inbox; receiver polls on next
heartbeat cycle (≤60 s latency fallback).

---

## 6. Network Resilience

### Connection Retries (Turso)

```text
attempt 1 → 500ms
attempt 2 → 1000ms + jitter(0..500ms)
attempt 3 → 2000ms + jitter(0..500ms)
...capped at 30s
```

Formula: `base_ms * 2^attempt + rand(0..jitter_ms)`, capped at `max_ms=30_000`.

### Circuit Breaker (`VOX_DB_CIRCUIT_BREAKER=1`)

| State | Condition | Behavior |
|-------|-----------|----------|
| Closed | < N failures | Normal operation |
| Open | ≥ N consecutive failures | Returns `StoreError::CircuitOpen` immediately |
| Half-Open | After reset_timeout (30s) | One probe request allowed |

Default: `N=5`, `reset_timeout=30s`.

When Open: write callers buffer to `AgentQueue` for retry on recovery.

### Mens HTTP Client Retries

`PopuliHttpClient` applies the same exponential backoff formula for join, heartbeat,
and A2A relay calls. Previously it had no retry logic at all.

---

## 7. Stale Lock Sweep

A background task (spawned by orchestrator at startup when DB is present) sweeps
expired rows from `distributed_locks` every 60 seconds:

```sql
DELETE FROM distributed_locks WHERE expires_at < datetime('now');
```

This prevents phantom locks from crashed nodes that never released their rows.
Lock TTL defaults: 30s for file edits, 5m for long-running tasks.

---

## 8. Environment Variables Reference

| Variable | Default | Purpose |
|----------|---------|---------|
| `VOX_MESH_ENABLED` | `false` | Activate mens coordination |
| `VOX_MESH_NODE_ID` | auto-generated | Stable node identity |
| `VOX_MESH_CONTROL_ADDR` | unset | HTTP control plane URL |
| `VOX_MESH_SCOPE_ID` | unset | Cluster tenancy ID |
| `VOX_DB_URL` | unset | Turso remote URL |
| `VOX_DB_TOKEN` | unset | Turso auth token |
| `VOX_DB_PATH` | `.vox/cache/db/local.db` | Local replica path |
| `VOX_DB_CIRCUIT_BREAKER` | `false` | Enable DB circuit breaker |
| `VOX_MESH_TOKEN` | unset | Bearer token for mens HTTP routes |

---

## 9. Gaps & Future Work

| Gap | Status | When |
|-----|--------|------|
| Turso `transform` hook for server-side conflict resolution | Not available in Rust SDK | When Turso Go SDK ports to Rust |
| NATS JetStream for durable A2A at scale | Not needed at current mens size | When >100 concurrent agents |
| Turso MVCC `BEGIN CONCURRENT` | Unstable | When Turso marks stable |
| CRDT-based memory merging (`cr-sqlite`) | Research phase | When memory conflicts become common |

---

## Related Documents

- `docs/src/adr/004-codex-arca-turso-ssot.md` — Turso naming conventions
- `docs/src/reference/orchestration-unified.md` — Orchestrator internals
- `docs/src/reference/external-repositories.md` — Repo discovery
- `crates/vox-orchestrator/src/locks.rs` — In-process + distributed advisory locks
- `crates/vox-orchestrator/src/a2a.rs` — A2A message bus
- `crates/vox-orchestrator/src/occ.rs` — OCC write guards
- `crates/vox-db/src/circuit_breaker.rs` — DB circuit breaker
- `crates/vox-db/src/schema/domains/sql/coordination.sql` — coordination DDL (Arca fragment; merged in `gamification_coordination.rs`)


