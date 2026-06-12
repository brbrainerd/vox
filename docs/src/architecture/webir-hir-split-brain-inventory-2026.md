---
title: "WebIR / HIR split-brain inventory (2026)"
description: "Concrete inventory of dual codegen paths, projection seams, Tauri hooks, and tests that guard against semantic drift between HIR and WebIR."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-05-11"
training_eligible: true
schema_type: "TechArticle"
---

# WebIR / HIR split-brain inventory (2026)

This document is the **baseline split-brain map** for the [WebIR/HIR unification compare-both ADR](../adr/036-webir-hir-unification-compare-both.md). It lists where semantics are duplicated, bridged, or owned by a single projection so follow-up work can remove drift without guessing.

## Pipeline shape (authoritative)

| Stage | Crate / module | Role |
| --- | --- | --- |
| AST → HIR | `vox-compiler` / `hir::lower` | Typed module shape (`HirModule`), migration fields, endpoints, reactive components. |
| HIR → WebIR | `vox-codegen` / `web_ir::lower` | `lower_hir_to_web_ir` — routes, styles, behaviors, DOM arena + `view_roots`. |
| WebIR validate | `vox-codegen` / `web_ir::validate` | Structural + a11y + route contract gates (`VOX_WEBIR_VALIDATE`, default on). |
| HIR → TS (legacy compat) | `vox-codegen` / `codegen_ts::hir_emit` | `emit_hir_expr` / JSX string emit; **must stay aligned** with `web_ir::emit_tsx` attribute matrix (`hir_emit::compat`, `web_ir::primitives`). |
| HIR → TS (reactive bridge) | `vox-codegen` / `codegen_ts::reactive` | WebIR `emit_component_view_tsx` is **canonical** for `view:`; `emit_hir_expr` is parity-only; blocking WebIR issues fail fast (see `ReactiveViewBridgeStats`). |
| HIR → AppContract | `vox-compiler` / `app_contract` | HTTP + `@endpoint` surface (also `bundle.app` via [`projection_bundle`](../../../crates/vox-codegen/src/projection_bundle.rs)). |
| HIR → RuntimeProjection | `vox-compiler` / `runtime_projection` | DB plan snapshots + task capability hints (**explicitly not WebIR**); also `bundle.runtime`. |
| HIR → ShellProjection | `vox-compiler` / `shell_projection` | Typed `@back_button` / `@deep_link` / `@push` mirror consumed as `bundle.shell` (e.g. `mobile_emit`). |
| HIR → RequiredRuntimeCapabilities | `vox-compiler` / `required_capabilities` | Sorted capability id set for packaging (`bundle.capabilities` → Tauri projection subset in `vox compile`). |
| HIR → `mobile.ts` | `vox-codegen` / `codegen_ts::mobile_emit` | Emits from **`ShellProjectionModule`** (not raw `HirModule` fields). |
| Tauri packaging | `vox-tauri-codegen` | `tauri.conf.json` hints under `target/generated/tauri-packaging/`; see [packaging SSOT](vox-application-packaging-ssot-2026.md). |

## Dual-path / parity hotspots

### 1. Reactive `view:` — WebIR canonical (legacy fallback removed)

- **Files:** [`crates/vox-codegen-ts/src/reactive/mod.rs`](../../../crates/vox-codegen-ts/src/reactive/mod.rs), [`crates/vox-codegen/src/web_ir/emit_tsx.rs`](../../../crates/vox-codegen/src/web_ir/emit_tsx.rs), [`crates/vox-codegen-ts/src/hir_emit/mod.rs`](../../../crates/vox-codegen-ts/src/hir_emit/mod.rs).
- **Mechanism:** Validated WebIR view TSX is **always** selected for the emitted return; `emit_hir_expr` runs **only** to classify parity (`WebIrViewEmitted` vs `WebIrViewEmittedParityMismatch`). Blocking `validate_web_ir` diagnostics or missing Web IR view roots **fail fast** (placeholder return + `reactive_view_emit_failures`).
- **Risk:** Divergence in JSX attribute names, primitive kwargs (`web_ir::primitives` vs `transform_hir_view_kwargs`), or handler `await` threading.
- **Mitigation in tree:** Shared `map_jsx_attr_name`, `expand_bind_hir_attribute` / `lower_jsx_attr_pair`, `transform_hir_view_kwargs` used from `web_ir::lower`.
- **Tests:** `crates/vox-compiler/tests/reactive_smoke_test.rs` (`reactive_codegen_uses_webir_canonical_view`), `web_ir_lower_emit_test.rs`, integration `pipeline` tests.

