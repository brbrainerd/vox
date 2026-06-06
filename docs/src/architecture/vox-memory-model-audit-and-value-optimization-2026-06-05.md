---
title: "Vox Memory Model: Audit & Value-Semantics Optimization Plan"
description: "Audit of how Vox reclaims memory across all targets (no GC; pure value semantics) and a phased plan to remove the clone-cost cliffs via copy-on-write and structural sharing — without a collector and without changing the actor invariant."
category: "Architecture SSOTs"
status: "current"
---

# Vox Memory Model: Audit & Value-Semantics Optimization Plan

*Authored 2026-06-05. Companion to [`zero-copy-rust-emission-plan-2026.md`](zero-copy-rust-emission-plan-2026.md) (which it extends) and supersedes the archived per-actor-GC research conclusion for the near-term horizon.*

## TL;DR

Vox has **no garbage collector, and as currently designed it does not need one.** The language exposes pure
value semantics — no references, no pointers, no shared mutable state — so reference cycles cannot form and
Rust's `Drop` reclaims everything deterministically. Memory management is *inherited* from each target's
runtime (Rust ownership for the interpreter / `--mode script` / Axum / Tauri; the JS engine's GC for emitted
web & React-Native code; wasm linear memory for WASI scripts). Users never manage memory manually today, and
that guarantee is not at risk.

The real problem is **not safety, it is cost and expressiveness**:

1. **Clone-cost cliffs.** The interpreter deep-copies `Vec<VoxValue>` collection payloads on every
   pass-by-value, and closures clone the *entire* lexical environment on creation and again on application
   (136 `.clone()` sites in `crates/vox-compiler/src/eval/`). Append-in-a-loop is accidentally O(n²) memory;
   closure-heavy code copies whole scopes repeatedly.
2. **Clone-heavy emitted Rust.** The zero-copy codegen plan elides clones only for `Copy` primitives; escape
   analysis and last-use/move analysis remain unimplemented.

**This plan keeps "no GC" and adds structural sharing.** The insight: *Vox needs sharing, not a collector.*
Copy-on-write (`Rc` + `Rc::make_mut`) and a cactus-stack scope give value semantics with O(1) clones and zero
collector machinery. Persistent (RRB) structures are a benchmark-gated phase-4 upgrade. None of this exposes
a memory API, and the design layers cleanly under a future per-actor arena should one ever be justified.

---

## Status — Phase 0 gate executed (2026-06-05)

The hard gate that blocked everything else is **resolved**. Verified against current code:

| Item | Result |
|---|---|
| `Rc` vs `Arc` for `VoxValue` | **`Rc` (sound)** — never `.await`-held, never crosses threads, no `Send` bound, never enters the actor mailbox. Multi-threaded tokio runtime exists but interp is synchronous on the main task → invariant to guard (Phase 5). |
| Actor messages "deep-copied" | **Falsified** — mailbox uses zero-copy `bytes::Bytes` (`MessagePayload`), and `VoxValue` is *not* the message type. Conclusion (no cross-actor sharing of `VoxValue`) stands; the stated mechanism was wrong and is corrected. |
| Codegen last-use/move analysis "unchecked" | **Falsified** — already implemented (`UsageTracker`). Pruned; only **escape analysis** remains in Workstream B. |
| Collection mutation clones whole `Vec` | **Confirmed** (`builtins.rs:290` `push` does `v.clone()`; `stmt.rs:235-255` index/key assign clone-and-reassign). Validates Phase 1 CoW. |
| Benchmark surface | **`crates/vox-compiler/benches/`** exists, criterion already dev-dep. No new crate needed. |
| Canonical eval home | **`crates/vox-compiler/src/eval/`** (not `crates/vox-eval/`, a re-exported shim). |
| CR-E1 `<50 ms` cold start | **Confirmed** (`v1-release-criteria.md:25`) — verification gate intact. |

