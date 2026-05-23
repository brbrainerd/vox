---
title: "Legacy retirement roadmap (2026)"
description: "Machine-readable guide identifying retired and retiring code pathways in Vox. Prevents LLMs and contributors from building on deprecated surfaces. Last research audit: 2026-04-06."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-04-10"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Legacy retirement roadmap (2026)

**Purpose:** This document is a navigation guard. Read it before writing new code to avoid building on pathways being retired. It is the companion to [orphan-surface-inventory.md](orphan-surface-inventory.md), [forward-migration-charter.md](forward-migration-charter.md), and [nomenclature-migration-map.md](nomenclature-migration-map.md).

## Critical: do not extend these surfaces

| Surface | Location | Status | Use instead |
|---------|----------|--------|-------------|
| `schema_cutover.rs` | `crates/vox-db/src/schema_cutover.rs` | **Deleted** (FTS moved to `schema_extensions`) | Core schema fragments |
| Ludus cutover module (removed) | (deleted) | **Removed** | Baseline gamification fragments in `schema/domains/` |
| `MemoryManager::recall()` (sync) | `crates/vox-orchestrator/src/memory/manager.rs` | **Incomplete — misses Codex** | Use `recall_async()` |
| `persist_fact()` (sync) | Same | **Loses writes on crash** | Use `recall_async()` / `sync_to_db()` |
| `@component fn Name() to Element` | Vox syntax | **Deprecated** — Path A (classic) | Use `component Name() { state ...; view: }` Path C |
| `hir.components` | `HirModule` | `MigrationOnly`; prefer `hir.reactive_components` | `hir.to_semantic_hir().reactive_components` |
| `TURSO_URL` / `TURSO_AUTH_TOKEN` | env vars | **Deprecated** | `VOX_DB_URL` / `VOX_DB_TOKEN` |
| `VOX_TURSO_URL` / `VOX_TURSO_TOKEN` | env vars | **Deprecated** (interim) | `VOX_DB_URL` / `VOX_DB_TOKEN` |
| `vox_db::codex_legacy` | crate module | Migration helper only | Do not use in new application code |
| `vox_continuous_trainer.ps1` | `scripts/populi/` | **Superseded** | `vox mens corpus` + `vox mens pipeline` |
| `extract_mcp_tool_registry.py` | `scripts/` | **Legacy migration** (requires `VOX_ALLOW_LEGACY_MCP_EXTRACT=1`) | `contracts/mcp/tool-registry.canonical.yaml` |
| Latin `ops_codex/` in `store/` | `crates/vox-db/src/store/ops_codex/` | Mixed naming; no new modules | English domain name, file under correct domain |

## Retirement domains — summary

### 1 · DB schema cutover machinery

**COMPLETED:** `schema_cutover.rs` is fully deleted. `routing_decisions` was ported to baseline. The 10 irrelevant DDL shims were stripped entirely. FTS functions securely sit in `schema_extensions.rs`. `ludus_schema_cutover.rs` and `legacy::apply_ludus_gamify_cutover` are **deleted**; Ludus DDL lives in baseline fragments only.

### 2 · File-based memory (MEMORY.md)

`MEMORY.md` is the *original* persistence layer, predating Codex. The `MemoryManager` now dual-writes to both MEMORY.md (synchronous) and Codex (non-blocking spawn). This dual-write causes:
- Silent write loss on process exit (spawn may not complete)
- Two divergent data sources requiring manual sync
- Synchronous blocking on every memory write

**Direction:** Codex `memories` table is the SSOT. MEMORY.md should become a diagnostic read-only export, not a write target. The `db: Option<Arc<VoxDb>>` field in `MemoryManager` should become non-Optional.

### 3 · Classic `@component fn` path

The compiler maintains two component stacks:

| Form | HIR field | Codegen | Status |
|------|-----------|---------|--------|
| `@component fn Name() to Element { JSX }` | `hir.components` (`MigrationOnly`) | `codegen_ts/component.rs` | **Deprecated** |
| `component Name() { state ...; view: JSX }` | `hir.reactive_components` (`SemanticCore`) | `codegen_ts/reactive.rs` + WebIR | **Canonical** |

**Immediate action needed:** Fix `crates/vox-compiler/src/llm_prompt.rs` — it shows classic `@component fn` syntax. LLMs reading this file learn the wrong form.

### 4 · HIR `MigrationOnly` fields (compiler-named legacy surface)

`HirModule.field_ownership_map()` formally classifies these fields as `MigrationOnly`:
`components`, `v0_components`, `layouts`, `pages`, `contexts`, `hooks`, `error_boundaries`, `loadings`, `not_founds`, `legacy_ast_nodes`, `lowering_migration`

