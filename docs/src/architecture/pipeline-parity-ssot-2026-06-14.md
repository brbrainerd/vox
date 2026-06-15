---
title: "End-to-End Pipeline Parity — SSOT"
description: "The contract that every Vox language feature must be wired identically across all IRs and all emit targets (interpreter, Rust/Axum, TypeScript/React, GUI/Tauri), with a hard build gate that turns silent divergence into a build failure — a compile error where possible, a required-gate test failure otherwise — instead of a runtime surprise."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
training_rationale: "Architecture contract + remediation plan; transient enforcement artifact, not language doctrine."
---

# End-to-End Pipeline Parity — SSOT

> **For agentic workers:** this is the *contract* document. The executable, task-by-task work lives in the companion plan [`docs/superpowers/plans/2026-06-14-pipeline-parity-enforcement.md`](../../superpowers/plans/2026-06-14-pipeline-parity-enforcement.md). Read this to understand *what parity means and why*; read the plan to *do the work*. Both are designed so independent agents can pick up disjoint slices in parallel without shared state.

> **⚠ Audit correction (2026-06-15).** A 5-agent read-only re-audit verified this SSOT against `main` and found the *thesis* sound but several §2 current-state facts stale (the gap-remediation work since `fb2842670e` already converted most silent catch-alls into explicit coded arms). Corrected facts are inlined below as **[CORRECTED 2026-06-15]** notes. Net effect: the silent-drop surface is smaller and concentrated in the TS/web stack; the work shifts from *removing catch-alls* to *centralizing the already-explicit per-emitter decisions into one matrix + adding the gate*. The three design artifacts (§3) are unchanged in shape; only their sizing and one crate-boundary detail are corrected.

**Goal (one sentence):** Every Vox language feature must be wired identically across all internal representations and all emit targets — and any feature that is partially wired must be **rejected by the build pipeline** (a compile error where the type system can enforce it, a required-gate test failure otherwise), not silently degraded, panicked on at runtime, or discovered by a user.

**The thesis:** Vox today has *four* IRs, *four* emit targets, and *three* builtin registries, but **no single place that says "here is the set of features, and here is who must implement each one."** Parity is therefore enforced by human vigilance and a handful of partial tests. That is exactly the failure mode that produced the R1–R15 adversarial-review bugs and the ~55 wiring gaps in the 2026-06-13 pipeline audit. This document defines the missing SSOT and the gate that turns parity from a convention into an invariant.

---

## 1. The parity matrix (the mental model)

Parity is a 2-D obligation. One axis is the **pipeline depth** (a feature must survive every lowering). The other is the **target breadth** (a feature must be honored by every backend that claims to support it).

```
                          TARGETS (breadth) ──────────────────────────────▶
                          │ Interpreter │ Rust/Axum │ TS/React │ GUI/Tauri │
  PIPELINE DEPTH          │  (eval)     │ codegen_  │ codegen_ │ RustApp   │
  (every feature must     │             │ rust      │ ts       │ Shell::   │
   pass through each row)  │             │           │          │ TauriApp  │
  ─────────────────────── ┼─────────────┼───────────┼──────────┼───────────┤
  Lexer / token (@deco)   │     ✓       │     ✓     │    ✓     │     ✓     │
  Parser (AST node)       │     ✓       │     ✓     │    ✓     │     ✓     │
  HIR lowering            │     ✓       │     ✓     │    ✓     │     ✓     │
  Typeck / effect check   │     ✓       │     ✓     │    ✓     │     ✓     │
  Emit / execute          │   eval_expr │ emit_*    │ emit_hir │ tauri emit│
```

**The invariant:** a cell may be `✓` (implemented), or `✗ (declared)` — *explicitly* unsupported with a coded diagnostic — but it may **never** be a silent `_ => …` catch-all that drops, no-ops, or panics. "Declared unsupported" is parity. "Silently different" is the bug class this SSOT exists to eliminate.