### 2. Route manifest — WebIR-first

- **Files:** [`crates/vox-codegen-ts/src/route_manifest.rs`](../../../crates/vox-codegen-ts/src/route_manifest.rs) (`WebIrModule` route trees; comment `// Source: WebIR RouteTree → TS`).
- **Risk:** Low if `lower_hir_to_web_ir` is the single route-tree source; HIR-only route emitters must not bypass WebIR for manifest rows.

### 3. Mobile / shell primitives — `ShellProjection` + required capabilities

- **Files:** [`crates/vox-codegen-ts/src/mobile_emit.rs`](../../../crates/vox-codegen-ts/src/mobile_emit.rs), [`crates/vox-compiler/src/shell_projection.rs`](../../../crates/vox-compiler/src/shell_projection.rs), [`crates/vox-compiler/src/required_capabilities.rs`](../../../crates/vox-compiler/src/required_capabilities.rs), [`crates/vox-compiler/src/hir/nodes/decl.rs`](../../../crates/vox-compiler/src/hir/nodes/decl.rs) (`HirFieldOwnership::Shell` for `back_button` / `deep_link` / `push`).
- **Risk:** Packaging permissions drifting from actual module needs.
- **Mitigation:** `RequiredRuntimeCapabilities` drives a **filtered** `runtime-capabilities.projection.json` on `vox compile` (full YAML mirror when no HIR is available, e.g. `vox init` templates).

### 4. Tauri Rust command path — stub only

- **Files:** [`crates/vox-codegen/src/codegen_rust/emit/tauri_stub.rs`](../../../crates/vox-codegen/src/codegen_rust/emit/tauri_stub.rs) (banner only; Axum remains primary for `@endpoint`).
- **Risk:** Future `#[tauri::command]` emission must reuse the same capability resolution as web + mobile.

## Anti–split-brain guards (tests)

| Test / artifact | What it proves |
| --- | --- |
| [`crates/vox-compiler/tests/projection_parity_test.rs`](../../../crates/vox-compiler/tests/projection_parity_test.rs) | Bundle + per-projection canonical bytes are **deterministic**; bundle fixture asserts **distinct hashes** and expected `capability_ids`; **`@back_button`** fixture guards shell + triplet stability. |
| `web_ir_lower_emit_test.rs` | Lower + validate + serde round-trip + validator edge cases. |
| `reactive_smoke_test.rs` | `reactive_codegen_uses_webir_canonical_view` (non-ignored) pins Path C WebIR emit + bridge stats. |
| `vox-arch-check` `[[forbidden_pattern]]` (`projection-bundle-*-emit-boundary` in `layers.toml`) | Bans direct `lower_hir_to_web_ir` / `project_*` in `codegen_ts/**` and `codegen_rust/**` outside `projection_bundle.rs`. |
| `web_ir_environment_gates_test.rs` | `VOX_WEBIR_VALIDATE` fail-closed behavior. |

## Quantitative grep anchors (maintenance signal)

Rough reference counts under `crates/vox-codegen` (subject to churn):

- `emit_hir_expr` — concentrated in `hir_emit/mod.rs` (compat string emitter + WebIR lowering helper).
- `lower_hir_to_web_ir` / `validate_web_ir` — `web_ir/*`, `emitter.rs`, `build.rs`, integration pipeline.
- **Interpretation:** Unification is not “delete WebIR calls” until `hir_emit` JSX paths are retired or folded behind a single printer; see ADR **Option B** recommendation.

## Related

- [Explanation: Compiler lowering phases](../explanation/expl-compiler-lowering.md)
- [ADR 012 — Internal web IR strategy](../adr/012-internal-web-ir-strategy.md) (historical rationale; superseded pointers in ADR header)
- [ADR 036 — WebIR/HIR unification compare-both](../adr/036-webir-hir-unification-compare-both.md)
- [Vox application packaging SSOT (2026)](vox-application-packaging-ssot-2026.md)
