---
title: "IR emission SSOT (HIR, WebIR, VoxIrModule)"
description: "Which CLI flags emit which JSON, and how they relate to ADR 012 WebIR."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-04-11"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# IR emission SSOT (HIR, WebIR, VoxIrModule)

## Three artifacts

| Artifact | Role | Typical consumer |
|----------|------|-------------------|
| **HIR** | Compiler-internal module after parse + lower + typecheck. | `vox-compiler` codegen, diagnostics. |
| **WebIR** | Validated frontend projection (DOM, behaviors, routes, interop). | TS/TSX emitters, `validate_web_ir`, Syntax-K / parity tests. See [ADR 012](../adr/012-internal-web-ir-strategy.md). |
| **VoxIrModule** | Stable JSON **bundle**: HIR-shaped `module` fields plus optional `module.web_ir`. | `vox check --emit-ir`, external auditors, agent tooling. |

Lowering today: `lower_hir_to_vox_ir` copies HIR vectors and sets `web_ir: Some(lower_hir_to_web_ir(hir))` when lowering runs.

## CLI emission (authoritative)

| Command | Output path | JSON root |
|---------|-------------|-----------|
| `vox check path/to/file.vox --emit-ir` | `path/to/file.vox-ir.json` (same directory as the source) | `VoxIrModule` (`version`, `metadata`, `module` with all HIR lists + `web_ir` when serialized). |
| `vox build path/to/file.vox --emit-ir` | `<out_dir>/web-ir.v1.json` (default `dist/web-ir.v1.json`) | **`WebIrModule` only** — debugging / parity; **not** a `VoxIrModule`. |

Do not describe `vox build --emit-ir` as “Vox IR”; use **WebIR dump** or **WebIR JSON**.

## JSON Schema (structural)

- Canonical published schema: [`vox-ir.schema.json`](../reference/vox-ir.schema.json) (draft-07, structural: required keys + array shapes).
- Crate mirror (keep in sync): `crates/vox-compiler/src/vox-ir.v1.schema.json`.
- CI: `crates/vox-compiler/tests/ir_emission_test.rs` serializes `lower_hir_to_vox_ir` output to JSON and validates against the docs schema (same shape as `vox check --emit-ir`).

HIR element invariants are enforced by the compiler and tests, not by every field in the JSON Schema (avoid unbounded schema drift).

## Emitter backlog

WebIR completeness vs emitters: [Internal Web IR implementation blueprint](internal-web-ir-implementation-blueprint.md) and the OP-\* checklist in that document.

