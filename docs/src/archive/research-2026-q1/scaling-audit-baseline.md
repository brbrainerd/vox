---
title: "Scaling audit baseline (workspace map)"
description: "Workspace scaling map keyed to contracts/scaling/policy.yaml baseline_id; high-level crate/subsystem notes and refresh via vox ci scaling-audit emit-reports."
category: "Architecture SSOTs"

schema_type: "TechArticle"
training_eligible: false
archived_date: 2026-04-18
---

# Scaling audit baseline (workspace map)

**Baseline id:** see `contracts/scaling/policy.yaml` → `baseline_id`.

This file anchors the crate inventory for scaling workstreams. Authoritative crate list: directories under `crates/` containing `Cargo.toml` (workspace members; excludes are listed in root `Cargo.toml`).

## Subsystems (high level)

| Area | Path | Scaling notes |
|------|------|----------------|
| Compiler / tooling | `crates/vox-compiler`, `vox-lsp` | CPU/memory per unit; incremental builds |
| Runtime / workflows | `crates/vox-runtime`, `vox-workflow-runtime` | LLM latency, actor mailboxes |
| Orchestration | `crates/vox-orchestrator` | Locks, budgets, agent caps |
| Data | `crates/vox-db`, `vox-corpus` | Remote RTT, CAS growth |
| Mens / ML | `crates/vox-populi`, `vox-schola`, `vox-cli` mens | GPU memory, corpus I/O |
| MCP / protocol | `crates/vox-mcp`, `vox-protocol` | Tool handler throughput |
| CI | `crates/vox-cli` `ci`, `.github/workflows` | Self-hosted capacity, feature matrix |

## Refresh

After adding/removing crates, run:

`cargo run -p vox-cli -- ci scaling-audit emit-reports`

to regenerate `contracts/reports/scaling-audit/**`.

