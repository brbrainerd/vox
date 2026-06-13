# Semantic Behavior Map — `vox-orchestrator-queue`

Deterministically synthesized from 42 distinct proven-behavior claims (of 42 extracted) across 24 symbols. 1 symbols have an explicit error-path proof; **15 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `FileAffinityMap::assign_v`  (edge, happy, invariant; EXTRACTED)
- [edge] assign_v() enforces 60-second hold-down preventing higher Lamport from remote daemon within window  (crates/vox-orchestrator-queue/src/affinity.rs)
- [happy] assign_v() allows same daemon to update assignment immediately without hold-down delay  (crates/vox-orchestrator-queue/src/affinity.rs)
- [invariant] assign_v() rejects lower Lamport timestamps from remote daemon even after hold-down window  (crates/vox-orchestrator-queue/src/affinity.rs)
- [edge] assign_v() breaks equal-Lamport ties by comparing daemon IDs, higher bytes win  (crates/vox-orchestrator-queue/src/affinity.rs)
- [invariant] assign_v bypasses hold-down period when the same daemon re-assigns a file, allowing Lamport updates immediately  (crates/vox-orchestrator-queue/src/affinity.rs)

### `LockLeaderElection::try_become_leader`  (edge, happy; EXTRACTED)
- [happy] first caller to try_become_leader succeeds and receives LeaderRole::Leader  (crates/vox-orchestrator-queue/tests/leader_election.rs)
- [happy] second caller when leader is alive receives LeaderRole::Follower with correct leader_node_id  (crates/vox-orchestrator-queue/tests/leader_election.rs)
- [edge] after lease expiration, a new leader election call succeeds and returns LeaderRole::Leader  (crates/vox-orchestrator-queue/tests/leader_election.rs)
- [happy] try_become_leader returns LeaderRole::Leader for the first caller  (crates/vox-orchestrator-queue/tests/leader_election.rs)
- [happy] try_become_leader returns LeaderRole::Follower with the leader node ID when a leader is already active  (crates/vox-orchestrator-queue/tests/leader_election.rs)

### `FileAffinityMap::conflicts`  (edge, happy, invariant; EXTRACTED)
- [happy] conflicts() returns a list of conflicting file-agent pairs when manifest files are owned by others  (crates/vox-orchestrator-queue/src/affinity.rs)
- [invariant] conflicts() returns empty list when agent queries against its own owned files  (crates/vox-orchestrator-queue/src/affinity.rs)
- [happy] conflicts() detects file conflicts between an agent's manifest and the affinity map  (crates/vox-orchestrator-queue/src/affinity.rs)
- [edge] conflicts() returns empty result when agent queries its own assigned files (no self-conflict)  (crates/vox-orchestrator-queue/src/affinity.rs)

### `FileLockManager.list_locks()`  (happy; EXTRACTED)
- [happy] list_locks() returns all lock entries that were acquired  (crates/vox-orchestrator-queue/src/locks/mod.rs)
- [happy] list_locks() includes all acquired paths in the returned list  (crates/vox-orchestrator-queue/src/locks/mod.rs)
- [happy] list_locks() correctly distinguishes and counts exclusive locks separately from shared locks  (crates/vox-orchestrator-queue/src/locks/mod.rs)

### `FileAffinityMap::best_agent_for`  (happy; EXTRACTED)
- [happy] best_agent_for returns the agent with the most matching experience patterns  (crates/vox-orchestrator-queue/src/affinity.rs)
- [happy] best_agent_for returns the agent with most recorded experience for similar file patterns (by extension and directory)  (crates/vox-orchestrator-queue/src/affinity.rs)

### `FileAffinityMap::owner_or_assign`  (happy; EXTRACTED)
- [happy] owner_or_assign() returns the existing owner when a file is already assigned  (crates/vox-orchestrator-queue/src/affinity.rs)
- [happy] owner_or_assign() assigns and returns the requesting agent when file is unassigned  (crates/vox-orchestrator-queue/src/affinity.rs)

### `FileLockManager.holder()`  (happy; EXTRACTED)
- [happy] holder() returns the correct AgentId that holds the lock  (crates/vox-orchestrator-queue/src/locks/mod.rs)
- [happy] holder() returns the correct LockKind of the lock  (crates/vox-orchestrator-queue/src/locks/mod.rs)

### `LockLeaderElection::heartbeat`  (happy; EXTRACTED)
- [happy] heartbeat method succeeds repeatedly without error  (crates/vox-orchestrator-queue/tests/leader_election.rs)
- [happy] heartbeat() succeeds and maintains leadership lease renewal  (crates/vox-orchestrator-queue/tests/leader_election.rs)

### `verify_entry`  (error, happy; EXTRACTED)
- [happy] verify_entry succeeds for properly signed entries and fails when payload is tampered  (crates/vox-orchestrator-queue/tests/sign_verify.rs)
- [error] verify_entry rejects unsigned entries with SignError::NoLocalKey  (crates/vox-orchestrator-queue/tests/sign_verify.rs)

### `FileAffinityMap::agent_load`  (happy; EXTRACTED)
- [happy] agent_load() returns accurate counts of files assigned to each agent  (crates/vox-orchestrator-queue/src/affinity.rs)

