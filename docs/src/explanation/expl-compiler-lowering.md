---
title: "Explanation: Compiler Lowering Phases"
description: "Official documentation for Explanation: Compiler Lowering Phases for the Vox language. Detailed technical reference, architecture guides,"
category: "Concepts"
last_updated: "2026-05-11"
training_eligible: true

schema_type: "TechArticle"
---
# Explanation: Compiler Lowering Phases

Understand how the Vox compiler transforms high-level source code into optimized Rust and TypeScript output.

**Operational summary (non-archive):** Parser → **HIR** → **WebIR** (with `lower_hir_to_web_ir` + `validate_web_ir`) → **`codegen_rust` / `codegen_ts`** inside [`vox-compiler`](../../../crates/vox-compiler). Milestones **`M1–M3`** and the operational catalog live in [`internal-web-ir-implementation-blueprint.md`](../architecture/internal-web-ir-implementation-blueprint.md). Archived research links below are **historical** context only.

Implementation note: current production code keeps these stages under `crates/vox-compiler/src/` with explicit modules for parser, HIR lowering, typecheck, and dual-target emitters.

## 1. Syntax to AST (Abstract Syntax Tree)

The **parser** converts the raw `.vox` file into a tree of declarations. This phase ensures the code is syntactically valid but does not yet understand types or decorators.

## 2. AST to HIR (High-level Intermediate Representation)

The **Lowering** phase begins by transforming the AST into the HIR.
- **Symbol Resolution**: Linking variable names to their definitions.
- **Decorator Processing**: Expanding decorators like `@endpoint(kind: server)` into their underlying architectural primitives (handlers, endpoints, clients).
- **Type Inference**: Deducing types for all expressions.

## 3. HIR to WebIR and LIR (Low-level intermediate layers)

[ADR 012](../adr/012-internal-web-ir-strategy.md) introduces **WebIR** (`crates/vox-codegen/src/web_ir/`) as the normative structured layer before React/TanStack printers. **`lower_hir_to_web_ir`** lowers reactive `view:` trees (from view-call syntax) into **`WebIrModule`**; **`validate_web_ir`** checks DOM id references; **`emit_component_view_tsx`** provides WebIR TSX projection used by the reactive bridge.

Current production behavior (important for migration planning):

- `codegen_ts` still assembles production TS/TSX output on the primary path.
- `VOX_WEBIR_VALIDATE` runs WebIR lower/validate as a fail-fast gate (default on).
- Reactive `view:` emission is **WebIR-only** (no legacy env toggle): validated WebIR TSX is canonical; blocking validate failures or missing Web IR view roots fail fast with diagnostics recorded in [`ReactiveViewBridgeStats::reactive_view_emit_failures`](../../../crates/vox-codegen/src/codegen_ts/reactive.rs); `emit_hir_expr` is used **only** for parity classification (`WebIrViewEmitted` vs `WebIrViewEmittedParityMismatch`).

Migration milestones for deprecating legacy emit reliance:

- **M1 (shipped):** WebIR validate gate defaults on.
- **M2 (shipped):** Reactive bridge emits WebIR view output even when parity differs.
- **M3 (shipped):** Legacy reactive env toggle removed; fail-fast on blocking WebIR validate errors for reactive views; `vox-codegen::projection_bundle::project_bundle_from_hir` is the SSOT entry for emit-time projections.

**Decision-of-record (2026-05-11):** Keep **HIR as semantic core** and **WebIR as the web UI projection** (do not fold WebIR into HIR as a single IR type for this phase). Rationale, rubric, and platform follow-ups: [ADR 036](../adr/036-webir-hir-unification-compare-both.md); concrete dual-path inventory: [WebIR/HIR split-brain inventory](../architecture/webir-hir-split-brain-inventory-2026.md).

