---
name = "populi-mesh"
description = "Align mens node labels with orchestrator task hints and inspect local/remote registry visibility."

[metadata]
"vox-id" = "vox.populi"
"vox-version" = "0.1.0"
"vox-author" = "vox-team"
"vox-category" = "infrastructure"
"vox-tools" = ["vox_populi_local_status", "vox_orchestrator_status", "vox_submit_task"]
"vox-tags" = ["mens", "labels", "gpu", "federation", "workers"]
"vox-permissions" = ["db_read"]
---

# Vox Populi (mesh) skill

Use this when tasks must land on **specific worker pools** (CPU vs GPU, region, pool name) in a **multi-process** setup.

## Label contract (SSOT)

1. **Workers** set the same tokens on every node: **`VOX_MESH_LABELS`** (comma-separated) and/or **`Vox.toml` `[mens].labels`**.
2. **Tasks** pass the same strings under **`vox_submit_task` → `capabilities.labels`** (same shape as **`TaskCapabilityHints`** in `vox-orchestrator`).
3. **Federation:** `vox_orchestrator_status` can include a cached mens snapshot when **`VOX_ORCHESTRATOR_MESH_CONTROL_URL`** (or `[mens].control_url`) is set — **read-only visibility**, not remote execution.
4. **Experimental:** **`VOX_ORCHESTRATOR_MESH_ROUTING_EXPERIMENTAL`** may adjust **local** routing scores using remote label hints — still **no remote execute**; see ADR 008 addendum.

## Tools

- **`vox_populi_local_status`** — env + on-disk registry for this process.
- **`vox_orchestrator_status`** — agent counts + optional **`populi_federation_cache`** / live **`mesh_snapshot`**.
- **`vox_submit_task`** — set **`capabilities.labels`**, **`prefer_gpu_compute`**, **`gpu_cuda` / `gpu_metal`** to match worker advertisements.

## Anti-patterns

- Do **not** invent new label spellings per task; mirror the operator's mens config exactly.
- Do **not** assume tasks run on remote mens nodes; placement beyond local queues requires future product scope.
