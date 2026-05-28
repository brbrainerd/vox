---
title: "Compiler IR Pipeline"
description: "Reference for the Vox Intermediate Representation emission, validation, and its role in machine-verifiable agent loops."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-04-11"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Compiler IR Pipeline

The Vox compiler features a structured Intermediate Representation (IR) pipeline that enables machine-verifiable introspection of programs. This pipeline is critical for high-fidelity agentic workflows, such as the "Doubt" loop and automated resolution agents.

## IR emission

The primary way to obtain a **full `VoxIrModule` JSON** bundle is:

```bash
vox check main.vox --emit-ir
```

This runs the full compiler frontend (lex, parse, typecheck) and writes `main.vox-ir.json` next to the source file.

`vox build … --emit-ir` writes **`web-ir.v1.json`** under the output directory containing **WebIR only** (frontend projection), not the full Vox bundle. See [IR emission SSOT](ir-emission-ssot.md) for the authoritative table.

## Validation and quality gates

1. **Structural JSON Schema**: Emitted `VoxIrModule` JSON is validated in CI against [`vox-ir.schema.json`](../reference/vox-ir.schema.json) (required top-level and `module` keys; HIR bodies remain loosely typed in the schema by design). See `crates/vox-compiler/tests/ir_emission_test.rs`.
2. **Semantic smoke**: That test asserts representative `functions` / `server_fns` entries round-trip from a small fixture after the full frontend.
3. **Golden `.vox`**: Every `examples/golden/**/*.vox` file is parsed, lowered, WebIR-validated, and checked for `legacy_ast_nodes` in `crates/vox-compiler/tests/golden_vox_examples.rs` (runs under the default workspace `nextest` CI job). Example layout + mdBook include policy is centralized in `examples/examples.ssot.v1.yaml` and enforced by `crates/vox-compiler/tests/examples_ssot.rs`.
4. **WebIR gates**: With `VOX_WEBIR_VALIDATE=1`, `web_ir_lower_emit` and `projection_parity` tests guard the TS/TSX pipeline (see `.github/workflows/ci.yml`).

**TOESTUB / completion-policy** applies to **Rust product code**, not to emitted IR JSON. Do not conflate skeleton detection on `crates/` with IR file validation.

## Role in the AI ecosystem

The IR pipeline provides a structured target for AI agents:

- **Auditing**: Resolution agents can analyze the IR without re-parsing `.vox` source.
- **Code generation**: Emitters consume HIR and/or WebIR depending on the target.
- **Documentation**: Prefer `{{#include}}` from `examples/golden/` so snippets stay parser-verified.

## Related

- [IR emission SSOT (check vs build)](ir-emission-ssot.md)
- [Vox IR Specification](../reference/vox-ir-specification.md)
- [Ecosystem & Tooling](../how-to/how-to-cli-ecosystem.md)


