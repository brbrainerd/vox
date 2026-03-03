---
id = "vox.orchestrator"
name = "Vox Orchestrator"
version = "0.1.0"
author = "vox-team"
description = "Multi-agent orchestration: submit tasks, check status, rebalance, monitor budgets and queues."
category = "monitoring"
tools = ["vox_submit_task", "vox_task_status", "vox_orchestrator_status", "vox_complete_task", "vox_fail_task", "vox_cancel_task", "vox_rebalance", "vox_queue_status", "vox_lock_status", "vox_budget_status", "vox_agent_events"]
tags = ["orchestrator", "tasks", "agents", "budget", "queue"]
permissions = ["db_read", "db_write"]
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

## Usage

Use `vox_submit_task` to delegate work to the most appropriate specialist agent.
Monitor with `vox_budget_status` to prevent token overruns.
Call `vox_rebalance` when one agent has significantly more load than others.
