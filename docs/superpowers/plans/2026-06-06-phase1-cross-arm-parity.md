---
title: Phase 1 — Cross-Arm Parity (CR-F2, all three arms) Implementation Plan
description: TDD plan to bring interp + codegen-rust + codegen-ts to byte-identical stdout parity on the EXPECT golden corpus, including a from-scratch Node execution harness and a ratcheting parity gate.
category: architecture
---

# Phase 1 — Cross-Arm Parity (CR-F2) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans. Steps use checkbox (`- [ ]`). This is Phase 1 of the master roadmap ([`2026-06-06-v1-completion-roadmap.md`](2026-06-06-v1-completion-roadmap.md)). It is **compiler work**, not gate-building — but it stands up its own ratcheting gate first so progress is measured.

**Goal:** For every executable golden (`fn main()` + `// EXPECT:`), all three arms — `--mode interp`, `--mode script` (codegen-rust), and codegen-ts under Node — produce **byte-identical** stdout matching the EXPECT block. CR-F2 is met when the parity set ≥ 8/10 with any residual documented in a non-growing allowlist.

**Architecture:** Build the **codegen-ts Node execution harness** (Task 1.A — does not exist today) → stand up a **ratcheting 3-arm parity gate** (Task 1.0) with per-arm non-growing allowlists → fix codegen-rust bug classes (Tasks 1.R*) lowest-effort-first → fix codegen-ts bug classes (Tasks 1.T*) once 1.A makes them measurable. Every fix decrements an allowlist baseline; the gate enforces no regression.

**Tech Stack:** Rust (`vox-codegen` emit modules, `vox-integration-tests`), Node + `tsc`/`tsx` (ts execution), the landed `vox audit` registry pattern (for the parity gate).

---

## 1. Current state (re-audited 2026-06-06, READ-ONLY survey)

| Arm | Pass on EXPECT corpus | Notes |
|---|---|---|
| `--mode interp` | **10/10** | reference; behavioral gate (CR-F1) already green here |
| `--mode script` (codegen-rust) | **3/10** | passes: `mesh/noop`, `while_loop_algorithms`, `decimal_math`. 7-class backlog below. |
| codegen-ts | **unmeasured** | emitter is mature (web/JSX/reactive/mobile) but **NO Node execution harness exists** — only `tsc --noEmit` typecheck (`crates/vox-integration-tests/tests/ts_emit_typecheck_test.rs`, `#[ignore]`, scoped to `examples/golden-ts/`, not `examples/golden/`). |

**EXPECT golden corpus (10):** `examples/golden/{adt_multi_field, closures_hof, decimal_math, json_as_typed, range_and_indexing, regex_free_functions, string_interpolation, tuple_destructure, while_loop_algorithms}.vox` + `examples/golden/mesh/noop.vox`.

**No cross-arm parity test exists** in `crates/vox-integration-tests/tests/` (only `golden_behavioral_gate.rs` = interp-only). Reference: [`cr-f2-arm-parity-findings-2026.md`](../../src/architecture/cr-f2-arm-parity-findings-2026.md).

**codegen-rust open bug classes (verified emit sites):**
| # | Class | Triggering golden(s) | Likely site |
|---|---|---|---|
| R1 | Missing `rust_decimal`/`regex` deps in generated Cargo.toml | decimal_math (dep), regex_free_functions | `crates/vox-codegen/src/codegen_rust/pipeline.rs:173-187` (template) |
| R2 | `\w` regex string-escape not re-escaped into Rust source (`unknown character escape: w`) | regex_free_functions | string-literal / method-call emit path (trace `regex.*` calls) |
| R3 | `Option::None` emitted as tuple variant (`E0532`) | while_loop_algorithms (partial), others | `emit_pattern` in `crates/vox-codegen/src/codegen_rust/emit/stmt_expr.rs:~487` |
| R4 | Loop/binding var out of scope (`E0425 cannot find value i`) | tuple_destructure | For-loop emit `crates/vox-codegen/src/codegen_rust/emit/stmt_expr_tail.rs:89-120` |
| R5 | `Json` type referenced but never defined (`E0425 cannot find type Json`) | json_as_typed | type emit `crates/vox-codegen/src/codegen_rust/emit/types.rs` (maps Json→`serde_json::Value` in some arms but not the decl path) |
| R6 | String-interpolation interpolant not coerced to `String` (`E0308`) | string_interpolation | binary-`+` desugar / coercion layer |
| R7 | ADT multi-field constructor/match arm emit (`E0308` incompatible match arms) | adt_multi_field, closures_hof | `emit_pattern` + Match arm in `stmt_expr_tail.rs:128-139` |

