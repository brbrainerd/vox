# Plan 3E — Frontend Emit Validation Gate

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. All embedded git commands use STRICT rules: `git -C /c/Users/Owner/vox-graphify-gui` with **add + commit only** (never push, never amend, never reset). Each task ends in a concrete commit so a sub-agent can execute *and* commit it (write-through-workflow).

> **Why this plan lives in the Vox Search plan set (cross-reference).** This bug class — a swallowed error **accumulator** (`ReactiveViewBridgeStats::reactive_view_emit_failures`) that is *written* on every blocking validation failure but **never read back to gate the build** — is the exact shape the Vox Search **data-flow layer** (master spec §2.3, plan **vs2**) is built to detect deterministically (the `accumulator_never_gates` detector, master spec §2.3 / §10.4). Plan 3E is the **runtime fix** for this specific instance; vs2 is the **structural-detection complement** that finds the *class* across the whole codebase. They are siblings: 3E makes "bad UI doesn't compile" true at the `vox build --target client` call site today; vs2's `vox_search_dead_signals` would have surfaced the dead accumulator before a human noticed. Land 3E now (self-contained, no Vox Search dependency); vs2 later proves the detector against this very fixture (master spec §10.4 success criterion).

---

## Goal

Make blocking WebIR validation failures (a11y / contrast / overlay / layer) **fail the frontend build** instead of silently emitting a `FAIL_PLACEHOLDER` — so "bad UI doesn't compile" is actually true on the production `vox build --target client` path. Today the production emitter runs the full validator per reactive component (`crates/vox-codegen-ts/src/reactive/view.rs:72` → `validate_web_ir`) and, on blocking diagnostics, emits a placeholder and records the errors into `ReactiveViewBridgeStats::reactive_view_emit_failures` (`view.rs:78`) — but `generate_with_options` **never reads that field back** and returns `Ok` regardless (`crates/vox-codegen-ts/src/emitter.rs:622`). This plan reads it back under an opt-in flag.

## Architecture

A new opt-in `strict_view_validation` flag on `CodegenOptions` plus a **pure partition-and-decide gate** (`view_validation_gate`) that returns `Err` when *quality* violations are present, while deliberately **exempting** the `codegen.reactive.no_web_ir_view_root` fallback (`view.rs:88` — a not-yet-WebIR-expressible component is *coverage*, not a defect; that is what the Sub-project A frontend-coverage ledger tracks). The gate is called inside `generate_with_options` immediately before the `Ok(CodegenOutput { … })` return. The CLI `vox build --target client` path enables the flag on the `CodegenOptions` it hands to `emit_frontend` (the landed Sub-project A seam at `crates/vox-cli/src/commands/build.rs:217`, which routes `Target::TypeScript → generate_with_options` — so the gate fires whether the call is direct or through the seam). Library/test callers keep `strict_view_validation: false` (the `#[derive(Default)]` default) → today's behavior is unchanged for them.

## Tech Stack

Rust. Crates: `vox-codegen-ts` (module `codegen_ts`, embedded into `vox-codegen` via `#[path = "mod.rs"]`), `vox-cli`. Key symbols (all verified present in this worktree):
- `vox_codegen::codegen_ts::emitter::{generate_with_options, CodegenOptions, CodegenOutput}` — `CodegenOptions` is `#[derive(... Default ...)]` at `emitter.rs:48`; `strict_ai: bool` at `:56`; `from_env` at `:77` (routes the strict-AI env read through `super::web_migration_env::ts_strict_ai_gate_enabled()` at `:86`); `generate_with_options` at `:198`, its `Ok(CodegenOutput { … reactive_stats … })` return at `:622`, and the advisory accumulator `ts_diagnostics` at `:202`.
- `vox_codegen::codegen_ts::reactive::ReactiveViewBridgeStats` — defined in `crates/vox-codegen-ts/src/reactive/view.rs:21`, re-exported as `super::reactive::ReactiveViewBridgeStats`; field `pub reactive_view_emit_failures: Vec<WebIrDiagnostic>` at `view.rs:25`.
- `vox_codegen::web_ir::WebIrDiagnostic` — `crates/vox-codegen/src/web_ir/mod.rs:526`, fields exactly `{ code: String, message: String, span: Option<SourceSpanId>, category: Option<String> }`; reachable from `codegen_ts` as `super::web_ir::WebIrDiagnostic`.
- `vox_codegen::web_ir::validate::format_web_ir_validate_failure` — `crates/vox-codegen/src/web_ir/validate.rs:775`, reachable as `super::web_ir::validate::format_web_ir_validate_failure`.
- `vox_codegen::frontend_backend::emit_frontend` — `crates/vox-codegen/src/frontend_backend.rs:25`, `Target::TypeScript → generate_with_options(hir, options)`.
- `crates/vox-codegen/src/web_migration_env.rs:24` — `pub fn ts_strict_ai_gate_enabled() -> bool` (the env-gate pattern Task 1 mirrors).

