# Vox Orchestrator — Coordination Layer

The Vox Orchestrator is a long-running Rust daemon exposed over MCP. It is the **single source
of truth** for all agent coordination, session state, memory, VCS history, and budget.

It is **backend-agnostic**: VS Code, `vox` CLI, and CI systems all call the same MCP API.

## Sole Responsibilities

| Domain | Owned by Orchestrator |
|---|---|
| Agent lifecycle | Registration, heartbeat, mood, energy, XP, companions |
| Task queue | Submission, priority, file-affinity routing, completion |
| File locks | One writer per file at a time; `vox_claim_file` / `vox_transfer_file` |
| Memory | Short-term scratchpad + long-term Turso-backed vector memory |
| Sessions | Create / compact / reset; compaction triggers |
| VCS oplog | JJ-inspired undo/redo, snapshot diff and restore |
| Budget | Per-provider cost, daily limits, `vox_budget_status` |
| A2A messaging | Agent-to-agent questions, broadcast, inbox, ack |
| Gamification state | Profile, quests, battle state (read by extension HUD) |
| Security gate | Permission checks before dangerous operations |
| Event bus | Broadcasts events to all subscribed clients |

## Does NOT Own

- Compilation, formatting, type-checking → delegated to `vox` CLI
- TOESTUB analysis → delegated to `vox stub-check` CLI
- Training → delegated to `vox train --native`
- Inference → delegated to `vox generate`

All delegation is done by shelling out to the `vox` binary and returning structured results.

## Crate Layout (`crates/vox-orchestrator/src/`)

```
orchestrator/
  core.rs           ← OrchestratorCore struct, startup, shutdown
  task_dispatch.rs  ← queue, scheduling, file-affinity routing
  agent_state.rs    ← agent registration, heartbeat, mood/XP
  tool_bridge.rs    ← delegates to CLI (cargo invocations, vox binary)
  vcs_ops.rs        ← snapshot, oplog, undo/redo
a2a.rs              ← agent-to-agent messaging
affinity.rs         ← file affinity scoring
budget.rs           ← cost tracking and limits
bulletin.rs         ← broadcast event bus
compaction.rs       ← session memory compaction
config.rs           ← runtime config (sourced from vox-config)
context.rs          ← per-agent context slots
events.rs           ← typed EventBus
handoff.rs          ← structured handoff payloads
heartbeat.rs        ← agent keepalive
locks.rs            ← file lock arbitration
memory.rs           ← agent memory store
memory_search.rs    ← semantic/BM25 memory search
models.rs           ← model registry, routing
oplog.rs            ← VCS change log
queue.rs            ← task queue implementation
rebalance.rs        ← queue rebalancing logic
runtime.rs          ← async runtime wiring
schema.rs           ← DB schema migrations
scope.rs            ← scope violation guard
security.rs         ← permission gate
session.rs          ← session CRUD and compaction triggers
snapshot.rs         ← DB snapshot create/restore
state.rs            ← global shared state
summary.rs          ← agent summary generation
types.rs            ← shared type definitions
usage.rs            ← provider usage counters
validation.rs       ← input validation helpers
workspace.rs        ← workspace management
```

## MCP Tool Reference

### Task & Orchestration
`vox_submit_task`, `vox_task_status`, `vox_complete_task`, `vox_fail_task`, `vox_cancel_task`,
`vox_orchestrator_status`, `vox_orchestrator_start`, `vox_rebalance`, `vox_agent_events`, `vox_poll_events`

### VS Code Bridge
`vox_map_vscode_session`, `vox_record_cost`, `vox_heartbeat`, `vox_cost_history`

### File & Affinity
`vox_check_file_owner`, `vox_my_files`, `vox_claim_file`, `vox_transfer_file`, `vox_file_graph`

### Agent Collaboration
`vox_ask_agent`, `vox_answer_question`, `vox_pending_questions`, `vox_broadcast`,
`vox_a2a_send`, `vox_a2a_inbox`, `vox_a2a_ack`, `vox_a2a_broadcast`, `vox_a2a_history`

### Queue, Lock & Budget
`vox_queue_status`, `vox_lock_status`, `vox_budget_status`

### Context Management
`vox_set_context`, `vox_get_context`, `vox_list_context`, `vox_context_budget`,
`vox_handoff_context`, `vox_agent_handoff`

### Memory & Knowledge
`vox_memory_store`, `vox_memory_recall`, `vox_memory_search`, `vox_memory_log`,
`vox_memory_list_keys`, `vox_knowledge_query`, `vox_memory_save_db`, `vox_memory_recall_db`

### Sessions
`vox_session_create`, `vox_session_list`, `vox_session_reset`, `vox_session_compact`,
`vox_session_info`, `vox_session_cleanup`, `vox_compaction_status`

### Preferences & Patterns
`vox_preference_get`, `vox_preference_set`, `vox_preference_list`,
`vox_learn_pattern`, `vox_behavior_record`, `vox_behavior_summary`

### Skills
`vox_skill_install`, `vox_skill_uninstall`, `vox_skill_list`, `vox_skill_search`,
`vox_skill_info`, `vox_skill_parse`

### VCS & Snapshots (JJ-inspired)
`vox_snapshot_list`, `vox_snapshot_diff`, `vox_snapshot_restore`, `vox_oplog`, `vox_undo`,
`vox_redo`, `vox_conflicts`, `vox_resolve_conflict`, `vox_conflict_diff`,
`vox_workspace_create`, `vox_workspace_merge`, `vox_workspace_status`,
`vox_change_create`, `vox_change_log`, `vox_vcs_status`

### Compiler & Tests (delegated to CLI)
`vox_validate_file`, `vox_run_tests`, `vox_check_workspace`, `vox_test_all`, `vox_generate_code`

### Build & Analysis (delegated to CLI)
`vox_build_crate`, `vox_lint_crate`, `vox_coverage_report`

### Git
`vox_git_log`, `vox_git_diff`, `vox_git_status`, `vox_git_blame`

### Config
`vox_get_config`

### Bulletin
`vox_publish_message`
