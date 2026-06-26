# Frontend Emit Validation Gate Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make blocking WebIR validation failures (a11y / contrast / overlay / layer) **fail the frontend build** instead of silently emitting a `FAIL_PLACEHOLDER` — so "bad UI doesn't compile" is actually true on the production `vox build --target client` path.

**Architecture:** The production TS emitter already runs the full validator per reactive component (`reactive/view.rs:72` → `validate_web_ir`) and, on blocking diagnostics, emits a placeholder and records the errors into `ReactiveViewBridgeStats::reactive_view_emit_failures`. But `generate_with_options` **never reads that field back** — it stores the stats into `CodegenOutput` and returns `Ok` regardless (`emitter.rs:624`). This plan adds an opt-in `strict_view_validation` flag and a pure partition-and-decide gate that returns `Err` when *quality* violations are present, while deliberately exempting the `codegen.reactive.no_web_ir_view_root` "not-yet-expressible" fallback (which is coverage, not a defect — that's what the Sub-project A ledger tracks). The CLI `vox build --target client` enables the flag; library/test callers keep today's behavior.

**Tech Stack:** Rust (`vox-codegen-ts`, `vox-cli`). Key symbols: `vox_codegen::codegen_ts::emitter::{generate_with_options, CodegenOptions, CodegenOutput}`; `vox_codegen::codegen_ts::reactive::ReactiveViewBridgeStats` (`reactive_view_emit_failures: Vec<WebIrDiagnostic>`); `vox_codegen::web_ir::WebIrDiagnostic` (`{ code, message, span, category }`); `vox_codegen::web_ir::validate::format_web_ir_validate_failure`.

**Spec:** Complements `docs/superpowers/specs/2026-06-20-vox-native-frontend-ssot-design.md`. Sub-project A formalizes the emission *seam* byte-identically (no behavior change) and therefore does **not** close this gap; this plan is the behavior change that makes the seam's validators bite. Independent of A — wire either at the `vox build` call site (here) or through `emit_frontend` once A lands.

**Execution model:** TDD mandatory — failing test first, observed-output verification before any "done". Tasks 1–4 are sequential (each builds on the prior symbol). Task 5 is optional and `[PARALLEL-SAFE]`.

---

## File Structure

| File | New/Modify | Responsibility |
|---|---|---|
| `crates/vox-codegen-ts/src/emitter.rs` | Modify (`:48` struct, `:77` `from_env`, `:198` `generate_with_options`) | Add `strict_view_validation` option; call the gate before the `Ok` return. |
| `crates/vox-codegen-ts/src/view_validation_gate.rs` | **New** | Pure `view_validation_gate(stats, strict) -> Result<(), String>`: partition failures into quality-violations vs `no_web_ir_view_root` fallback; decide. |
| `crates/vox-codegen-ts/src/lib.rs` (or `codegen_ts/mod.rs`) | Modify (add `mod`) | Register the new module. |
| `crates/vox-cli/src/commands/build.rs` | Modify (the `--target client` emit call site) | Enable `strict_view_validation` for production frontend builds. |

---

## Task 1: Add the `strict_view_validation` option `[SEQUENTIAL base]`

**Files:**
- Modify: `crates/vox-codegen-ts/src/emitter.rs` (struct at `:48`, `from_env` at `:77`)

`CodegenOptions` already `#[derive(... Default ...)]` (`emitter.rs:47`), so a new `bool` field defaults to `false` automatically — no manual `Default` impl to touch. This mirrors the existing `strict_ai` field/env pattern.

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` block in `crates/vox-codegen-ts/src/emitter.rs`:

```rust
#[test]
fn strict_view_validation_defaults_off() {
    assert!(
        !CodegenOptions::default().strict_view_validation,
        "strict_view_validation must default to false so existing callers are unchanged"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-codegen-ts strict_view_validation_defaults_off`
Expected: FAIL — `no field strict_view_validation on type CodegenOptions` (compile error).

- [ ] **Step 3: Add the field**

In `crates/vox-codegen-ts/src/emitter.rs`, inside `pub struct CodegenOptions { … }` (starts `:48`), add directly after the `strict_ai` field (`:56`):

```rust
    /// Fail the frontend build when a reactive view has **blocking** WebIR
    /// validation diagnostics (a11y / contrast / overlay / layer), instead of
    /// emitting a placeholder. Excludes the `codegen.reactive.no_web_ir_view_root`
    /// fallback (a not-yet-expressible component is coverage, not a defect).
    /// Set by `VOX_TS_STRICT_VIEW=1` via [`Self::from_env`]; the production
    /// `vox build --target client` path enables it explicitly.
    pub strict_view_validation: bool,
```

- [ ] **Step 4: Wire `from_env`**

In `fn from_env()` (`:77`), where `Self { … }` is constructed, set the field from the env var (mirroring how `strict_ai` reads `VOX_TS_STRICT_AI`):

```rust
            strict_view_validation: std::env::var("VOX_TS_STRICT_VIEW").as_deref() == Ok("1"),
```

- [ ] **Step 5: Add the env-on test**

```rust
#[test]
fn strict_view_validation_reads_env() {
    // Safety: single-threaded test; restore after.
    std::env::set_var("VOX_TS_STRICT_VIEW", "1");
    assert!(CodegenOptions::from_env().strict_view_validation);
    std::env::remove_var("VOX_TS_STRICT_VIEW");
}
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p vox-codegen-ts strict_view_validation`
Expected: PASS (2 tests).

- [ ] **Step 7: Commit**

```bash
git add crates/vox-codegen-ts/src/emitter.rs
git commit -m "feat(codegen-ts): add strict_view_validation option (default off)"
```

---

## Task 2: Pure validation gate `view_validation_gate` `[SEQUENTIAL after 1]`

**Files:**
- Create: `crates/vox-codegen-ts/src/view_validation_gate.rs`
- Modify: `crates/vox-codegen-ts/src/lib.rs` (or `src/codegen_ts/mod.rs` — wherever sibling `mod emitter;` / `mod reactive;` are declared) to add the module.

The gate is a pure function so it is tested without parsing `.vox` or running the emitter. The plumbing that *populates* `reactive_view_emit_failures` is already covered by `reactive/view.rs:72` plus the existing a11y test `web_ir_validate_a11y_img_missing_alt_fires_via_validate_web_ir` (`crates/vox-compiler/tests/web_ir_lower_emit_test.rs:1705`).

- [ ] **Step 1: Write the failing tests**

Create `crates/vox-codegen-ts/src/view_validation_gate.rs`:

```rust
//! Production build gate: turn **blocking** reactive-view WebIR validation
//! failures into a hard build error, instead of the silent `FAIL_PLACEHOLDER`
//! emitted by `reactive/view.rs`. The `no_web_ir_view_root` fallback is exempt:
//! a component that isn't WebIR-expressible yet is *coverage* (tracked by the
//! frontend coverage ledger), not a UI defect — failing on it would break every
//! un-migrated surface.

use crate::codegen_ts::reactive::ReactiveViewBridgeStats;
use crate::web_ir::validate::format_web_ir_validate_failure;

/// Diagnostics with this code mean "this component's view did not lower to a
/// WebIR view root" — a coverage fallback, NOT a quality violation. Exempt.
const NO_VIEW_ROOT_CODE: &str = "codegen.reactive.no_web_ir_view_root";

/// Returns `Err` with a formatted report when `strict` is set and at least one
/// recorded view-emit failure is a real quality violation (anything other than
/// `NO_VIEW_ROOT_CODE`). Never errors when `strict` is false or when the only
/// failures are no-view-root fallbacks.
#[must_use = "the gate result decides whether the build fails"]
pub fn view_validation_gate(
    stats: &ReactiveViewBridgeStats,
    strict: bool,
) -> Result<(), String> {
    if !strict {
        return Ok(());
    }
    let quality: Vec<_> = stats
        .reactive_view_emit_failures
        .iter()
        .filter(|d| d.code != NO_VIEW_ROOT_CODE)
        .cloned()
        .collect();
    if quality.is_empty() {
        return Ok(());
    }
    Err(format!(
        "frontend build rejected: {} blocking view validation diagnostic(s)\n{}",
        quality.len(),
        format_web_ir_validate_failure(&quality)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen_ts::reactive::ReactiveViewBridgeStats;
    use crate::web_ir::WebIrDiagnostic;

    fn diag(code: &str) -> WebIrDiagnostic {
        WebIrDiagnostic {
            code: code.to_string(),
            message: format!("{code} fired"),
            span: None,
            category: Some("test".to_string()),
        }
    }

    fn stats_with(codes: &[&str]) -> ReactiveViewBridgeStats {
        let mut s = ReactiveViewBridgeStats::default();
        s.reactive_view_emit_failures = codes.iter().map(|c| diag(c)).collect();
        s
    }

    #[test]
    fn quality_violation_fails_under_strict() {
        let s = stats_with(&["web_ir_validate.a11y.img_missing_alt"]);
        let err = view_validation_gate(&s, true).expect_err("must reject");
        assert!(err.contains("img_missing_alt"), "report must name the diag: {err}");
    }

    #[test]
    fn quality_violation_passes_when_not_strict() {
        let s = stats_with(&["web_ir_validate.a11y.img_missing_alt"]);
        assert!(view_validation_gate(&s, false).is_ok());
    }

    #[test]
    fn no_view_root_fallback_is_exempt_even_under_strict() {
        let s = stats_with(&["codegen.reactive.no_web_ir_view_root"]);
        assert!(
            view_validation_gate(&s, true).is_ok(),
            "un-migrated components must not break the build"
        );
    }

    #[test]
    fn empty_failures_pass() {
        assert!(view_validation_gate(&ReactiveViewBridgeStats::default(), true).is_ok());
    }

    #[test]
    fn mixed_fails_on_the_quality_one_only() {
        let s = stats_with(&[
            "codegen.reactive.no_web_ir_view_root",
            "web_ir_validate.overlay.duplicate_z",
        ]);
        let err = view_validation_gate(&s, true).expect_err("must reject the quality diag");
        assert!(err.contains("duplicate_z"));
        assert!(err.contains("1 blocking"), "no-view-root must not be counted: {err}");
    }
}
```

- [ ] **Step 2: Register the module**

In the file declaring the sibling modules (`crates/vox-codegen-ts/src/lib.rs` or `src/codegen_ts/mod.rs` — run `grep -rn "mod emitter" crates/vox-codegen-ts/src` to find it), add:

```rust
pub mod view_validation_gate;
```

Match the surrounding `pub mod` / `mod` visibility convention of the neighbouring declarations. If the imports in Step 1 (`crate::codegen_ts::reactive::…`, `crate::web_ir::…`) don't resolve from this crate's root, adjust the `use` paths to mirror how `emitter.rs` refers to the same items (`super::reactive::…`, `super::web_ir::…`).

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo test -p vox-codegen-ts view_validation_gate`
Expected: PASS (5 tests).

- [ ] **Step 4: Commit**

```bash
git add crates/vox-codegen-ts/src/view_validation_gate.rs crates/vox-codegen-ts/src/lib.rs
git commit -m "feat(codegen-ts): pure view_validation_gate (quality fails, no-view-root exempt)"
```

---

## Task 3: Call the gate in `generate_with_options` `[SEQUENTIAL after 2]`

**Files:**
- Modify: `crates/vox-codegen-ts/src/emitter.rs` (`generate_with_options`, before the `Ok(CodegenOutput { … })` return near `:624`)

`reactive_stats` is fully populated by the component loop (`:235-267`) before the output is assembled. Insert the gate after the loop and before the success return.

- [ ] **Step 1: Capture a green baseline**

Run: `cargo test -p vox-codegen-ts`
Expected: PASS — record the passing-test count. (Existing fixtures emit valid UI or fall back via no-view-root, so enabling the gate must not regress them.)

- [ ] **Step 2: Add the gate call**

In `crates/vox-codegen-ts/src/emitter.rs`, in `generate_with_options`, immediately before the function assembles/returns `Ok(CodegenOutput { … reactive_stats … })` (the construction near `:624`), add:

```rust
    crate::codegen_ts::view_validation_gate::view_validation_gate(
        &reactive_stats,
        options.strict_view_validation,
    )?;
```

(Use the module path that matches Task 2 Step 2's registration. The `?` propagates the `String` error, matching this function's `Result<CodegenOutput, String>` signature.)

- [ ] **Step 3: Build and run the full crate tests (no regression)**

Run: `cargo test -p vox-codegen-ts`
Expected: PASS — same count as Step 1. (All existing callers use `strict_view_validation: false` by default, so behavior is unchanged; this proves the gate is inert until opted in.)

- [ ] **Step 4: Add an integration test proving strict mode propagates through `generate_with_options`**

Add to the `emitter.rs` test module. This drives the *real* pipeline: a `.vox` source whose view is expected to produce a blocking a11y diagnostic, emitted with `strict_view_validation: true`.

```rust
#[test]
fn generate_with_options_strict_rejects_blocking_view_diag() {
    // An <img> with no alt and no aria-hidden fires
    // `web_ir_validate.a11y.img_missing_alt` (see web_ir_lower_emit_test.rs).
    const SRC: &str = r#"
component Logo() {
    state _x: str = ""
    view: image(src="logo.png")
}
"#;
    let hir = {
        use vox_compiler::hir::lower_module;
        use vox_compiler::lexer::lex;
        use vox_compiler::parser::parse;
        lower_module(&parse(lex(SRC)).expect("parse"))
    };

    let mut strict = CodegenOptions::default();
    strict.strict_view_validation = true;

    // Probe: confirm this fixture actually produces a blocking failure. If the
    // `image` primitive auto-injects alt (or routes around the reactive bridge),
    // the build will NOT fail — in that case swap `image(...)` for another known
    // blocking violation (e.g. an `overlay { modal{z:"modal"} modal{z:"modal"} }`
    // → `web_ir_validate.overlay.duplicate_z`) until the probe below fails.
    let non_strict = generate_with_options(&hir, CodegenOptions::default());
    assert!(
        non_strict.is_ok(),
        "baseline (non-strict) must still emit (placeholder) ok: {non_strict:?}"
    );

    let result = generate_with_options(&hir, strict);
    assert!(
        result.is_err(),
        "strict build must reject a view with a blocking a11y violation"
    );
}
```

- [ ] **Step 5: Run the integration test**

Run: `cargo test -p vox-codegen-ts generate_with_options_strict_rejects_blocking_view_diag -- --nocapture`
Expected: PASS. If it FAILS because `result.is_err()` was false, the chosen fixture did not produce a blocking diagnostic on the reactive-view path — follow the in-test comment and switch to the `overlay duplicate_z` fixture, then re-run. Do not weaken the assertion.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-codegen-ts/src/emitter.rs
git commit -m "feat(codegen-ts): gate generate_with_options on blocking view validation under strict"
```

---

## Task 4: Enable the gate for `vox build --target client` `[SEQUENTIAL after 3]`

**Files:**
- Modify: `crates/vox-cli/src/commands/build.rs` (the `--target client` emit call site — `generate_with_options(&hir, ts_opts)` near `:217`, or `emit_frontend(Target::TypeScript, &hir, ts_opts)` if Sub-project A has landed)

- [ ] **Step 1: Capture a green baseline**

Run: `cargo test -p vox-cli`
Expected: PASS — record the count.

- [ ] **Step 2: Set the flag on the production options**

In `crates/vox-cli/src/commands/build.rs`, where `ts_opts` is constructed for the client/frontend emission (the variable passed to `generate_with_options` / `emit_frontend` at `:217`), set:

```rust
    ts_opts.strict_view_validation = true;
```

(If `ts_opts` is built with a struct literal, add `strict_view_validation: true,` to it instead. Keep it scoped to the client/frontend target only — do not enable it for non-frontend emits.)

- [ ] **Step 3: Build to verify it compiles**

Run: `cargo build -p vox-cli`
Expected: success.

- [ ] **Step 4: Run CLI tests (no regression)**

Run: `cargo test -p vox-cli`
Expected: PASS — same count as Step 1. (Any `vox build` test fixture that now fails is emitting genuinely-invalid UI — that is the intended catch, not a regression; fix the fixture, do not disable the gate. If a fixture legitimately relies on a not-yet-expressible component, it falls under the exempt `no_web_ir_view_root` path and will not fail.)

- [ ] **Step 5: Commit**

```bash
git add crates/vox-cli/src/commands/build.rs
git commit -m "feat(cli): enable strict_view_validation for vox build --target client"
```

---

## Task 5 (Optional): Surface the fallback count as advisory `[PARALLEL-SAFE]`

**Files:**
- Modify: `crates/vox-codegen-ts/src/view_validation_gate.rs`

Make the silent `no_web_ir_view_root` fallback *visible* (it currently vanishes into a placeholder with no signal). This is advisory only — it never fails the build — so un-migrated coverage is reported, not hidden. Skip this task if the team prefers the coverage ledger (Sub-project A) as the single fallback signal.

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `view_validation_gate.rs`:

```rust
#[test]
fn counts_no_view_root_fallbacks() {
    let s = stats_with(&[
        "codegen.reactive.no_web_ir_view_root",
        "codegen.reactive.no_web_ir_view_root",
        "web_ir_validate.a11y.img_missing_alt",
    ]);
    assert_eq!(fallback_count(&s), 2, "only no-view-root entries are fallbacks");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-codegen-ts counts_no_view_root_fallbacks`
Expected: FAIL — `cannot find function fallback_count`.

- [ ] **Step 3: Implement `fallback_count`**

Add to `view_validation_gate.rs` (above the `tests` module):

```rust
/// Number of components that fell back to an unvalidated emit path because no
/// WebIR view root was produced. Advisory: surfaces un-migrated coverage; does
/// not fail the build.
#[must_use]
pub fn fallback_count(stats: &ReactiveViewBridgeStats) -> usize {
    stats
        .reactive_view_emit_failures
        .iter()
        .filter(|d| d.code == NO_VIEW_ROOT_CODE)
        .count()
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-codegen-ts counts_no_view_root_fallbacks`
Expected: PASS.

- [ ] **Step 5: Emit the advisory in `generate_with_options`**

In `crates/vox-codegen-ts/src/emitter.rs`, after the gate call (Task 3 Step 2) and before the `Ok` return, push an advisory diagnostic when fallbacks exist:

```rust
    let fallbacks = crate::codegen_ts::view_validation_gate::fallback_count(&reactive_stats);
    if fallbacks > 0 {
        ts_diagnostics.push(super::web_ir::WebIrDiagnostic {
            code: "codegen.view.unvalidated_fallback".to_string(),
            message: format!(
                "{fallbacks} component view(s) emitted via an unvalidated fallback path \
                 (no WebIR view root) — not covered by a11y/contrast/overlay checks"
            ),
            span: None,
            category: Some("codegen".to_string()),
        });
    }
```

(`ts_diagnostics` is the advisory accumulator already present in `generate_with_options` at `:202`.)

- [ ] **Step 6: Run the crate tests**

Run: `cargo test -p vox-codegen-ts`
Expected: PASS (unchanged count + the new gate/fallback tests).

- [ ] **Step 7: Commit**

```bash
git add crates/vox-codegen-ts/src/view_validation_gate.rs crates/vox-codegen-ts/src/emitter.rs
git commit -m "feat(codegen-ts): advisory diagnostic for unvalidated view fallbacks"
```

---

## Definition of Done

- [ ] `CodegenOptions::strict_view_validation` exists, defaults `false`, reads `VOX_TS_STRICT_VIEW` (Task 1).
- [ ] `view_validation_gate` returns `Err` on quality violations under strict and exempts `no_web_ir_view_root` (Task 2 — 5 unit tests green).
- [ ] `generate_with_options` fails under strict on a real blocking view diagnostic, and is inert (Ok) by default (Task 3 — integration test green, full crate unchanged).
- [ ] `vox build --target client` enables the gate; `cargo test -p vox-cli` unchanged (Task 4).
- [ ] (If Task 5) unvalidated fallbacks surface as an advisory diagnostic.
- [ ] Full check before handoff:
  ```bash
  cargo test -p vox-codegen-ts
  cargo test -p vox-cli
  cargo clippy -p vox-codegen-ts -p vox-cli -- -D warnings
  ```
  Expected: all green.

## What this deliberately does NOT do

- It does **not** validate components that never reach the reactive WebIR view path beyond reporting them as fallbacks — closing that coverage is Sub-projects B–G (authoring primitives + surface migration), measured by the Sub-project A ledger.
- It does **not** change the seam shape — formalizing `emit_frontend`/`Target` is Sub-project A. This plan only makes the validators that already run actually fail the build.
- It does **not** add new validators (contrast-chain, forms) — that is Phase 6 / future authoring work.
