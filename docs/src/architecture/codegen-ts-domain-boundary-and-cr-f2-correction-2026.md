---
title: "Codegen-TS Domain Boundary & CR-F2 Correction (Handoff)"
description: "What the Rust-vs-TypeScript emit boundary actually is (logic→Rust, browser/GUI→TypeScript), why CR-F2's 'three-arm byte-parity' framing was wrong, and the scoped plan to reach the intended state."
category: "Architecture SSOTs"
status: roadmap
training_eligible: false
---

# Codegen-TS Domain Boundary & CR-F2 Correction — Handoff

> **Status:** roadmap / handoff. Corrects the CR-F2 definition in
> [`v1-release-criteria.md`](./v1-release-criteria.md) and the Phase 1 plan
> [`2026-06-06-phase1-cross-arm-parity.md`](../../superpowers/plans/2026-06-06-phase1-cross-arm-parity.md).
> Written 2026-06-07 on branch `cc_bdesktop2/phase1-cross-arm-parity`.

## 1. The intended model (maintainer steer, 2026-06-07)

> "Most logical code should live in Rust and be emitted as such; anything that
> requires a browser or GUI is emitted in TypeScript only. I do **not** want to
> emit all my code as both Rust *and* TypeScript."

So Vox has **two emit domains**, split by *responsibility*, not duplicated:

| Domain | Emitter | Emits | Does NOT emit |
|---|---|---|---|
| **Logic / backend** | `codegen_rust` | `fn main`, free functions, `@server`/`@query`/`@mutation`, data layer, control flow, arithmetic, ADTs, the whole computational program → native Rust (Axum/Tauri backend) | UI |
| **Browser / GUI** | `codegen_ts` | `component`s, `@island`s, JSX, forms, client `routes`, reactive views, the typed **client SDK** that *calls* the Rust backend → TypeScript/React | logic / `main` |

The browser gets a **typed client** (`vox-client.ts`) + UI components; the
computation stays in Rust on the server. This is one program with two
projections of *different* surfaces — not one program transpiled twice.

## 2. What we actually have (verified 2026-06-07)

**This is substantially already true.** Evidence:

- `vox build` (default `target = "fullstack"`): writes **TypeScript to `out_dir`**
  and **Rust to `target/generated/`** (`crates/vox-cli/src/commands/build.rs`).
  `--target=server` = Rust-only; `--target=client` = TS SDK only.
- `codegen_ts::emitter::generate*` lowers **`hir.components` and
  `hir.client_routes`** (+ forms, JSX, reactive, OpenAPI client) — *not* plain
  functions or `main`. (`crates/vox-codegen/src/codegen_ts/emitter.rs`.)
- `codegen_rust` lowers the computational program (`main`, free fns, data ops).
- The TS corpus `examples/golden-ts/` is entirely web fixtures
  (`component_state`, `form_basic`, `routes_with_loader`, `safe_area`, …).
- **Probe result** (`crates/vox-integration-tests/tests/ts_emit_goldens_probe.rs`):
  the 10 *logic* goldens in `examples/golden/` (the `fn main` + `// EXPECT:`
  stdout corpus) emit through codegen-ts with **0/10 defining `main`** — codegen-ts
  correctly has **nothing computational to emit** for logic-only programs.

**Conclusion:** the boundary is the intended one. The earlier alarm ("codegen-ts
drops `main`") was codegen-ts behaving correctly for its domain.

### 2.1 Known warts (small, real)

1. **Empty-app boilerplate.** For a logic-only program (no components/routes),
   codegen-ts still emits a minimal app shell — `vox-app-contract.json` (empty
   routes/server-fns) + `vox-tanstack-query.tsx` boilerplate — instead of
   emitting **nothing** (or a clear "no web surface" no-op). Cosmetic, but it
   muddies "did this program even have a browser surface?".
2. **No explicit split contract.** Nothing *asserts* that every top-level
   construct lands in exactly one arm. A construct that is silently dropped by
   *both* (or emitted by both) would not be caught today.

## 3. What was wrong (and is hereby corrected)

**CR-F2 as written** in `v1-release-criteria.md` says *"Cross-arm parity (all
three arms). Every executable golden produces `{interp_out, script_out, ts_out,
all_agree}`."* That is **wrong for the TS arm**: codegen-ts does not produce a
stdout rendering of a logic program, by design. There is no `ts_out` for
`decimal_math` because decimal math is not a browser concern.

The roadmap rewrite's "codegen-ts in full" steer is still honored — but "in
full" means **codegen-ts is fully correct for its domain (browser/GUI)**, *not*
"codegen-ts can emit every stdout program."