**Phase 0 baseline benchmark — WRITTEN & RUN (`crates/vox-compiler/benches/eval_memory.rs`).** Drives the
interpreter end-to-end on fresh `Interpreter` instances; lex/parse/lower excluded from timing. Median wall-time
(10 samples, release; hardware-dependent — treat as relative, not absolute):

| Case | Workload | Baseline (pre-CoW) | Cliff it isolates |
|---|---|---|---|
| `append_loop` | 1 500 `push` into one list | **~26.9 ms** | O(n²) whole-`Vec` clone per push |
| `big_list_pass` | 3 000-elem list passed 250× | **~147 ms** | deep-copy on pass-by-value (~0.6 ms / pass) |
| `closure_pipeline` | map→filter→fold over 3 000 | **~421 ms** | intermediate-list alloc + lambda scope capture |
| `object_pass` | 2-field record passed 60 000× | **~802 ms** | record `Vec<(String,VoxValue)>` clone (~13 µs / call) |
| `deep_recursion` | `fib(24)` (~75 k calls) | **~1.47 s** | whole-scope clone per call (Phase 2 target) |

These validate the thesis quantitatively: passing/copying values dominates runtime. **Phase 0 is complete.**

## Status — Phase 1 (CoW collections) LANDED & VERIFIED (2026-06-05)

`VoxValue::{List,Object,Tuple}` now hold `Rc<Vec<…>>` payloads (`value.rs`), with `VoxValue::list/object/tuple`
constructors and a new `Scope::get_mut` (`env.rs`). Clones are O(1) refcount bumps; in-place mutation
(`push`/`pop`/index-assign/key-assign) uses `Rc::make_mut` — mutate-in-place when the binding solely owns the
payload, clone-once when aliased. Construction sites across `builtins.rs`/`mod.rs`/`expr.rs`/`db.rs`/`stmt.rs`
were migrated to the helpers (read-pattern matches auto-deref unchanged).

