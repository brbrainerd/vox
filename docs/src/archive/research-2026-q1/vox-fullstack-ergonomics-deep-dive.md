---
title: "Vox full-stack ergonomics deep dive"
description: "Repository-grounded full-stack boilerplate hotspots and implementation roadmap for Vox."
category: "Architecture SSOTs"
last_updated: "2026-03-25"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Vox full-stack ergonomics deep dive

## Current full-stack surface map

### Compiler and codegen
- Parser scope and exclusions: `crates/vox-compiler/src/parser/mod.rs`
- HIR declaration model with `legacy_ast_nodes`: `crates/vox-compiler/src/hir/nodes/decl.rs`
- Lowering entry: `crates/vox-compiler/src/hir/lower/mod.rs`
- Rust route emit: `crates/vox-compiler/src/codegen_rust/emit/http.rs`
- TS route emit: `crates/vox-rn-codegen/src/routes.rs`
- Shared path prefixes: `crates/vox-compiler/src/web_prefixes.rs`

### CLI and command contracts
- CLI root and dispatch: `crates/vox-cli/src/lib.rs`, `crates/vox-cli/src/cli_dispatch/mod.rs`
- Command contract files: `contracts/cli/command-registry.yaml`, `contracts/cli/command-registry.schema.json`
- Compliance gates: `crates/vox-cli/src/commands/ci/command_compliance/`
- Command sync generation: `crates/vox-cli/src/commands/ci/command_sync.rs`

### MCP tooling
- Canonical tool registry: `contracts/mcp/tool-registry.canonical.yaml`
- Tool dispatch: `crates/vox-orchestrator/src/mcp_tools/tools/dispatch.rs`
- Input schema definitions: `crates/vox-orchestrator/src/mcp_tools/tools/input_schemas.rs`
- Alias surface: `crates/vox-orchestrator/src/mcp_tools/tools/tool_aliases.rs`
- Metadata subsets: `crates/vox-mcp-meta/src/lib.rs`

### API/data surfaces
- Codex API contract: `contracts/codex-api.openapi.yaml`
- Populi OpenAPI: `contracts/populi/control-plane.openapi.yaml`
- Populi router: `crates/vox-populi/src/transport/router.rs`
- DB facade: `crates/vox-db/src/lib.rs`
- Ludus data integration: `crates/vox-ludus/src/`

## Boilerplate hotspots in current repository
- Parser/docs drift for full-stack declarations and error syntax claims.
- HIR fallback (`legacy_ast_nodes`) causes mixed typed/untyped downstream handling.
- Duplicated route semantics in Rust and TS emitters.
- MCP identity is registry-driven, but behavior/schema wiring remains manual in multiple places.
- CLI command metadata must stay aligned across clap, contract YAML, generated docs, and CI checks.
- Mixed OpenAPI placement (`contracts/` and `schemas/`) increases contributor cognitive overhead.

## Gap-to-action map

### Gap 1: parser and language claims drift
- Execute B001-B010 + E001.
- Outcome: language docs and parser behavior converge; `?` semantics no longer ambiguous.

### Gap 2: typed lowering debt
- Execute C001-C013.
- Outcome: web declarations lower into typed HIR vectors, eliminating fallback-heavy paths.

### Gap 3: route duplication across emitters
- Execute F001-F010.
- Outcome: one route IR drives Rust and TS generation, lowering drift risk.

### Gap 4: command/tool wiring duplication
- Execute H001-H010.
- Outcome: higher single-source generation coverage for CLI and MCP surfaces.

### Gap 5: weak autofix loop
- Execute I001-I012.
- Outcome: actionable diagnostics with safe auto-remediation for common repetitive edits.

## Implementation sequencing

### Wave 1 (foundation)
- Parser/HIR/error/registry/autofix scaffolding.
- Target result: hard architecture debt removed; behavior parity checks active.

### Wave 2 (leverage)
- Syntax ergonomics, type system improvements, shared contracts, data-layer API simplification.
- Target result: visible code-size and effort reduction for common full-stack features.

### Wave 3 (scale)
- Governance, migration hardening, KPIs, and long-term anti-drift automation.
- Target result: sustainable ergonomics with low regression risk.

## Verification framework
- Golden tests for each ergonomics feature.
- CI parity checks for registry/docs/contracts.
- Regression benchmarks for compile behavior and feature implementation touchpoints.
- Migration tests ensuring old syntax/functionality paths fail with useful guidance, not silent breakage.

## Practical guidance for smaller models
- Prefer stream-local edits and tests.
- Do not mix parser, typechecker, and codegen refactors in one PR unless task explicitly demands it.
- For C3/C4 tasks, always include:
  - behavior diff summary,
  - migration notes,
  - risk notes,
  - rollback trigger criteria.