**Pruned:** the Phase 1 idea of a codegen-ts "script/console emit mode" that
lowers `main` + free functions to runnable TS. That would emit logic as
TypeScript — the exact thing the maintainer does **not** want. Do **not** build
it. (The `ts_emit_goldens_probe.rs` census ratchet that asserts `defines_main ==
0/10` now encodes this as a guardrail: if codegen-ts ever starts emitting `main`
for logic goldens, that's a regression against the domain boundary, not
progress.)

## 4. Corrected CR-F2 — the target state

CR-F2 splits into **two independent correctness criteria**, one per domain:

### CR-F2a — Logic parity (interp ≡ codegen-rust)
Every `examples/golden/**` program with `fn main` + `// EXPECT:` produces
**byte-identical stdout** under `--mode interp` and `--mode script`
(codegen-rust). This is the real two-arm parity that matters for "the language
computes the same thing however you run it."
- **Today:** interp 10/10; codegen-rust ~3/10 (7-class backlog).
- **Gate:** the Rust compile-and-run harness (Phase 1 Task B) — see §5.

### CR-F2b — Web emit correctness (codegen-ts)
Every `examples/golden-ts/**` web fixture emits TypeScript that (i) type-checks
and (ii) behaves correctly in a browser/DOM.
- **Today:** `tsc --noEmit` typecheck only
  (`crates/vox-integration-tests/tests/ts_emit_typecheck_test.rs`, `#[ignore]`).
- **Gap:** no **behavioral** check (render under jsdom/Playwright, assert DOM /
  event outcomes). Typecheck ≠ correctness.

### CR-F2c — Split discipline (the boundary itself)
A **routing contract** asserting that each top-level construct lands in exactly
the right arm: logic → Rust only; browser/GUI → TS only; nothing silently
dropped by both or emitted by both. This is what *guarantees* the maintainer's
"logic in Rust, GUI in TS, not both" intent holds as the language grows.
- **Today:** none.
- **Gap:** a `vox audit --gate emit-routing` (or arch-check rule) over a
  per-construct classification table.

## 5. Scoped plan to reach the intended state

Ordered by leverage; each is an independent slice.

1. **Rewrite CR-F2 in `v1-release-criteria.md`** into CR-F2a/b/c (above). Remove
   the `ts_out`/`all_agree` three-way-stdout language. *(Docs; do first so the
   gates target the right thing.)*
2. **Build the logic-parity harness (CR-F2a) — Phase 1 Task B (STARTING NOW).**
   A compile-and-run harness on the `crates/vox-codegen/tests/emit_compile_harness.rs`
   pattern: for each logic golden, generate the Rust script crate, build it in an
   **isolated `CARGO_TARGET_DIR`** (so one failure can't poison others — see the
   `~/.vox/script-target` corruption finding in the Phase 1 plan), run it, capture
   stdout (strip the `INFO vox.script:` line), compare to interp + `// EXPECT:`.
   Ratcheting allowlist seeded at the current divergence set. CI-tier `#[ignore]`.
3. **R0 — script-lane robustness.** Make the script lane not corrupt the shared
   `~/.vox/script-target` on build failure (and honor `resolve_target_dir`'s
   currently-ignored `_isolation` param). Helps every `vox run --mode script` user.
4. **Codegen-rust fix stream (CR-F2a → 100%).** Work the R-class backlog
   golden-by-golden under the harness's ratchet. **Re-verify each class against a
   clean build first** — R1 ("missing `rust_decimal`") is suspect since
   `decimal_math` is in the passing 3/10.
5. **Web behavioral gate (CR-F2b).** Add a jsdom/Playwright render+assert pass
   over `examples/golden-ts/` on top of the existing `tsc` gate.
6. **Emit-routing gate (CR-F2c).** Per-construct classification table + a gate
   asserting each construct lands in exactly one arm; fix the empty-app
   boilerplate wart (logic-only program → no web emit / explicit no-op).

## 6. Handoff checklist

- [ ] CR-F2 rewritten (CR-F2a/b/c) in `v1-release-criteria.md`.
- [ ] Logic-parity harness landed (Task B) + ratchet allowlist committed.
- [ ] R0 script-lane robustness fix.
- [ ] Codegen-rust backlog burned down to CR-F2a green.
- [ ] Web behavioral gate over `golden-ts`.
- [ ] Emit-routing gate + boilerplate wart fixed.

**Do NOT:** build a codegen-ts emit path for `fn main`/free-function logic. That
violates the domain boundary. The `ts_emit_goldens_probe` ratchet guards it.
