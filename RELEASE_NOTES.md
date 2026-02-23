# Vox Compiler Release Notes - 2026-02-16

## New Features
- **Testing Framework**: Native support for `@test` decorated functions.
  - New built-in `assert(condition)` function.
  - Generates Rust unit tests compatible with `cargo test`.
  - Usage: `vox test <file.vox>`.
- **Language Server Protocol (LSP)**: Initial implementation of `vox-lsp`.
  - Supports diagnostics (syntax and type errors).
  - Integration: `vox lsp` command launches the server.
- **Async Code Generation**: Automatic detection of async function calls (e.g. `actor.send`) and generation of `async`/`await` code.

## Improvements
- **Type Checker**: Now validates `@test` function bodies.
- **Error Handling**: Improved error reporting in CLI.
- **Standard Library**: `str()` cast now supports integers and other primitive types.

## Fixes
- Fixed `TupleLit` compilation issue in HIR lowering.
- Resolved unused import warnings in various crates.

---

# Release Notes - Vox Compiler Refactoring & Quality Improvements (2026-02-22)

## Overview
A comprehensive technical refactoring effort has been completed across the Vox compiler codebase. The goal was to split several monolithic modules into smaller, more maintainable submodules, resolve technical debt, fix outstanding integration test failures, and address all `clippy` lint warnings on a workspace-level.

## Key Changes
- **Parser (`vox-parser`)**: Modularized the original parser into logical submodules, explicitly separating errors (`error.rs`) and formatting behaviors (`indent.rs`).
- **Type Checker (`vox-typeck`)**: Decomposed the massive 1,500+ line `check.rs` file into structured domains: `builtins`, `diagnostics`, `env`, `infer`, `ty`, and `unify`.
- **Rust Code Generation (`vox-codegen-rust`)**: Structured `emit.rs` with clean functional borders, fixing `table` schema definition bugs and implicit AST return discrepancies.
- **TypeScript Code Generation (`vox-codegen-ts`)**: Refactored the emitter into topical modules (`jsx`, `component`, `activity`, `routes`, `adt`), repairing redundant closures and duplicate mappings.
- **High-Level Intermediate Representation (`vox-hir`)**: Upgraded public-facing exports to streamline how `lower_module` and `def_map` are composed.

## Testing & Quality
- Fixed compilation failures inside `vox-integration-tests` regarding explicit vs implicit return variants.
- Replaced 100+ structural `clippy` deficiencies: deprecated `map_or` usages natively transformed to `.is_some_and(...)`, resolved single-char `.push_str()` memory traps, collapsed disjointed `else if` chains.
- Achieved a resilient, 100% warning-free build across all crates leveraging `cargo clippy`.
