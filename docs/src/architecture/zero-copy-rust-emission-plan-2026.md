---
title: "Implementation Plan: Zero-Copy Vox Codegen"
category: "Architecture SSOTs"
status: "current"
---

# Implementation Plan: Zero-Copy Vox Codegen

## Overview
This plan outlines the technical steps to transition Vox from a "Clone-Heavy" Rust backend to a "Zero-Copy" systems-native backend. This is Phase 1 of the Native Code Emission strategy.

## 1. Enrich HIR with Type Information
The primary blocker for smart codegen is that HIR nodes do not know their types. We store resolved types in a centralized map within the `HirModule`.

- [x] **Extend `HirModule` in `crates/vox-compiler/src/hir/nodes/decl.rs`**:
    - Add `pub inferred_types: HashMap<Span, HirType>` field.
- [x] **Update `Checker` in `crates/vox-compiler/src/typeck/checker/mod.rs`**:
    - Add `inferred_types` map to the `Checker` struct.
    - Write resolved types to the map during `check_expr` in `expr.rs`.
- [x] **Refactor `typecheck_hir`**:
    - Pass the module's `inferred_types` to the checker.
    - Resolve borrow-checker conflicts by temporarily taking ownership of the map during checking.

## 2. Implement Smart Ownership Tracking in Codegen
Update `vox-codegen` to use type information to avoid unnecessary `.clone()` calls.

- [x] **Refactor Codegen Layers**:
    - [x] Propagate `inferred_types` through `emit_lib`, `emit_fn`, `emit_stmt`, and `emit_expr_with`.
    - [x] Update all call sites in `stmt_expr.rs`, `stmt_expr_tail.rs`, `workflow.rs`, and `http.rs`.
- [x] **Optimize Identifiers**:
    - [x] Check for `Copy` types: If the inferred type is a primitive (`int`, `bool`, `float`, `char`, `dec`), omit `.clone()`.
- [ ] **Advanced String Optimization**:
    - [ ] **Escape Analysis**: If an identifier is passed to a function that takes a reference (e.g., `str` -> `&str`), emit `&n` or `n.as_str()` instead of `n.clone()`.
    - [ ] **Last-Use Detection**: If the compiler can prove an identifier is used for the last time in a scope, "move" it instead of cloning.

## 3. Native UI Prototype (Visus)
Establish a pathway for non-React UI emission.

- [ ] **Create `crates/vox-native-gui`**:
    - [ ] Define a `NativeRenderer` trait.
    - [ ] Implement a `Slint` or `egui` backend that maps `HirJsxElement` to native widgets.
- [ ] **Update `vox-codegen`**:
    - [ ] Add `--target=native` flag to the CLI.
    - [ ] Branch the `Component` lowering path: `HIR` → `WebIR` (for React) vs `HIR` → `NativeIR` (for systems UI).

## 4. WASM Logic Offloading
Enable high-performance logic offloading to WASM for `@pure` functions.

- [ ] **Identify `@pure` performance-critical functions**:
    - Use the `@intrinsic` decorator (proposed in research) to flag these functions.
- [ ] **Lower to WASM**:
    - Use `vox-wasm-engine` to compile these functions to `.wasm` blobs.
    - Emit Rust glue code in the server that calls into the WASM module instead of executing interpreted logic.

## Verification
- [ ] **Benchmark**: Run the `vox-bench` suite before and after the "Zero-Copy" changes.
- [ ] **Regression**: Ensure `cargo test --workspace` passes, specifically checking `vox-codegen` tests that previously relied on `.clone()` behavior.

---
*Last Updated: 2026-05-12*
*Status: In Progress*
