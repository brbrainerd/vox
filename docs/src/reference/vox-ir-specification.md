---
title: "Vox IR Specification"
description: "Stability-first Intermediate Representation (IR) for machine-verifiable Vox programs."
category: "Language Reference"
status: current
last_updated: "2026-04-11"

schema_type: "TechArticle"
---

# Vox IR Specification

The **Vox Intermediate Representation (IR)** is the canonical, platform-agnostic, and machine-verifiable **JSON bundle** for a Vox program after type checking. It is primarily produced by `vox check --emit-ir` as a `VoxIrModule` (HIR-shaped `module` plus optional embedded **WebIR**).

## Purpose

1. **Tooling interoperability**: Linters, auditors, and visualizers consume JSON without embedding the compiler.
2. **Deterministic auditing**: Stable target for agentic “Doubt” loops and resolution agents.
3. **Compiler decoupling**: High-level language features vs Rust/TypeScript emitters; frontend validation often targets **WebIR** ([ADR 012](../adr/012-internal-web-ir-strategy.md)).

## Emission

| CLI | Output | Contents |
|-----|--------|----------|
| `vox check path/to/file.vox --emit-ir` | `<stem>.vox-ir.json` beside the source | Full **`VoxIrModule`**: `version`, `metadata`, `module` (HIR lists + `web_ir` when serialized). |
| `vox build path/to/file.vox --emit-ir` | `<out_dir>/web-ir.v1.json` | **WebIR only** — not a `VoxIrModule`. Use for WebIR debugging; use **`vox check --emit-ir`** for the full bundle. |

```bash
vox check main.vox --emit-ir
```

Authoritative naming table: [IR emission SSOT](../archive/research-2026-q1/ir-emission-ssot.md).

## Schema version 2.0.0

The `version` field is `"2.0.0"`. The structural JSON Schema lives at [`vox-ir.schema.json`](./vox-ir.schema.json) (required keys and `module` array fields; individual HIR nodes are intentionally permissive to limit churn).

A crate-local mirror used for tooling alignment: `crates/vox-compiler/src/vox-ir.v1.schema.json` (**keep in sync** with the docs copy).

### Top-level structure (`VoxIrModule`)

| Field | Type | Description |
| :--- | :--- | :--- |
| `version` | `string` | IR schema version (today: `"2.0.0"`). |
| `metadata` | `VoxIrMetadata` | Compilation context and integrity markers. |
| `module` | `VoxIrContent` | Lowered program logic + optional `web_ir`. |

### Metadata (`VoxIrMetadata`)

| Field | Type | Description |
| :--- | :--- | :--- |
| `compiler_version` | `string` | Version of the `vox` compiler that produced the IR. |
| `generated_at` | `string` | RFC 3339 timestamp of emission. |
| `source_hash` | `string` | SHA3-256 hash of the original `.vox` source file. |

### Content (`VoxIrContent`)

Vectors of lowered constructs (may be empty arrays):

- `imports`, `rust_imports`
- `functions`, `types`
- `routes`, `actors`, `workflows`, `activities`
- `server_fns`, `query_fns`, `mutation_fns`
- `tables`, `mcp_tools`, `mcp_resources`, `agents`
- `web_ir` — optional embedded **WebIR** module (`WebIrModule`); omitted when `None` after serde.

## Stability guarantees

While internal HIR layouts may evolve between compiler versions, **Vox IR** (v2.x) aims for predictable JSON **shape** at the `module` key level. Breaking changes bump `version` and are documented with migration notes.

## Verification

- CI: `crates/vox-compiler/tests/ir_emission_test.rs` lowers a fixture through the full frontend, serializes `VoxIrModule`, and validates against `vox-ir.schema.json` (same JSON shape as `vox check --emit-ir`).
- Golden examples: `crates/vox-compiler/tests/golden_vox_examples.rs` (parse + lower + WebIR validate + Syntax-K metrics).

## Canonical example (`*.vox-ir.json`)

```json
{
  "version": "2.0.0",
  "metadata": {
    "compiler_version": "0.4.0",
    "generated_at": "2026-04-10T12:00:00Z",
    "source_hash": "a1b2c3d4e5f6..."
  },
  "module": {
    "imports": [],
    "rust_imports": [],
    "functions": [],
    "types": [],
    "routes": [],
    "actors": [],
    "workflows": [],
    "activities": [],
    "server_fns": [],
    "query_fns": [],
    "mutation_fns": [],
    "tables": [],
    "mcp_tools": [],
    "mcp_resources": [],
    "agents": []
  }
}
```

---

**Related**:

- [IR emission SSOT](../archive/research-2026-q1/ir-emission-ssot.md)
- [Compiler IR pipeline](../archive/research-2026-q1/compiler-ir-pipeline.md)
- [Internal Web IR blueprint](../archive/research-2026-q1/internal-web-ir-implementation-blueprint.md)
- [HIR Reference](./hir-legacy-inventory.md)
- [WebIR Strategy](../adr/012-internal-web-ir-strategy.md)