### 1.1 What "a feature" is

A **feature** is any one of:

- a **decorator** (`@server`, `@auth`, `@rate_limit`, `@public`, `@inference`, …) — enumerated as `At*` tokens in `crates/vox-compiler/src/lexer/token.rs`;
- an **expression / statement kind** — a `HirExpr` / `HirStmt` variant (`Jsx`, `Spawn`, `With`, `WorkflowVersion`, `Pipe`, …);
- a **declaration kind** — a `HirModule` field / `Decl` variant (`Const`, `Table`, `Endpoint`, `ReactiveComponent`, …);
- a **builtin** — an entry in any of the three builtin registries (`uuid`, `now_ms`, `http.get_text`, `std.mobile.*`, `OpenClaw.*`, …).

Every feature has a **support declaration** for every target: `Implemented`, or `Unsupported { code }` where `code` is a stable diagnostic from the registry. There is no third state.

---

## 2. Verified current state (the gap audit)

The following was confirmed by direct code read on `main` at `fb2842670e` (2026-06-14). These are facts, not estimates.

### 2.1 Four IRs, not the planned two

| IR | Defined in | Root type | Lowered by |
|----|-----------|-----------|-----------|
| **AST** | `crates/vox-ast/` (re-exported `vox_compiler::ast`) | `ast::decl::Module` | `parser::parse()` / `parse_script()` |
| **HIR** | `crates/vox-compiler/src/hir/` | `HirModule` | `hir::lower::lower_module()` |
| **WebIR** | `crates/vox-codegen/src/web_ir/` | `WebIrModule` | `web_ir::lower::lower_hir_to_web_ir()` |
| **VoxIR** | `crates/vox-codegen/src/vox_ir/` | `VoxIrModule` | `vox_ir::lower` |

The "codegen SSOT unification (4 IRs → 2)" initiative is **not complete**. `VoxIR` wraps the others (it embeds `WebIrModule` as an optional field) but did not eliminate them; WebIR still has its own validator chain (`web_ir/validate.rs`, `web_ir/validate_a11y.rs`). **Each extra IR is an extra place a feature can be dropped.**

### 2.2 Four targets, no `Target` enum

There is **no `enum Target { Rust, TypeScript, Interpreter, … }`** anywhere. Target selection is scattered:

- `crates/vox-cli/src/commands/build.rs` branches on a `--mode` string (`script`, `interp`, `tauri`, `web`) plus heuristics (does a `view:` body exist? is there a config section?);
- `codegen_rust::RustAppShell` (`codegen_rust/mod.rs:21`) enumerates *shell* variants (`AxumLocalServer` vs `TauriApp`) — not targets;
- `crates/vox-cli-core/src/artifact_policy.rs::TargetLane` is a *semantic classification*, not a codegen target.

**Consequence:** nothing in the type system forces a new feature to be considered against all targets. You can add a `HirExpr` variant, handle it in `codegen_rust`, and the compiler is perfectly happy that `codegen_ts` and `eval` never heard of it.

### 2.3 Three builtin registries, no master feature list

| Registry | File | Scope |
|----------|------|-------|
| Compiler/Rust | `crates/vox-compiler/src/builtin_registry.rs` | typeck + Rust codegen (~30 entries) |
| TypeScript | `crates/vox-codegen-ts/src/builtin_registry.rs` | TS lowering (separate list) |
| Interpreter | `crates/vox-compiler/src/eval/builtins.rs` | ~130 KB of interp-only impls |

These three lists are **maintained by hand and drift independently.** There is no test that asserts they cover the same set of names.

### 2.4 The silent-divergence surface

Catch-all `_ => …` arms in emit code (the sites where a feature can vanish without a trace):

