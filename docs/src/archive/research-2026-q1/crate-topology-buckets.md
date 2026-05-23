---
title: "Crate topology buckets"
description: "Like-with-like map for workspace crates under crates/* and major modules."
category: "Language Reference"
last_updated: "2026-03-26"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Crate topology buckets

**Like-with-like** map for workspace members under `crates/*`. Root `[workspace.exclude]` is only the stub **`vox-py`** tree (no `Cargo.toml`). An optional minimal **`vox-dei`** staging crate may exist under `crates/vox-dei` when checked in; it is not part of the default product graph. Use this when choosing dependencies and file placement.

| Bucket | Crates / location | Notes |
|--------|-------------------|--------|
| **Compiler pipeline** | **`vox-compiler`** | Monolith: `lexer`, `parser`, `ast`, `hir`, `typeck`, `fmt`, `codegen_rust`, `codegen_ts`, `web_ir`, etc. — not separate workspace crates. |
| **Data / Codex** | `vox-db`, `vox-pm` | Canonical DB facade: **`vox_db::VoxDb`**. Schema SSOT in `vox-db` + `vox-pm` artifacts. |
| **Mesh + native ML** | **`vox-populi`**, `vox-tensor`, `vox-corpus`, `vox-oratio` | **Populi** = mesh/registry/HTTP (`transport`). **Mens** ML = `vox_populi::mens` (+ features `mens-train`, `mens-gpu`, …). Gate via **`vox-cli`** `populi`, `gpu`, `oratio`, `mens-candle-cuda`. |
| **Repository / config** | `vox-repository`, `vox-config` | `Vox.toml`, `repository_id` — do not reimplement layout detection ad hoc. |
| **Runtime** | `vox-runtime` | Actor / workflow helpers; optional `database` feature. |
| **HTTP dashboards / Codex APIs** | **`vox-db`** + **`vox-cli`** | Historical name `vox-codex-api` is **not** a package; HTTP helpers live in **`vox-db`** and CLI feature gates. |
| **Agent / MCP / orchestration** | `vox-mcp`, `vox-orchestrator`, `vox-skills`, `vox-tools`, `vox-capability-registry`, `vox-workflow-runtime` | Tooling and routing; often feature-gated in CLI. |
| **Quality / policy** | `vox-toestub`, `vox-socrates-policy`, `vox-eval`, `vox-doc-inventory`, `vox-scaling-policy` | CI and doc SSOT. |
| **Integration** | `vox-integration-tests`, `vox-test-harness` | Not in default `vox-cli` dependency graph. |
| **Product / CLI / tooling** | `vox-cli`, `vox-lsp`, `vox-bootstrap`, `vox-container`, `vox-doc-pipeline`, `vox-forge`, `vox-git`, `vox-ludus`, `vox-skills`, `vox-ssg`, `vox-webhook`, `vox-schola`, `vox-protocol`, `vox-publisher`, `vox-scientia-*` | **`vox-cli`** fans out by feature; keep default builds lean. |

## Anti-patterns

- New `vox_codex::` imports — use **`vox_db::`**.
- Heavy ML deps on `vox-lsp` or default `vox-cli` without a feature gate.
- Duplicating `repository_id` / repo-root logic outside **`vox-repository`**.
- Docs or scripts referring to removed package names **`vox-mens`** / **`vox-codex-api`** — use **`vox-populi`** and **`vox-db`** (see [nomenclature migration map](nomenclature-migration-map.md)).

## Telemetry-driven topology policy

Use `vox ci build-timings` / `--deep` telemetry as the decision gate for crate-organization changes:

- **Module refactor first** when compile regression is localized and dependency-shape metrics remain stable.
- **Feature-gate next** when an optional domain inflates default build lanes but ownership stays cohesive.
- **Split crate last** when both are true over a stable window:
  - sustained lane regression (median and p95 trend, not one noisy run),
  - sustained coupling pressure (fan-in/fan-out hotspot remains in the top set).
- **Fail gate only on sustained regressions** (multi-run corroboration), not single-run spikes.

## See also

- [crate-build-lanes-migration.md](crate-build-lanes-migration.md)
- [vox-cli-build-feature-inventory.md](vox-cli-build-feature-inventory.md)
- [external-repositories.md](../reference/external-repositories.md)