**Operations catalog + gates:** [WebIR operations catalog](../archive/research-2026-q1/internal-web-ir-implementation-blueprint.md) and [acceptance gates G1–G6](../archive/research-2026-q1/internal-web-ir-implementation-blueprint.md) (includes supplemental **OP-S049–OP-S220** rustc/doc gates). **Roadmap link pass A (OP-S130, OP-S131, OP-S209–OP-S211):** keep lowering docs aligned when renaming validation stages.

Separately, **backend-oriented** lowering remains optimized for Rust emission (database, actors, HTTP). The older “Frontend LIR” label maps to this split: **WebIR** for structured web UI, **HIR emitters** for expedient TS until the printer fully migrates.

### 3b. Projection bundle (emit SSOT)

[`vox_codegen::projection_bundle::project_bundle_from_hir`](../../../crates/vox-codegen/src/projection_bundle.rs) builds one in-memory bundle per module:

- `WebIrModule` (via `lower_hir_to_web_ir` **only inside the bundle**),
- `AppContractModule` (`project_app_contract`),
- `RuntimeProjectionModule` (`project_runtime_from_hir`),
- `ShellProjectionModule` (`project_shell_from_hir` — `@back_button` / `@deep_link` / `@push`),
- `RequiredRuntimeCapabilities` (`project_required_capabilities` — packaging capability id set).

`codegen_ts` / `codegen_rust` emitters consume bundle fields; `vox-arch-check` **forbidden_pattern** rules block direct `lower_hir_to_web_ir` / `project_*` calls under `codegen_ts/**` and `codegen_rust/**` outside this module (see `docs/src/architecture/layers.toml`).

### 3c. HIR to AppContract and RuntimeProjection (contract layers; also in bundle)

Two additional HIR-derived contract layers are authoritative for non-UI emitters and orchestration (also reachable as `bundle.app` / `bundle.runtime`):

- `app_contract::project_app_contract` produces `AppContractModule` (HTTP routes, server/query/mutation functions, client routes, server config).
- `runtime_projection::project_runtime_from_hir` produces `RuntimeProjectionModule` (DB planning policy snapshots and inferred task capability hints).

These projections are generated from the same lowered HIR input as WebIR and are validated in parity tests to prevent split semantic ownership. Prefer **`project_bundle_from_hir`** for emit paths so WebIR, contracts, shell, and required capabilities stay in lockstep.

## 4. Code Generation (Emission)

The final phase where lowered IR is converted into source files:
- **`vox-codegen::codegen_rust`**: Produces generated Rust app files (`src/main.rs`, `src/lib.rs`, API client output, and DB scaffolding).
- **`vox-codegen::codegen_ts`**: Produces TS/TSX output (`App.tsx`/route trees, server-fn wrappers, component files, and generated contracts).

For frontend IR layering and migration phases, see [ADR 012 — Internal web IR strategy](../adr/012-internal-web-ir-strategy.md).
For detailed implementation sequencing, see [Internal Web IR implementation blueprint](../archive/research-2026-q1/internal-web-ir-implementation-blueprint.md).
For ordered file-by-file migration operations, see [WebIR operations catalog](../archive/research-2026-q1/internal-web-ir-implementation-blueprint.md).
For exact current-vs-target representation mapping, see [Internal Web IR side-by-side schema](../archive/research-2026-q1/internal-web-ir-implementation-blueprint.md).
For quantified token+grammar+escape-hatch savings on the canonical app, see [WebIR K-complexity quantification](../archive/research-2026-q1/internal-web-ir-implementation-blueprint.md).
For reproducible counting registries and equation trace, see [WebIR K-metric appendix](../archive/research-2026-q1/internal-web-ir-implementation-blueprint.md).

## 5. Why Lowering Matters?

By having multiple intermediate representations, Vox can perform complex architectural optimizations—like automatically grouping database queries or optimizing actor communication—that would be impossible in a single-pass compiler.

---

**Related Reference**:
- [Architecture Index](expl-architecture.md) — High-level map of the current compiler module layout.
- Historical HIR reference material lives under `crates/vox-compiler/src/hir/` (monolith); older standalone crate docs were folded into this tree.

