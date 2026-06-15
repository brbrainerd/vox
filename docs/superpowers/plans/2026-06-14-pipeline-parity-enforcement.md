# Pipeline Parity Enforcement — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. The *contract* this plan implements is [`docs/src/architecture/pipeline-parity-ssot-2026-06-14.md`](../../src/architecture/pipeline-parity-ssot-2026-06-14.md) — read it first.

> **⚠ Audit correction (2026-06-15) — read before executing.** A 5-agent read-only audit verified this plan against `main`. The architecture is unchanged and sound; four execution assumptions drifted (the gap-remediation work already landed most of the catch-all removal). **Corrections, in priority order:**
> 1. **Wave 2 is smaller and concentrated, not a symmetric 4-emitter fan-out.** `codegen_rust/emit/stmt_expr.rs` and `eval/expr.rs` are **already exhaustive** with explicit coded arms (`compile_error!` / `EvalError`) — Tasks 4 and 7 change from "remove catch-alls" to "route the existing explicit arms through the matrix and add the exhaustiveness regression test." The real silent-drop work is **~6 sites, all in TS/web**: `codegen_ts/hir_emit/mod.rs` lines 222, 878, 1178, 1181, 1979/1980, and `web_ir/lower.rs` line 250. Re-anchor Tasks 5 & 6 on these exact lines, not the stale counts ("13+", "8").
> 2. **`emit_unsupported` must NOT return one `Diagnostic` type across crates.** vox-codegen uses `miette::Error` + a local `WebIrDiagnostic`, never `vox_compiler::Diagnostic`. Rename to `unsupported_diagnostic(feature, target) -> UnsupportedCell { code, message }`; `support()` (returning the `&str` code) is the shared truth; each emitter adapts via a one-line native helper. See SSOT §3.2 [CORRECTED].
> 3. **Task 2's premise is wrong.** `build.rs` has no single `--mode` string site — `--mode` is `app|library` only; target intent is scattered across `BuildMode`, `RunMode` (script/interp/app), `CompileKind`, `BuildTarget` (server/mobile/client/fullstack), and `RustAppShell` (AxumLocalServer/TauriApp). Task 2 becomes "introduce `Target` as the projection these existing selectors map *into*, with round-trip tests," not "rewrite `--mode` parsing." (This scatter is *more* evidence for the thesis.)
> 4. **Sizes & names.** Matrix = **133 rows × 4 = 532 cells** (56 decorators + 36 expr/stmt + 41 decls; builtins deferred). Canonical names: module/file **`feature_matrix.rs`** (`crate::feature_matrix`; the `run_feature_matrix` CI gate is a different crate + a fn, not a real collision), query **`support()`**, helper **`unsupported_diagnostic() -> UnsupportedCell`** (renamed from `emit_unsupported` — that name collides with the existing `emit_unsupported_endpoint` in vox-codegen-ts, and the helper returns raw `(code, message)`, not a `Diagnostic`, per item 2). The three builtin registries have **three different shapes** (struct-array / HashMap / nested-match), so Task 10's "prove the three lists agree" needs an extractor per registry — eval has no flat list. All four named diagnostic codes DO exist; the real `Diagnostic` ctor is `Diagnostic::error(message, span, source).with_code(code)`.

**Goal:** Make a partially-wired Vox feature a *build error*, not a runtime surprise, by introducing a `Target` enum, a `Feature` support matrix, and a hard build gate that kills silent `_ => …` catch-alls across all four emit targets.

**Architecture:** A single `feature_matrix.rs` declares, for every language feature, whether each target `Implemented`s it or declares it `Unsupported(code)`. Rust's own exhaustiveness checking (no catch-alls) forces every new feature/target to be handled everywhere; a `feature_matrix_parity_test` proves the declared matrix matches real emitter behavior. Both run in the required `Check, Build, and Test (Rust)` gate.

