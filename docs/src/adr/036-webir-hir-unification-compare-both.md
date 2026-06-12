---
title: "ADR 036 — WebIR vs HIR unification (compare-both)"
description: "Scores full IR merge vs core+projection; records decision, rubric, and follow-up execution gates for GUI-any-platform and AI-first goals."
category: "Architecture Decisions (ADRs)"
status: "current"
last_updated: "2026-05-11"
training_eligible: true
training_rationale: "ADR locks IR/projection decision for GUI-any-platform and AI-first codegen; trains agents on canonical WebIR+HIR posture."
schema_type: "TechArticle"
---

# ADR 036 — WebIR vs HIR unification (compare-both)

## Status

**Accepted** (2026-05-11): adopt **Option B — HIR semantic core + WebIR (and sibling projections) as typed lowering targets**, with explicit work to **collapse duplicate emit paths** and **wire platform capability contracts** into packaging/codegen. Option A (fold WebIR into HIR as a single IR type) is **rejected** for the current product phase.

## Context

- The compiler pipeline is staged **HIR → WebIR → TS/Rust emitters**; see [Explanation: Compiler lowering phases](../explanation/expl-compiler-lowering.md).
- `HirModule` already mixes semantic core, app-contract surfaces, migration-only vectors, and mobile primitives; see [HIR `HirFieldOwnership`](../../../crates/vox-compiler/src/hir/nodes/decl.rs).
- WebIR exists to centralize **web UI structure + validation** that string emitters duplicated (ADR 012 rationale); reactive views now prefer WebIR TSX when validation passes ([`reactive.rs`](../../../crates/vox-codegen-ts/src/reactive/mod.rs)).
- Tauri is a **packaging + webview shell** around the same Axum+Vite/React core; native command lowering is still a stub ([`tauri_stub.rs`](../../../crates/vox-codegen/src/codegen_rust/emit/tauri_stub.rs)).
- Baseline split-brain map: [WebIR/HIR split-brain inventory (2026)](../architecture/webir-hir-split-brain-inventory-2026.md).

## Decision drivers (goals)

| Goal | Implication for IR shape |
| --- | --- |
| **GUI on any platform** | Need **multiple projections** (web UI, HTTP contract, runtime/orchestration, Tauri/mobile manifests) from one semantic program — not one monolithic IR that encodes DOM + HTTP + DB policy in one node type. |
| **AI-first** | Minimize **duplicate pattern-match surfaces** and ambiguous emit paths; maximize **compiler-verified** structure and stable machine-readable artifacts for agents/tests. |

## Options compared

### Option A — Full unification (WebIR folded into HIR)

- **Definition:** DOM/style/route contract nodes become native `Hir*` variants; remove `WebIrModule` / `lower_hir_to_web_ir` / `validate_web_ir` as separate stages.
- **Pros:** One serialized IR type; fewer named pipeline stages.
- **Cons:** HIR becomes a **god-IR** for every target (Rust server, React, Tauri, future native GUI). Every typecheck/lint pass pays the cost of UI DOM nodes. Higher regression blast radius for non-UI work. Conflicts with existing **projection parity** model (`AppContract`, `RuntimeProjection` already separate).

### Option B — Core + projection (chosen)

- **Definition:** Keep **`HirModule` as semantic core**; keep **`WebIrModule` as the web UI projection** with `validate_web_ir` gates; add/extend **contract-driven** projections for Tauri/mobile (e.g. [`runtime-capabilities.v1.yaml`](../../../contracts/capability/runtime-capabilities.v1.yaml)) so packaging does not fork from `@uses` metadata.
- **Pros:** Preserves separation of concerns; matches shipped **triplet parity** tests ([`projection_parity_test.rs`](../../../crates/vox-compiler/tests/projection_parity_test.rs)) including a fixture with **`@back_button`**; allows GUI-any-platform emitters without polluting HIR with DOM-only invariants.
- **Cons:** Requires discipline: **no second “shadow” UI IR**; retire legacy string emitters as WebIR coverage reaches parity (M3 in lowering doc).

## Scoring rubric (1 = worst, 5 = best)

| Criterion | Weight | Option A | Option B | Notes |
| --- | --- | --- | --- | --- |
| AI-first (single canonical story for agents) | 2× | 2 | 4 | A reduces *names* but increases *HIR complexity* agents must reason about. B keeps “semantic vs web projection” as teachable boundary. |
| Maintenance / blast radius | 2× | 2 | 4 | A touches all HIR consumers for every UI tweak. B localizes UI churn to `web_ir/*` + TS emit. |
| GUI-any-platform extensibility | 2× | 2 | 5 | B maps naturally to **new projections** (Tauri manifest, mobile plist). A risks encoding platform packs into HIR ad hoc. |
| Migration risk | 1× | 2 | 4 | A is a Big Bang rewrite of lowering + validators + serde snapshots. B is incremental (already underway). |
| Testability | 1× | 3 | 5 | B already has WebIR gates + projection parity; A must recreate equivalent coverage inside HIR tests. |
| **Weighted sum** (higher better) | — | **20** | **39** | — |

## Decision

Adopt **Option B** as the plan-of-record. Track **Option A** only as a hypothetical end-state if the team later proves a single IR reduces *total* complexity after B has eliminated legacy emitters and shrank `HirModule` migration surface (see GUI-native roadmap Phase 2 primitive collapse).

## Consequences

1. **Documentation:** Lowering explainer + packaging SSOT link this ADR and the [split-brain inventory](../architecture/webir-hir-split-brain-inventory-2026.md).
2. **Implementation:** Continue **WebIR as canonical reactive view**; `hir_emit` JSX remains compat for non-reactive paths until fully retired.
3. **Platform:** Emit **machine-readable capability projection** beside Tauri hints from [`runtime-capabilities.v1.yaml`](../../../contracts/capability/runtime-capabilities.v1.yaml) (wired in `vox-tauri-codegen`); future work merges module `@uses` into that projection.
4. **Non-goals (this ADR):** Changing public `.vox` syntax; deleting `HirModule` mobile fields without a lowering target in WebIR or a sibling `ShellIr` projection (separate ADR if needed).

## Verification

- `cargo test -p vox-compiler --test projection_parity_test` (bundle + per-projection determinism, **`@back_button`**, and bundle fixture `capability_ids` + distinct hashes)
- `cargo test -p vox-tauri-codegen`
- `cargo test -p vox-compiler --test web_ir_environment_gates_test` (or full `web_ir_lower_emit_test` subset in CI as already configured)
- `cargo run -p vox-arch-check` (includes **`projection-bundle-*-emit-boundary`** forbidden patterns in `layers.toml`)

## References

- [ADR 012 — Internal web IR strategy](012-internal-web-ir-strategy.md)
- [External frontend interop plan (2026)](../architecture/external-frontend-interop-plan-2026.md)
- [Vox application packaging SSOT (2026)](../architecture/vox-application-packaging-ssot-2026.md)