## Spec

Master spec: `docs/superpowers/specs/2026-06-26-vox-search-unified-code-intelligence-design.md` (§2.3 data-flow layer / `accumulator_never_gates` detector; §10.4 success criterion "flags the frontend-emit class deterministically"). Also complements `docs/superpowers/specs/2026-06-20-vox-native-frontend-ssot-design.md` — Sub-project A formalized the emission *seam* (`emit_frontend`/`Target`) byte-identically (no behavior change) and therefore does **not** close this gap; this plan is the behavior change that makes the seam's validators bite. The seam HAS landed in this worktree (`build.rs:217` already calls `emit_frontend`), so Task 4 wires the flag at the seam call site.

## Dependencies

**Cross-plan:** NONE inbound — Plan 3E is self-contained and has no dependency on any Vox Search plan (P0–P8). It may land at any time. Outbound: it is the **runtime instance** that Vox Search plan **vs2** (data-flow / `vox_search_dead_signals`) later detects structurally and uses as a fixture (master spec §10.4) — vs2 should be authored *after* 3E lands so the dead-accumulator pattern still exists in git history as a regression fixture, but vs2 does **not** block on 3E and 3E does **not** block on vs2.

**Intra-plan ordering:** Tasks 1→2→3→4 are **strictly sequential** (each builds on the prior task's new symbol: 1 adds the field, 2 adds the gate fn, 3 calls the gate, 4 enables the flag). Task 5 is **optional** and `[PARALLEL-SAFE]` (it only adds to `view_validation_gate.rs` + an independent advisory push; it depends on Task 2's module existing but is otherwise independent of 3/4 and may be dispatched in the same fan-out batch as Task 3 once Task 2 is committed, or skipped entirely).

**Execution model:** TDD mandatory — failing test first, observed-output verification before any "done". No placeholders.

### Fan-out batch structure (for a dispatching workflow)

```
Batch A (1 task,  [SEQUENTIAL base]):   Task 1
        ▼  (Task 1 committed)
Batch B (1 task,  [SEQUENTIAL after 1]): Task 2
        ▼  (Task 2 committed)
Batch C (1–2 tasks):                     Task 3 [SEQUENTIAL after 2]
                                         Task 5 [PARALLEL-SAFE] (optional — only depends on Task 2)
        ▼  (Task 3 committed; Task 5 if run)
Batch D (1 task,  [SEQUENTIAL after 3]): Task 4
```

Batches A, B, D are single-task sequential gates. Batch C may dispatch Task 3 and (optional) Task 5 in parallel — they touch different concerns (Task 3 edits `emitter.rs` gate-call site near `:622`; Task 5 edits `view_validation_gate.rs` + the advisory-push site, which is *after* the gate call). If both run concurrently and both touch `emitter.rs`, the workflow must serialize their commits (rebase-free: Task 5's advisory push goes immediately after Task 3's gate call); if the workflow cannot guarantee non-conflicting concurrent edits to `emitter.rs`, run Task 5 **after** Task 3 instead. Conservative default: run Task 5 after Task 3.

---

## File Structure