**Verification:**
- Library compiles clean (0 errors).
- **Full `vox-compiler` test suite green** (~40 binaries, 0 failures) — no behavioral regression.
- **Safety-net `eval_cow_semantics_test.rs` (6 aliasing tests) green** — value semantics preserved bit-for-bit
  (assignment copies, callee mutation doesn't leak, `map` non-mutating, single-owner `push` still mutates).
- **`eval_memory` benchmark, post-CoW vs pre-CoW baseline (all p < 0.05, "Performance has improved"):**

| Case | Pre-CoW | Post-CoW | Change |
|---|---|---|---|
| `append_loop` | ~26.9 ms | **~9.33 ms** | **−64%** (O(n²) push → in-place `make_mut`) |
| `big_list_pass` | ~147 ms | **~31.4 ms** | **−79%** (deep-copy → O(1) `Rc` clone) |
| `closure_pipeline` | ~421 ms | **~43.1 ms** | **−90%** (intermediate lists now shared) |
| `object_pass` | ~802 ms | **~234 ms** | **−71%** (record clone → O(1)) |
| `deep_recursion` | ~1.47 s | **~450 ms** | **−69%** (scope/arg copies cheap) |

Even `deep_recursion` improved 69% despite Phase 2 not being done — because argument/return value copies are
now O(1). **Phase 1 done.**

## Status — Phase 2 (Rc-shared scopes + Rc closure body) LANDED & VERIFIED (2026-06-05)

`Scope.frames` is now `Vec<Rc<HashMap<…>>>` (`env.rs`) so cloning a `Scope` — done on **every** closure capture
and **every** closure application (`Fn.env`) — is O(frames) refcount bumps instead of deep-cloning all
bindings; writes use `Rc::make_mut` (CoW). `Fn.body` is now `Rc<Vec<HirStmt>>` (`value.rs`) so the per-call
body clone is O(1) (the `deep_recursion` killer). All `scope.clone()` capture sites were unchanged — the win
came purely from the representation. Surface: 9 mechanical compile fixes across `expr.rs`/`mod.rs`.

**Verification:** lib clean; full `vox-compiler` suite green (**339 passed + all bins, 0 failed**); safety net
**8/8** (added closure-capture-by-value + recursion tests). Benchmark **vs post-Phase-1**: `deep_recursion`
−80%, `object_pass` −79%, `closure_pipeline` −18%, `append`/`big_list` ~−6%.

### Combined Phase 1 + Phase 2 vs original pre-CoW baseline

| Case | Pre-CoW | After P1+P2 | Total |
|---|---|---|---|
| `append_loop` | ~26.9 ms | ~8.89 ms | **−67%** |
| `big_list_pass` | ~147 ms | ~31.0 ms | **−79%** |
| `closure_pipeline` | ~421 ms | ~36.1 ms | **−91%** |
| `object_pass` | ~802 ms | ~48.7 ms | **−94%** (16×) |
| `deep_recursion` | ~1.47 s | ~90.1 ms | **−94%** (16×) |

**No GC, no new dependencies, zero behavioral regression** — pure structural sharing. Next: Workstream B
escape analysis (codegen, independent) and the Phase 5 `Rc`-soundness guard.

---

## Part 1 — Audit: how Vox manages memory today

### 1.1 The runtime value model (interpreter)

The core runtime type is [`VoxValue`](../../../crates/vox-compiler/src/eval/value.rs) — a plain
`#[derive(Debug, Clone)]` enum:

```rust
pub enum VoxValue {
    Int(i64), Float(f64), Decimal(rust_decimal::Decimal),
    Str(String),
    List(Vec<VoxValue>),                 // payload deep-copied on clone
    Object(Vec<(String, VoxValue)>),     // payload deep-copied on clone
    Tuple(Vec<VoxValue>),                // payload deep-copied on clone
    Fn { params: Vec<String>, body: Vec<HirStmt>, env: crate::eval::env::Scope },  // whole scope cloned
    Option(Option<Box<VoxValue>>),
    Result(Result<Box<VoxValue>, Box<VoxValue>>),
    // ... Constructor / Tagged / control-flow sentinels
}
```

- **By-value primitives**, heap `Box` for `Option`/`Result`, **owned `Vec`** for collections.
- **No `Rc`/`Arc`/`RefCell` anywhere in the value enum** — so values are *copied*, never shared.
- `#[derive(Clone)]` means every assignment, argument pass, and return that the interpreter treats as
  pass-by-value performs a **deep recursive copy** of the whole structure.

The scope is a stack of `HashMap<String, VoxValue>` frames
([`env.rs`](../../../crates/vox-compiler/src/eval/env.rs)), itself `#[derive(Clone)]`.

### 1.2 Why there is no GC — and why that is correct

Because nothing is shared by reference, **cycles are structurally impossible**, which is precisely the case a
tracing GC exists to handle. Reclamation is automatic and deterministic:

- A function returns → its frame is popped → all locals `Drop` → owned heap (`String`, `Vec`, `Box`) freed.
- A binding is reassigned → the old `VoxValue` is dropped.
- The interpreter is dropped at program end → everything frees.

The only "cycle detection" in the codebase is the *import* cycle guard
(`loaded_imports: HashSet<PathBuf>` in `eval/mod.rs`), which is a module-loading concern, not a memory one.

### 1.3 Memory management is per-target, inherited from the host runtime

"Does it only work on our Rust code?" — No. Vox lowers from a single semantic core (`HirModule`) to several
targets, each of which brings *its own* memory management:

| Target | How invoked | Memory managed by |
|---|---|---|
| Tree-walking interpreter | `vox run --mode interp` | Rust ownership / `Drop` over `VoxValue` |
| Native script | `vox run --mode script` | Rust ownership (tokio) — currently clone-heavy |
| Full-stack / server | `vox build` (Axum) | Rust ownership |
| Desktop / mobile | `vox compile --target desktop\|mobile-*` (Tauri) | Rust ownership (backend) + **JS GC** (webview) |
| Web | emitted TS/React | **JS engine GC** (V8) |
| React Native | emitted TS + uniffi bridge | **JS GC** (Hermes/JSC) + Rust ownership (bridge) |
| WASI script | wasm32-wasip1 | Rust allocator over wasm **linear memory** (no GC) |

So when Vox emits web/RN code, programs already ride a real garbage collector — the browser's. The
interpreter and Rust targets ride Rust's ownership instead. **Value semantics make the choice of collector
irrelevant to the Vox programmer**, which is the property we want to preserve.

### 1.4 What the docs already commit to (and don't)

- `expl-rosetta-inventory.md` (current): Vox enforces **"copy-by-value on assignment"**; "the struct *is* the
  schema" (structs lower to Rust value types).
- `expl-runtime.md` (current): **actors outlaw shared mutable state**; mailboxes are `mpsc` channels.
  *Correction (verified 2026-06-05): the message payload is **not** `VoxValue` and is **not** deep-copied —
  `crates/vox-actor-runtime/src/mailbox.rs` uses `MessagePayload::{Text,Json,Binary}(bytes::Bytes)`, an
  atomically ref-counted view (O(1) clone, shared allocation). The interpreter's `VoxValue` never enters the
  mailbox path at all, which is what makes intra-actor `Rc` sharing sound.*
- `v1-release-criteria.md` (current): **no memory-model commitment** in the gates. CR-E1 requires
  `vox run --interp` Hello-World cold start **< 50 ms**.
- `memory-management-llm-research-2026.md` (**archived**, `status: research`, 2026-04-18): proposed a
  **per-actor GC** (Erlang model) and explicitly rejected a **global tracing GC** for its stop-the-world
  pauses and runtime tech-debt. Never shipped.
- `zero-copy-rust-emission-plan-2026.md` (current, in progress): generated Rust is `.clone()`-saturated;
  type-enrichment + `Copy`-primitive elision done. *Correction (verified 2026-06-05): **last-use / move
  analysis is already implemented** — `codegen_rust/emit/usage.rs` (`UsageTracker`) + `stmt_expr.rs:406`
  emit a bare identifier (move) at last use. Only **escape analysis** (`&n` / `n.as_str()` for borrowing
  callees) remains, and it is partially present via `OwnershipMode::Borrowed`.*

**Conclusion of the audit:** the memory model is *safe but inefficient, and safe but inexpressive*. There is
no defect to fix in correctness; there is a performance cliff to remove and an undocumented model to ratify.

### 1.5 The concrete cost cliffs

| Cliff | Location | Symptom |
|---|---|---|
| Collection deep-copy on pass-by-value | `List`/`Object`/`Tuple` are owned `Vec` | Passing a 10k-element list copies 10k elements |
| Append-in-loop | mutate path clones the whole `Vec` per step | Accidentally O(n²) memory/time |
| Closure capture | `env: interp.scope.clone()` ([expr.rs:370](../../../crates/vox-compiler/src/eval/expr.rs)) | Every lambda snapshots the *entire* variable environment |
| Closure application | `params/body/env` all re-cloned ([expr.rs:710](../../../crates/vox-compiler/src/eval/expr.rs)) | Each call re-copies the captured environment + HIR body |
| Builtin arg double-pass | `eval_args.clone()` before global-builtin dispatch ([expr.rs:382](../../../crates/vox-compiler/src/eval/expr.rs)) | Args copied even on the builtin fast-path |
| Emitted-Rust clones | codegen lacks escape/last-use analysis | `.clone()` on every non-`Copy` identifier use |

---

## Part 2 — Goal & non-goals

**Goal.** Remove the clone-cost cliffs while keeping (a) pure value semantics, (b) "no GC", (c) the actor
no-shared-mutable-state invariant, and (d) zero user-visible memory management.

**Non-goals (explicit).**
- **No tracing/global GC** — rejected for STW pauses & tech-debt (consistent with the archived research).
- **No per-actor GC in this plan** — that is the deferred higher-expressiveness option (graphs, cyclic data
  within an actor). This plan is sequenced so a per-actor arena could layer on later *without rework*.
- **No user-visible memory API** — no references, lifetimes, `new`/`free`, or handles exposed to Vox source.

**Core technique.** Copy-on-write via reference-counted payloads. Cloning a value becomes an O(1) refcount
bump; the deep copy is deferred to the moment of a *mutation through an aliased value* (`Rc::make_mut`). Value
semantics are preserved bit-for-bit: two bindings to "the same list" remain independent because the first
write through either one triggers exactly one copy.

---

## Part 3 — Workstream A: interpreter structural sharing

### Phase 0 — Measurement & the `Rc` vs `Arc` decision (do first)

- [x] **Audit `Send`/`Sync` bounds on `VoxValue` — RESOLVED 2026-06-05: use `Rc`.** Verified exhaustively:
  `VoxValue` is never held across an `.await`, never sent across a thread/channel, never enters the actor
  mailbox (which uses its own `Bytes` payload), and carries **no `Send`/`Sync` trait bound** anywhere in
  `eval/` or its callers. The two `std::thread::spawn` sites (`eval/builtins.rs:2605,2623`) move only
  `String`. `Fn.env` already holds non-`Send` data (`Vec<HashMap>`). **`Rc` is sound — no `Arc` needed.**
  - **Invariant to protect (NEW):** the binary uses a multi-threaded `#[tokio::main]` runtime
    (`vox-cli/src/main.rs:52`); `Rc`-soundness depends on the interp loop staying *synchronous on the main
    task*. Add a guard so this can't silently regress — see Phase 5. Do **not** add a `Send` bound to
    `VoxValue` or `tokio::spawn` an eval closure holding a `VoxValue`.
- [ ] **Build a memory/time micro-benchmark harness** in the **existing** `crates/vox-compiler/benches/`
  (criterion is already a dev-dep; follow the `compiler_pipeline.rs` / `golden_examples.rs` `harness = false`
  pattern — do **not** create a new `vox-bench` crate). Adversarial cases: (1) pass-a-big-list,
  (2) append-in-loop, (3) deep recursion, (4) closure-heavy / map-filter pipelines, (5) large immutable
  object shared across calls. Record **baseline** wall-time and peak RSS per case.
  *(Note: the eval code lives in `crates/vox-compiler/src/eval/`; `crates/vox-eval/` is a near-empty shim
  re-exported via `pub use vox_eval::*` — edit the former, not the latter.)*

### Phase 1 — Copy-on-write collection payloads (biggest win, zero new deps) — ✅ DONE 2026-06-05

- [x] **Wrapped collection payloads in `Rc`:** `List(Rc<Vec<VoxValue>>)`, `Object(Rc<Vec<(String, VoxValue)>>)`,
  `Tuple(Rc<Vec<VoxValue>>)` + `VoxValue::list/object/tuple` constructors (`value.rs`).
  (`Str(Rc<str>)` deferred — strings weren't a measured cliff.)
- [x] **Reads** borrow through the `Rc` via auto-deref — unchanged, no copy (most match arms compiled as-is).
- [x] **Mutations** go through `Rc::make_mut` via new `Scope::get_mut` (`env.rs`): in place if uniquely held,
  else clone-once. Applied to push/pop/index-assign/key-assign (`stmt.rs`, `builtins.rs`).
- [x] **Clone is O(1)** — verified: `big_list_pass` −79%, `object_pass` −71%.
- [x] **Aliasing-independence tests** (`tests/eval_cow_semantics_test.rs`, 6 tests, green). *Follow-up: also add
  stdout-asserting behavioral goldens to the corpus per the golden-corpus-and-compiler-reality plan.*

### Phase 2 — Rc-shared scope frames + Rc closure body — ✅ DONE 2026-06-05

- [x] **`Scope` frames are `Vec<Rc<HashMap<…>>>`** (`env.rs`); cloning a `Scope` is O(frames) refcount bumps.
  (Chose per-frame `Rc`+CoW over a full linked cactus stack — same win, far lower risk, fully contained to
  `env.rs` since `frames` is private.)
- [x] **Closure creation & application** capture is now O(1) automatically — `scope.clone()` call sites
  unchanged; the representation does the work. Writes CoW via `Rc::make_mut` (`get_mut`/`set`/`set_mut`).
- [x] **Body sharing:** `Fn.body` is `Rc<Vec<HirStmt>>` (`value.rs`) — per-call clone is O(1).
- [x] **Scoping semantics preserved** — full suite + 8/8 safety net (incl. closure-capture-by-value &
  recursion) green. Verified: `deep_recursion` −80% incremental / −94% total.

### Phase 4 — Persistent structures (benchmark-gated; optional)

- [ ] **Only if Phase-0/1 benchmarks show CoW thrashing** (two aliased bindings mutated alternately clone each
  time), introduce a persistent RRB vector / HAMT map (`im`/`imbl` or `rpds`) for `List`/`Object` so even
  *aliased* mutation is O(log n) with structural sharing. This is a dependency + iteration-code change, hence
  gated. CoW (Phase 1) handles the overwhelmingly common unaliased case; this is the tail.

---

## Part 4 — Workstream B: finish zero-copy Rust emission

Extends [`zero-copy-rust-emission-plan-2026.md`](zero-copy-rust-emission-plan-2026.md) §2 (the two unchecked
items) — the interpreter work above does **not** touch emitted Rust, so both workstreams are needed for
end-to-end coverage.

- [x] **Last-use / liveness move analysis — ALREADY IMPLEMENTED** (`UsageTracker` in
  `codegen_rust/emit/usage.rs`, applied at `stmt_expr.rs:406`; built per-function in `workflow.rs:201`).
  No further work; this item was stale in the prior draft.
- [ ] **Escape analysis (the remaining gap):** when an identifier is passed to a callee that borrows
  (`&str`, `&[T]`), emit `&n` / `n.as_str()` instead of `n.clone()`. Partially present via
  `OwnershipMode::Borrowed` → `.as_str()`; needs interprocedural promotion to cover the general case.
- [ ] **Collections:** prefer move/borrow; consider `Cow<>` at API boundaries before reaching for `Arc`.
- [ ] **Parity check:** emitted-Rust value semantics must match the interpreter's CoW semantics exactly
  (assignment independence). Add a cross-mode golden asserting identical stdout for interp vs script.

---

## Part 5 — Workstream C: guardrails, observability & ratifying the model

- [ ] **Optional allocation/heap ceiling** alongside the existing `step_limit`
  ([`Interpreter`](../../../crates/vox-compiler/src/eval/mod.rs)): a runaway Vox program should fail *loudly*
  with a diagnostic rather than OOM the host — this keeps "users never manage memory" honest by making the
  runtime self-limiting.
- [x] **Ratified the official memory model — DONE 2026-06-05.** Authored
  [`docs/src/explanation/expl-memory-model.md`](../explanation/expl-memory-model.md) (`status: current`,
  `training_eligible: true`): states plainly that Vox programs never manage memory, the model is value
  semantics implemented by copy-on-write, there is no GC, and memory management is inherited per target
  (JS GC for web/RN; Rust ownership for compiled/interp; wasm linear memory for WASI).
- [x] **`Rc`-soundness guard (the headline Phase 5 item) — DONE 2026-06-05.** Realization: the `Rc` change
  makes the "interpreter values never cross threads" invariant **self-enforcing** — `Rc: !Send`, so sending a
  `VoxValue` across threads now fails to compile automatically. Pinned with a compile-time tripwire
  `static_assertions::assert_not_impl_any!(VoxValue: Send, Sync)` (in `tests/eval_cow_semantics_test.rs`, +
  `static_assertions` dev-dep): an `Rc`→`Arc` regression now STOPS COMPILING, forcing conscious review.
  Verified passing.
- [x] **Added the `where-things-live.md` row** for the runtime value representation / interpreter memory model
  → `crates/vox-compiler/src/eval/{value.rs,env.rs}` (per CLAUDE.md same-PR rule).
- [ ] **Consider a v1 criterion** (advisory to the v1-foundation work): a CR gate asserting an
  append-in-loop / big-list-pass micro-benchmark stays within a linear-memory bound, so the cliff cannot
  silently regress. Coordinate with the v1 foundation-criteria advisory owner.

---

## Part 6 — Sequencing, risks, verification

### Sequencing

```
Phase 0  Measurement + Rc/Arc decision        ── gate ──►
Phase 1  CoW collection payloads (no new deps)            ── highest leverage
Phase 2  Cactus-stack scopes for closures
Phase 3  Codegen escape + last-use (Workstream B)         ── parallelizable with 1–2
Phase 4  Persistent structures   (only if benchmarks demand)
Phase 5  Guardrails + ratify model + where-things-live row
```

### Risk register

| Risk | Likelihood | Mitigation |
|---|---|---|
| ~~`VoxValue` must be `Send` → `Rc` unsound~~ **RESOLVED**: `Rc` is sound (Phase-0 audit). Residual: a *future* `Send` requirement (e.g. spawning eval onto a worker thread) would break it | Low (now) | Phase-5 guard rejects adding a `Send` bound / spawning a `VoxValue`-holding closure; keep interp synchronous on the main task |
| CoW thrashing on alternately-mutated aliases | Low–Med | Benchmark in Phase 0/1; Phase 4 persistent structures eliminate it |
| Behavioral regression (value semantics must be bit-identical) | Med | Aliasing-independence goldens as **behavioral** tests; cross-mode interp-vs-script parity golden |
| `PartialEq`/`Debug` churn from `Rc`-wrapping | Low | `Rc<T: Eq>` is `Eq`; derives mostly transparent; targeted manual impls if needed |
| Scope refactor (cactus stack) breaks edge-case scoping | Med | Land Phase 2 behind closure-capture goldens; keep `Vec<HashMap>` fallback until green |
| Scope creep into per-actor GC | Low | Non-goal stated; this plan deliberately stops at sharing |

### Verification (evidence before "done")

- [ ] Phase-0 baseline vs post-Phase-1/2 benchmark deltas recorded in this doc (peak RSS + wall-time per case).
      Target: pass-a-big-list and closure-heavy cases drop from O(n) copy to O(1); append-in-loop from O(n²) to
      ~O(n).
- [ ] `cargo test -p vox-compiler` green, including new aliasing/closure goldens.
- [ ] Full golden corpus green, with the new behavioral (stdout-asserting) cases.
- [ ] `vox run --mode interp` Hello-World cold start still **< 50 ms** (CR-E1 not regressed).
- [ ] `cargo run -p vox-arch-check` clean (no new layer/fan-in violations from the refactor).
- [ ] Codegen (Workstream B): `vox-codegen` tests pass; an interp-vs-script parity golden produces identical
      stdout.

---

## Appendix — answers to the originating questions

1. **Does Vox have a GC, and does it only work on our Rust code?** No GC; it isn't needed because of pure value
   semantics. Memory management is inherited per-target (Rust ownership for interpreter/script/Axum/Tauri; the
   JS engine's GC for emitted web/RN; wasm linear memory for WASI) — not "only Rust".
2. **Do we need to add a GC to the language?** No global/tracing GC (harmful: STW pauses, tech-debt, breaks the
   actor invariant). A per-actor GC is a *separate, deferred* option that only buys cyclic/shared-mutable data
   within an actor — not pursued here.
3. **How do we avoid users managing memory themselves?** They already never do; Vox exposes no
   pointers/`free`. This plan preserves that and adds a self-limiting heap ceiling so the runtime fails loudly
   instead of leaking the guarantee.
4. **Improvements?** Yes — the cost cliffs: copy-on-write payloads, cactus-stack closure scopes, and finishing
   zero-copy Rust emission. Sharing, not collection.
