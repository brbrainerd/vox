---
name = "skill-orchestrator"
description = "Multi-agent orchestration: submit tasks, check status, rebalance, monitor budgets and queues."

[metadata]
"vox-id" = "vox.orchestrator"
"vox-version" = "0.1.0"
"vox-author" = "vox-team"
"vox-category" = "monitoring"
"vox-tools" = ["vox_submit_task", "vox_task_status", "vox_orchestrator_status", "vox_complete_task", "vox_fail_task", "vox_cancel_task", "vox_rebalance", "vox_queue_status", "vox_lock_status", "vox_budget_status", "vox_agent_events"]
"vox-tags" = ["orchestrator", "tasks", "agents", "budget", "queue"]
"vox-permissions" = ["db_read", "db_write"]
---

# Vox Orchestrator Skill

Provides full access to the multi-agent task orchestration system.

## Tools

- `vox_submit_task` — submit a new task for an agent to execute
- `vox_task_status` — check the status of a submitted task
- `vox_orchestrator_status` — get overall system health and agent count
- `vox_complete_task` / `vox_fail_task` — mark task completion or failure
- `vox_cancel_task` — cancel a queued or running task
- `vox_rebalance` — rebalance tasks across agents
- `vox_queue_status` — inspect the task queue for an agent
- `vox_budget_status` — check token usage and approximate costs
- `vox_agent_events` — stream event history

## Mens alignment

For **multi-node** or **GPU-pool** routing hints, keep **`vox_submit_task` → `capabilities.labels`** in sync with worker **`VOX_MESH_LABELS`** / **`Vox.toml` `[mesh]` or `[mens]`** (legacy) **`.labels`**. See the built-in mesh skill and **`docs/src/reference/populi.md`**.

## Usage

Use `vox_submit_task` to delegate work to the most appropriate specialist agent.
Monitor with `vox_budget_status` to prevent token overruns.
Call `vox_rebalance` when one agent has significantly more load than others.
