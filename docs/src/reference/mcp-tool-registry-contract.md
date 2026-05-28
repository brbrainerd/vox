---
title: MCP tool registry contract
description: Canonical MCP tool registry contract, metadata, and compliance checks.
category: "Language Reference"

schema_type: "TechArticle"
---

# MCP tool registry (contract SSOT)

Machine-readable **MCP tool names, descriptions, `product_lane`, and optional `http_read_role_eligible`** (bell-curve lanes matching CLI `command-registry.yaml`) live in the repository at:

**[`contracts/mcp/tool-registry.canonical.yaml`](../../../contracts/mcp/tool-registry.canonical.yaml)** (from repo root)

JSON Schema: **[`contracts/mcp/tool-registry.schema.json`](../../../contracts/mcp/tool-registry.schema.json)** — enforced by `vox ci command-compliance`.

Rust code consumes this file via **`crates/vox-mcp-registry`** (`build.rs` emits `TOOL_REGISTRY` as `[McpToolRegistryEntry]`).
`vox-mcp`, `vox-corpus`, and `vox-mcp-registry` re-export that table — do not hand-edit duplicate lists in Rust.
**Do not hand-edit** `tool-registry.canonical.yaml`; it is generated from [`contracts/operations/catalog.v1.yaml`](../../../contracts/operations/catalog.v1.yaml) via `vox ci operations-sync --target mcp [--write]` (or `--target all`). `vox ci operations-verify` enforces strict parity (including dispatch + input schema arms + read-role governance vs catalog) before `command-compliance` reruns the same projections.

List tools returned to MCP clients include **`_meta.vox_product_lane`** and **`_meta.vox_http_read_role_eligible`** on each RMCP `Tool` descriptor (see `crates/vox-orchestrator/src/mcp_tools/tools/registry.rs`).

**`vox_repo_status`** — same discovery JSON as **`vox repo status`**; schema [`contracts/repository/repo-workspace-status.schema.json`](../../../contracts/repository/repo-workspace-status.schema.json).

**`vox_project_init`** — scaffolds the same tree as **`vox init`** under the bound repo (optional **`target_subdir`**); success schema [`contracts/repository/vox-project-scaffold-result.schema.json`](../../../contracts/repository/vox-project-scaffold-result.schema.json).

**`vox_generate_code`** — optional **`output_path`** (repository-relative, no `..`) writes validated `.vox` UTF-8 under the bound repo root; on success, **`meta.file_outcomes`** matches [`contracts/orchestration/vox-generate-code-file-outcomes.schema.json`](../../../contracts/orchestration/vox-generate-code-file-outcomes.schema.json). Optional **`vcs_agent_id`** with **`output_path`** triggers a post-write filesystem snapshot and sets **`meta.file_outcomes.post_write_snapshot_id`**. Shared agent VCS JSON (`vox_snapshot_*`, `vox_workspace_*`, `vox_oplog`, `vox dei …`) is described by [`contracts/orchestration/agent-vcs-facade.schema.json`](../../../contracts/orchestration/agent-vcs-facade.schema.json) `$defs`.

- Legacy-only recovery path (disabled by default): set `VOX_ALLOW_LEGACY_MCP_EXTRACT=1` and run `python scripts/extract_mcp_tool_registry.py --allow-legacy write`, then **`python scripts/mcp_registry_fill_product_lanes.py`**.
- Compliance: `vox ci command-compliance` checks the registry YAML against JSON Schema, `product_lane` enums, YAML ↔ `handle_tool_call` wiring, and read-role policy parity with [MCP HTTP read-role governance contract](mcp-http-read-role-governance-contract.md).

Optional **orchestrator daemon IPC pilots** (TCP **`VOX_ORCHESTRATOR_DAEMON_SOCKET`** on MCP as peer): see [Environment variables](env-vars.md) — read umbrella **`VOX_MCP_ORCHESTRATOR_RPC_READS`**, write umbrella **`VOX_MCP_ORCHESTRATOR_RPC_WRITES`**, per-slice overrides (**`*_TASK_*` / `*_AGENT_*`), plus **`VOX_MCP_ORCHESTRATOR_DAEMON_REPOSITORY_ID_STRICT`**.

See also [`contracts/README.md`](../../../contracts/README.md) and [SSOT convergence roadmap](../archive/research-2026-q1/ssot-convergence-roadmap.md).
