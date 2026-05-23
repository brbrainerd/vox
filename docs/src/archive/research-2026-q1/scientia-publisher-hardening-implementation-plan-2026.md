---
title: "Vox Publication and Orchestration Hardening: Implementation Plan 2026"
description: "Ordered execution plan for de-factoring God Objects across vox-publisher, vox-orchestrator, and vox-cli to adhere to the 500-line TOESTUB architectural policy."
category: "Architecture SSOTs"
status: "experimental"

last_updated: "2026-04-13"
training_eligible: false
archived_date: 2026-04-18
---

# Vox Publication and Orchestration Hardening: Implementation Plan 2026

This plan tracks the decomposition of monolithic "God Objects" across the Vox workspace to ensure long-term maintainability and adherence to the 500-line TOESTUB policy.

## Objectives
- **Hardness:** Enforce the 500-line limit for all new and refactored modules.
- **Domain Decomposition:** Use standard Vox directory-module patterns (e.g., `feature/mod.rs` hub) rather than flat `utils.rs` files.
- **Stability:** Resolve all compilation and `Send` bound regressions during structural migrations.

---

## Status Dashboard

| Target File | Lines | Status | New Location |
| :--- | :--- | :--- | :--- |
| `vox-clavis/src/spec.rs` | 5,400+ | **[COMPLETE]** | `vox-clavis/src/spec/` |
| `vox-populi/src/mens/tensor/candle_qlora_train/training_loop.rs` | 1,192 | **[COMPLETE]** | `training_loop/` |
| `vox-orchestrator/src/orchestrator/task_dispatch/complete/success.rs` | 1,247 | **[COMPLETE]** | `complete/success/` |
| `vox-publisher/src/scientia_evidence.rs` | 1,217 | **[COMPLETE]** | `scientia_evidence/` |
| `vox-orchestrator/src/mcp_tools/task_tools.rs` | 1,184 | **[COMPLETE]** | `mcp_tools/task_tools/` |
| `vox-orchestrator/src/orchestrator/persistence_outbox.rs` | 984 | **[ACTIVE]** | `orchestrator/persistence/` |
| `vox-orchestrator/src/orchestrator/agent_lifecycle.rs` | 825 | **[PLANNED]** | `orchestrator/agent/` |
| `vox-orchestrator/src/budget.rs` | 856 | **[PLANNED]** | `budget/` |
| `vox-publisher/src/submission/mod.rs` | 852 | **[PLANNED]** | `submission/` |
| `vox-publisher/src/scholarly_external_jobs.rs` | 833 | **[PLANNED]** | `scholarly_external_jobs/` |
| `vox-orchestrator/src/orchestrator/core.rs` | 526 | **[PLANNED]** | `orchestrator/init/` |

archived_date: 2026-04-18
---

## Active & Upcoming Waves

### Wave 4: Persistence Outbox Reliability (ACTIVE)
**Target:** `crates/vox-orchestrator/src/orchestrator/persistence_outbox.rs` (984 lines)
**De-factoring Strategy:**
- `mod.rs`: Hub logic and `tick_persistence_outbox_lifecycle`.
- `lifecycle.rs`: `run_persistence_outbox_lifecycle_pass` and `ack_persistence_outbox_lane`.
- `replay.rs`: `try_replay_persistence_outbox` and `replay_one_entry`.

### Wave 5: Agent Lifecycle & Topology
**Target:** `crates/vox-orchestrator/src/orchestrator/agent_lifecycle.rs` (825 lines)
**De-factoring Strategy:**
- `spawn.rs`: Spawning and dynamic agent registration.
- `lifecycle_ops.rs`: Retire, cancel, reorder, and drain.
- `doubt.rs`: Doubt resolution and verification loop.
- `handoff.rs`: Handoff acceptance and validation.

### Wave 6: Budget & Usage Tracking
**Target:** `crates/vox-orchestrator/src/orchestrator/core/budget.rs` (856 lines)
**De-factoring Strategy:**
- `mod.rs`: `BudgetManager` core.
- `session.rs`: Session-level attribution.
- `persistence.rs`: DB loading/saving for budgets.

### Wave 7: Scholarly Jobs & Submission Packaging
**Target:** `vox-publisher/src/submission/mod.rs` (852 lines) & `scholarly_external_jobs.rs` (833 lines)
**De-factoring Strategy:**
- Extract scholarly metadata generation from submission logic.
- Modularize external job probing (OpenReview, Zenodo).

---

## Verification Ritual
After each decomposition:
1. `vox ci sync-ignore-files` (if ignore files were touched).
2. `cargo check --all-targets`.
3. Mental verify: No module exceeds 500 lines.