### `FileAffinityMap::assign_v with DaemonId tiebreaking`  (invariant; EXTRACTED)
- [invariant] assign_v breaks Lamport clock ties by comparing DaemonId bytes, with lexicographically higher DaemonId winning  (crates/vox-orchestrator-queue/src/affinity.rs)

### `FileAffinityMap::assign_v with Lamport clock hold-down`  (invariant; EXTRACTED)
- [invariant] assign_v enforces 60-second hold-down period where local daemon's ownership is preserved despite higher Lamport updates, then yields to higher Lamport after hold-down expires  (crates/vox-orchestrator-queue/src/affinity.rs)

### `FileAffinityMap::assign_v with Lamport comparison`  (invariant; EXTRACTED)
- [invariant] assign_v rejects updates with lower Lamport clock values even after hold-down expires, preserving higher Lamport ownership  (crates/vox-orchestrator-queue/src/affinity.rs)

### `FileAffinityMap::lookup`  (happy; EXTRACTED)
- [happy] lookup() returns the assigned agent for a file, and None for an unassigned file  (crates/vox-orchestrator-queue/src/affinity.rs)

### `FileAffinityMap::release`  (happy; EXTRACTED)
- [happy] release() removes only the specified file's assignment, not others for the same agent  (crates/vox-orchestrator-queue/src/affinity.rs)

### `FileAffinityMap::release_all`  (happy; EXTRACTED)
- [happy] release_all() removes all files assigned to an agent, leaving other agents' assignments intact  (crates/vox-orchestrator-queue/src/affinity.rs)

### `FileLockManager::hydrate_from_db`  (happy; EXTRACTED)
- [happy] hydrate_from_db reconstructs lock state from database with correct holder and kind  (crates/vox-orchestrator-queue/tests/persisted_locks.rs)

### `FileLockManager::release_persisted`  (happy; EXTRACTED)
- [happy] release_persisted removes lock entry from database  (crates/vox-orchestrator-queue/tests/persisted_locks.rs)

### `LockLeaderElection with TTL lease expiration`  (invariant; EXTRACTED)
- [invariant] try_become_leader allows a new node to claim leadership after a prior leader's TTL-based lease expires  (crates/vox-orchestrator-queue/tests/leader_election.rs)

### `OpLog::lookup`  (happy; EXTRACTED)
- [happy] recorded operation persists to vox-db and is retrievable after reopening OpLog  (crates/vox-orchestrator-queue/tests/oplog_persist.rs)

### `OpLog::record_persisted and OpLog::warm_load_recent`  (happy; EXTRACTED)
- [happy] record_persisted stores a record to VoxDb that survives reopen when warm_load_recent reloads from the same db  (crates/vox-orchestrator-queue/tests/oplog_persist.rs)

### `OpLog::warm_load_recent`  (happy; EXTRACTED)
- [happy] warm_load_recent correctly deserializes and loads OperationKind from persisted JSON  (crates/vox-orchestrator-queue/tests/oplog_persist.rs)

### `OperationId`  (happy; EXTRACTED)
- [happy] OperationId formats to_string() as zero-padded 6-digit decimal with 'OP-' prefix  (crates/vox-orchestrator-queue/src/oplog/mod.rs)

### `ProjectionRegistry::snapshot_blake3`  (invariant; EXTRACTED)
- [invariant] live and replay registries produce bit-identical snapshots when processing same operations  (crates/vox-orchestrator-queue/tests/projection_replay.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`FileAffinityMap::agent_load`** — only: _agent_load() returns accurate counts of files assigned to each agent_
- **`FileAffinityMap::best_agent_for`** — only: _best_agent_for returns the agent with the most matching experience patterns_
- **`FileAffinityMap::lookup`** — only: _lookup() returns the assigned agent for a file, and None for an unassigned file_
- **`FileAffinityMap::owner_or_assign`** — only: _owner_or_assign() returns the existing owner when a file is already assigned_
- **`FileAffinityMap::release`** — only: _release() removes only the specified file's assignment, not others for the same agent_
- **`FileAffinityMap::release_all`** — only: _release_all() removes all files assigned to an agent, leaving other agents' assignments intact_
- **`FileLockManager.holder()`** — only: _holder() returns the correct AgentId that holds the lock_
- **`FileLockManager.list_locks()`** — only: _list_locks() returns all lock entries that were acquired_
- **`FileLockManager::hydrate_from_db`** — only: _hydrate_from_db reconstructs lock state from database with correct holder and kind_
- **`FileLockManager::release_persisted`** — only: _release_persisted removes lock entry from database_
- **`LockLeaderElection::heartbeat`** — only: _heartbeat method succeeds repeatedly without error_
- **`OpLog::lookup`** — only: _recorded operation persists to vox-db and is retrievable after reopening OpLog_
- **`OpLog::record_persisted and OpLog::warm_load_recent`** — only: _record_persisted stores a record to VoxDb that survives reopen when warm_load_recent reloads from the same db_
- **`OpLog::warm_load_recent`** — only: _warm_load_recent correctly deserializes and loads OperationKind from persisted JSON_
- **`OperationId`** — only: _OperationId formats to_string() as zero-padded 6-digit decimal with 'OP-' prefix_
