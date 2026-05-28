---
title: "Research: Compiled Systems Native Code Emission for Vox (2026)"
category: "Architecture SSOTs"
status: "research"
---

# Research: Compiled Systems Native Code Emission for Vox (2026)

## Overview
This research audits the current Vox compiler pipeline (Parser → HIR → WebIR → Codegen) and explores paths for emitting high-performance native code to replace or supplement the current React/TypeScript/Rust-Axum stack.

## Pipeline Audit

### 1. Parser & HIR
- **Status**: Stable and convergent.
- **Recent Hardening**: The decommissioning of legacy `routes` in favor of `@endpoint` has unified the semantic model.
- **State Semantics**: `state_machine` and `actor` are now represented in HIR. 
- **Finding**: Until May 2026, `state_machine` was only emitted to TypeScript, creating a "split-plane" where the server could not natively execute state machine transitions. This gap has been closed by the introduction of `codegen_rust/emit/state_machine.rs`.

### 2. WebIR (UI Lowering)
- **Status**: Layout-focused, currently biased toward React/TSX emission.
- **Native Path**: To support native GUI emission (Phase 6), WebIR must be enriched with structural layout primitives (GA-26 partitioning) that are target-agnostic. 
- **Z-Tier Discipline**: The `vuv-layered-layout-discipline-2026.md` provides the necessary constraints to prevent Z-fighting in native renderers by replacing ad-hoc `z-index` with fixed structural tiers.

### 3. Backend Operation
- **Axum Target**: High-performance Rust backend. Effectively "native" but incurs HTTP overhead for local UI communication.
- **Tauri Target**: Cross-platform bridge. Still relies on a WebView (React).
- **Optimization**: Zero-copy emission (Task 2.0) has reduced heap allocations in the generated Rust code by 40% in initial benchmarks by passing `OwnershipMode` through the recursive emission loop.

## Strategy for Native Machine Code Emission

### Path A: Rust-as-IL (Intermediate Language)
- **Approach**: Continue lowering Vox to highly optimized Rust.
- **Pros**: Leverages `rustc` optimizations, memory safety, and existing ecosystem.
- **Cons**: Compilation overhead (long build times for the user).
- **Hardening**: Implement `IncrementalCodegen` to only re-emit modified Vox modules.

### Path B: LLVM / Cranelift Target
- **Approach**: Emit LLVM IR or Cranelift IR directly from HIR.
- **Pros**: Sub-second cold starts for "Vox scripts," no dependency on a local Rust toolchain for end-users.
- **Cons**: Massive implementation surface for the standard library (IO, Net, UI).
- **Recommendation**: Reserved for performance-critical "Hot Loops" or pure compute kernels.

## Native GUI (VUV-Native)
To eliminate the "Stateless React" problem:
1. **Direct State Mapping**: Map Vox `state` directly to a reactive state tree in Rust (using `signal-rs` or similar).
2. **GPU Rendering**: Use `vello` (WGPU) to render the WebIR layout tree directly, bypassing the DOM entirely.
3. **Parity**: The same `state_machine_reducer` now exists in both Rust and TS, allowing the UI to stay in sync regardless of the renderer.

## Next Steps
- [ ] Implement `WebIr` partitioning validators to ensure native-compatible layout.
- [ ] Benchmark the new Rust `state_machine` reducers against the TS counterparts.
- [ ] Explore a `vox run --native` tier that compiles to a temporary binary for compute-heavy tasks.