> **[CORRECTED 2026-06-15]** The numbers below were the state at `fb2842670e`. The gap-remediation work (R1–R15, the 2026-06-13 audit fixes, the `ts-noemit` repair `275c09edc8`) has since converted most of these into *explicit* arms. **Verified current state on `main`:**
>
> | Emitter | Original claim | Verified now | What's actually there |
> |---|---|---|---|
> | `codegen_rust/emit/stmt_expr.rs` | 4 catch-alls (979/988/1092/1264) | **0 silent feature-enum catch-alls** | `emit_expr_with` (749–789) is exhaustive: explicit `compile_error!` arms for `Jsx`/`AsyncView`/`WorkflowVersion`/`With`, `tokio::spawn` for `Spawn`, and an `unreachable!` safety net (785). The 4 claimed lines match strings/ints/`Option`, not features. |
> | `codegen_ts/hir_emit/mod.rs` | 13+ catch-alls | **5 real** (222, 1178, 1181, 1979, 1980) + 1 explicit silent drop `WorkflowVersion => String::new()` (878) | `emit_hir_expr` (297) and `emit_hir_stmt` (1088) are now **exhaustive**. Real silent drops: `emit_hir_pattern` `_ => "_"` (1181, loses Constructor/Tuple), bind fallback → inert `onChange` (222), kwarg shapes → `None` (1979/1980). |
> | `web_ir/lower.rs` | 8 catch-alls | **1 primary** (250) + 1 benign stmt no-op (72) | `DomArena::lower_expr` line 250 `_ =>` is the honest `expr_fallback_count++` → `DomNode::Expr` accumulator. `HirEndpointKind`/`HirDbTableOp` matches are exhaustive. |
> | `eval/expr.rs` | fallback ~781–803 | **Accurate** (span is 781–**807**) | `eval_expr` is fully exhaustive; `Jsx`/`AsyncView`/`Spawn`/`With`/`WorkflowVersion` return actionable `EvalError`. **This is the model the other emitters should match.** |
>
> **Real remaining silent-drop surface ≈ 6 sites, all in the TS/web stack.** The two Rust-side dispatchers (`codegen_rust`, `eval`) are already exhaustive with explicit coded arms — they are *not yet matrix-driven* (each hand-rolls its own `compile_error!`/`EvalError` string), which is the remaining gap this SSOT closes.

The interpreter fallback is the *good* pattern (an error, not a silent drop) but it is a **runtime** error, not a **build-time** one — the user still finds it, not the compiler. The codegen_rust `compile_error!` arms are better still (build-time), but their codes are hand-written string literals, not entries in the codes registry — so nothing proves they stay in sync with the matrix. Centralizing them is the point of §3.2.

### 2.5 What parity enforcement exists today (partial)

> **[CORRECTED 2026-06-15]** Two tests this section originally listed as existing **do not exist in the codebase**: `webir_contract_parity_test` (described here as "the one real cross-emitter gate") and `tauri_endpoint_client_parity_test`. Both are only *referenced* — the former is a "(Create)" task in the 2026-06-13 gap-remediation plan, the latter appears only in doc inventory. Do not assume either as a baseline. The verified-existing parity tests are:

- `golden_arm_parity_test.rs` — **EXISTS** at `crates/vox-integration-tests/tests/`. Emit-only ratchet: asserts codegen-rust (`--mode script`) stdout matches each golden's `// EXPECT:` block (the interpreter's verified output), ratcheted against `contracts/eval/arm-parity-allowlist-script.txt`. `#[ignore]` (compiles generated crates).
- `ztier_and_layertier_agree_on_names_and_order()` — **EXISTS** as an inline smoke test at `crates/vox-codegen/src/web_ir/mod.rs:554` — two enums kept lock-step on the z-ladder.
- `webir_contract_parity_test` — **DOES NOT EXIST** (planned only). When built, it would assert `lower_hir_to_web_ir()` and `app_contract::project_app_contract()` agree on endpoint name sets.
- `tauri_endpoint_client_parity_test` — **DOES NOT EXIST** (referenced only). Would assert Tauri command names ↔ `vox_client.invoke()` call sites.