(R-classes are the codegen-rust backlog. codegen-ts classes T* are enumerated after Task 1.A makes them observable — they are unknown until the corpus actually runs under Node.)

---

## Task 1.A: codegen-ts Node execution harness (build from scratch)

**Files:**
- Create: `crates/vox-integration-tests/tests/support/ts_exec.rs` (or a `mod` in the parity test) — emit→run→capture helper.
- Reference: `crates/vox-codegen/src/codegen_ts/emitter.rs`; existing `ts_emit_typecheck_test.rs` for how emit is invoked.

- [ ] **Step 1 — Write the failing test** `crates/vox-integration-tests/tests/ts_exec_smoke.rs`:

```rust
// Runs the trivial golden through codegen-ts and Node, asserts stdout "0".
#[test]
fn ts_exec_runs_noop_golden() {
    if which_node().is_none() { eprintln!("skip: node not on PATH"); return; }
    let out = run_golden_under_node("examples/golden/mesh/noop.vox").expect("ts exec");
    assert_eq!(out.trim(), "0");
}
```

- [ ] **Step 2 — Run, expect FAIL** (`run_golden_under_node`/`which_node` undefined).
- [ ] **Step 3 — Implement** `run_golden_under_node(path)`:
  1. Invoke the codegen-ts emit for the single program (reuse the entry point `ts_emit_typecheck_test.rs` uses; emit a self-contained `.mjs`/`.ts` that `console.log`s `main()`'s result — mirror how `--mode script` wraps `main` for stdout).
  2. Write the emitted TS + a tiny runner to a temp dir.
  3. Execute via `node` (for `.mjs`) or `npx tsx` (for `.ts`); capture stdout with a **bounded timeout** (mirror the CR-F1 gate's `run_golden` timeout pattern — drain stdout on a thread, kill on deadline).
  4. Return trimmed stdout.
- [ ] **Step 4 — Run, expect PASS** (skips cleanly if node absent — never a false-fail).
- [ ] **Step 5 — Commit** `feat(test): codegen-ts Node execution harness (CR-F2 ts arm)`.

> Decision to confirm at implementation: emit target = ESM `.mjs` run by `node` (fewer deps) vs `.ts` run by `tsx`. Prefer `.mjs`+`node` unless the emitter only produces `.ts` with types that Node can't strip — then `tsx`. Record the choice in the harness module doc.

---

## Task 1.0: Ratcheting 3-arm parity gate (stand up FIRST so progress is measured)

**Files:**
- Create: `crates/vox-integration-tests/tests/golden_arm_parity_test.rs`
- Create: `contracts/eval/arm-parity-allowlist-script.txt`, `contracts/eval/arm-parity-allowlist-ts.txt`
- Reuse: `parse_expect` + golden collection from `golden_behavioral_gate.rs`; `run_golden_under_node` from Task 1.A.

- [ ] **Step 1 — Write the failing test**: for each golden with `main()` + `// EXPECT`, run interp (reference), `--mode script`, and (if node present) the ts harness; normalize stdout (strip the INFO tracing line the script lane prints, trailing whitespace); assert each arm equals the EXPECT block. Maintain two **non-growing allowlists**; assert `live_divergence(arm) ⊆ allowlist(arm)` AND `allowlist.len() <= committed_baseline(arm)`.
- [ ] **Step 2 — Seed baselines** from the audit: `script` baseline = 7 (the 7 failing), `ts` baseline = 10 (unmeasured → start fully allowlisted, ratchet down as 1.A measurements land). Run → PASS at the seeded baselines.
- [ ] **Step 3 — Register** as a `vox audit --gate arm-parity` registry gate (Phase-0 pattern: `CrlGate::F2ArmParity` Foundation variant + `subcommands/arm_parity.rs` + `main.rs` arm + bump registry size); artifact `contracts/reports/arm-parity/<UTC>.json` with the per-golden×arm table (`{interp_out, script_out, ts_out, all_agree}`).
- [ ] **Step 4 — Commit** `feat(vox-audit): ratcheting 3-arm parity gate (CR-F2)`. Every fix below now decrements a baseline; the gate enforces no regression.

---

## Tasks 1.R1–1.R7: codegen-rust bug-class fixes (TDD loop, lowest-effort first)

Suggested order by leverage (from the audit): **R1 (deps) → R5 (Json type) → R3 (Option::None) → R2 (regex escape) → R4 (loop scope) → R6 (interp coercion) → R7 (ADT match)**.

For **each** class the loop is identical:
- [ ] **Step 1** — `VOX_BIN=$(which vox) vox run --mode script examples/golden/<g>.vox` → capture the exact Rust compile error (or runtime panic).
- [ ] **Step 2** — open the emit site named in the R-table; add/extend a `vox-codegen` unit test (in `crates/vox-codegen/.../tests` or the module's `#[cfg(test)]`) asserting the emitted Rust snippet for that construct.
- [ ] **Step 3** — make the **minimal** emitter fix; rebuild `vox`.
- [ ] **Step 4** — re-run `--mode script <g>` → now compiles + stdout matches interp.
- [ ] **Step 5** — **decrement** `arm-parity-allowlist-script.txt` by one; run `cargo test -p vox-codegen` (no regression to App/web emit) + the parity gate.
- [ ] **Step 6** — commit `fix(codegen-rust): <class> (CR-F2 N/10 script)`.

Concrete starting points (verify live, they may have shifted):
- **R1:** add `rust_decimal` + `regex` to the Native Cargo.toml template at `pipeline.rs:173-187` (only when the program uses them — gate on emitted-symbol detection to avoid bloating every emit).
- **R5:** emit `type Json = serde_json::Value;` (or fully-qualify) in the program prelude when `@json_as`/`Json` is referenced; reconcile with `types.rs`'s existing `Json => serde_json::Value` map.
- **R3:** fix `emit_pattern` so `None` emits as the unit variant `None` (not `None()`).

**Exit for the rust arm:** codegen-rust parity ≥ 8/10; `arm-parity-allowlist-script.txt` ratcheted to ≤ 2 with documented reasons.

---

## Tasks 1.T*: codegen-ts bug-class fixes (after 1.A measures them)

- [ ] **Step 0 — Measure:** with Task 1.A live, run the parity gate's ts column over all 10 goldens; record the actual failing set + error per golden into the ts allowlist. **This enumerates the T-classes** (unknown until run — the emitter is mature for web/JSX but the plain-stdout `main()` path on this scalar/ADT corpus is unexercised).
- [ ] Then apply the **same 6-step TDD loop** as the R-tasks, but against `crates/vox-codegen/src/codegen_ts/emitter.rs` (and `jsx.rs`/`reactive.rs` only if a golden touches them — the corpus is plain `main()` programs, so emitter.rs is the likely site), decrementing `arm-parity-allowlist-ts.txt` per fix.

**Exit for the ts arm:** codegen-ts parity ≥ 8/10; ts allowlist ratcheted to ≤ 2 with documented reasons.

---

## Phase-1 exit criteria

- [ ] `golden_arm_parity_test` green; both allowlists at their ratcheted floors (≤ 2 each, each entry annotated).
- [ ] `vox audit --gate arm-parity` registered, green at baseline, artifact emitted with the 3-arm table.
- [ ] `cargo test -p vox-codegen` + `cargo test -p vox-integration-tests` green (no App/web emit regressions).
- [ ] CR-F2 row in `v1-release-criteria.md` status updated to reflect measured 3-arm parity.

## Self-review notes
- **Harness before fixes:** 1.A + 1.0 land first so every fix is measured and regression-guarded.
- **No arm descoped:** codegen-ts is fully in scope; its bug classes are discovered empirically via 1.A, not guessed.
- **TDD throughout:** each fix is golden-driven (arm output == interp), each gated by a `vox-codegen` snippet test.
- **Determinism:** the parity gate normalizes the script-lane tracing line; node-absent CI skips the ts column cleanly rather than false-failing (the allowlist keeps it honest).
- **Dependency:** Task 1.0's ts column depends on Task 1.A; the R-tasks are independent of the T-tasks and can run as a parallel stream.
