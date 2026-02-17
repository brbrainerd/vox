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