**None of the existing tests assert "every feature is handled by every target."** That gate does not exist yet. Building it (Artifact C, §3.3) is the heart of this initiative.

### 2.6 Diagnostics infrastructure (the good news)

This part is already SSOT-shaped and is the lever we pull:

- `crates/vox-compiler/src/typeck/diagnostics.rs` — `Diagnostic { severity, message, span, category, code, fixes }`, `TypeckSeverity::{Error, Warning}`, and a **single code registry** in the `codes` module (`ALL_COMPILER_DIAGNOSTIC_CODES`, uniqueness-guarded by a test).
- Precedent for "explicitly unsupported" codes already exists: `vox/codegen/mens-decorator-unimplemented`, `vox/codegen/pii-unimplemented`, `vox/codegen/embed-unimplemented`, `vox/lower/unlowered-decl`.

We extend this, not reinvent it.

---

## 3. The target design

Three artifacts, built in order. Each is independently testable.

### 3.1 Artifact A — the `Target` enum (the breadth axis, made real)

A new SSOT type, `crates/vox-compiler/src/target.rs`:

```rust
/// Every backend that can consume a lowered Vox program.
/// Adding a variant deliberately breaks every parity-checked match downstream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Target {
    Interpreter,   // vox_compiler::eval
    RustAxum,      // vox_codegen::codegen_rust (AxumLocalServer shell)
    RustTauri,     // vox_codegen::codegen_rust (TauriApp shell)
    TypeScript,    // vox_codegen::codegen_ts
}
```

> **Design note — deliberately NOT `#[non_exhaustive]`.** The point of this enum is that adding a target must break *every* downstream `match` (in `vox-codegen`, `vox-codegen-ts`, `eval`) so the compiler forces each emitter to decide how it handles the new target. `#[non_exhaustive]` does the opposite — it *requires* downstream crates to keep a `_` arm, which would reintroduce exactly the silent catch-all this initiative removes. So `Target` and `Feature` are both plain exhaustive enums. The "additions are deliberate" property comes from the build breaking, not from an attribute.

The CLI's `--mode` parsing in `build.rs` is rewritten to produce a `Target`, so target selection has exactly one home. `RustAppShell` (today's `AxumLocalServer` vs `TauriApp` shell choice in `codegen_rust/mod.rs`) becomes a projection of `Target`, not a parallel notion.

### 3.2 Artifact B — the `Feature` SSOT + support matrix (the contract)

A single declaration of every feature and its per-target support state. The shape (location: `crates/vox-compiler/src/feature_matrix.rs`):

```rust
pub enum Support {
    Implemented,
    Unsupported(&'static str), // a stable diagnostic code from the codes registry
}

/// The one place that answers "who implements what."
/// A feature with no row here fails the build (see §3.3).
pub fn support(feature: Feature, target: Target) -> Support { /* exhaustive match */ }

/// The single helper that resolves "what code + message does this unsupported
/// cell carry." Lives in vox-compiler beside the matrix (it reads `support()`).
/// Returns the raw (code, message) — NOT a concrete Diagnostic — so each emitter
/// adapts it into its own native error channel. There is no per-emitter copy of
/// the decision; there IS a thin per-emitter adapter (see crate-boundary note).
pub fn unsupported_diagnostic(feature: Feature, target: Target) -> UnsupportedCell { /* surfaces support()'s code + message */ }
```

