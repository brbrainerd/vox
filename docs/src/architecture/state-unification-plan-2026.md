---
title: "Implementation Plan: State Unification & TS Hardening (2026)"
category: "Architecture SSOTs"
status: "current"
---

# Implementation Plan: State Unification & TS Hardening (2026)

This plan addresses the "split plane" issue in Vox web development by ensuring that structural state (state machines, component state, and reactive modules) is emitted to React/TypeScript with the same fidelity and safety as the Native Rust target.

## Objectives
1. **SSOT State Semantics**: Ensure `.vox` is the single source of truth for both Native (Rust) and Web (React/TS) state logic.
2. **Eliminate Split Plane**: Automate the synchronization of state, props, and effects between client and server via generated code.
3. **Curtail TS Problems**: Enforce non-nullability, sum types (discriminated unions), and structural Z-tiers in the generated TypeScript.
4. **Reduce Maintenance**: Leverage the shared HIR and WebIR to prevent "ballooning maintenance" of duplicate logic paths.

## Phase 1: State Machine Unification
As of TASK-4.1, `state_machine` declarations emit both a pure reducer and a `use<Name>StateMachine` React hook. 

- [ ] **Task 1.1: Multi-target Reducer Verification.** Verify that the emitted `lightReducer` in TS matches the logic of the `LightStateMachine` in Rust.
- [ ] **Task 1.2: Actor-State Bridge.** Enable `actor` state to be projected to the frontend as a reactive "remote state" object, eliminating manual `fetch` calls.

## Phase 2: Reactive Component Hardening (Path C)
Path C (`component { state; view }`) is the current UI model.

- [ ] **Task 2.1: Automated Dependency Tracking.** Refine `extract_state_deps_with_diagnostics` to ensure `useEffect` and `useMemo` in generated TS have 100% accurate dependency arrays.
- [ ] **Task 2.2: Sum Type Codecs.** Ensure all ADTs used in component state have Zod-backed codecs for safe serialization across the network boundary.

## Phase 3: Structural Z-Tier (VUV-TS)
Bring the "VUV Layered Layout Discipline" to the React emitter.

- [ ] **Task 3.1: Tier-Aware Portals.** Update `jsx.rs` to emit React Portals that land in a set of fixed Z-tier containers (`Background`, `Content`, `Popover`, `Modal`, `Toast`, `SystemOverlay`).
- [ ] **Task 3.2: Mark<T> Integration.** Implement `Mark<T>` handles in the TS emitter to replace ad-hoc `id` attributes and enable type-safe cross-tree focus/scroll.

## Phase 4: Maintenance Optimization
- [ ] **Task 4.1: Shared WebIR Projection.** Converge `codegen_rust` and `codegen_ts` on the same `WebIrModule` representation for layout, ensuring that "native-only" UI improvements (like partitioning containers) propagate to the web automatically.
- [ ] **Task 4.2: SSOT Metadata Injection.** Inject `vox-bearer` and other security tokens via the same pipeline that emits the native shell bridge.

## Verification
- `cargo check --workspace` must pass.
- `vox doc-pipeline --mode check` to verify new code snippets.
- Integration tests in `vox-codegen` to verify parity between Rust and TS state transitions.
