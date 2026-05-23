---
title: "Orchestrator AgentEventKind → Ludus matrix"
description: "Maps each orchestrator bus `type` string to Ludus base_reward and process_rewards behavior."
category: "Architecture SSOTs"
last_updated: "2026-03-25"

schema_type: "TechArticle"
training_eligible: false
archived_date: 2026-04-18
---

# AgentEventKind → Ludus wiring

Orchestrator events serialize with `#[serde(tag = "type", rename_all = "snake_case")]`. Ludus reads `type`, applies [`base_reward`](../../../crates/vox-ludus/src/reward_policy.rs), then [`process_event_rewards`](../../../crates/vox-ludus/src/db/process_rewards.rs) for companions, counters, and quests.

**Policy-only** means non-zero (or intentional zero) reward from policy, but no extra branch in the `match event_type` companion/quest block (counters may still increment when listed).

| `type` | Base XP / crystals | Companion / quest / counters |
|--------|-------------------|------------------------------|
| `agent_spawned` | 25 / 2 | policy-only |
| `agent_retired` | 10 / 0 | policy-only |
| `activity_changed` | 0 / 0 | companion `Writing` / `Idle` from `activity` field |
| `task_submitted` | 8 / 1 | `TaskAssigned`; counters `tasks_submitted` |
| `task_started` | 5 / 1 | `TaskAssigned` |
| `task_completed` | 50 / 5 | `TaskCompleted`; counters; Improve + AgentComplete quests |
| `task_failed` | 0 / 0 | `TaskFailed` |
| `lock_acquired` | 3 / 0 | `LockAcquired`; `vcs_locks_acquired` |
| `lock_released` | 1 / 0 | `Rest`; `vcs_locks_released` |
| `agent_idle` | 0 / 0 | policy-only |
| `agent_busy` | 2 / 0 | policy-only |
| `message_sent` | 1 / 0 | counters `inter_agent_messages` |
| `cost_incurred` | 0 / 0 | energy spend |
| `continuation_triggered` | 10 / 2 | policy-only |
| `plan_handoff` | 40 / 8 | Collaborate quests |
| `scope_violation` | 0 / 0 | policy-only |
| `compaction_triggered` | 0 / 0 | policy-only (default arm) |
| `memory_flushed` | 0 / 0 | policy-only |
| `session_created` | 0 / 0 | policy-only |
| `session_reset` | 0 / 0 | policy-only |
| `snapshot_captured` | 30 / 6 | +1 `code_quality` cap; `workspace_snapshots` |
| `conflict_detected` | 0 / 0 | policy-only |
| `operation_undone` | 5 / 0 | policy-only |
| `operation_redone` | 5 / 0 | policy-only |
| `agent_handoff_rejected` | 0 / 0 | policy-only |
| `agent_handoff_accepted` | 50 / 10 | Collaborate quests |
| `urgent_rebalance_triggered` | 0 / 0 | policy-only |
| `token_streamed` | 0 / 0 | policy-only |
| `injection_detected` | 0 / 0 | policy-only |
| `prompt_conflict_detected` | 0 / 0 | policy-only |
| `planning_routed` | 0 / 0 | policy-only |
| `plan_session_created` | 0 / 0 | policy-only |
| `plan_version_created` | 0 / 0 | policy-only |
| `replan_triggered` | 0 / 0 | policy-only |
| `workflow_handoff_requested` | 0 / 0 | policy-only |
| `workflow_handoff_completed` | 0 / 0 | policy-only |
| `workflow_started` | 0 / 0 | policy-only |
| `workflow_completed` | 1200 / 240 (see `reward_policy`) | policy-only |
| `workflow_failed` | 0 / 0 | policy-only |
| `activity_started` | 0 / 0 | policy-only |
| `activity_completed` | 0 / 0 | policy-only |
| `activity_retried` | 0 / 0 | policy-only |
| `conflict_resolved` | 100 / 20 + lumens | policy-only |
| `workspace_created` | 0 / 0 | policy-only |
| `endpoint_reliability_observation` | 0 / 0 | policy-only |
| `orchestrator_idle` | 0 / 0 | policy-only |
| `task_expired` | 0 / 0 | policy-only |

**Note {** CLI/MCP-only event types (e.g. `check_completed`, `mcp_tool_called`) are documented in [`ludus-integration-contract`](ludus-integration-contract.md) and [`reward_policy`](../../../crates/vox-ludus/src/reward_policy.rs).

**Grind taper:** High-frequency bus types (`task_submitted`, `lock_*`, `snapshot_captured`, `message_sent`, `mcp_tool_called`, …) use the faster anti-grind window in [`apply_policy`](../../../crates/vox-ludus/src/reward_policy.rs).