| File | New/Modify | Responsibility |
|---|---|---|
| `crates/vox-codegen-ts/src/emitter.rs` | Modify (`:48` struct, `:77` `from_env`, `:198` `generate_with_options`) | Add `strict_view_validation` option; call the gate before the `Ok` return. |
| `crates/vox-codegen-ts/src/web_migration_env.rs` *(actually `crates/vox-codegen/src/web_migration_env.rs`)* | Modify | Add `ts_strict_view_gate_enabled()` mirroring `ts_strict_ai_gate_enabled()` (env `VOX_TS_STRICT_VIEW`). |
| `crates/vox-codegen-ts/src/view_validation_gate.rs` | **New** | Pure `view_validation_gate(stats, strict) -> Result<(), String>`: partition failures into quality-violations vs `no_web_ir_view_root` fallback; decide. |
| `crates/vox-codegen-ts/src/mod.rs` | Modify (add `pub mod`) | Register the new module (sibling to `pub mod emitter;` at `:27`). |
| `crates/vox-cli/src/commands/build.rs` | Modify (the `--target client` emit call site, the `CodegenOptions { … }` literal at `:211` feeding `emit_frontend` at `:217`) | Enable `strict_view_validation` for production frontend builds. |

> **Path note vs. the original main-repo plan.** The original `2026-06-26-frontend-emit-validation-gate.md` was written against `vox_codegen::*` symbol paths. In this `vox-graphify-gui` worktree the canonical *crate* is `vox-codegen-ts` (module `codegen_ts`, embedded into `vox-codegen` via `#[path]`), `web_ir` is re-exported from the sibling `vox-codegen` crate and reached as `super::web_ir::…` from inside `codegen_ts`, and `ReactiveViewBridgeStats` lives in `reactive/view.rs` (not a `reactive.rs`). The module is registered in `mod.rs` (not `lib.rs`, which only `#[path]`-includes `mod.rs`). All `use`/`mod` paths below use `super::`-relative form to match how `emitter.rs` already refers to these items.

---

## Task 1: Add the `strict_view_validation` option `[SEQUENTIAL base]`

**Files:**
- Modify: `crates/vox-codegen-ts/src/emitter.rs` (struct at `:48`, `from_env` at `:77`)
- Modify: `crates/vox-codegen/src/web_migration_env.rs` (add the gate fn next to `ts_strict_ai_gate_enabled` at `:24`)

