---
title: "Orchestrator"
description: "Agent support documentation for orchestrator"
category: "contributor"
status: "current"
training_eligible: true
---
# Vox Orchestrator — Coordination Layer

The **orchestrator** is the in-process Rust library in `crates/vox-orchestrator`. The default agent stack exposes it through **`vox-mcp`**, a Model Context Protocol **stdio server** that constructs `ServerState` and embeds an `Orchestrator` — not a separate always-on network daemon.

Optional ADR 022 sidecar: **`vox-orchestrator-d`** can run as a long-lived process (`VOX_ORCHESTRATOR_DAEMON_SOCKET` TCP/stdio). MCP currently uses a split-plane transition model: daemon-aligned RPC pilots may own task/agent lifecycle slices, but many VCS/context/event/session features still read embedded stores unless explicitly moved behind daemon contracts.

VS Code, `vox` CLI helpers, and CI attach to **`vox-mcp`** over MCP; tests and other binaries may also construct an `Orchestrator` directly.

**Construction:** Use **`vox_orchestrator::build_repo_scoped_orchestrator`** (or **`build_repo_scoped_orchestrator_for_repository`**) from `bootstrap.rs` so MCP, **`vox dei`**, `vox live`, and other entrypoints agree on `repository_id`, affinity groups, and memory shard paths. With **`VOX_MESH_ENABLED`**, durable coordination still lives in Turso while each process keeps its own in-memory `Orchestrator`; see [Mens coordination](../src/reference/populi-coordination.md) and [Unified orchestration](../src/reference/orchestration-unified.md).

**Authoritative MCP tool names + descriptions:** `crates/vox-orchestrator/src/mcp_tools/tools/mod.rs` → `TOOL_REGISTRY`. The grouped lists below are for humans and may lag that array when new tools land.

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

## Socrates (grounding & completion gate)

- **Policy SSOT** — `vox_socrates_policy::ConfidencePolicy` (crate `vox-socrates-policy`); thresholds must match TOESTUB review and MCP surfaces.
- **Task metadata** — `AgentTask.socrates` (`SocratesTaskContext`: factual mode, citation counts, contradiction hints). When `socrates_gate_enforce` is on, `complete_task` may requeue if the gate returns `Ask` / `Abstain`. `socrates_gate_shadow` logs decisions only.
- **Env** — `VOX_ORCHESTRATOR_SOCRATES_GATE_SHADOW`, `VOX_ORCHESTRATOR_SOCRATES_GATE_ENFORCE`, `VOX_ORCHESTRATOR_SOCRATES_REPUTATION_ROUTING`, optional **`VOX_ORCHESTRATOR_TRUST_GATE_RELAX_ENABLED`** + **`VOX_ORCHESTRATOR_TRUST_GATE_RELAX_MIN_RELIABILITY`** (relax Socrates + completion-grounding enforce + strict scope when `agent_reliability` is high enough; requires DB).
- **Reliability** — Arca schema V10 `agent_reliability`; successful/failed tasks update EMA when a `CodeStore` is wired; optional routing blend via `socrates_reputation_routing`.
- **Handoffs** — `validate_handoff_invariants` / `execute_handoff`: non-empty `pending_tasks` requires non-empty `verification_criteria`.

See `docs/src/architecture/socrates-protocol-ssot.md` and ADR 005.

## Pilot attention and interruption policy

- Dynamic interruption policy lives in `vox-orchestrator::attention::interruption_policy` and is reused by MCP chat/plan/task/A2A surfaces.
- Hard budget exhaustion guard: `BudgetGate::check_attention_snapshot` (MCP infer, task submit, and pilot-visible A2A send paths).
- Runtime outcomes include `InterruptNow`, `DeferUntilCheckpoint`, `BatchWithExistingPrompt`, `ProceedAutonomously`, and `RequireHumanBeforeContinue`.
- Durable attention telemetry uses `AttentionEvent` + `AttentionTracker`; deferred/auto decisions are persisted for calibration (not only shown prompts).
- `Vox.toml` `[orchestrator].interruption_calibration` tunes per-channel gain offsets plus backlog/trust pressure without code changes.
- Task submit `RequireHumanBeforeContinue` accepts explicit marker tokens in description: `[approval:confirm]`, `[approval:reviewed]`, `[human-approved]`.
- Task questioning gate (`VOX_SUBMIT_TASK_BYPASS_QUESTIONING_GATE`) applies to submits with a concrete MCP `session_id` when DB-backed questioning state is available.

## Does NOT Own

- Compilation, formatting, type-checking → surfaced via MCP compiler/git helpers and CLI integration (see `TOOL_REGISTRY` and `crates/vox-orchestrator/src/mcp_tools/tools/compiler_tools.rs`).
- TOESTUB analysis → `bash scripts/quality/toestub_scoped.sh` or `cargo run -p vox-code-audit --bin toestub -- <PATH>`; optional `vox stub-check` when built with **`--features stub-check`** (see `docs/src/reference/cli.md`).
- Mens **native LoRA** training → **`vox mens train`** (`vox-populi`); not orchestrator core.
- Inference / codegen → `vox generate` and related CLI surfaces where enabled.

Some MCP tools spawn subprocesses (`cargo`, `git`, etc.); behavior is **per tool** — do not assume every capability shells out to a single monolithic `vox` invocation.

## Crate layout (`crates/vox-orchestrator/src/`)

Modules live **flat** under `src/` (there is no nested `orchestrator/` package). Principal files:

| File / directory | Role |
|------------------|------|
| `lib.rs` | Crate root |
| `orchestrator.rs` | Core orchestrator API |
| `types.rs` | Shared task/agent types |
| `queue.rs`, `affinity.rs`, `rebalance.rs` | Task queue & routing |
| `locks.rs` | File lock arbitration |
| `session.rs`, `compaction.rs` | Sessions & compaction |
| `memory.rs`, `memory_search.rs` | Memory & hybrid search |
| `events.rs`, `bulletin.rs` | Event bus / broadcasts |
| `a2a.rs` | Agent-to-agent messaging |
| `budget.rs`, `usage.rs` | Cost & usage |
| `handoff.rs`, `context.rs` | Handoffs & context |
| `oplog.rs`, `snapshot.rs`, `jj_backend.rs`, `workspace.rs`, `conflicts.rs` | VCS-inspired flows |
| `security.rs`, `gate.rs`, `socrates.rs` | Permissions & Socrates gate |
| `config.rs`, `state.rs`, `runtime.rs`, `schema.rs` | Config & persistence |
| `services/` | Embeddings, routing, policy, gateway, scaling |
| Other | `lsp.rs`, `monitor.rs`, `models.rs`, `qa.rs`, `continuation.rs`, `groups.rs`, `heartbeat.rs`, `summary.rs`, `scope.rs`, `validation.rs`, … — see tree in repo |

## MCP Tool Reference

Grouped for readability only — **names and descriptions** must match `TOOL_REGISTRY` in `vox-mcp`.

### Task & Orchestration
`vox_submit_task`, `vox_task_status`, `vox_complete_task`, `vox_fail_task`, `vox_cancel_task`,
`vox_orchestrator_status`, `vox_orchestrator_start`, `vox_rebalance`, `vox_agent_events`, `vox_poll_events`

### Session ↔ orchestrator bridge
`vox_map_agent_session`, `vox_record_cost`, `vox_heartbeat`, `vox_cost_history`

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
`vox_config_get` (wire alias: `vox_get_config`)

### Bulletin
`vox_publish_message`
