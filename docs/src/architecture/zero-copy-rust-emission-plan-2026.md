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

## §2.1 Escape Analysis — grounded implementation plan (Workstream B, scoped 2026-06-05)

*Companion to [`vox-memory-model-audit-and-value-optimization-2026-06-05.md`](vox-memory-model-audit-and-value-optimization-2026-06-05.md) (its "Workstream B"). The interpreter CoW work landed; this is the **codegen** counterpart — eliminating `.clone()` in emitted Rust by borrowing when the callee only reads. Grounded in a code audit of the current ownership machinery.*

### Current state (verified)
- **Last-use / move analysis EXISTS** — `codegen_rust/emit/usage.rs` (`UsageTracker::build` / `is_last_use`); at last use, `emit_ident_expr` emits a bare identifier (move). So the *remaining* clones are for **non-last-use** arguments (value reused later) — and those cannot be elided without the callee borrowing.
- **`OwnershipMode {Owned, Borrowed}`** (`emit/ownership.rs`) is threaded through emit. **`Borrowed` is only ever set for ~9 hardcoded builtin string args** (`is_builtin_arg_borrowed`, `stmt_expr.rs:~903`). **User-defined function calls always pass `OwnershipMode::Owned`** (`stmt_expr.rs:~565`) → every reused argument clones.
- **Type info is available at emit time** (`inferred_types: HashMap<Span, HirType>`) in `emit_ident_expr`.
- **Generated function params are all owned** (`workflow.rs:~181`): `name: String`, `items: Vec<T>` — never `&str`/`&[T]`. There is **no per-parameter ownership metadata** on `HirFn`/`HirParam`.

### Latent bug to fix first (cheap, safe)
`emit_ident_expr` (`stmt_expr.rs:~412`) emits `.as_str()` **unconditionally** in `Borrowed` mode, ignoring the type. It's currently masked because `Borrowed` only fires for string builtins, but ANY expansion to lists would emit `.as_str()` on a `Vec` → uncompilable.
- [ ] **Make borrow emission type-aware**: `String`→`.as_str()` (or `&n`), `Vec<T>`→`&n` / `n.as_slice()`, else `&n`. Add a unit test pinning each. Do this BEFORE widening `Borrowed`.

### The feature (the real work)
- [ ] **1. Param-borrow metadata.** Add per-parameter ownership to `HirFn` (e.g. `params_borrowable: Vec<bool>`), computed by a body analysis: a param is borrowable iff the body only *reads* it (never moved into a return value, stored, mutated, or passed to an owning callee). MVP heuristic: borrowable for `str`/`list[T]` params whose only uses are reads/borrowing-calls; conservative-owned otherwise. (Soundness: when unsure → `Owned`. Never borrow something the callee escapes.)
- [ ] **2. Emit borrowed signatures.** In `workflow.rs` param emission, emit `&str`/`&[T]` for borrowable params; adjust the body to deref as needed.
- [ ] **3. Call-site lookup.** Build a `HashMap<&str, &HirFn>` (or reuse module function table) so the generic-call emitter (`stmt_expr.rs:~565`) can look up the callee and set per-argument `OwnershipMode::Borrowed` for borrowable params — emitting `&x` instead of `x.clone()` for non-last-use args.
- [ ] **4. Replace the hardcoded builtin table** with the same metadata mechanism where feasible (or leave the small table and layer user-fn inference on top).

### Tests / verification
- [ ] New `crates/vox-codegen/tests/escape_analysis_emit.rs`: assert `fn takes(s: str)`+`takes(x)` (x reused) emits a borrow, not `x.clone()`; mixed borrowed/owned params; list params; and **a negative case** (param that escapes → still owned).
- [ ] `cargo test -p vox-codegen` green; emitted Rust still compiles (golden e2e).
- [ ] Optional: a codegen micro-benchmark or generated-LoC `.clone()` count to quantify.

### Risk / sequencing
Signature changes ripple (body deref, return cloning, nested calls), so land **type-aware borrow fix → metadata+inference (conservative) → signatures → call-site borrows**, test-gating each. Soundness rule: **borrow only when provably read-only; default to owned.** This is independent of the interpreter and can proceed on its own branch.

---
*Last Updated: 2026-06-05*
*Status: In Progress (Workstream B / escape analysis grounded & scoped; not yet implemented)*