`CodegenOptions` already `#[derive(... Default ...)]` (`emitter.rs:48`), so a new `bool` field defaults to `false` automatically — no manual `Default` impl to touch. This mirrors the existing `strict_ai` field/env pattern, which reads through `web_migration_env` (so this plan adds a sibling gate fn rather than a raw `std::env::var`, matching the codebase's env-read discipline).

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

- [ ] **Step 3: Add the gate fn to `web_migration_env.rs`**

In `crates/vox-codegen/src/web_migration_env.rs`, directly after `pub fn ts_strict_ai_gate_enabled()` (ends near `:30`), add (read the body of `ts_strict_ai_gate_enabled` first to mirror its exact `env::var` truthiness convention — `"1"` / `"true"`):

```rust
/// Whether the frontend build should **fail** on blocking reactive-view WebIR
/// validation diagnostics (a11y / contrast / overlay / layer) instead of
/// emitting a placeholder. Env `VOX_TS_STRICT_VIEW`. Mirrors
/// [`ts_strict_ai_gate_enabled`] so the CLI/codegen resolve the flag through one
/// module rather than reading `VOX_*` ad hoc in consumer code.
#[must_use]
pub fn ts_strict_view_gate_enabled() -> bool {
    std::env::var("VOX_TS_STRICT_VIEW")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}
```

(If `ts_strict_ai_gate_enabled` reads the env through a shared helper instead of `std::env::var` directly, route this one through the same helper for byte-consistency — read `:24`–`:30` and match it.)

- [ ] **Step 4: Add the field**

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

- [ ] **Step 5: Wire `from_env`**

In `fn from_env()` (`:77`), inside the `Self { … }` construction (the field list ending near `:88`), set the field from the new gate (mirroring how `strict_ai` reads `ts_strict_ai_gate_enabled()` at `:86`):

```rust
            strict_view_validation: super::web_migration_env::ts_strict_view_gate_enabled(),
```

- [ ] **Step 6: Add the env-on test**

Add to the `tests` module in `emitter.rs`:

```rust
#[test]
fn strict_view_validation_reads_env() {
    // Safety: single-threaded test path; restore after.
    std::env::set_var("VOX_TS_STRICT_VIEW", "1");
    assert!(CodegenOptions::from_env().strict_view_validation);
    std::env::remove_var("VOX_TS_STRICT_VIEW");
}
```

- [ ] **Step 7: Run tests to verify they pass**

Run: `cargo test -p vox-codegen-ts strict_view_validation`
Expected: PASS (2 tests).

- [ ] **Step 8: Commit**

```bash
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-codegen-ts/src/emitter.rs crates/vox-codegen/src/web_migration_env.rs
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(codegen-ts): add strict_view_validation option (default off)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 2: Pure validation gate `view_validation_gate` `[SEQUENTIAL after 1]`

**Files:**
- Create: `crates/vox-codegen-ts/src/view_validation_gate.rs`
- Modify: `crates/vox-codegen-ts/src/mod.rs` (add `pub mod view_validation_gate;` next to `pub mod emitter;` at `:27`)

The gate is a pure function so it is tested without parsing `.vox` or running the emitter. The plumbing that *populates* `reactive_view_emit_failures` is already covered by `reactive/view.rs:72`+`:78` plus the existing a11y test in `crates/vox-compiler/tests/web_ir_lower_emit_test.rs`.

- [ ] **Step 1: Write the failing tests**

Create `crates/vox-codegen-ts/src/view_validation_gate.rs`:

```rust
//! Production build gate: turn **blocking** reactive-view WebIR validation
//! failures into a hard build error, instead of the silent `FAIL_PLACEHOLDER`
//! emitted by `reactive/view.rs`. The `no_web_ir_view_root` fallback is exempt:
//! a component that isn't WebIR-expressible yet is *coverage* (tracked by the
//! frontend coverage ledger), not a UI defect — failing on it would break every
//! un-migrated surface.
//!
//! This reads back the `ReactiveViewBridgeStats::reactive_view_emit_failures`
//! accumulator that `generate_with_options` writes but never inspected — the
//! `accumulator_never_gates` shape the Vox Search data-flow layer (plan vs2)
//! detects structurally.

use super::reactive::ReactiveViewBridgeStats;
use super::web_ir::validate::format_web_ir_validate_failure;

/// Diagnostics with this code mean "this component's view did not lower to a
/// WebIR view root" — a coverage fallback, NOT a quality violation. Exempt.
/// Matches the code pushed at `reactive/view.rs:88`.
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
    use super::super::reactive::ReactiveViewBridgeStats;
    use super::super::web_ir::WebIrDiagnostic;

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

> **Import-path resolution.** `emitter.rs` reaches the same items as `super::reactive::ReactiveViewBridgeStats` (see `emitter.rs:31`, `:229`) and `super::web_ir::WebIrDiagnostic` (`emitter.rs:202`). This new module is a sibling of `emitter` under `mod.rs`, so the identical `super::reactive::…` / `super::web_ir::…` paths resolve. If `format_web_ir_validate_failure` is not re-exported under `super::web_ir::validate`, fall back to the path `emitter.rs` would use and confirm with `grep -n "format_web_ir_validate_failure" crates/vox-codegen-ts/src crates/vox-codegen/src` — it lives at `crates/vox-codegen/src/web_ir/validate.rs:775` and `web_ir` is a public module of the embedding crate reachable via `super::web_ir`.

- [ ] **Step 2: Register the module**

In `crates/vox-codegen-ts/src/mod.rs`, add directly after `pub mod emitter;` (`:27`), matching the surrounding `pub mod` visibility convention:

```rust
pub mod view_validation_gate;
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo test -p vox-codegen-ts view_validation_gate`
Expected: PASS (5 tests).

- [ ] **Step 4: Commit**

```bash
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-codegen-ts/src/view_validation_gate.rs crates/vox-codegen-ts/src/mod.rs
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(codegen-ts): pure view_validation_gate (quality fails, no-view-root exempt)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 3: Call the gate in `generate_with_options` `[SEQUENTIAL after 2]`

**Files:**
- Modify: `crates/vox-codegen-ts/src/emitter.rs` (`generate_with_options`, before the `Ok(CodegenOutput { … })` return at `:622`)

`reactive_stats` is declared at `:229` and fully populated by the component loop (around `:238`) before the output is assembled. Insert the gate after the loop and before the success return at `:622`.

- [ ] **Step 1: Capture a green baseline**

Run: `cargo test -p vox-codegen-ts`
Expected: PASS — record the passing-test count. (Existing fixtures emit valid UI or fall back via no-view-root, so enabling the gate must not regress them.)

- [ ] **Step 2: Add the gate call**

In `crates/vox-codegen-ts/src/emitter.rs`, in `generate_with_options`, immediately before the `Ok(CodegenOutput { … reactive_stats … })` construction at `:622`, add:

```rust
    super::view_validation_gate::view_validation_gate(
        &reactive_stats,
        options.strict_view_validation,
    )?;
```

(`super::view_validation_gate::…` matches the Task 2 Step 2 registration as a sibling module under `mod.rs`. The `?` propagates the `String` error, matching this function's `Result<CodegenOutput, String>` signature. Note `options` must still be in scope at the return site — confirm it is not moved earlier; if it was consumed, read `options.strict_view_validation` into a `let strict_view = options.strict_view_validation;` near the top of the function and use that local here.)

- [ ] **Step 3: Build and run the full crate tests (no regression)**

Run: `cargo test -p vox-codegen-ts`
Expected: PASS — same count as Step 1. (All existing callers use `strict_view_validation: false` by default, so behavior is unchanged; this proves the gate is inert until opted in.)

- [ ] **Step 4: Add an integration test proving strict mode propagates through `generate_with_options`**

Add to the `emitter.rs` test module. This drives the *real* pipeline: a `.vox` source whose view is expected to produce a blocking validation diagnostic, emitted with `strict_view_validation: true`.

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

> **If `lower_module`/`parse`/`lex` signatures differ in this worktree**, mirror the exact construction used by an existing `generate_with_options` test in the same `emitter.rs` test module (grep `grep -n "generate_with_options(" crates/vox-codegen-ts/src/emitter.rs` and copy its HIR-building preamble verbatim). Do not invent a builder.

- [ ] **Step 5: Run the integration test**

Run: `cargo test -p vox-codegen-ts generate_with_options_strict_rejects_blocking_view_diag -- --nocapture`
Expected: PASS. If it FAILS because `result.is_err()` was false, the chosen fixture did not produce a blocking diagnostic on the reactive-view path — follow the in-test comment and switch to the `overlay duplicate_z` fixture, then re-run. Do not weaken the assertion.

- [ ] **Step 6: Commit**

```bash
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-codegen-ts/src/emitter.rs
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(codegen-ts): gate generate_with_options on blocking view validation under strict

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 4: Enable the gate for `vox build --target client` `[SEQUENTIAL after 3]`

**Files:**
- Modify: `crates/vox-cli/src/commands/build.rs` (the `--target client` emit call site — the `CodegenOptions { … }` struct literal at `:211`, fed to `vox_codegen::frontend_backend::emit_frontend(Target::TypeScript, &hir, ts_opts)` at `:217`. The Sub-project A seam has landed, so the flag is set on the literal that flows into the seam.)

- [ ] **Step 1: Capture a green baseline**

Run: `cargo test -p vox-cli`
Expected: PASS — record the count.

- [ ] **Step 2: Set the flag on the production options**

In `crates/vox-cli/src/commands/build.rs`, in the `if resolved_target == vox_config::BuildTarget::Client { … }` block, add `strict_view_validation: true,` to the `CodegenOptions { … }` struct literal at `:211` (the one passed to `emit_frontend` at `:217`):

```rust
        let ts_opts = vox_codegen::codegen_ts::CodegenOptions {
            tanstack_start: vox_config::VoxConfig::load().web_tanstack_start,
            target: mobile_target.clone(),
            mode: vox_codegen::codegen_ts::emitter::BuildMode::Library,
            strict_view_validation: true,
            ..Default::default()
        };
```

(Keep it scoped to the Client/frontend target literal only — do **not** add it to the other `CodegenOptions` literals at `:151` (RN) or `:277` (non-client emit). Those keep the default `false`.)

- [ ] **Step 3: Build to verify it compiles**

Run: `cargo build -p vox-cli`
Expected: success.

- [ ] **Step 4: Run CLI tests (no regression)**

Run: `cargo test -p vox-cli`
Expected: PASS — same count as Step 1. (Any `vox build` test fixture that now fails is emitting genuinely-invalid UI — that is the intended catch, not a regression; fix the fixture, do not disable the gate. If a fixture legitimately relies on a not-yet-expressible component, it falls under the exempt `no_web_ir_view_root` path and will not fail.)

- [ ] **Step 5: Commit**

```bash
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-cli/src/commands/build.rs
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(cli): enable strict_view_validation for vox build --target client

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 5 (Optional): Surface the fallback count as advisory `[PARALLEL-SAFE]`

**Files:**
- Modify: `crates/vox-codegen-ts/src/view_validation_gate.rs`
- Modify: `crates/vox-codegen-ts/src/emitter.rs` (advisory push after the gate call from Task 3)

Make the silent `no_web_ir_view_root` fallback *visible* (it currently vanishes into a placeholder with no signal). This is advisory only — it never fails the build — so un-migrated coverage is reported, not hidden. Skip this task if the team prefers the coverage ledger (Sub-project A) as the single fallback signal. **Depends on Task 2 (the module); independent of Tasks 3/4 except for the shared `emitter.rs` advisory site — run after Task 3 to avoid a concurrent edit to `emitter.rs`.**

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

In `crates/vox-codegen-ts/src/emitter.rs`, after the gate call (Task 3 Step 2) and before the `Ok` return at `:622`, push an advisory diagnostic when fallbacks exist:

```rust
    let fallbacks = super::view_validation_gate::fallback_count(&reactive_stats);
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

(`ts_diagnostics` is the advisory accumulator already present in `generate_with_options` at `:202`. Confirm it is mutable and still in scope at this point — it is `let mut ts_diagnostics` at `:202` and consumed into the `Ok` at `:625`.)

- [ ] **Step 6: Run the crate tests**

Run: `cargo test -p vox-codegen-ts`
Expected: PASS (unchanged count + the new gate/fallback tests).

- [ ] **Step 7: Commit**

```bash
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-codegen-ts/src/view_validation_gate.rs crates/vox-codegen-ts/src/emitter.rs
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(codegen-ts): advisory diagnostic for unvalidated view fallbacks

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Definition of Done

- [ ] `CodegenOptions::strict_view_validation` exists, defaults `false`, reads `VOX_TS_STRICT_VIEW` via `ts_strict_view_gate_enabled()` (Task 1).
- [ ] `view_validation_gate` returns `Err` on quality violations under strict and exempts `no_web_ir_view_root` (Task 2 — 5 unit tests green).
- [ ] `generate_with_options` fails under strict on a real blocking view diagnostic, and is inert (Ok) by default (Task 3 — integration test green, full crate unchanged).
- [ ] `vox build --target client` enables the gate via the `emit_frontend` seam literal; `cargo test -p vox-cli` unchanged (Task 4).
- [ ] (If Task 5) unvalidated fallbacks surface as an advisory diagnostic.
- [ ] Full check before handoff:
  ```bash
  cargo test -p vox-codegen-ts
  cargo test -p vox-cli
  cargo clippy -p vox-codegen-ts -p vox-cli -- -D warnings
  ```
  Expected: all green.

## What this deliberately does NOT do

- It does **not** validate components that never reach the reactive WebIR view path beyond reporting them as fallbacks — closing that coverage is the frontend Sub-projects B–G (authoring primitives + surface migration), measured by the Sub-project A ledger.
- It does **not** change the seam shape — formalizing `emit_frontend`/`Target` was Sub-project A (already landed in this worktree). This plan only makes the validators that already run actually fail the build.
- It does **not** add new validators (contrast-chain, forms) — that is Phase 6 / future authoring work.
- It does **not** implement the structural *detection* of this bug class — that is Vox Search plan vs2 (`accumulator_never_gates` / `vox_search_dead_signals`, master spec §2.3 / §10.4), the complement to this runtime fix.

---

## Self-Review (spec coverage)

**Against the master spec (`2026-06-26-vox-search-unified-code-intelligence-design.md`):**

- §2.3 (data-flow layer / `accumulator_never_gates`): the spec names the frontend-emit `reactive_view_emit_failures` accumulator as the canonical swallowed-error shape vs2 detects. ✅ Covered as the **cross-reference / motivation** — this plan is the runtime fix, explicitly framed (header note + "What this does NOT do") as the complement to vs2's structural detection, not a duplicate of it. The plan does not attempt vs2's detector; it fixes the instance.
- §10.4 (success criterion: "data-flow layer flags the frontend-emit `accumulator_never_gates` class deterministically"): ✅ The header preserves this as the sibling deliverable and recommends authoring vs2 *after* 3E lands so the dead-accumulator persists in history as a regression fixture. No conflict.
- §8 non-goals (no engine rewrite, overlays separate): N/A — 3E touches only codegen, not the Vox Search engine/overlays. No structural-graph mutation. ✅ Consistent.

**Against the source design (the original `2026-06-26-frontend-emit-validation-gate.md`):** all five tasks reproduced VERBATIM in intent and code, adapted only for the verified worktree symbol paths (crate `vox-codegen-ts`, `super::`-relative module paths, `ReactiveViewBridgeStats` in `reactive/view.rs`, env gate routed through `web_migration_env::ts_strict_view_gate_enabled`, CLI seam call `emit_frontend` at `build.rs:217` with the struct literal at `:211`). Goal/Architecture/Tech Stack/Spec/Dependencies header present. ✅

**Codebase reality (verified this session):**
- `CodegenOptions` `#[derive(Default)]` at `emitter.rs:48`, `strict_ai:56`, `from_env:77`/`:86`. ✅
- `ReactiveViewBridgeStats.reactive_view_emit_failures: Vec<WebIrDiagnostic>` at `reactive/view.rs:25`; written `:78`/`:88`. ✅
- `WebIrDiagnostic { code, message, span, category }` at `vox-codegen/src/web_ir/mod.rs:526` — test fixture fields match exactly. ✅
- `format_web_ir_validate_failure` at `web_ir/validate.rs:775`. ✅
- `generate_with_options` `Ok` return at `emitter.rs:622`, `ts_diagnostics` accumulator at `:202`/`:625`. ✅
- CLI Client emit: `CodegenOptions { … ..Default::default() }` literal at `build.rs:211` → `emit_frontend(Target::TypeScript, &hir, ts_opts)` at `:217`; `emit_frontend` routes `Target::TypeScript → generate_with_options` (`frontend_backend.rs:25`). ✅ — so the gate fires through the landed seam.
- `ts_strict_ai_gate_enabled` at `web_migration_env.rs:24` — the env-gate pattern Task 1 mirrors. ✅

**Workflow-readiness:** every task tagged `[SEQUENTIAL]`/`[PARALLEL-SAFE]`; fan-out batch structure (A→B→C→D) stated up front; every task ends in an add+commit with STRICT `git -C` rules (no push/amend/reset); cross-plan dependency (none inbound; vs2 sibling) stated at top. ✅

**Gaps / risks flagged honestly:**
- Task 3 Step 4/5 and Task 5 share `emitter.rs`; the conservative default (run Task 5 after Task 3) is stated to avoid a concurrent-edit conflict in a parallel dispatch.
- The integration-test fixture (`image(...)` → `img_missing_alt`) has an in-test probe + documented fallback (`overlay duplicate_z`) if the `image` primitive auto-injects alt — the assertion is never weakened.
- `from_env`/HIR-builder signatures are pinned to the verified lines but include a "mirror an existing test's preamble" escape hatch if a helper indirection differs — no invented APIs.