The `SemanticHirModule` projection (`hir.to_semantic_hir()`) excludes all migration-only fields. New compiler code should operate on `SemanticHirModule` where possible.

**Ambiguity alert:** `hir.components` (classic, MigrationOnly) appears before `hir.reactive_components` (canonical, SemanticCore) in the struct declaration. LLMs will prefer the first match unless warned.

### 5 · Legacy env var shim chain

```
TURSO_URL  ──deprecated──►  VOX_TURSO_URL  ──deprecated──►  VOX_DB_URL  (canonical)
TURSO_AUTH_TOKEN            VOX_TURSO_TOKEN                 VOX_DB_TOKEN
```

**Known leak:** `crates/vox-compiler/src/codegen_rust/emit/tables/codegen.rs` emits an error message mentioning `TURSO_URL+TURSO_AUTH_TOKEN`. This surfaces legacy names in user-generated code. Fix this string.

Retirement prerequisite: Clavis `doctor` must warn on deprecated vars + telemetry must confirm zero usage.

### 6 · Training telemetry sidecar DB (`vox_training_telemetry.db`)

May remain on disk from older releases beside `vox.db`. Current code uses `VoxDb::connect_default` only; legacy primary surfaces `LegacySchemaChain` in `crates/vox-db/src/store/open.rs` until migration. Remove or archive after operators complete baseline cutover.

### 7 · Script surface (dead / replaceable)

| Script | Status | Canonical replacement |
|--------|--------|-----------------------|
| `scripts/populi/vox_continuous_trainer.ps1` | **Deleted** | `vox mens corpus` + `vox mens pipeline` |
| `scripts/mens/release_training_gate.*` | **Deleted** | `vox ci mens-gate` |
| Root-level `fix_docs.py`, `*.txt` session artifacts | **Ignored / Deleted** | `.gitignore` or delete |

## Completed retirements (April 2026)
*   **FTS Re-anchoring:** `schema_cutover.rs` deleted.
*   **File-based memory mutability:** Gutted active write path in `MemoryManager::persist_fact`.
*   **Classic @component fn syntax:** Compiler lint and explicit AST deprecated declarations applied.
*   **Stale Env Vars:** Removed `VOX_TURSO_*` dependencies.
*   `vox-scientia-social` zombie crate deleted.

## Partial migrations that block new work

These must be completed before new features can build correctly on top of them:

| Migration | Missing piece | Risk if incomplete |
|-----------|--------------|-------------------|
| Language surface SSOT | `contracts/language/vox-language-surface.json` generator not built | New decorators/keywords require 6-way updates; drift guaranteed |
| CLI command metadata generation | Stream H (boilerplate roadmap) not shipped | Commands added 3 times manually; drift in compliance gate |
| `@component` deprecation lint | Lint exists for `use_*` hooks but not for the classic form itself | LLMs keep generating classic forms |

## What is safe to extend

The following surfaces are stable and canonical — new code should live here:

| Surface | Location | Notes |
|---------|----------|-------|
| Baseline schema domains | `crates/vox-db/src/schema/domains/*.rs` | Add new tables/columns here |
| `HirModule.reactive_components` | Compiler HIR | Canonical component vector |
| `HirModule.agents` / `environments` | Compiler HIR | Latest agent/env declarations |
| `build_repo_scoped_orchestrator` | `crates/vox-orchestrator/src/bootstrap.rs` | Sole factory (ADR 022) |
| `VOX_DB_URL` / `VOX_DB_TOKEN` / `VOX_DB_PATH` | env vars | Canonical Codex config |
| `vox_db::VoxDb` / `Codex` | `crates/vox-db/src/lib.rs` | Facade for all DB ops |
| `vox-skills` | `crates/vox-skills/` | Skills/ARS SSOT (was vox-ars) |
| `vox-orchestrator` | `crates/vox-orchestrator/` | Orchestrator SSOT (was large vox-dei crate) |
| `vox-dei` | `crates/vox-dei/` | HITL Doubt/Resolution logic crate |
| `vox-constrained-gen` | `crates/vox-constrained-gen/` | Grammar-constrained decoding logic |

## Related

- [Orphan surface inventory](orphan-surface-inventory.md) — per-surface keep/port/archive/delete table
- [Forward migration charter](forward-migration-charter.md) — policy (no restore-based workflows)
- [Codex / Arca compatibility boundaries](codex-arca-compatibility-boundaries.md) — DB naming SSOT
- [Nomenclature migration map](nomenclature-migration-map.md) — Latin/English naming SSOT
- [Script surface audit](script-surface-audit.md) — script lifecycle tracking
- [Boilerplate reduction roadmap](vox-boilerplate-reduction-master-roadmap.md) — Stream H (CLI/MCP) and Stream C (HIR debt)
- Research backing: `legacy-retirement-research.md` (conversation artifact, April 2026)


