# Pipeline Parity Enforcement — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. The *contract* this plan implements is [`docs/src/architecture/pipeline-parity-ssot-2026-06-14.md`](../../src/architecture/pipeline-parity-ssot-2026-06-14.md) — read it first.

> **⚠ Audit correction (2026-06-15) — read before executing.** A 5-agent read-only audit verified this plan against `main`. The architecture is unchanged and sound; four execution assumptions drifted (the gap-remediation work already landed most of the catch-all removal). **Corrections, in priority order:**
> 1. **Wave 2 is smaller and concentrated, not a symmetric 4-emitter fan-out.** `codegen_rust/emit/stmt_expr.rs` and `eval/expr.rs` are **already exhaustive** with explicit coded arms (`compile_error!` / `EvalError`) — Tasks 4 and 7 change from "remove catch-alls" to "route the existing explicit arms through the matrix and add the exhaustiveness regression test." The real silent-drop work is **~6 sites, all in TS/web**: `codegen_ts/hir_emit/mod.rs` lines 222, 878, 1178, 1181, 1979/1980, and `web_ir/lower.rs` line 250. Re-anchor Tasks 5 & 6 on these exact lines, not the stale counts ("13+", "8").
> 2. **`emit_unsupported` must NOT return one `Diagnostic` type across crates.** vox-codegen uses `miette::Error` + a local `WebIrDiagnostic`, never `vox_compiler::Diagnostic`. Rename to `unsupported_diagnostic(feature, target) -> UnsupportedCell { code, message }`; `support()` (returning the `&str` code) is the shared truth; each emitter adapts via a one-line native helper. See SSOT §3.2 [CORRECTED].
> 3. **Task 2's premise is wrong.** `build.rs` has no single `--mode` string site — `--mode` is `app|library` only; target intent is scattered across `BuildMode`, `RunMode` (script/interp/app), `CompileKind`, `BuildTarget` (server/mobile/client/fullstack), and `RustAppShell` (AxumLocalServer/TauriApp). Task 2 becomes "introduce `Target` as the projection these existing selectors map *into*, with round-trip tests," not "rewrite `--mode` parsing." (This scatter is *more* evidence for the thesis.)
> 4. **Sizes & collisions.** Matrix = **133 rows × 4 = 532 cells** (56 decorators + 36 expr/stmt + 41 decls; builtins deferred). The names `emit_unsupported` and `feature_matrix` are **already taken** (`emit_unsupported_endpoint` in vox-codegen-ts; `run_feature_matrix` CI gate) — use `parity_matrix.rs` / `unsupported_diagnostic()`. The three builtin registries have **three different shapes** (struct-array / HashMap / nested-match), so Task 10's "prove the three lists agree" needs an extractor per registry — eval has no flat list. All four named diagnostic codes DO exist; the real `Diagnostic` ctor is `Diagnostic::error(message, span, source).with_code(code)`.

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
    fn every_target_round_trips_through_cli_mode() {
        for t in Target::ALL {
            let mode = t.cli_mode();
            assert_eq!(Target::from_cli_mode(mode), Some(t), "mode {mode} must round-trip");
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

    #[must_use]
    pub fn cli_mode(self) -> &'static str {
        match self {
            Target::Interpreter => "interp",
            Target::RustAxum => "script",
            Target::RustTauri => "tauri",
            Target::TypeScript => "web",
        }
    }

    #[must_use]
    pub fn from_cli_mode(mode: &str) -> Option<Target> {
        Target::ALL.into_iter().find(|t| t.cli_mode() == mode)
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

### Task 2: Route `build.rs` through `Target`

**Files:**
- Modify: `crates/vox-cli/src/commands/build.rs` (the `--mode` parsing site)
- Test: `crates/vox-cli/tests/build_target_selection_test.rs` (create)

- [ ] **Step 1: Read `build.rs`** to find the current `--mode` string matching. Note exact function + line.

- [ ] **Step 2: Write the failing test** — assert that the build command maps each `--mode` string to the correct `Target` via `Target::from_cli_mode`, and that an unknown mode is a clean error (not a panic, not a silent default).

```rust
#[test]
fn unknown_mode_is_a_clean_error() {
    // Construct the build args with --mode bogus; assert Err with a helpful message.
    // (Adapt to the actual build entry signature found in Step 1.)
}
```

- [ ] **Step 3: Run** `cargo test -p vox-cli build_target 2>&1` — expect FAIL.

- [ ] **Step 4: Rewrite** the `--mode` site to call `Target::from_cli_mode(mode).ok_or_else(...)`. Replace any downstream `RustAppShell` derivation so `AxumLocalServer`/`TauriApp` is chosen from the `Target`, not re-parsed from the string.

- [ ] **Step 5: Run** `cargo build -p vox-cli && cargo test -p vox-cli build_target 2>&1` — expect PASS.

- [ ] **Step 6: Format + commit**

```bash
cargo fmt -p vox-cli
git add crates/vox-cli/src/commands/build.rs crates/vox-cli/tests/build_target_selection_test.rs
git commit -m "refactor(vox-cli): select emit Target via SSOT enum, reject unknown --mode"
```

### Task 3: The `Feature` enum + FULLY-SEEDED matrix + `emit_unsupported` helper

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
use crate::typeck::diagnostics::{Diagnostic, TypeckSeverity};

/// Called by every emitter when it reaches a feature its target doesn't implement.
/// Single home; emitters import this rather than hand-rolling a diagnostic.
#[must_use]
pub fn emit_unsupported(feature: Feature, target: Target) -> Diagnostic {
    match support(feature, target) {
        Support::Unsupported(code) => Diagnostic::error(code, /* message from feature+target */),
        Support::Implemented => unreachable!("emit_unsupported called for an Implemented cell"),
    }
}
```
(Adapt `Diagnostic::error` to the real constructor found in `diagnostics.rs`.)

- [ ] **Step 6: Run** `cargo test -p vox-compiler feature_matrix 2>&1` — expect PASS.

- [ ] **Step 7: Format + commit**

```bash
cargo fmt -p vox-compiler
git add crates/vox-compiler/src/feature_matrix.rs crates/vox-compiler/src/lib.rs crates/vox-compiler/src/typeck/diagnostics.rs
git commit -m "feat(compiler): add fully-seeded Feature support-matrix SSOT + emit_unsupported helper"
```

**WAVE 1 GATE:** Tasks 1–3 must be merged before Wave 2 starts. They define the shared types Wave 2 imports.

---

## Wave 2 — Emitter fan-out (PARALLEL, one agent per task, disjoint files)

Each task: replace feature-handling `_ => …` catch-alls in *one* emitter with explicit arms, routing genuine gaps through `emit_unsupported(feature, target)` (created in Task 3). **No Wave-2 task edits `feature_matrix.rs`** — the matrix is already fully seeded. A Wave-2 agent touches only its emitter's files and its own exhaustiveness test. If you find a cell the matrix got wrong, do NOT edit it here — note it for Wave 3, which reconciles matrix vs reality sequentially.

### Task 4: `codegen_rust` exhaustiveness (Target::RustAxum / RustTauri)

**Files:**
- Modify: `crates/vox-codegen/src/codegen_rust/emit/stmt_expr.rs` (catch-alls ≈ 979, 988, 1092, 1264)
- Test: `crates/vox-codegen/tests/rust_emit_exhaustiveness_test.rs` (create)
- (Do NOT edit `crates/vox-compiler/src/feature_matrix.rs`.)

- [ ] **Step 1:** Read each catch-all arm; classify each currently-dropped variant as Implemented-elsewhere vs genuinely-unsupported.
- [ ] **Step 2:** Write a failing test asserting that emitting an unsupported expr yields the *declared* diagnostic code (not a panic, not empty output).
- [ ] **Step 3:** Run it — expect FAIL.
- [ ] **Step 4:** Replace catch-alls with explicit arms; unsupported variants call `vox_compiler::feature_matrix::emit_unsupported(feature, Target::RustAxum)` (or `RustTauri`) — the helper from Task 3 — which surfaces `support(...)`'s declared code.
- [ ] **Step 5:** `cargo test -p vox-codegen rust_emit_exhaustiveness 2>&1` — expect PASS. `cargo build -p vox-codegen` clean.
- [ ] **Step 6:** `cargo fmt -p vox-codegen` + commit `"fix(codegen-rust): replace silent catch-alls with declared Unsupported diagnostics"`.

### Task 5: `codegen_ts` exhaustiveness (Target::TypeScript)

**Files:**
- Modify: `crates/vox-codegen-ts/src/hir_emit/mod.rs` (13+ catch-alls; see SSOT §2.4 for line list)
- Test: `crates/vox-codegen-ts/tests/ts_emit_exhaustiveness_test.rs` (create)

- [ ] Same 6-step TDD shape as Task 4, for the TypeScript emitter. **Note the `#[path]` embedding trap** (SSOT context): when feature `standalone` is OFF, `crate::` in vox-codegen-ts resolves to vox-codegen — use `super::` for intra-module refs. Commit `"fix(codegen-ts): replace silent catch-alls with declared Unsupported diagnostics"`.

### Task 6: `web_ir/lower` exhaustiveness (frontend projection)

**Files:**
- Modify: `crates/vox-codegen/src/web_ir/lower.rs` (8 catch-alls)
- Test: `crates/vox-codegen/tests/web_ir_exhaustiveness_test.rs` (create)

- [ ] Same 6-step TDD shape. The R14/R15 fixes (match-arm `expr_fallback_count`, if-branch non-Expr guard) are the template for what "declared, counted, not silent" looks like here. Commit `"fix(web-ir): make lowering catch-alls explicit and counted"`.

### Task 7: `eval` interpreter exhaustiveness (Target::Interpreter)

**Files:**
- Modify: `crates/vox-compiler/src/eval/expr.rs` (fallback block ≈ 781–803)
- Test: `crates/vox-compiler/tests/interp_exhaustiveness_test.rs` (create)

- [ ] Same 6-step TDD shape. The interpreter already errors (good) but at runtime — convert the blanket fallback into explicit per-variant arms so a *new* `HirExpr` variant cannot silently fall into the catch-all. Each unsupported variant returns the matrix's declared code. Commit `"fix(eval): make interpreter unsupported-expr handling exhaustive and matrix-driven"`.

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
- **Type consistency:** `Target`, `Feature`, `Support`, `support()`, `emit_unsupported()` are named identically in SSOT and plan; `feature_matrix.rs` is `crates/vox-compiler/src/feature_matrix.rs` everywhere.
- **Parallel-safety (resolved, not conditional):** the only shared file, `feature_matrix.rs`, is written in full in Wave 1 (Task 3) and corrected in Wave 3 (Task 8) — both single-agent sequential waves. Wave 2's parallel agents call `emit_unsupported()` but never edit the matrix, so the fan-out has no shared mutable state. There is no "serialize if it gets conflict-prone" fallback; the design is conflict-free by construction.
- **`#[non_exhaustive]` deliberately omitted** from `Target`/`Feature` (SSOT §3.1): the gate depends on downstream matches breaking when a variant is added, which `#[non_exhaustive]` would prevent.