**Tech Stack:** Rust (vox-compiler, vox-codegen, vox-codegen-ts), the existing `typeck::diagnostics::codes` registry. (Reuse the repo's existing snapshot harness for any golden fixtures; do not introduce a new test framework.)

**Windows constraints (MANDATORY):** NEVER `cargo fmt --all` (os error 206) — use `cargo fmt -p <crate>`. NEVER `cargo test --workspace` — use `cargo test -p <crate>`. Scripts are `.vox`, not `.ps1`/`.sh`/`.py`.

**Parallelism contract:** Wave 1 is sequential (defines shared types). Wave 2 fans out — one agent per emitter, each touching a *disjoint* file tree (`codegen_rust` / `codegen_ts` / `web_ir` / `eval`); no Wave-2 agent edits `feature_matrix.rs`'s shape or another emitter's files. Wave 3 is sequential (needs all columns present).

---

## Wave 1 — Foundation (SEQUENTIAL, one agent, lands alone)

### Task 1: The `Target` enum

**Files:**
- Create: `crates/vox-compiler/src/target.rs`
- Modify: `crates/vox-compiler/src/lib.rs` (add `pub mod target;`)
- Test: `crates/vox-compiler/src/target.rs` (inline `#[cfg(test)]`)

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_target_round_trips_through_id() {
        for t in Target::ALL {
            let id = t.id();
            assert_eq!(Target::from_id(id), Some(t), "id {id} must round-trip");
        }
    }

    #[test]
    fn all_contains_every_variant() {
        // Guards against adding a variant but forgetting ALL.
        assert_eq!(Target::ALL.len(), 4);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-compiler target:: 2>&1`
Expected: FAIL — `Target` not defined.

- [ ] **Step 3: Write minimal implementation**

```rust
//! The single source of truth for "which backend consumes a lowered program."
//! Adding a variant deliberately breaks every parity-checked match (see feature_matrix.rs).
//! NOT #[non_exhaustive] — see SSOT §3.1: we WANT downstream matches to break on a new
//! variant; #[non_exhaustive] would force a `_` arm, reintroducing the silent catch-all.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Target {
    Interpreter,
    RustAxum,
    RustTauri,
    TypeScript,
}

impl Target {
    pub const ALL: [Target; 4] =
        [Target::Interpreter, Target::RustAxum, Target::RustTauri, Target::TypeScript];

    // NOTE [CORRECTED 2026-06-15]: expose `id()`/`from_id()` returning canonical
    // target ids ("interp", "rust-axum", "rust-tauri", "typescript"), NOT
    // `cli_mode()` — there is no CLI `--mode` value that maps to these targets
    // (`--mode` is `app|library`). The selector→Target projection is Task 2.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Target::Interpreter => "interp",
            Target::RustAxum => "rust-axum",
            Target::RustTauri => "rust-tauri",
            Target::TypeScript => "typescript",
        }
    }

    #[must_use]
    pub fn from_id(id: &str) -> Option<Target> {
        Target::ALL.into_iter().find(|t| t.id() == id)
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-compiler target:: 2>&1`
Expected: PASS.

- [ ] **Step 5: Format + commit**

```bash
cargo fmt -p vox-compiler
git add crates/vox-compiler/src/target.rs crates/vox-compiler/src/lib.rs
git commit -m "feat(compiler): add Target enum as SSOT for emit backends"
```

### Task 2: Make `Target` the projection of the existing selectors

> **[CORRECTED 2026-06-15] — reframed.** There is **no single `--mode` parsing site** to rewrite. `vox build --mode` is `app|library` only (`BuildMode`); the interp/script/tauri/web intent is spread across `RunMode` (`run.rs`), `CompileKind`, `vox_config::BuildTarget` (`server`/`mobile`/`client`/fullstack), and `RustAppShell` (`AxumLocalServer`/`TauriApp`, chosen at call sites like `compile.rs:rust_app_shell_for_compile_app`). So this task does not *replace* a parser — it makes `Target` the one thing all those selectors **project into**, with round-trip tests proving the mapping is total and unambiguous.

**Files:**
- Modify: `crates/vox-compiler/src/target.rs` (add `From`/`from_*` projections — keep the type's home in vox-compiler)
- Modify: the call sites that derive `RustAppShell` (`crates/vox-cli/src/commands/compile.rs`, and any other `RustAppShell::…` selection) so the shell is derived from a `Target`, not re-decided independently
- Test: `crates/vox-compiler/src/target.rs` (inline) + `crates/vox-cli/tests/target_projection_test.rs` (create)

- [ ] **Step 1: Inventory the selector sites.** Grep for the real selectors — `BuildMode`, `RunMode`, `CompileKind`, `BuildTarget`, `RustAppShell` — and list every place that decides "which backend." (The 2026-06-15 audit found these in `vox-cli-core/src/cli_args.rs`, `vox-cli/src/commands/run.rs`, `compile.rs`, `build.rs`.) Note which already imply a unique `Target` and which need a documented mapping.

- [ ] **Step 2: Write the failing test** — assert each selector maps to exactly one `Target`, and that the `Target → RustAppShell` projection is correct (`RustAxum → AxumLocalServer`, `RustTauri → TauriApp`).

```rust
#[test]
fn compile_kind_maps_to_target_and_shell() {
    // CompileKind::Desktop/MobileAndroid/MobileIos -> Target::RustTauri -> RustAppShell::TauriApp
    // CompileKind::NativeBinary/Server            -> Target::RustAxum  -> RustAppShell::AxumLocalServer
    // (Adapt to the real CompileKind variants found in Step 1.)
}
```

- [ ] **Step 3: Run** `cargo test -p vox-compiler target:: && cargo test -p vox-cli target_projection 2>&1` — expect FAIL.

- [ ] **Step 4: Implement** the projections on `Target` (`from_compile_kind`, `rust_app_shell()`, etc.) and refactor each `RustAppShell` decision site to call `target.rust_app_shell()` instead of re-deciding from a `CompileKind`/string. Do **not** invent a `--mode`→`Target` parser that does not exist.

- [ ] **Step 5: Run** `cargo build -p vox-cli && cargo test -p vox-compiler target:: && cargo test -p vox-cli target_projection 2>&1` — expect PASS.

- [ ] **Step 6: Format + commit**

```bash
cargo fmt -p vox-compiler -p vox-cli
git add crates/vox-compiler/src/target.rs crates/vox-cli/tests/target_projection_test.rs
git commit -m "refactor(vox-cli): derive RustAppShell from Target; project selectors into the SSOT enum"
```

### Task 3: The `Feature` enum + FULLY-SEEDED matrix + `unsupported_diagnostic` helper

> This task populates **every** cell (all features × all four targets) and creates the shared helper Wave 2 calls. After this task, Wave 2 agents never edit this file. This is the keystone that makes the parallel wave conflict-free — do not defer cells to Wave 2.

**Files:**
- Create: `crates/vox-compiler/src/feature_matrix.rs`
- Modify: `crates/vox-compiler/src/lib.rs` (add `pub mod feature_matrix;`)
- Modify: `crates/vox-compiler/src/typeck/diagnostics.rs` (register any new `Unsupported` codes)

- [ ] **Step 1: Enumerate features.** Read `crates/vox-compiler/src/lexer/token.rs` (all `At*` decorator tokens) and `crates/vox-compiler/src/hir/nodes/` (every `HirExpr`/`HirStmt`/`Decl` variant). Build the `Feature` enum from them. Start with **decorators + expr/stmt/decl kinds** (builtins are added in Task 10 to keep this reviewable — but all decorator/expr/stmt/decl cells are fully seeded here for all four targets).

- [ ] **Step 2: Write the failing test** — exhaustiveness is the test:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::target::Target;

    #[test]
    fn matrix_is_total() {
        // Every (Feature, Target) must return a Support without panicking.
        for f in Feature::ALL {
            for t in Target::ALL {
                let _ = support(f, t); // must not panic; compile-time exhaustiveness does the rest
            }
        }
    }

    #[test]
    fn every_unsupported_code_is_registered() {
        use crate::typeck::diagnostics::codes::ALL_COMPILER_DIAGNOSTIC_CODES;
        for f in Feature::ALL {
            for t in Target::ALL {
                if let Support::Unsupported(code) = support(f, t) {
                    assert!(
                        ALL_COMPILER_DIAGNOSTIC_CODES.contains(&code),
                        "{f:?}/{t:?} declares unregistered code {code}"
                    );
                }
            }
        }
    }
}
```

- [ ] **Step 3: Run** `cargo test -p vox-compiler feature_matrix 2>&1` — expect FAIL.

- [ ] **Step 4: Implement** `Feature` (with `ALL`), `Support`, and `support(Feature, Target) -> Support` as an **exhaustive match** (no `_` arm). Seed **every cell for all four targets** from SSOT §2 (current-state) and the SSOT §7.2 R1–R15 seed table: known-wired → `Implemented`; known-unsupported → `Unsupported(<code>)`. Use existing codes (`mens-decorator-unimplemented`, `pii-unimplemented`, `embed-unimplemented`, `unlowered-decl`) where they fit; add new codes to the `codes` registry in this same step where they don't.

- [ ] **Step 5: Create the shared helper.** In the same file:

```rust
/// The (code, message) for an unsupported (feature, target) cell. Crate-agnostic
/// on purpose: vox-codegen does NOT use vox_compiler::Diagnostic (it errors via
/// miette::Error / WebIrDiagnostic), so the helper returns raw data and each
/// emitter adapts it into its own channel (SSOT §3.2 [CORRECTED]).
pub struct UnsupportedCell {
    pub code: &'static str,
    pub message: String,
}

/// Called by every emitter when it reaches a feature its target doesn't implement.
/// Single home; emitters import this rather than hand-rolling the code/message.
#[must_use]
pub fn unsupported_diagnostic(feature: Feature, target: Target) -> UnsupportedCell {
    match support(feature, target) {
        Support::Unsupported(code) => UnsupportedCell {
            code,
            message: format!("{feature:?} is not supported by the {} target", target.id()),
        },
        Support::Implemented => {
            unreachable!("unsupported_diagnostic called for an Implemented cell")
        }
    }
}
```
Each emitter adapts the cell to its native error: `compile_error!("{code}: {message}")` in codegen_rust, a `WebIrDiagnostic`/throw in codegen_ts, an `EvalError` in eval, a `Diagnostic::error(message, span, src).with_code(code)` in typeck.

- [ ] **Step 6: Run** `cargo test -p vox-compiler feature_matrix 2>&1` — expect PASS.

- [ ] **Step 7: Format + commit**

```bash
cargo fmt -p vox-compiler
git add crates/vox-compiler/src/feature_matrix.rs crates/vox-compiler/src/lib.rs crates/vox-compiler/src/typeck/diagnostics.rs
git commit -m "feat(compiler): add fully-seeded Feature support-matrix SSOT + unsupported_diagnostic helper"
```

**WAVE 1 GATE:** Tasks 1–3 must be merged before Wave 2 starts. They define the shared types Wave 2 imports.

---

## Wave 2 — Emitter fan-out (PARALLEL, one agent per task, disjoint files)

Each task makes *one* emitter route its unsupported-feature handling through `unsupported_diagnostic(feature, target)` (created in Task 3) and adds an exhaustiveness regression test. **No Wave-2 task edits `feature_matrix.rs`** — the matrix is already fully seeded. A Wave-2 agent touches only its emitter's files and its own exhaustiveness test. If you find a cell the matrix got wrong, do NOT edit it here — note it for Wave 3, which reconciles matrix vs reality sequentially.

> **[CORRECTED 2026-06-15] — Wave 2 is smaller and concentrated than originally drawn.** The audit verified that `codegen_rust/emit/stmt_expr.rs` (`emit_expr_with`, exhaustive with explicit `compile_error!` arms at 749–789) and `eval/expr.rs` (exhaustive `EvalError` arms at 781–807) **already have no silent feature-enum catch-alls** — Tasks 4 and 7 therefore *route the existing explicit arms through the matrix and add a regression test*, they do not remove catch-alls. The real remaining silent-drop sites are all TS/web: `codegen_ts/hir_emit/mod.rs` lines 222, 878, 1178, 1181, 1979–1980, and `web_ir/lower.rs` line 250. Anchor Tasks 5 & 6 on those exact lines.

### Task 4: route `codegen_rust` unsupported arms through the matrix (Target::RustAxum / RustTauri)

> **[CORRECTED 2026-06-15] — already exhaustive; this is a routing + regression task.** `emit_expr_with` (stmt_expr.rs:749–789) and `emit_stmt` are already exhaustive over `HirExpr`/`HirStmt` with explicit `compile_error!` arms for `Jsx`/`AsyncView`/`WorkflowVersion`/`With` and an `unreachable!` safety net (no silent `_ =>`). So: do **not** hunt for catch-alls to remove. Instead make those explicit arms matrix-*driven* and add a test that locks them in.

**Files:**
- Modify: `crates/vox-codegen/src/codegen_rust/emit/stmt_expr.rs` (the explicit unsupported arms at 749–782)
- Test: `crates/vox-codegen/tests/rust_emit_exhaustiveness_test.rs` (create)
- (Do NOT edit `crates/vox-compiler/src/feature_matrix.rs`.)

- [ ] **Step 1:** For each hand-written `compile_error!("vox.codegen_rust.…")` arm, find the matching `(Feature, Target)` cell; confirm the matrix declares it `Unsupported(code)` with a registered code.
- [ ] **Step 2:** Write a failing test asserting each unsupported expr emits the *declared* code (via `unsupported_diagnostic(feature, Target::RustAxum/RustTauri).code`), not an ad-hoc string.
- [ ] **Step 3:** Run it — expect FAIL.
- [ ] **Step 4:** Replace the ad-hoc `compile_error!` string literals with `compile_error!("{code}: {message}")` derived from `vox_compiler::feature_matrix::unsupported_diagnostic(feature, target)`. Keep the `unreachable!` safety net (it guards delegate-order bugs, not features).
- [ ] **Step 5:** `cargo test -p vox-codegen rust_emit_exhaustiveness 2>&1` — expect PASS. `cargo build -p vox-codegen` clean.
- [ ] **Step 6:** `cargo fmt -p vox-codegen` + commit `"fix(codegen-rust): route unsupported-expr arms through the parity matrix"`.

### Task 5: `codegen_ts` — close the real silent drops (Target::TypeScript)

**Files:**
- Modify: `crates/vox-codegen-ts/src/hir_emit/mod.rs` — the **verified** silent-drop sites: `WorkflowVersion => String::new()` (878), `emit_hir_pattern` `_ => "_"` losing Constructor/Tuple (1181) and its inner literal `_` (1178), the bind fallback emitting an inert `onChange` (222), and the kwarg `_ => None` (1979–1980). `emit_hir_expr` (297) / `emit_hir_stmt` (1088) are already exhaustive — do not add catch-alls there.
- Test: `crates/vox-codegen-ts/tests/ts_emit_exhaustiveness_test.rs` (create)

- [ ] Same TDD shape: for each site, route through `unsupported_diagnostic(feature, Target::TypeScript)` and emit a `WebIrDiagnostic`/throw carrying the declared code instead of a silent empty/`_`/inert value. **Note the `#[path]` embedding trap**: when feature `standalone` is OFF, `crate::` in vox-codegen-ts resolves to vox-codegen — use `super::` for intra-module refs. Commit `"fix(codegen-ts): replace silent drops with declared Unsupported diagnostics"`.

### Task 6: `web_ir/lower` — make the fallback declared (frontend projection)

**Files:**
- Modify: `crates/vox-codegen/src/web_ir/lower.rs` — the primary `DomArena::lower_expr` `_ =>` fallback at **line 250** (`expr_fallback_count++` → `DomNode::Expr`). (The stmt no-op at 72 is benign name-collection; leave it.)
- Test: `crates/vox-codegen/tests/web_ir_exhaustiveness_test.rs` (create)

- [ ] Same TDD shape. The R14/R15 fixes (match-arm `expr_fallback_count`, if-branch non-Expr guard) are the template for "declared, counted, not silent" — assert the fallback count is exposed and that genuinely-unsupported exprs surface the matrix's declared code. Commit `"fix(web-ir): make the lowering fallback declared and matrix-checked"`.

### Task 7: route `eval` unsupported arms through the matrix (Target::Interpreter)

> **[CORRECTED 2026-06-15] — already exhaustive; this is a routing + regression task.** `eval_expr` (eval/expr.rs:781–807) is already exhaustive with explicit `EvalError` arms for `Jsx`/`AsyncView`/`Spawn`/`With`/`WorkflowVersion` (no silent `_ =>`). This is the *good* model. The task is to make those errors carry the matrix's declared code and add a regression test, not to remove a catch-all.

**Files:**
- Modify: `crates/vox-compiler/src/eval/expr.rs` (the explicit unsupported arms at 781–807)
- Test: `crates/vox-compiler/tests/interp_exhaustiveness_test.rs` (create)

- [ ] Make each unsupported arm return an `EvalError` carrying `unsupported_diagnostic(feature, Target::Interpreter).code`; add a test asserting a new `HirExpr` variant cannot silently fall through (the match stays exhaustive) and that each unsupported construct yields its declared code. Commit `"fix(eval): route interpreter unsupported-expr arms through the parity matrix"`.

**WAVE 2 GATE:** all four columns explicit + all four emitters catch-all-free before Wave 3.

---

## Wave 3 — The gate (SEQUENTIAL)

### Task 8: `feature_matrix_parity_test` — matrix vs reality

**Files:**
- Create: `crates/vox-codegen/tests/feature_matrix_parity_test.rs`
- Create: minimal `.vox` fixtures under `examples/parity/` (one tiny program per feature)

- [ ] **Step 1:** For each `(Feature, Target)` where `support == Implemented`, drive its fixture through that target; assert non-degenerate output (no panic, no `Unsupported` diagnostic, non-empty).
- [ ] **Step 2:** For each `Unsupported(code)` cell, assert the pipeline emits *exactly that code*.
- [ ] **Step 3:** Run — fix any matrix-vs-reality mismatch by correcting the *matrix* (if a cell lied) or the *emitter* (if a feature regressed).
- [ ] **Step 4:** Commit `"test(parity): feature-matrix vs reality gate across all targets"`.

### Task 9: CI grep-gate — no new silent catch-alls

**Files:**
- Modify: the arch-check / CI gate config (a forbidden-pattern rule, mirroring the existing `raw-git-exec` rule shape)

- [ ] Add a forbidden-pattern check: a feature-enum `match` arm of the form `_ =>` in the four emitter files is a violation, with a reason pointing at this plan. Allowlist any genuinely non-feature matches explicitly. Commit `"ci(arch): forbid silent feature catch-alls in emitters"`.

### Task 10: Builtins matrix backfill

**Files:**
- Modify: `crates/vox-compiler/src/feature_matrix.rs` (add builtin features)
- Test: extend `feature_matrix_parity_test`

- [ ] Reconcile the three builtin registries (SSOT §2.3) against the matrix: every builtin name becomes a `Feature` with a per-target support cell; the parity test then proves the three hand-maintained lists agree. Commit `"feat(compiler): bring builtins under the feature matrix; prove registry parity"`.

---

## Final review

- [ ] Re-read SSOT §6 Done Criteria; confirm each of the 6 holds with a command + its output (use superpowers:verification-before-completion — evidence, not assertion).
- [ ] Run `cargo run -p vox-arch-check` — clean (no new warnings).
- [ ] Map each R1–R15 / 2026-06-13-audit gap to a matrix cell; record the mapping in the SSOT appendix so the audit becomes a regression baseline.
- [ ] Dispatch the final code-reviewer subagent over the whole branch, then superpowers:finishing-a-development-branch.

## Self-review notes (author)

- **Spec coverage:** Artifacts A/B/C of SSOT §3 map to Waves 1/1/2-3 respectively; Done Criteria §6.1–6.6 map to Tasks 1–2 / 3 / 4–7 / 8 / 3+10 / final-review. No SSOT requirement is unassigned.
- **Type consistency:** `Target`, `Feature`, `Support`, `support()`, `unsupported_diagnostic()` are named identically in SSOT and plan; `feature_matrix.rs` is `crates/vox-compiler/src/feature_matrix.rs` everywhere. `Target` exposes `id()`/`from_id()` (not `cli_mode`).
- **Parallel-safety (resolved, not conditional):** the only shared file, `feature_matrix.rs`, is written in full in Wave 1 (Task 3) and corrected in Wave 3 (Task 8) — both single-agent sequential waves. Wave 2's agents call `unsupported_diagnostic()` but never edit the matrix, so the fan-out has no shared mutable state. There is no "serialize if it gets conflict-prone" fallback; the design is conflict-free by construction.
- **`#[non_exhaustive]` deliberately omitted** from `Target`/`Feature` (SSOT §3.1): the gate depends on downstream matches breaking when a variant is added, which `#[non_exhaustive]` would prevent.
