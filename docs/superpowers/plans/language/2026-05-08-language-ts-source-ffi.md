# Language: TS-source FFI from Vox components — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans.

> **amended (2026-05-08):** Surface syntax simplified. **Used:** `extern fn name(args) to T = "./module"` (one decl per extern; module path as a string literal after `=`). The original plan's `@ts.import("./mat", { fn1, fn2 })` decorator group is dropped — single-decl form is unambiguous, parses with one new keyword (`extern`), avoids the need to add an `@`-decorator token, and lets each extern carry its own module independently. **Implementation surface:** lexer adds `Token::Extern`; AST `FnDecl` gains `ts_extern_module: Option<String>`; HIR `HirFn` mirrors the field; parser's `parse_extern_fn` produces a regular `Decl::Function` with the field set and an empty body; typeck `check_function` returns early when the field is `Some`; HirFn construction sites in lower/decl.rs and vox-test-harness updated. **Codegen TS deferred** — emitting `import { name } from "<module>"` at the top of each consuming TS file requires per-file usage tracking (which extern names are referenced from which generated file). The codegen TS layer doesn't track this today; building it is a separate piece. Until codegen lands, generated TS will reference extern names without imports — fine for typeck/build verification, not yet runtime-runnable in the browser. **Codegen Rust check deferred** — the plan's "error if extern is reached on the Rust path" diagnostic is a follow-up. **A7 (app integration) deferred** to the app-phase work. **A6 (reference doc) deferred** — `docs/src/reference/lang-*` doesn't exist yet.

**Goal:** Allow Vox `.vox` files (especially components and endpoints emitted by `codegen_ts`) to import and call functions defined in sibling `.ts` files within the same app tree, with a typed surface declared on the Vox side.

Surface syntax (proposed):
```
@ts.import("./ts/materializer", { resolveCorrections, weeklyAggregate })
extern fn resolveCorrections(rows: List[HealthEventRow]) to List[MaterializedEvent]
extern fn weeklyAggregate(events: List[MaterializedEvent], now_ms: int, window_days: int) to WeeklyAggregate
```

**Why now:** The vox-mental-tracker uses `src/ts/materializer.ts` as the SSOT for derived state (correction-chain collapse, daily grouping, weekly aggregates with deterministic ordering). The Vox `WeeklyPage` and `TimelinePage` components currently can't call it — they call a Vox endpoint that does a coarser per-kind tally. To consume the rich materialized output in the UI today, we'd have to port the materializer logic into Vox (rewriting and re-testing 100+ lines of TDD'd code), or duplicate enough of it twice.

This is a structural problem: **TS already underlies all `codegen_ts` output**, so the runtime can call any TS module. We're paying for not letting Vox source say so.

**Architecture:**
- New AST decl: `TsImportDecl { module: String, names: Vec<String>, span }`.
- New AST decl: extend existing `FnDecl` with an `extern: Option<TsExternBody>` mode, OR add a separate `ExternFnDecl`. Recommend the latter for clarity.
- Typeck: register externs as opaque function bindings with the user-supplied signature; treat callees as black boxes (no body to lower).
- HIR: lower extern fn decls to a HIR node carrying the import module path; calls become normal function calls.
- Codegen TS: emit `import { resolveCorrections, weeklyAggregate } from "./ts/materializer"` at the top of the generated file, then map calls directly.
- Codegen Rust: extern functions are **not callable** from server-only contexts unless an equivalent Rust impl is registered (a separate plan; for the tracker we only need them in the TS surface, used by browser components).
- Compile-time check: extern fn declarations are only legal inside `.vox` files that produce TS output (component-only modules / `kind: query|mutation` endpoints emitted to TS). Rust-only contexts (server-only handlers) reject them with a clear error pointing at this plan's Rust-side gap.

**Tech Stack:** Rust (parser + AST + codegen), no runtime change beyond emitted imports.

**Out of scope:**
- Auto-generating typings (`.d.ts`) from Vox struct decls (would close the loop, but requires struct types — separate plan — and a reverse codegen pass).
- Calling extern functions from Rust contexts (need a parallel Rust-source FFI plan).
- npm-package imports (`@scope/pkg`) — separate plan; for the tracker, sibling-file imports are enough.

---

## File Structure

**Created:**
- `examples/golden/ts_source_ffi.vox` (+ `examples/golden/ts_source_ffi_helper.ts`) — minimal compile + call.
- `docs/src/reference/lang-ts-source-ffi.md` — user-facing reference + the "only emit-TS contexts" constraint.

**Modified:**
- `crates/vox-compiler/src/parser/descent/decl/**` — parse `@ts.import(...)` and `extern fn`.
- `crates/vox-compiler/src/ast/decl/**` — new `TsImportDecl`, `ExternFnDecl`.
- `crates/vox-compiler/src/hir/**` — lower the new decls.
- `crates/vox-compiler/src/typeck/**` — register externs; reject non-TS-callsite usage.
- `crates/vox-compiler/src/codegen_ts/**` — emit imports + pass through calls.
- `crates/vox-compiler/src/codegen_rust/**` — error if an extern is reached on the Rust path.

---

## Tasks

- [ ] **A1.** Decide on the decl syntax. Two options: (a) `@ts.import("./mat", { fn1, fn2 }) extern fn fn1(...) to ...` — split decoration + signatures; (b) module-level `import ts:./mat { fn1: ..., fn2: ... }` block. Recommend (a) — composes with existing `@-decorator` style.
- [ ] **A2.** Parser support for `@ts.import` decorator and `extern fn`.
- [ ] **A3.** AST + HIR nodes.
- [ ] **A4.** Typeck registration; reject in Rust-emitting contexts.
- [ ] **A5.** TS codegen — gather all `@ts.import` decls per file, emit a single `import { ... } from "..."` line per source.
- [ ] **A6.** Golden example (Vox file imports a tiny TS helper that returns a list and the Vox file iterates it).
- [ ] **A7.** Update `apps/vox-mental-tracker/src/main.vox`: in the relevant pages, declare externs for the materializer and use them. Once this plan lands and structs land (cross-plan dependency), the WeeklyPage can render true materialized rollups.

---

## Verification

- [ ] `cargo nextest run -p vox-compiler` passes.
- [ ] `vox check examples/golden/ts_source_ffi.vox` passes; emitted TS imports the helper correctly.
- [ ] After A7, the tracker's WeeklyPage calls into the materializer and round-trips with realistic fixture data (Playwright lane).
- [ ] Negative test: declaring an `extern fn` in a Rust-only `@server fn` errors with a specific diagnostic mentioning this plan.