> **[CORRECTED 2026-06-15] — crate-boundary: do NOT return one `Diagnostic` type.** The original draft had `emit_unsupported(...) -> Diagnostic` returning `vox_compiler::typeck::diagnostics::Diagnostic`. The audit found that **`vox-codegen` does not use that type at all** — it errors via `miette::Error` (the Rust emit path) and a codegen-local `WebIrDiagnostic` struct (the web path), and the existing `*-unimplemented` codes actually fire in vox-compiler's *typeck* pass (`ast_decl_lints.rs`), not in codegen. Forcing `vox_compiler::Diagnostic` across the crate boundary would be a brand-new, awkward usage pattern. **Corrected design:** the shared SSOT is `support()` returning the stable `&'static str` code (crate-agnostic). The helper returns a small `UnsupportedCell { code, message }`; each emitter has a one-line adapter into its native channel — `compile_error!("{code}: {message}")` in codegen_rust (already the pattern at stmt_expr.rs:749–782), a `WebIrDiagnostic`/throw in codegen_ts, an `EvalError` in eval, a `Diagnostic` in typeck. The matrix declares the truth once; the adapters are trivial and live with each emitter (so they may be written in Wave 2 without touching `feature_matrix.rs`). Also note the real `Diagnostic` constructor is `Diagnostic::error(message, span, source)` + `.with_code(code)` — there is no `error(code, message)`.

> **[CORRECTED 2026-06-15] — second code registry.** `WebIrDiagnostic` carries codes (e.g. `vox/codegen/missing-ts-ai-lowering`) that are **not** in `ALL_COMPILER_DIAGNOSTIC_CODES`. There are effectively *two* code registries that drift. Done Criterion §6.5 must therefore either (a) fold the WebIr codes into the compiler registry, or (b) explicitly scope parity codes to the compiler registry and leave WebIr codes governed separately. Decide this in Wave 1 Task 3; default recommendation: (a) — one registry, since the whole point is no parallel SSOTs.

`Feature` enumerates decorators, expr/stmt/decl kinds, and builtins. The matrix is an **exhaustive match** on `(Feature, Target)` — so adding either a `Feature` *or* a `Target` variant makes the matrix fail to compile until every new cell is filled in. This is the first half of the build gate.

> **[CORRECTED 2026-06-15] — matrix size (sizing the Wave-1 seed).** Verified current inventory: **56 decorators** (`At*` tokens), **36 expr/stmt kinds** (28 `HirExpr` + 8 `HirStmt`), **41 declaration kinds** (AST `Decl`). That is **133 feature rows × 4 targets = 532 cells** to seed in Wave 1 (builtins deferred to Task 10). This is a large but mechanical single-agent task. Keep rows terse with named row-builders — `all_targets()`, `frontend_only(code)`, `server_only(code)` — so each `Feature` arm is one line, not four. Do not split the seed across agents (it is one exhaustive match in one file; see §5).

> **Ownership:** both `support()` and `unsupported_diagnostic()` live in `crates/vox-compiler/src/feature_matrix.rs`. The matrix is **seeded in full (all features × all targets) in Wave 1** and is touched again only in Wave 3 (reconciliation). Emitter agents in Wave 2 never edit it — they only call `support()` / `unsupported_diagnostic()`. This is what keeps the parallel fan-out conflict-free (see §5).

> **Design note — avoid a second source of truth.** The matrix must be *derived from or checked against* the real emitters, not maintained beside them. The enforcement (§3.3) is what prevents the matrix itself from becoming a fourth registry that drifts. The matrix declares intent; the gate proves the emitters match the declaration.

### 3.3 Artifact C — the hard build gate (the enforcement)

Two complementary mechanisms. **Be precise about what each one is:** C1 is a literal compile error (`cargo build` fails); C2 is a test in the *required* CI gate (`cargo test` fails the merge). Both block landing code; only C1 blocks the local `build`. "Refuse to compile" in the headline is shorthand for "the build pipeline refuses to land it" — C1 is the compiler, C2 is the required gate.

**(C1) Compile-time exhaustiveness — kill the silent catch-alls.**
Every `_ => …` arm in `codegen_rust`, `codegen_ts`, `web_ir/lower`, and `eval` that handles a feature enum is replaced with explicit arms. Where a target genuinely does not support a feature, the arm calls a shared helper that emits the feature's declared `Unsupported(code)` diagnostic. Removing the catch-all means **the Rust compiler's own exhaustiveness checking** forces every new variant to be handled in every emitter — the cheapest possible gate, and it runs on every `cargo build`.

**(C2) Matrix-vs-reality parity test — the cross-emitter gate.**
A new `feature_matrix_parity_test` that, for every `(Feature, Target)` the matrix marks `Implemented`, drives a minimal `.vox` fixture through that target and asserts non-degenerate output (not a panic, not an `Unsupported` diagnostic, not empty). For every cell marked `Unsupported(code)`, it asserts the *declared code* is actually what the pipeline emits. This is the generalization of `webir_contract_parity_test` from "endpoint names" to "the whole feature set." A divergence between the declared matrix and observed behavior **fails the build** (it is a normal test, run in the `Check, Build, and Test (Rust)` required gate).

**Severity policy:** matrix violations and exhaustiveness failures are **errors** (block the build), per the chosen "hard gate" posture. The existing `Warning` severity remains available for genuinely advisory lints, but parity is not advisory.

---

## 4. Scope boundaries

**In scope:** the four targets and four IRs named in §2; decorators, expr/stmt/decl kinds, and builtins as defined in §1.1; the build-time gate.

**Out of scope (explicitly):**
- Collapsing 4 IRs → 2. That is the separate, still-open codegen-unification initiative. This SSOT makes the *current* 4-IR reality safe; it does not require the merge first. (If unification later lands, the parity matrix's pipeline-depth axis simply gets shorter — the contract is unchanged.)
- New language features. This is a wiring-integrity effort, not a feature effort.
- Runtime/semantic equivalence of *output* across targets (e.g. proving the Rust and TS emit produce byte-identical behavior). The gate proves *coverage and explicit-ness*, not behavioral equivalence; behavioral goldens are a separate, complementary track (see the 2026-06-02 golden-corpus plan).

---

## 5. Parallel-execution contract (how agents share this safely)

This work is decomposed so that independent agents touch **disjoint files**. The single rule that makes parallelism safe: **`feature_matrix.rs` is edited only in the two sequential waves (1 and 3); the parallel wave (2) never touches it.**

- **Wave 1 (foundation, sequential, one agent):** `target.rs`, then the *complete* `feature_matrix.rs` — every feature × every target seeded from the §2 audit and the §7 R1–R15 baseline — plus the `unsupported_diagnostic()` helper. The matrix is fully populated here, not incrementally per emitter. These define the types and the helper everything else imports, so they land first and alone.
- **Wave 2 (fan-out, one agent per emitter):** each agent routes its *one* emitter's unsupported-feature handling (`codegen_rust` / `codegen_ts` / `web_ir` / `eval`) through `unsupported_diagnostic()` and adds an exhaustiveness regression test. (Per the 2026-06-15 audit, `codegen_rust` and `eval` are already exhaustive — those two are routing tasks; the real silent drops are in TS/web.) These are separate file trees → no merge conflicts. **A Wave-2 agent edits only its emitter's files and its own exhaustiveness test — never `feature_matrix.rs`, never another emitter.**
- **Wave 3 (gate, sequential, one agent):** the `feature_matrix_parity_test`. If a cell's *declared* support disagrees with the *observed* emitter behavior, the fix is a one-line matrix correction made here, sequentially — never during the parallel wave.

Because the only shared file (`feature_matrix.rs`) is written in Wave 1 and corrected in Wave 3, and both are single-agent sequential waves, the parallel fan-out has **no shared mutable state**. This is a committed design, not a "serialize if it gets conflict-prone" fallback.

> **Scope of the gate — breadth, not depth.** The enforcement (§3.3) gates the *target-breadth* axis (`Feature × Target`): every feature is handled by every target, explicitly. It does **not** independently gate the *pipeline-depth* axis (per-IR survival) — that is covered transitively, because a feature that fails to lower through HIR/WebIR/VoxIR will fail its target's emit and be caught by C2. If the 4→2 IR unification later lands, the depth axis shrinks and nothing in this contract changes.

---

## 6. Done criteria

The initiative is complete when **all** hold:

1. `target.rs` exists; `build.rs` selects a `Target`; no other code re-derives target from `--mode`.
2. `feature_matrix.rs` exists; `support(Feature, Target)` is an exhaustive match (adding a variant breaks the build).
3. Zero feature-handling `_ => …` catch-alls remain in `codegen_rust`, `codegen_ts`, `web_ir/lower`, `eval` (verified by a grep-gate in CI).
4. `feature_matrix_parity_test` passes and is wired into the required Rust gate.
5. Every `Unsupported` cell names a real code in `ALL_COMPILER_DIAGNOSTIC_CODES`.
6. The R1–R15 and 2026-06-13-audit gaps each map to a now-`✓` or now-`✗ (declared)` matrix cell (the audit becomes a regression baseline, not a memory).

---

## 7. Appendix — provenance and the R1–R15 seed baseline

### 7.1 Provenance
- Current-state facts: direct code read on `main@fb2842670e`, 2026-06-14.
- Gap baseline: R1–R15 adversarial review ([PR #295](https://github.com/vox-foundation/vox/pull/295)) and the 2026-06-13 pipeline gap audit (`graphify-out/PIPELINE_GAP_AUDIT.md`; 7 wiring-gap patterns, ~55 verified gaps).
- Diagnostics SSOT: `crates/vox-compiler/src/typeck/diagnostics.rs` (`codes` module).

### 7.2 R1–R15 seed (inline, so Wave 1 needs no external doc)
These are the verified fixes from PR #295. Each maps to one or more matrix cells; Wave 1 seeds those cells as `Implemented` (and any cell the fix proved *un*supported as `Unsupported(code)`). This list is the authoritative seed — do not re-derive it from the PR.

| Ref | Fix | Feature(s) → cells affected |
|-----|-----|----------------------------|
| R6  | `HirAuth` struct + `@auth` decorator → HIR lowering + Rust auth-guard middleware | `@auth` → RustAxum/RustTauri `Implemented`; TS/Interp seed per real state |
| R2  | `scaffold.rs` uses `serde_json` for `package.json`; dropped `standalone` feature guard | TS scaffold path (not a Feature cell; build-infra) |
| R12 | `lower_warnings` drained into the diagnostic stream in `typecheck_hir_module_with_path` | diagnostics plumbing (enables every `Unsupported(code)` to surface) |
| R13 | stale `vox-tauri-stt-guest` entry removed from `pnpm-lock.yaml` | build-infra, no cell |
| R14 | WebIR match-arm `expr_fallback_count` incremented on catch-all | `web_ir/lower` catch-all accounting — template for Task 6 |
| R15 | WebIR if-branch non-`Expr` stmt pre-flight guard | `web_ir/lower` — template for Task 6 |
| R8J | `@public` decorator parsing + golden backfill; prefix path now sets `is_auth_exempt` | `@public` → all targets; interacts with `@auth` (public-auth-conflict typeck) |

The 7 wiring-gap patterns from the 2026-06-13 audit (catch-alls hide gaps three ways; decorator cliff; context-dependent silent drop; dead emitters; half-wired `when{}`; structural-only goldens; split-brain) are the *categories* the matrix exists to make impossible. Each becomes either a now-`✓` cell or a now-`✗ (declared)` cell — Done Criterion §6.6.

### 7.3 Notes
- "Non-degenerate output" in §3.3/C2 means *not a panic, not an `Unsupported` diagnostic, not empty* — it proves **coverage and explicit-ness**, not behavioral correctness. Behavioral equivalence across targets is the separate golden-corpus track (§4).
- This document's development conversation is linkable for readers who want the reasoning trail.
