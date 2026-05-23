---
title: "Capability registry SSOT"
description: "Transport-independent capability IDs for CLI, MCP, Mens, and model manifests."
category: "Architecture SSOTs"
status: "current"
sort_order: 12
last_updated: "2026-03-31"
training_eligible: false
training_rationale: "Key architecture constraints and definitions required for agent context"

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Capability registry SSOT

Vox maps **semantic capabilities** (what an agent or human is allowed to do) separately from **transports** (CLI, MCP, runtime builtins, HTTP). The machine-readable source of truth lives under **`contracts/capability/`**.

## Canonical artifacts

| Artifact | Role |
|----------|------|
| [`contracts/capability/capability-registry.yaml`](../../../contracts/capability/capability-registry.yaml) | **Generated** from [`catalog.v1.yaml`](../../../contracts/operations/catalog.v1.yaml) (`capability:` block + curated projections); do not hand-edit |
| [`contracts/capability/capability-registry.schema.json`](../../../contracts/capability/capability-registry.schema.json) | JSON Schema for the YAML |
| [`contracts/capability/model-manifest.generated.json`](../../../contracts/capability/model-manifest.generated.json) | Planner-oriented manifest (generated; do not hand-edit) |

The Rust crate **`vox-capability-registry`** loads the document, validates cross-registry consistency against the MCP tool registry and **active** CLI paths from [`contracts/cli/command-registry.yaml`](../../../contracts/cli/command-registry.yaml) (also catalog-projected), and builds the model manifest.

## ID conventions

- **Curated IDs** use dotted namespaces such as `mcp.vox_oratio_transcribe` or `cli.repo.status` and must align with real registry paths or MCP tool names when `cli_paths` / `mcp_tool` are set.
- **Implicit MCP**: when `auto_mcp_capabilities` is true, every tool in `contracts/mcp/tool-registry.canonical.yaml` receives `mcp.<tool_name>` unless exempted.
- **Implicit CLI**: when `auto_cli_capabilities` is true, every active `vox-cli` path in the command registry receives `cli.<segment1>.<segment2>…` unless the path appears under `exemptions.cli_paths` (umbrella commands that are intentionally not one-to-one with a single capability).

## CI and local workflows

- **`vox ci command-compliance`** — JSON Schema validation for `capability-registry.yaml`, parse + `validate_cross_registry` (curated CLI paths and MCP tools must exist).
- **`vox ci capability-sync [--write]`** — Regenerates or verifies `model-manifest.generated.json` from the live capability doc + MCP + CLI registries. **`ssot-drift`** runs capability-sync in verify-only mode after command-compliance.
- **MCP** — read-only tool **`vox_capability_model_manifest`** returns the same merged JSON live from the workspace root (no args), for agents connected to `vox-mcp`.
- **CLI (`--features dei`)** — **`vox dei workspace …`**, **`vox dei snapshot …`**, **`vox dei oplog …`**, and **`vox dei takeover-status`** (aggregated handoff JSON) share payloads with MCP tools via **`vox_orchestrator::json_vcs_facade`**.

## Agent VCS and codegen contracts

- [`contracts/orchestration/agent-vcs-facade.schema.json`](../../../contracts/orchestration/agent-vcs-facade.schema.json) — JSON Schema `$defs` for snapshot list, workspace status, oplog list, and takeover-handoff bundle.
- [`contracts/orchestration/vox-generate-code-file-outcomes.schema.json`](../../../contracts/orchestration/vox-generate-code-file-outcomes.schema.json) — optional **`meta.file_outcomes`** when **`vox_generate_code`** writes **`output_path`** (optional **`post_write_snapshot_id`** when **`vcs_agent_id`** is set).
- [`contracts/repository/repo-path-resolution.schema.json`](../../../contracts/repository/repo-path-resolution.schema.json) — documents **`vox_repository`** path-safety mode names shared by MCP writes and repo catalog.
- [`contracts/repository/repo-workspace-status.schema.json`](../../../contracts/repository/repo-workspace-status.schema.json) — discovery payload for **`vox repo status`** and **`vox_repo_status`** (same `RepoWorkspaceStatus` struct in **`vox_repository`**).
- [`contracts/repository/vox-project-scaffold-result.schema.json`](../../../contracts/repository/vox-project-scaffold-result.schema.json) — success payload for **`vox_project_init`** / **`vox_project_scaffold::ScaffoldSummary`** (shared with **`vox init`** file layout).

## Naming across transports

- **MCP** — tool ids use **`vox_snake_case`** in [`tool-registry.canonical.yaml`](../../../contracts/mcp/tool-registry.canonical.yaml).
- **CLI** — segments use **kebab-case**; implicit capability ids join segments with dots (e.g. **`vox dei workspace create`** ↔ **`cli.dei.workspace.create`**).

| Surface | Example |
|---------|---------|
| CLI | `vox repo status` |
| MCP | `vox_repo_status` |
| Implicit capability | `cli.repo.status` / `mcp.vox_repo_status` |
| CLI | `vox init …` |
| MCP | `vox_project_init` |
| Implicit capability | `cli.init` / `mcp.vox_project_init` |

Cross-repo catalog queries stamp **`CrossRepoQueryTrace.source_plane`** as **`cli`** or **`mcp`** via **`vox_repository::repo_query_*_with_plane`**.

## Visualization

Concrete view sketches and data sources: [Capability visualization views](capability-visualization-views.md). Until those ship, use **`vox_capability_model_manifest`**, **`vox dei takeover-status`**, and **`vox ci capability-sync`** for inspection.

After editing **capability metadata**, change [`contracts/operations/catalog.v1.yaml`](../../../contracts/operations/catalog.v1.yaml) (operation rows + `capability:` block), then:

```powershell
cargo run -p vox-cli -- ci operations-sync --target capability --write
cargo run -p vox-cli -- ci capability-sync --write
```

(from the repo root; Bash equivalent: same args after `cargo run -p vox-cli --`.)

## Mens and legacy aliases

Mens-oriented chat tool schemas may still accept legacy capability labels such as `oratio.transcribe`; canonical curated IDs in the registry use **`mcp.vox_oratio_*`**. Parameter schemas are resolved in **`vox-capability-registry`** (`mens_chat_parameters`).

## Runtime builtins vs CLI / MCP

Language builtins such as `std.fs` / path / process helpers are **not** the same transport as MCP tools or `vox` CLI commands. Where semantics align, `capability-registry.yaml` may list **`runtime_builtin_maps`** so planners see a single capability id across surfaces. Prefer MCP or CLI for repo-scoped, policy-governed work; keep builtins for in-script sandboxed I/O. Detailed interop tiers: [Interop tier policy](interop-tier-policy.md).

## Source of truth

**Edit only** [`contracts/operations/catalog.v1.yaml`](../../../contracts/operations/catalog.v1.yaml). Regenerate `capability-registry.yaml` with `vox ci operations-sync --target capability --write`. Implicit `mcp.*` / `cli.*` coverage plus curated rows stay enforced via **`vox ci command-compliance`** / **`vox ci operations-verify`**.

## Related docs

- [Command compliance](../reference/command-compliance.md) — full `command-compliance` matrix
- [CLI reference](../reference/cli.md) — human-facing needles for `ref_cli_required` paths
- [MCP exposure from the Vox language](mcp-vox-language-exposure.md) — how `@mcp.tool` relates to shipped tools
- [Operations catalog SSOT](operations-catalog-ssot.md) — unified operation identity and MCP/CLI projections


