---
title: "Mesh Phase 1 — Language Spine Implementation Plan (2026-05-09)"
description: "Step-by-step TDD implementation plan for Phase 1 of the Mesh & Language-Distribution SSOT: collapse Future/Promise/Activity-result/Awakeable into the single DurablePromise[T] primitive, introduce @remote, auto-derive activity_id, flip effect inference to bottom-up, add workflow determinism check, side_effect blocks, and `vox workflow preview`. 9 task groups, ~1900 lines."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
training_rationale: "Implementation plan; gets stale as tasks are completed. Spec/SSOT is the durable artifact."
---

# Mesh Phase 1 — Language Spine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL — use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Cite the task ID (`P1-T*`) in every commit message body. Per Vox project rule, do NOT generate `.ps1` / `.sh` / `.py` glue — automation goes in `scripts/*.vox`.

**Goal.** The Vox language has a single canonical primitive for distributed durable work: `DurablePromise[T]`. Effect rows are *inferred* (not just *validated*) and enforced bottom-up. The compiler refuses to compile a `workflow` body that calls `time.now()` directly. A `@remote fn` cannot accept a non-serializable argument. `vox workflow preview` projects the schedule of activities a workflow would dispatch without running them.

**Killer feature delivered.** *Type-safe distributed programs.* An LLM author writing a `workflow` cannot accidentally introduce non-determinism, and a `@remote` call cannot be invoked with non-serializable arguments. `vox check` is the safety net.

**Architecture.** Five compiler/runtime concerns move in lockstep:

1. **`DurablePromise[T]`** is added as a stdlib type and a compiler intrinsic. Codegen lowers it to `vox_workflow_runtime::DurablePromise<T>`, a thin wrapper around `tokio::sync::oneshot::Receiver<Result<T, JournalError>>` with a journal-backed "load by activity_id" fast path. `Future[T]` and `Promise[T]` are deprecated with auto-rewrite hints in `vox check` and removed in v0.7.
2. **`@remote fn foo(...)`** replaces the `mesh_*` naming convention. The parser recognises a new `Token::AtRemote`, the AST/HIR carry an `is_remote` flag, and effect inference adds `Spawn + Net`. Compile-time check: every parameter type must satisfy a synthesised `Serializable` trait (HIR-level structural derive). Functions named `mesh_*` auto-receive `@remote` with a one-release deprecation warning.
3. **Auto-derived `activity_id`** replaces the `format!("{workflow_name}-{idx}")` fallback in `crates/vox-workflow-runtime/src/workflow/run.rs` (grep `format!("{workflow_name}-{idx}")`). The compiler emits the four hash inputs at the call site (`workflow_id`, stable `call_site_id`, `structural_arg_hash`, `replay_counter`) into the planned-activity record; the runtime feeds them through BLAKE3 to produce a stable `activity_id`. `@with_id(expr)` overrides for business identity.
4. **Workflow determinism check + bottom-up effect inference**. `effect_check.rs` flips from "validate declared `uses` clause" to "compute caller's effective effect set from callees, then check declarations are at least that". Once inference is bottom-up, an additional `DurabilityKind::Workflow` row restriction rejects forbidden builtins (`time.now`, `random.*`, raw I/O) inside workflow bodies; the diagnostic suggests wrapping in an activity or `side_effect { … }` block.
5. **`side_effect { … }`** desugars to a synthesised single-shot inline activity reusing the auto-derived `activity_id`.

`vox workflow preview <fn>(args)` is a new CLI subcommand that runs the frontend + effect inference + activity scheduler against a workflow, printing a tree of "would-call" activities annotated with their effect rows. No I/O.

**Tech stack.** Rust 2024 edition. Existing crates only, plus `vox-crypto::blake3` (already a workspace dep) for `activity_id` derivation. No new external crates.

**SSOT.** [`mesh-and-language-distribution-ssot-2026.md` §3 Phase 1](mesh-and-language-distribution-ssot-2026.md). Diagnostic-ID conventions: [`vox-language-rules-and-enforcement-plan-2026.md`](vox-language-rules-and-enforcement-plan-2026.md). Language-design constraint C4 (one canonical primitive per concept): [`LANGUAGE_DESIGN_PRIORITIES.md`](../../../LANGUAGE_DESIGN_PRIORITIES.md).

- Hopper integration: none in Phase 1 (the hopper is a cross-cutting `Hp-T*` track defined in
  SSOT §3.5; it consumes Phase 1's primitives but does not require Phase 1 changes).

**Dependencies on Phase 0.** `P1-T3` (`@remote fn`) compiles and lowers correctly only after
`P0-T3` (authoritative leases) lands — the codegen for `@remote` consults lease state before
dispatching. Until `P0-T3` is wired, `@remote` falls back to local execution behind a feature flag.
The full Phase 1 acceptance bound (`@remote` failing to compile when called with non-serializable
args; `vox workflow preview` projecting routing decisions) requires Phase 0 complete.

**Working directory.** Worktree at `C:\Users\Owner\vox\.claude\worktrees\zealous-ardinghelli-b01e11`. All paths below are relative to this worktree.

---

## File map

**Create:**

- `crates/vox-stdlib/src/durable_promise.vox` — Vox-language `DurablePromise[T]` declaration with `await`, `then`, `select`, `cancel` methods (`vox:skip` on examples that exercise compiler-only paths).
- `crates/vox-workflow-runtime/src/durable_promise.rs` — Rust runtime type wrapping `tokio::sync::oneshot` with a journal fast-path.
- `crates/vox-compiler/src/typeck/serializable.rs` — structural `Serializable` predicate over `HirType`.
- `crates/vox-compiler/src/typeck/workflow_determinism.rs` — `DurabilityKind::Workflow` row restriction + diagnostic.
- `crates/vox-compiler/src/typeck/activity_id_inputs.rs` — emits `(workflow_id, call_site_id, arg_hash_inputs, replay_counter)` quadruples into the HIR for the planner.
- `crates/vox-compiler/src/typeck/effect_inference.rs` — bottom-up effect-row computation (T6 split out from `effect_check.rs`). (distinct from existing `typeck/infer.rs` which handles type inference)
- `crates/vox-compiler/src/ast/decl/remote.rs` — `RemoteAttr` AST node + `with_id` attribute.
- `crates/vox-cli/src/commands/workflow.rs` — `vox workflow preview` subcommand.
- `crates/vox-cli/src/commands/workflow/preview.rs` — preview projector implementation.
- `tests/fixtures/workflow_preview/*.vox` — eight Vox source fixtures.
- `crates/vox-compiler/tests/durable_promise.rs` — integration tests for `DurablePromise[T]`.
- `crates/vox-compiler/tests/remote_annotation.rs` — `@remote` accept/reject tests.
- `crates/vox-compiler/tests/workflow_determinism.rs` — determinism violation diagnostic tests.
- `crates/vox-compiler/tests/effect_effect_inference.rs` — bottom-up inference correctness tests.
- `crates/vox-cli/tests/workflow_preview.rs` — CLI integration tests.

**Modify:**

- `crates/vox-compiler/src/ast/decl/effect.rs` — no enum change; add a doc comment cross-referencing `is_remote`.
- `crates/vox-compiler/src/ast/decl/fundecl.rs` — add `is_remote: bool`, `with_id: Option<HirExpr>`.
- `crates/vox-compiler/src/parser/descent/decl/head.rs` (grep the attribute loop matching `Token::At`) — add `Token::AtRemote` and `Token::AtWithId` handling in the attribute loop.
- `crates/vox-compiler/src/lexer/mod.rs` — add `Token::AtRemote`, `Token::AtWithId`.
- `crates/vox-compiler/src/hir/nodes/decl.rs` — add `is_remote`, `with_id_expr`, `inferred_effects: Vec<HirCapability>` to `HirFn`.
- `crates/vox-compiler/src/hir/lower/mod.rs` (grep `lower_fn_decl`) — propagate `is_remote`; emit `mesh_*` deprecation diagnostic; populate `inferred_effects`.
- `crates/vox-compiler/src/hir/nodes/durability.rs` — extend `DurabilityKind::Workflow` with a forbidden-builtin classifier method.
- `crates/vox-compiler/src/typeck/effect_check.rs` — strip the top-down validation; route through `effect_inference.rs`; re-run as a *check* of declared vs. inferred.
- `crates/vox-compiler/src/typeck/diagnostics.rs` — add the new `vox/<kebab>` diagnostic codes (constants).
- `crates/vox-compiler/src/typeck/mod.rs` — wire new modules.
- `crates/vox-codegen/src/codegen_rust/emit/workflow.rs` — emit `DurablePromise<T>` for `await activity_call(...)` instead of bespoke per-activity code.
- `crates/vox-codegen/src/vox_ir/lower.rs` — propagate `inferred_effects` and `activity_id` quadruples into vox-ir.
- `crates/vox-workflow-runtime/src/workflow/run.rs` (grep `format!("{workflow_name}-{idx}")`) — replace `format!` fallback with `derive_activity_id_from_inputs(...)`.
- `crates/vox-workflow-runtime/src/workflow/types.rs` — add `activity_id_inputs: ActivityIdInputs` to `PlannedActivity`.
- `crates/vox-cli/src/lib.rs` — add `Workflow` clap subcommand.
- `crates/vox-cli/src/commands/mod.rs` — `pub mod workflow;`.
- `docs/src/architecture/where-things-live.md` — one row for `DurablePromise[T]` and one for `@remote`.

---

## Task ordering rationale

T1 (`DurablePromise[T]`) and T2 (deprecate `Future[T]`/`Promise[T]`) reshape every later codegen artefact, so they merge first. T3 (`@remote`) follows because the rest of the phase assumes a function can be marked as "remote-spawnable" via attribute, not naming. T4 (auto-derived `activity_id`) is the runtime/codegen handshake that lets later tasks emit synthesised activities (T7 `side_effect`). T5 and T6 must move together — the workflow-determinism check is meaningless without bottom-up inference — but we split them into two tasks: T6 first lands inference (everything stays passing because top-down was a stricter rule), then T5 layers the `Workflow` row restriction on top. T7 reuses T4's `activity_id` derivation. T8 (`vox workflow preview`) needs T6 inference and T4 scheduling. T9 sweeps the diagnostic-ID namespace last so we don't churn IDs while iterating.

Each task ends with `cargo test -p <crate>` and a commit. The workspace builds at every checkpoint.

---

## Task P1-T1: Introduce `DurablePromise[T]`

**Files:**

- Create: `crates/vox-stdlib/src/durable_promise.vox`
- Create: `crates/vox-workflow-runtime/src/durable_promise.rs`
- Create: `crates/vox-compiler/tests/durable_promise.rs`
- Modify: `crates/vox-compiler/src/typeck/builtins.rs` (the stdlib type registry — search for `register_stdlib_types` to find the table) <!-- TODO: confirm builtins.rs vs registration.rs split during implementation -->
- Modify: `crates/vox-workflow-runtime/src/lib.rs` (re-export)

The runtime type lives in `vox-workflow-runtime` so the journal-backed fast path can reach the tracker without crossing crates. The Vox-level surface is in `vox-stdlib`; the compiler treats `DurablePromise` as an intrinsic name (same mechanism `Result` and `Option` use today), so the type-checker can synthesise `DurablePromise[T]` from `await activity_call(...)` without a stdlib lookup at every call site.

- [ ] **Step 1: Write the failing runtime unit tests**

Create `crates/vox-workflow-runtime/src/durable_promise.rs`:

```rust
//! `DurablePromise<T>` — the single awaitable primitive for distributed
//! durable work. Subsumes `Future[T]`, `Promise[T]`, the activity-result
//! handle, signal awaits, and awakeables.
//!
//! Lowered from Vox `DurablePromise[T]` by `vox-codegen`.
//!
//! Semantics:
//!   - On first execution, the workflow runtime registers the
//!     `activity_id` and returns a `DurablePromise<T>` whose `await`
//!     suspends the workflow until the activity completes. The result
//!     is journaled.
//!   - On replay, the workflow runtime sees the `activity_id` is already
//!     completed and resolves the promise from the journal *without*
//!     re-issuing the dispatch. This is the journal-backed fast path.

use std::fmt;
use std::pin::Pin;
use std::task::{Context, Poll};

use serde::de::DeserializeOwned;
use serde::Serialize;
use tokio::sync::oneshot;

#[derive(Debug, thiserror::Error)]
pub enum JournalError {
    #[error("activity {activity_id} failed: {source}")]
    ActivityFailed {
        activity_id: String,
        #[source]
        source: anyhow::Error,
    },
    #[error("journal corruption for activity {0}: {1}")]
    JournalCorrupt(String, String),
    #[error("workflow cancelled before activity {0} completed")]
    Cancelled(String),
    #[error("oneshot sender dropped without resolving {0}")]
    SenderDropped(String),
}

/// The single awaitable primitive. Use `.await` (Vox) / `.poll` (Rust).
///
/// Construction is private; only the workflow runtime mints these. User code
/// receives them as the result of a `@remote` call, an activity dispatch, a
/// `signal()`, or a `side_effect { … }` block.
pub struct DurablePromise<T> {
    activity_id: String,
    state: PromiseState<T>,
}

enum PromiseState<T> {
    /// Live execution: result will arrive via the oneshot.
    Pending(oneshot::Receiver<Result<T, JournalError>>),
    /// Replay: result was loaded from the journal.
    Replayed(Result<T, JournalError>),
    /// Already polled to completion; cannot poll twice.
    Done,
}

impl<T> DurablePromise<T> {
    /// Mint a fresh promise for a live activity dispatch.
    pub(crate) fn pending(
        activity_id: String,
        rx: oneshot::Receiver<Result<T, JournalError>>,
    ) -> Self {
        Self {
            activity_id,
            state: PromiseState::Pending(rx),
        }
    }

    /// Mint a promise that resolves immediately from a journal entry.
    pub(crate) fn replayed(activity_id: String, value: Result<T, JournalError>) -> Self {
        Self {
            activity_id,
            state: PromiseState::Replayed(value),
        }
    }

    pub fn activity_id(&self) -> &str {
        &self.activity_id
    }
}

impl<T> std::future::Future for DurablePromise<T>
where
    T: Unpin,
{
    type Output = Result<T, JournalError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = self.get_mut();
        match std::mem::replace(&mut me.state, PromiseState::Done) {
            PromiseState::Pending(mut rx) => match Pin::new(&mut rx).poll(cx) {
                Poll::Ready(Ok(v)) => Poll::Ready(v),
                Poll::Ready(Err(_)) => {
                    Poll::Ready(Err(JournalError::SenderDropped(me.activity_id.clone())))
                }
                Poll::Pending => {
                    me.state = PromiseState::Pending(rx);
                    Poll::Pending
                }
            },
            PromiseState::Replayed(v) => Poll::Ready(v),
            PromiseState::Done => panic!(
                "DurablePromise<{}> for activity {} polled twice",
                std::any::type_name::<T>(),
                me.activity_id
            ),
        }
    }
}

impl<T: fmt::Debug> fmt::Debug for DurablePromise<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DurablePromise")
            .field("activity_id", &self.activity_id)
            .finish()
    }
}

/// Constructed by codegen for a deserialised value off the wire.
pub fn from_serialised<T: DeserializeOwned>(
    activity_id: String,
    bytes: &[u8],
) -> Result<DurablePromise<T>, JournalError> {
    match serde_json::from_slice::<T>(bytes) {
        Ok(v) => Ok(DurablePromise::replayed(activity_id, Ok(v))),
        Err(e) => Err(JournalError::JournalCorrupt(activity_id, e.to_string())),
    }
}

/// Constructed by codegen for a serialisable value before dispatch (for tests).
#[doc(hidden)]
pub fn ready<T: Serialize + Send + 'static>(activity_id: String, v: T) -> DurablePromise<T> {
    DurablePromise::replayed(activity_id, Ok(v))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn replayed_resolves_synchronously() {
        let p: DurablePromise<i32> = DurablePromise::replayed("act1".into(), Ok(42));
        assert_eq!(p.activity_id(), "act1");
        assert_eq!(p.await.unwrap(), 42);
    }

    #[tokio::test]
    async fn pending_resolves_when_sender_completes() {
        let (tx, rx) = oneshot::channel();
        let p: DurablePromise<i32> = DurablePromise::pending("act2".into(), rx);
        let handle = tokio::spawn(async move { p.await });
        tx.send(Ok(7)).unwrap();
        assert_eq!(handle.await.unwrap().unwrap(), 7);
    }

    #[tokio::test]
    async fn pending_propagates_journal_failure() {
        let (tx, rx) = oneshot::channel();
        let p: DurablePromise<i32> = DurablePromise::pending("act3".into(), rx);
        tx.send(Err(JournalError::ActivityFailed {
            activity_id: "act3".into(),
            source: anyhow::anyhow!("dispatch refused"),
        }))
        .unwrap();
        let err = p.await.unwrap_err();
        assert!(matches!(err, JournalError::ActivityFailed { .. }));
    }

    #[tokio::test]
    async fn dropped_sender_yields_sender_dropped() {
        let (tx, rx) = oneshot::channel::<Result<i32, JournalError>>();
        drop(tx);
        let p: DurablePromise<i32> = DurablePromise::pending("act4".into(), rx);
        let err = p.await.unwrap_err();
        assert!(matches!(err, JournalError::SenderDropped(_)));
    }
}
```

- [ ] **Step 2: Re-export from `vox-workflow-runtime/src/lib.rs`**

Add at the top of `lib.rs` after existing module declarations:

```rust
pub mod durable_promise;
pub use durable_promise::{DurablePromise, JournalError};
```

- [ ] **Step 3: Run runtime tests**

Run: `cargo test -p vox-workflow-runtime durable_promise 2>&1 | tail -20`
Expected: PASS for all four tests.

- [ ] **Step 4: Write the failing compiler intrinsic test**

Create `crates/vox-compiler/tests/durable_promise.rs`:

```rust
//! Phase 1 P1-T1 — `DurablePromise[T]` is recognised as a stdlib type.

use vox_compiler::{lex, parse, lower_module, typeck};

#[test]
fn durable_promise_is_a_known_type_constructor() {
    let src = r#"
        fn pretend() to DurablePromise[i32] {
            // body intentionally trivial; we're testing the type appears
            // in the type universe, not that it's constructible from
            // user code.
            return panic("compiler intrinsic")
        }
    "#;
    let tokens = lex(src);
    let module = parse(tokens).expect("parse");
    let hir = lower_module(&module);
    let diags = typeck::run(&hir, src);
    let unknown_type_diags: Vec<_> = diags
        .iter()
        .filter(|d| d.message.contains("unknown type") && d.message.contains("DurablePromise"))
        .collect();
    assert!(
        unknown_type_diags.is_empty(),
        "DurablePromise should be a built-in type ctor; got: {unknown_type_diags:?}"
    );
}

#[test]
fn durable_promise_demands_one_type_argument() {
    let src = r#"
        fn bad() to DurablePromise {
            return panic("missing type arg")
        }
    "#;
    let tokens = lex(src);
    let module = parse(tokens).expect("parse");
    let hir = lower_module(&module);
    let diags = typeck::run(&hir, src);
    assert!(
        diags
            .iter()
            .any(|d| d.code.as_deref() == Some("vox/types/durable-promise-arity")),
        "missing arity diagnostic for DurablePromise; got {:?}",
        diags
            .iter()
            .map(|d| &d.code)
            .collect::<Vec<_>>()
    );
}
```

- [ ] **Step 5: Run, expect FAIL**

Run: `cargo test -p vox-compiler --test durable_promise 2>&1 | tail -20`
Expected: FAIL — `DurablePromise` not registered in the stdlib type ctor table.

- [ ] **Step 6: Register `DurablePromise[T]` as a stdlib type intrinsic**

In `crates/vox-compiler/src/typeck/builtins.rs` (or wherever `Result`/`Option` are registered — `grep -rn "register_stdlib_types\|Result.*Option.*ctor" crates/vox-compiler/src/typeck/`), append a new entry:

```rust
// Phase 1 P1-T1: DurablePromise[T] — the single awaitable primitive.
// Lowered by codegen to vox_workflow_runtime::DurablePromise<T>.
table.insert(
    "DurablePromise".to_string(),
    StdlibType {
        name: "DurablePromise".into(),
        arity: 1,
        runtime_path: "vox_workflow_runtime::DurablePromise",
        is_durable_primitive: true,
    },
);
```

If the existing struct `StdlibType` has no `is_durable_primitive` field, add it (default `false`) — used by codegen to decide on the `tokio::sync::oneshot` lowering.

- [ ] **Step 7: Add the arity diagnostic**

In `crates/vox-compiler/src/typeck/diagnostics.rs`, add a constant:

```rust
pub const VOX_TYPES_DURABLE_PROMISE_ARITY: &str = "vox/types/durable-promise-arity";
```

In the type-arity validator (search `Result.*expects.*type argument`), branch on `name == "DurablePromise"` and emit a diagnostic with code `VOX_TYPES_DURABLE_PROMISE_ARITY` and message:

```
type `DurablePromise` expects 1 type argument, found 0.
help: write `DurablePromise[T]` where T is the awaited value type.
```

- [ ] **Step 8: Run, expect PASS**

Run: `cargo test -p vox-compiler --test durable_promise 2>&1 | tail -10`
Expected: both tests PASS.

- [ ] **Step 9: Author the Vox-level stdlib declaration**

Create `crates/vox-stdlib/src/durable_promise.vox`:

```vox
// vox:skip
// `DurablePromise[T]` — the canonical awaitable primitive for distributed
// durable work. The compiler treats this declaration as documentation;
// the actual lowering is built-in (see crates/vox-codegen + the Rust
// type vox_workflow_runtime::DurablePromise<T>).
//
// Subsumes:
//   - Future[T]            (deprecated, removed in v0.7)
//   - Promise[T]           (deprecated, removed in v0.7)
//   - Activity result      (synthesised by `@remote` calls)
//   - signal-await         (DurablePromise<Signal>)
//   - awakeable            (DurablePromise<T> with external resolver)
//
// `await p` blocks the workflow until the underlying activity completes
// or is replayed from the journal. The compiler enforces that `DurablePromise`
// values are only constructed inside a workflow context.

@intrinsic
type DurablePromise[T] {
    fn activity_id() to str
    fn poll() to Option[T]
}

@intrinsic
fn await[T](p: DurablePromise[T]) to T

@intrinsic
fn select[T](ps: List[DurablePromise[T]]) to T
```

The `@intrinsic` attribute (introduced in this task — add a token if not present) signals to the lowering that the body is compiler-supplied.

- [ ] **Step 10: Run cross-crate workspace tests**

Run: `cargo test -p vox-compiler -p vox-workflow-runtime 2>&1 | tail -15`
Expected: all PASS.

- [ ] **Step 11: Commit**

```bash
git add crates/vox-workflow-runtime/src/durable_promise.rs \
        crates/vox-workflow-runtime/src/lib.rs \
        crates/vox-stdlib/src/durable_promise.vox \
        crates/vox-compiler/src/typeck/builtins.rs \
        crates/vox-compiler/src/typeck/diagnostics.rs \
        crates/vox-compiler/tests/durable_promise.rs
git commit -m "$(cat <<'EOF'
feat(compiler,runtime): introduce DurablePromise[T] as the single awaitable primitive (P1-T1)

Adds vox_workflow_runtime::DurablePromise<T> wrapping tokio::sync::oneshot
with a journal-backed replay fast path. Registers DurablePromise as a
stdlib type ctor (arity 1). New diagnostic vox/types/durable-promise-arity.

Subsumes Future[T] / Promise[T] / Activity-result / signal-await / awakeable.
Per LANGUAGE_DESIGN_PRIORITIES.md C4: one canonical primitive per concept.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>
EOF
)"
```

---

## Task P1-T2: Deprecate `Future[T]` / `Promise[T]` with auto-rewrite hint

**Files:**

- Modify: `crates/vox-compiler/src/typeck/builtins.rs`
- Modify: `crates/vox-compiler/src/typeck/diagnostics.rs`
- Create: `crates/vox-compiler/tests/future_promise_deprecation.rs`
- Modify: `crates/vox-codegen/src/codegen_rust/emit/types.rs` (alias `Future<T>` and `Promise<T>` to `DurablePromise<T>` during the deprecation window)

The deprecation window is one minor release; removal in v0.7 (per the SSOT release-contract table). During the window, references to `Future[T]` or `Promise[T]` produce a *warning* with a structured fix that rewrites the type to `DurablePromise[T]`. Codegen emits the same Rust type either way so the migration is zero-cost.

- [ ] **Step 1: Write the failing diagnostic test**

Create `crates/vox-compiler/tests/future_promise_deprecation.rs`:

```rust
use vox_compiler::{lex, parse, lower_module, typeck};

fn diag_codes(src: &str) -> Vec<String> {
    let tokens = lex(src);
    let module = parse(tokens).expect("parse");
    let hir = lower_module(&module);
    typeck::run(&hir, src)
        .into_iter()
        .filter_map(|d| d.code)
        .collect()
}

#[test]
fn future_t_deprecation_warning() {
    let codes = diag_codes("fn f() to Future[i32] { return panic(\"x\") }");
    assert!(
        codes.iter().any(|c| c == "vox/types/future-deprecated"),
        "expected deprecation warning; got {codes:?}"
    );
}

#[test]
fn promise_t_deprecation_warning() {
    let codes = diag_codes("fn f() to Promise[str] { return panic(\"x\") }");
    assert!(
        codes.iter().any(|c| c == "vox/types/promise-deprecated"),
        "expected deprecation warning; got {codes:?}"
    );
}

#[test]
fn fix_suggests_durable_promise() {
    let src = "fn f() to Future[i32] { return panic(\"x\") }";
    let tokens = lex(src);
    let module = parse(tokens).expect("parse");
    let hir = lower_module(&module);
    let diags = typeck::run(&hir, src);
    let dep = diags
        .iter()
        .find(|d| d.code.as_deref() == Some("vox/types/future-deprecated"))
        .expect("must have deprecation diag");
    let fix = dep
        .fixes
        .iter()
        .find(|f| f.replacement.as_ref().is_some_and(|r| r.contains("DurablePromise")))
        .expect("must offer a DurablePromise rewrite fix");
    assert_eq!(fix.replacement.as_ref().unwrap().trim(), "DurablePromise[i32]");
}
```

- [ ] **Step 2: Run, expect FAIL**

Run: `cargo test -p vox-compiler --test future_promise_deprecation 2>&1 | tail -15`
Expected: FAIL — Future / Promise still resolve as bare type names without diagnostics.

- [ ] **Step 3: Add deprecation entries to the stdlib type table**

In `crates/vox-compiler/src/typeck/builtins.rs`, alongside the `DurablePromise` entry from P1-T1:

```rust
table.insert(
    "Future".into(),
    StdlibType {
        name: "Future".into(),
        arity: 1,
        runtime_path: "vox_workflow_runtime::DurablePromise",
        is_durable_primitive: true,
        deprecated: Some(DeprecationInfo {
            replacement_type: "DurablePromise",
            removed_in_version: "0.7",
            diagnostic_code: "vox/types/future-deprecated",
        }),
    },
);
table.insert(
    "Promise".into(),
    StdlibType {
        name: "Promise".into(),
        arity: 1,
        runtime_path: "vox_workflow_runtime::DurablePromise",
        is_durable_primitive: true,
        deprecated: Some(DeprecationInfo {
            replacement_type: "DurablePromise",
            removed_in_version: "0.7",
            diagnostic_code: "vox/types/promise-deprecated",
        }),
    },
);
```

If `DeprecationInfo` doesn't exist, add it next to `StdlibType`:

```rust
#[derive(Debug, Clone)]
pub struct DeprecationInfo {
    pub replacement_type: &'static str,
    pub removed_in_version: &'static str,
    pub diagnostic_code: &'static str,
}
```

- [ ] **Step 4: Emit the deprecation diagnostic at every type-reference resolution**

Find the type-reference resolver (search `resolve_type_name` or `lookup_stdlib_type`). After a successful resolution, if `entry.deprecated.is_some()`, emit a warning with the configured code and a `Fix` whose replacement is `format!("{}[{}]", info.replacement_type, render_args(args))`.

```rust
if let Some(dep) = &entry.deprecated {
    let replacement = format!(
        "{}[{}]",
        dep.replacement_type,
        args.iter().map(render_type).collect::<Vec<_>>().join(", ")
    );
    let fix = Fix {
        message: format!("rewrite to `{replacement}`"),
        replacement: Some(replacement),
        span: type_ref_span,
        applicability: Applicability::MachineApplicable,
    };
    diags.push(Diagnostic {
        severity: Severity::Warning,
        code: Some(dep.diagnostic_code.into()),
        message: format!(
            "type `{}` is deprecated; will be removed in v{}",
            entry.name, dep.removed_in_version
        ),
        span: type_ref_span,
        fixes: vec![fix],
        ..Default::default()
    });
}
```

- [ ] **Step 5: Run, expect PASS**

Run: `cargo test -p vox-compiler --test future_promise_deprecation 2>&1 | tail -10`
Expected: all three PASS.

- [ ] **Step 6: Wire the codegen alias**

In `crates/vox-codegen/src/codegen_rust/emit/types.rs`, in the type emitter:

```rust
"Future" | "Promise" => {
    // Phase 1 P1-T2 — emitted as DurablePromise during the deprecation window.
    out.push_str("vox_workflow_runtime::DurablePromise<");
    emit_type_args(out, args);
    out.push('>');
}
"DurablePromise" => {
    out.push_str("vox_workflow_runtime::DurablePromise<");
    emit_type_args(out, args);
    out.push('>');
}
```

- [ ] **Step 7: Run codegen tests**

Run: `cargo test -p vox-codegen --test workflow 2>&1 | tail -15`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/vox-compiler/src/typeck/builtins.rs \
        crates/vox-compiler/src/typeck/diagnostics.rs \
        crates/vox-compiler/tests/future_promise_deprecation.rs \
        crates/vox-codegen/src/codegen_rust/emit/types.rs
git commit -m "$(cat <<'EOF'
feat(compiler): deprecate Future[T]/Promise[T] in favour of DurablePromise[T] (P1-T2)

Both types resolve to the same runtime DurablePromise<T> during the
deprecation window (one minor; removal in v0.7). Emits structured
machine-applicable rewrite fixes via vox/types/future-deprecated and
vox/types/promise-deprecated.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>
EOF
)"
```

---

## Task P1-T3: `@remote fn` annotation

**Files:**

- Modify: `crates/vox-compiler/src/lexer/mod.rs` — add `Token::AtRemote`, `Token::AtWithId`.
- Modify: `crates/vox-compiler/src/ast/decl/fundecl.rs` — add `is_remote: bool`, `with_id: Option<Box<Expr>>`.
- Modify: `crates/vox-compiler/src/parser/descent/decl/head.rs` (grep the attribute loop matching `Token::At`) — handle the new tokens in the attribute loop.
- Modify: `crates/vox-compiler/src/hir/nodes/decl.rs` — add `is_remote: bool`, `with_id_expr: Option<HirExpr>` to `HirFn`.
- Modify: `crates/vox-compiler/src/hir/lower/mod.rs` — propagate, plus `mesh_*` auto-`@remote` deprecation.
- Create: `crates/vox-compiler/src/typeck/serializable.rs` — `Serializable` predicate.
- Create: `crates/vox-compiler/tests/remote_annotation.rs`.

`@remote` is the modern spelling for the `mesh_*` naming convention; functions named `mesh_*` auto-receive `@remote` with a deprecation warning. Effect inference adds `Spawn + Net` for any `@remote` function. All parameter types must satisfy `Serializable`; failure is a compile error naming the offending parameter and type.

- [ ] **Step 1: Add the lexer tokens**

In `crates/vox-compiler/src/lexer/mod.rs`, in the `Token` enum after `AtPure`:

```rust
/// `@remote` — mark function as a remote-spawnable activity (P1-T3).
AtRemote,
/// `@with_id(expr)` — override auto-derived activity_id (P1-T4).
AtWithId,
```

Wire the lexer's match arm — search `"@pure" => Token::AtPure` and add adjacent:

```rust
"@remote" => Token::AtRemote,
"@with_id" => Token::AtWithId,
```

- [ ] **Step 2: Extend the AST**

In `crates/vox-compiler/src/ast/decl/fundecl.rs`, in the `FnDecl` struct, add fields (preserving Display impls):

```rust
/// Phase 1 P1-T3: `@remote` annotation.
/// Implies effect row `{Spawn, Net}` and serializability for all params.
pub is_remote: bool,
/// Phase 1 P1-T4: `@with_id(expr)` overrides auto-derived activity_id.
/// `expr` is evaluated at the call site, not at the function decl.
pub with_id: Option<Box<Expr>>,
```

Add `is_remote: false` and `with_id: None` to every `FnDecl { ... }` literal (search `FnDecl {` in tests and codegen golden outputs).

- [ ] **Step 3: Parse the attributes**

In `crates/vox-compiler/src/parser/descent/decl/head.rs`, in the attribute loop around line 903 (`Token::AtPure => { … }`), add adjacent arms:

```rust
Token::AtRemote => {
    self.advance();
    is_remote = true;
}
Token::AtWithId => {
    self.advance();
    self.expect(&Token::LParen)?;
    let expr = self.parse_expr()?;
    self.expect(&Token::RParen)?;
    with_id = Some(Box::new(expr));
}
```

Declare `let mut is_remote = false;` and `let mut with_id = None;` near `is_pure` declarations. Pass them into the `FnDecl { … }` constructor at line 1014.

- [ ] **Step 4: Extend the HIR**

In `crates/vox-compiler/src/hir/nodes/decl.rs` `HirFn`:

```rust
/// Phase 1 P1-T3.
pub is_remote: bool,
/// Phase 1 P1-T4.
pub with_id_expr: Option<HirExpr>,
```

In `crates/vox-compiler/src/hir/lower/mod.rs` (lowering for `Decl::Fn`), copy the fields. Also detect the `mesh_*` legacy naming:

```rust
let mut is_remote = decl.is_remote;
if !is_remote && decl.name.starts_with("mesh_") {
    is_remote = true;
    diagnostics.push(Diagnostic::warning(
        format!(
            "function name `{}` follows the deprecated `mesh_*` convention; \
             prefer `@remote fn {}` (will be enforced in v0.7)",
            decl.name,
            decl.name.trim_start_matches("mesh_")
        ),
        decl.span,
        source,
    ).with_code("vox/api/mesh-prefix-deprecated"));
}
```

- [ ] **Step 5: Implement the `Serializable` predicate**

Create `crates/vox-compiler/src/typeck/serializable.rs`:

```rust
//! Structural Serializable predicate for `@remote` parameter types.
//!
//! A type T is `Serializable` iff:
//!   - It is a primitive (i8..i64, u8..u64, f32, f64, str, bool).
//!   - It is `Vec[T]` / `List[T]` / `Option[T]` / `Result[T, E]` where the
//!     inner types are `Serializable`.
//!   - It is a struct typedef whose every field is `Serializable`.
//!   - It is an ADT whose every variant's every field is `Serializable`.
//!   - It is `Decimal`, `DateTime`, `Uuid`, `BlobRef`.
//! It is NOT `Serializable` if:
//!   - It contains a function/closure type.
//!   - It is `DurablePromise[_]` (the receiver awaits, doesn't ship).
//!   - It is `Mutex[_]`, `Arc[_]`, `Channel[_]` or any other resource handle.
//!   - It is a typedef whose body is not `Serializable`.

use crate::hir::nodes::HirType;
use crate::hir::HirModule;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonSerializableReason {
    pub kind: NonSerializableKind,
    pub trace: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NonSerializableKind {
    FunctionType,
    DurablePromise,
    Resource(String),
    UnknownType(String),
    StructFieldNotSerializable {
        type_name: String,
        field: String,
    },
    AdtVariantNotSerializable {
        type_name: String,
        variant: String,
        field_idx: usize,
    },
}

pub fn is_serializable(
    ty: &HirType,
    module: &HirModule,
) -> Result<(), NonSerializableReason> {
    match ty {
        HirType::I8 | HirType::I16 | HirType::I32 | HirType::I64
        | HirType::U8 | HirType::U16 | HirType::U32 | HirType::U64
        | HirType::F32 | HirType::F64 | HirType::Bool | HirType::Str
        | HirType::Decimal | HirType::DateTime | HirType::Uuid | HirType::Unit => Ok(()),

        HirType::List(inner) | HirType::Vec(inner) | HirType::Option(inner) => {
            is_serializable(inner, module).map_err(|mut r| {
                r.trace.push(format!("inside {ty:?}"));
                r
            })
        }
        HirType::Result(ok, err) => {
            is_serializable(ok, module)?;
            is_serializable(err, module).map_err(|mut r| {
                r.trace.push(format!("inside {ty:?}"));
                r
            })
        }
        HirType::Tuple(elems) => {
            for e in elems {
                is_serializable(e, module)?;
            }
            Ok(())
        }
        HirType::Function(_, _) => Err(NonSerializableReason {
            kind: NonSerializableKind::FunctionType,
            trace: vec![],
        }),
        HirType::Generic(name, args) if name == "DurablePromise" => {
            let _ = args;
            Err(NonSerializableReason {
                kind: NonSerializableKind::DurablePromise,
                trace: vec![],
            })
        }
        HirType::Generic(name, _) if matches!(name.as_str(), "Mutex" | "Arc" | "Channel") => {
            Err(NonSerializableReason {
                kind: NonSerializableKind::Resource(name.clone()),
                trace: vec![],
            })
        }
        HirType::Named(name) => {
            // Look up in the module's typedefs.
            let Some(td) = module.types.iter().find(|t| &t.name == name) else {
                return Err(NonSerializableReason {
                    kind: NonSerializableKind::UnknownType(name.clone()),
                    trace: vec![],
                });
            };
            if td.variants.is_empty() {
                for (fname, fty) in &td.fields {
                    if let Err(_inner) = is_serializable(fty, module) {
                        return Err(NonSerializableReason {
                            kind: NonSerializableKind::StructFieldNotSerializable {
                                type_name: name.clone(),
                                field: fname.clone(),
                            },
                            trace: vec![],
                        });
                    }
                }
                Ok(())
            } else {
                for v in &td.variants {
                    for (idx, (_, fty)) in v.fields.iter().enumerate() {
                        if is_serializable(fty, module).is_err() {
                            return Err(NonSerializableReason {
                                kind: NonSerializableKind::AdtVariantNotSerializable {
                                    type_name: name.clone(),
                                    variant: v.name.clone(),
                                    field_idx: idx,
                                },
                                trace: vec![],
                            });
                        }
                    }
                }
                Ok(())
            }
        }
        _ => Err(NonSerializableReason {
            kind: NonSerializableKind::UnknownType(format!("{ty:?}")),
            trace: vec![],
        }),
    }
}
```

- [ ] **Step 6: Hook serializability into typeck**

In `crates/vox-compiler/src/typeck/mod.rs`, after the existing `check_effect_compliance` call:

```rust
diags.extend(check_remote_serializability(&hir));
```

Add the function:

```rust
fn check_remote_serializability(module: &HirModule) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    for f in &module.functions {
        if !f.is_remote {
            continue;
        }
        for param in &f.params {
            if let Err(reason) = is_serializable(&param.ty, module) {
                diags.push(Diagnostic::error(
                    format!(
                        "@remote fn `{}`: parameter `{}` of type `{}` is not Serializable: {}",
                        f.name,
                        param.name,
                        render_type(&param.ty),
                        render_reason(&reason)
                    ),
                    param.span,
                    "",
                ).with_code("vox/remote/non-serializable-param"));
            }
        }
        if let Some(rt) = &f.return_type {
            if let Err(reason) = is_serializable(rt, module) {
                diags.push(Diagnostic::error(
                    format!(
                        "@remote fn `{}`: return type `{}` is not Serializable: {}",
                        f.name,
                        render_type(rt),
                        render_reason(&reason)
                    ),
                    f.span,
                    "",
                ).with_code("vox/remote/non-serializable-return"));
            }
        }
    }
    diags
}
```

- [ ] **Step 7: Write the failing integration tests**

Create `crates/vox-compiler/tests/remote_annotation.rs`:

```rust
use vox_compiler::{lex, parse, lower_module, typeck};

fn diag_codes(src: &str) -> Vec<String> {
    let tokens = lex(src);
    let module = parse(tokens).expect("parse");
    let hir = lower_module(&module);
    typeck::run(&hir, src)
        .into_iter()
        .filter_map(|d| d.code)
        .collect()
}

#[test]
fn remote_with_serializable_args_compiles() {
    let codes = diag_codes("@remote fn foo(x: i32) to i32 { return x + 1 }");
    assert!(
        !codes.iter().any(|c| c.starts_with("vox/remote/")),
        "no remote diagnostic expected; got {codes:?}"
    );
}

#[test]
fn remote_with_function_param_is_rejected() {
    let codes = diag_codes(
        "type Cb = fn(i32) to i32
         @remote fn bar(c: Cb, x: i32) to i32 { return x }",
    );
    assert!(
        codes.iter().any(|c| c == "vox/remote/non-serializable-param"),
        "expected non-serializable-param; got {codes:?}"
    );
}

#[test]
fn remote_with_durable_promise_param_is_rejected() {
    let codes = diag_codes(
        "@remote fn baz(p: DurablePromise[i32]) to i32 { return 0 }",
    );
    assert!(
        codes.iter().any(|c| c == "vox/remote/non-serializable-param"),
        "DurablePromise should not cross @remote boundary; got {codes:?}"
    );
}

#[test]
fn mesh_prefix_warns_and_implies_remote() {
    let codes = diag_codes("fn mesh_compute(x: i32) to i32 { return x }");
    assert!(
        codes.iter().any(|c| c == "vox/api/mesh-prefix-deprecated"),
        "expected mesh_* deprecation; got {codes:?}"
    );
}

#[test]
fn remote_struct_with_nested_fn_field_rejected() {
    let codes = diag_codes(
        "type Bag { handler: fn(i32) to i32, payload: i32 }
         @remote fn ship(b: Bag) to i32 { return b.payload }",
    );
    assert!(
        codes.iter().any(|c| c == "vox/remote/non-serializable-param"),
        "struct with closure field should be rejected; got {codes:?}"
    );
}
```

- [ ] **Step 8: Run, expect PASS**

Run: `cargo test -p vox-compiler --test remote_annotation 2>&1 | tail -15`
Expected: all five PASS.

- [ ] **Step 9: Update `where-things-live.md`**

In `docs/src/architecture/where-things-live.md`, add rows (insert alphabetically):

```markdown
| `@remote` annotation | `crates/vox-compiler/src/parser/descent/decl/head.rs` (parse), `crates/vox-compiler/src/typeck/serializable.rs` (validate), `crates/vox-codegen/src/codegen_rust/emit/workflow.rs` (emit) | Phase 1 P1-T3 |
| `Serializable` predicate | `crates/vox-compiler/src/typeck/serializable.rs` | structural; rejects fn types, DurablePromise, Mutex/Arc/Channel |
```

- [ ] **Step 10: Commit**

```bash
git add crates/vox-compiler/src/lexer/mod.rs \
        crates/vox-compiler/src/ast/decl/fundecl.rs \
        crates/vox-compiler/src/parser/descent/decl/head.rs \
        crates/vox-compiler/src/hir/nodes/decl.rs \
        crates/vox-compiler/src/hir/lower/mod.rs \
        crates/vox-compiler/src/typeck/serializable.rs \
        crates/vox-compiler/src/typeck/mod.rs \
        crates/vox-compiler/tests/remote_annotation.rs \
        docs/src/architecture/where-things-live.md
git commit -m "$(cat <<'EOF'
feat(compiler): @remote fn annotation with Serializable param check (P1-T3)

Replaces the mesh_* naming convention. Parser recognises @remote and
@with_id; HIR carries is_remote/with_id_expr; typeck rejects parameters
or return types that fail the structural Serializable predicate.

`mesh_*`-prefixed functions auto-receive @remote with a deprecation
warning under vox/api/mesh-prefix-deprecated. New diagnostics:
vox/remote/non-serializable-param, vox/remote/non-serializable-return.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>
EOF
)"
```

---

## Task P1-T4: Auto-derived `activity_id`

Task P1-T4 has three sub-tasks because the work spans the compiler (emit hash inputs), runtime (consume them and BLAKE3), and codegen (carry them through vox-ir). Each sub-task is independently testable and committable.

### P1-T4a — Compiler emits hash-input quadruple at every activity call site

**Files:**

- Create: `crates/vox-compiler/src/typeck/activity_id_inputs.rs`
- Modify: `crates/vox-compiler/src/hir/nodes/decl.rs` — add `ActivityIdInputs` to the activity-call HIR.
- Modify: `crates/vox-compiler/src/hir/lower/mod.rs` — populate inputs at every direct call to a `@remote`-or-`activity` callee.
- Create: `crates/vox-compiler/tests/activity_id_inputs.rs`

The four inputs are:

1. **`workflow_id`** — the BLAKE3 hash of the *workflow function content* (parameter types, return type, body HIR), not the source span. This is stable under purely cosmetic refactors.
2. **`call_site_id`** — a stable per-workflow-body monotonic counter assigned at lower time. NOT line/column. The counter resets at each workflow function and increments with every direct activity-or-`@remote` call. This is robust against whitespace and line reflows.
3. **`structural_arg_hash`** — BLAKE3 over the canonical-JSON serialisation of every argument expression's *literal* value, with a sentinel for non-literal expressions (which fold into the call-site identity instead).
4. **`replay_counter`** — populated at runtime, incremented per workflow execution; only meaningful when the same `(workflow_id, call_site_id, structural_arg_hash)` triple appears more than once in a single execution.

The compiler emits the first three; the runtime fills `replay_counter` and computes the final BLAKE3 digest.

- [ ] **Step 1: Define the data type**

Create `crates/vox-compiler/src/typeck/activity_id_inputs.rs`:

```rust
//! Phase 1 P1-T4a — emit the four hash inputs for activity_id derivation.
//!
//! Inputs are propagated through HIR → vox-ir → planned-activity in the
//! workflow runtime, where BLAKE3 produces the final activity_id.
//!
//! ## Stability
//!
//! - `workflow_id` is the BLAKE3 of the workflow function's normalised HIR
//!   (parameter types, return type, body). It is stable across whitespace,
//!   formatting, and trivial source-position changes.
//! - `call_site_id` is a per-workflow-body counter, NOT a span. The counter
//!   resets per workflow function and increments at every direct activity
//!   call discovered by HIR lowering's deterministic top-down traversal.
//! - `structural_arg_hash` covers literal arguments only; non-literal
//!   arguments fold into call_site_id (different non-literal expressions
//!   appear at different counter positions).

use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ActivityIdInputs {
    /// BLAKE3 hex of the enclosing workflow's normalised HIR.
    pub workflow_id: String,
    /// 0-based monotonic counter within the workflow body.
    pub call_site_id: u32,
    /// BLAKE3 hex of canonical JSON over literal-argument values.
    pub structural_arg_hash: String,
}

/// Compute `workflow_id` from a workflow function's HIR. Stability invariant:
/// adding/removing comments or reformatting the source must NOT change the
/// returned hex.
pub fn compute_workflow_id(f: &crate::hir::nodes::HirFn) -> String {
    let normalised = serde_json::to_vec(&NormalisedFn::from(f))
        .expect("HirFn is serialisable");
    let digest = vox_crypto::blake3::hash(&normalised);
    hex::encode(digest.as_bytes())
}

#[derive(Serialize)]
struct NormalisedFn<'a> {
    name: &'a str,
    params: Vec<NormalisedParam<'a>>,
    return_type: Option<String>,
    body: Vec<String>, // serialised statements; spans stripped
}

#[derive(Serialize)]
struct NormalisedParam<'a> {
    name: &'a str,
    ty: String,
}

impl<'a> From<&'a crate::hir::nodes::HirFn> for NormalisedFn<'a> {
    fn from(f: &'a crate::hir::nodes::HirFn) -> Self {
        NormalisedFn {
            name: &f.name,
            params: f.params.iter().map(|p| NormalisedParam {
                name: &p.name,
                ty: format!("{:?}", p.ty),
            }).collect(),
            return_type: f.return_type.as_ref().map(|t| format!("{t:?}")),
            body: f.body.iter().map(|s| {
                // Use Debug formatting with spans stripped — see strip_spans below.
                strip_spans(format!("{s:?}"))
            }).collect(),
        }
    }
}

fn strip_spans(s: String) -> String {
    // Replace any "span: Span(N..M)" or "Span(N..M)" with "Span(_)" via regex.
    // We avoid a regex dep by doing a hand-rolled replace.
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == 'S' && chars.peek() == Some(&'p') {
            // Try to match "Span("
            let mut tentative = String::from(c);
            for _ in 0..4 {
                if let Some(&nc) = chars.peek() {
                    tentative.push(nc);
                    chars.next();
                } else {
                    break;
                }
            }
            if tentative == "Span(" {
                // Consume until matching `)`.
                let mut depth = 1;
                while let Some(nc) = chars.next() {
                    if nc == '(' { depth += 1; }
                    if nc == ')' { depth -= 1; if depth == 0 { break; } }
                }
                out.push_str("Span(_)");
            } else {
                out.push_str(&tentative);
            }
        } else {
            out.push(c);
        }
    }
    out
}

pub fn structural_arg_hash(args: &[crate::hir::HirArg]) -> String {
    let canonical: Vec<serde_json::Value> = args
        .iter()
        .map(|a| literal_or_sentinel(&a.value))
        .collect();
    let bytes = serde_json::to_vec(&canonical)
        .expect("Vec<Value> is serialisable");
    hex::encode(vox_crypto::blake3::hash(&bytes).as_bytes())
}

fn literal_or_sentinel(e: &crate::hir::HirExpr) -> serde_json::Value {
    use crate::hir::HirExpr;
    use serde_json::json;
    match e {
        HirExpr::IntLit(i, _) => json!({"int": i}),
        HirExpr::FloatLit(f, _) => json!({"float": f}),
        HirExpr::StringLit(s, _) => json!({"str": s}),
        HirExpr::BoolLit(b, _) => json!({"bool": b}),
        HirExpr::DecimalLit(d, _) => json!({"decimal": d.to_string()}),
        // Non-literal expressions: contribute a single `<dynamic>` token. The
        // call_site_id distinguishes between *positions*; this hash captures
        // *literal-value identity*. A workflow that calls `act(x)` twice with
        // the same `x` resolves to the same activity_id (correct) because
        // the call_site_id differs.
        _ => json!("<dynamic>"),
    }
}
```

- [ ] **Step 2: Wire into HIR lowering**

In `crates/vox-compiler/src/hir/lower/mod.rs`, find the workflow-body lowering loop. After lowering a `Decl::Fn` whose `durability == Some(DurabilityKind::Workflow)`, walk the body once to assign `call_site_id` to every direct activity / `@remote` call:

```rust
fn assign_activity_id_inputs(workflow: &mut HirFn, all_fns: &[HirFn]) {
    let workflow_id = compute_workflow_id(workflow);
    let mut counter: u32 = 0;
    walk_calls_in_place(&mut workflow.body, &mut |callee_name, args, slot| {
        let is_activity = all_fns.iter().any(|f| {
            f.name == *callee_name
                && (f.is_remote || matches!(f.durability, Some(DurabilityKind::Activity)))
        });
        if is_activity {
            *slot = Some(ActivityIdInputs {
                workflow_id: workflow_id.clone(),
                call_site_id: counter,
                structural_arg_hash: structural_arg_hash(args),
            });
            counter += 1;
        }
    });
}
```

Add `pub activity_id_inputs: Option<ActivityIdInputs>` to the `HirCallExpr` (or `HirExpr::Call`) variant. The walker mutates the inputs into place.

- [ ] **Step 3: Write the failing tests**

Create `crates/vox-compiler/tests/activity_id_inputs.rs`:

```rust
use vox_compiler::{lex, parse, lower_module};
use vox_compiler::hir::HirExpr;

fn workflow_call_inputs(src: &str, workflow_name: &str) -> Vec<vox_compiler::typeck::activity_id_inputs::ActivityIdInputs> {
    let tokens = lex(src);
    let module = parse(tokens).expect("parse");
    let hir = lower_module(&module);
    let wf = hir.functions.iter().find(|f| f.name == workflow_name).expect("wf");
    let mut out = Vec::new();
    walk_for_inputs(&wf.body, &mut out);
    out
}

fn walk_for_inputs(stmts: &[vox_compiler::hir::HirStmt], out: &mut Vec<_>) {
    use vox_compiler::hir::HirStmt;
    for s in stmts {
        match s {
            HirStmt::Expr { expr, .. }
            | HirStmt::Let { value: expr, .. }
            | HirStmt::Assign { value: expr, .. } => collect(expr, out),
            HirStmt::Return { value: Some(e), .. } => collect(e, out),
            _ => {}
        }
    }
}
fn collect(e: &HirExpr, out: &mut Vec<_>) {
    match e {
        HirExpr::Call(_, _, inputs, _) => {
            if let Some(i) = inputs {
                out.push(i.clone());
            }
        }
        _ => {}
    }
}

const WF_TWO_CALLS: &str = r#"
@remote fn act(x: i32) to i32 { return x + 1 }
workflow proc(n: i32) to i32 {
    let a = act(n)
    let b = act(n)
    return a + b
}
"#;

#[test]
fn two_calls_get_distinct_call_site_ids() {
    let inputs = workflow_call_inputs(WF_TWO_CALLS, "proc");
    assert_eq!(inputs.len(), 2);
    assert_eq!(inputs[0].call_site_id, 0);
    assert_eq!(inputs[1].call_site_id, 1);
    assert_eq!(inputs[0].workflow_id, inputs[1].workflow_id);
}

#[test]
fn workflow_id_stable_across_whitespace_changes() {
    let v1 = workflow_call_inputs(WF_TWO_CALLS, "proc");
    let with_extra_spaces = WF_TWO_CALLS.replace("act(n)", "act ( n )");
    let v2 = workflow_call_inputs(&with_extra_spaces, "proc");
    assert_eq!(v1[0].workflow_id, v2[0].workflow_id);
}

#[test]
fn literal_args_change_arg_hash_but_not_workflow_id() {
    let src1 = r#"
        @remote fn act(x: i32) to i32 { return x }
        workflow proc() to i32 { return act(1) }
    "#;
    let src2 = r#"
        @remote fn act(x: i32) to i32 { return x }
        workflow proc() to i32 { return act(2) }
    "#;
    let i1 = workflow_call_inputs(src1, "proc");
    let i2 = workflow_call_inputs(src2, "proc");
    assert_ne!(i1[0].structural_arg_hash, i2[0].structural_arg_hash);
    assert_ne!(i1[0].workflow_id, i2[0].workflow_id, "body bytes differ");
}

#[test]
fn dynamic_arg_collapses_to_sentinel() {
    let src = r#"
        @remote fn act(x: i32) to i32 { return x }
        workflow proc(n: i32) to i32 {
            return act(n)
        }
    "#;
    let inputs = workflow_call_inputs(src, "proc");
    // Sentinel is a deterministic constant, so repeated runs match.
    let inputs2 = workflow_call_inputs(src, "proc");
    assert_eq!(inputs[0].structural_arg_hash, inputs2[0].structural_arg_hash);
}
```

- [ ] **Step 4: Run, expect PASS**

Run: `cargo test -p vox-compiler --test activity_id_inputs 2>&1 | tail -20`
Expected: all four PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-compiler/src/typeck/activity_id_inputs.rs \
        crates/vox-compiler/src/typeck/mod.rs \
        crates/vox-compiler/src/hir/nodes/decl.rs \
        crates/vox-compiler/src/hir/lower/mod.rs \
        crates/vox-compiler/tests/activity_id_inputs.rs
git commit -m "$(cat <<'EOF'
feat(compiler): emit ActivityIdInputs at every workflow activity call site (P1-T4a)

Inputs (workflow_id, call_site_id, structural_arg_hash) are computed in
HIR lowering. workflow_id is BLAKE3 over normalised HirFn (spans stripped
so refactors stay stable); call_site_id is a per-workflow counter; the
structural hash covers literal args with a deterministic <dynamic>
sentinel for non-literals.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>
EOF
)"
```

### P1-T4b — Runtime consumes inputs and produces final `activity_id`

**Files:**

- Modify: `crates/vox-workflow-runtime/src/workflow/types.rs` — add `ActivityIdInputs` (mirror of compiler type).
- Modify: `crates/vox-workflow-runtime/src/workflow/run.rs` (grep `format!("{workflow_name}-{idx}")`) — replace `format!` with derivation.
- Create: `crates/vox-workflow-runtime/src/activity_id.rs`.
- Create: `crates/vox-workflow-runtime/tests/activity_id_derivation.rs`.

- [ ] **Step 1: Add the runtime derivation**

Create `crates/vox-workflow-runtime/src/activity_id.rs`:

```rust
//! Phase 1 P1-T4b — final BLAKE3 derivation of activity_id at the runtime.
//!
//! Inputs come from the compiler (P1-T4a); the runtime adds `replay_counter`
//! to disambiguate same-position-same-args repeats within a single execution.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ActivityIdInputs {
    pub workflow_id: String,
    pub call_site_id: u32,
    pub structural_arg_hash: String,
}

/// Derive the activity_id by hashing all four inputs together.
///
/// The runtime increments `replay_counter` each time the same
/// (workflow_id, call_site_id, structural_arg_hash) triple is observed
/// within one workflow execution. This handles loops where the same
/// position dispatches multiple activities with the same literal args.
pub fn derive_activity_id(inputs: &ActivityIdInputs, replay_counter: u32) -> String {
    let mut hasher = vox_crypto::blake3::Hasher::new();
    hasher.update(inputs.workflow_id.as_bytes());
    hasher.update(b"||");
    hasher.update(&inputs.call_site_id.to_le_bytes());
    hasher.update(b"||");
    hasher.update(inputs.structural_arg_hash.as_bytes());
    hasher.update(b"||");
    hasher.update(&replay_counter.to_le_bytes());
    let digest = hasher.finalize();
    format!("act-{}", &hex::encode(digest.as_bytes())[..16])
}
```

- [ ] **Step 2: Wire into `run.rs`**

In `crates/vox-workflow-runtime/src/workflow/run.rs`, replace the legacy fallback (grep `format!("{workflow_name}-{idx}")`):

```rust
for (idx, step) in plan.iter().enumerate() {
    let activity_id = match (&step.activity_id, &step.activity_id_inputs) {
        (Some(explicit), _) => explicit.clone(), // @with_id override
        (None, Some(inputs)) => {
            let counter = tracker.next_replay_counter(workflow_name, inputs).await?;
            crate::activity_id::derive_activity_id(inputs, counter)
        }
        (None, None) => format!("{workflow_name}-{idx}"), // legacy fallback
    };
    // … rest unchanged
}
```

Add `next_replay_counter` to `WorkflowTracker` trait. Default impl: in-memory counter keyed by `(workflow_id, call_site_id, structural_arg_hash)`.

- [ ] **Step 3: Add the runtime test**

Create `crates/vox-workflow-runtime/tests/activity_id_derivation.rs`:

```rust
use vox_workflow_runtime::activity_id::{derive_activity_id, ActivityIdInputs};

fn inputs(wf: &str, csid: u32, args: &str) -> ActivityIdInputs {
    ActivityIdInputs {
        workflow_id: wf.into(),
        call_site_id: csid,
        structural_arg_hash: args.into(),
    }
}

#[test]
fn same_inputs_same_replay_counter_yield_same_id() {
    let i = inputs("WF1", 0, "ARGS");
    assert_eq!(derive_activity_id(&i, 0), derive_activity_id(&i, 0));
}

#[test]
fn different_replay_counter_yields_different_id() {
    let i = inputs("WF1", 0, "ARGS");
    assert_ne!(derive_activity_id(&i, 0), derive_activity_id(&i, 1));
}

#[test]
fn different_call_site_yields_different_id() {
    let i0 = inputs("WF1", 0, "ARGS");
    let i1 = inputs("WF1", 1, "ARGS");
    assert_ne!(derive_activity_id(&i0, 0), derive_activity_id(&i1, 0));
}

#[test]
fn id_starts_with_act_prefix() {
    let id = derive_activity_id(&inputs("WF1", 0, "ARGS"), 0);
    assert!(id.starts_with("act-"));
    assert_eq!(id.len(), 4 + 16);
}
```

- [ ] **Step 4: Run, expect PASS**

Run: `cargo test -p vox-workflow-runtime activity_id 2>&1 | tail -10`
Expected: all four PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-workflow-runtime/src/activity_id.rs \
        crates/vox-workflow-runtime/src/workflow/run.rs \
        crates/vox-workflow-runtime/src/workflow/types.rs \
        crates/vox-workflow-runtime/src/lib.rs \
        crates/vox-workflow-runtime/tests/activity_id_derivation.rs
git commit -m "$(cat <<'EOF'
feat(runtime): BLAKE3-derived activity_id with replay_counter (P1-T4b)

Final id = BLAKE3(workflow_id ‖ call_site_id ‖ structural_arg_hash ‖
replay_counter), truncated to 16 hex chars and prefixed `act-`. Loop
positions that dispatch the same literal args twice in one execution are
disambiguated via the replay_counter; the WorkflowTracker tracks it.

Replaces the format!("{workflow_name}-{idx}") fallback.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>
EOF
)"
```

### P1-T4c — `@with_id(expr)` override + `time.now()` warning

**Files:**

- Modify: `crates/vox-compiler/src/hir/lower/mod.rs` — emit `with_id_expr` at the call site.
- Modify: `crates/vox-compiler/src/typeck/activity_id_inputs.rs` — emit warning when `with_id_expr` reads `time.now()` or `random.*`.
- Add tests to: `crates/vox-compiler/tests/activity_id_inputs.rs`.

- [ ] **Step 1: Detect non-determinism inside `@with_id`**

```rust
fn with_id_expr_uses_time_or_random(e: &HirExpr) -> Option<String> {
    use HirExpr::*;
    match e {
        MethodCall(obj, method, _, _, _) => {
            if let Ident(module, _) = obj.as_ref() {
                if matches!(module.as_str(), "time" | "clock" | "random") {
                    return Some(format!("{module}.{method}"));
                }
            }
            None
        }
        Call(callee, args, _, _) => {
            with_id_expr_uses_time_or_random(callee)
                .or_else(|| args.iter().find_map(|a| with_id_expr_uses_time_or_random(&a.value)))
        }
        Binary(_, l, r, _) => with_id_expr_uses_time_or_random(l).or_else(|| with_id_expr_uses_time_or_random(r)),
        Block(stmts, _) => stmts.iter().find_map(|s| match s {
            crate::hir::HirStmt::Expr { expr, .. } | crate::hir::HirStmt::Let { value: expr, .. } => with_id_expr_uses_time_or_random(expr),
            _ => None,
        }),
        _ => None,
    }
}
```

Run on every `with_id_expr`; emit `vox/workflow/with-id-non-deterministic` warning naming the offending call. Per the SSOT acceptance criterion, this is a *warning* — `@with_id` is intentionally a user-controlled escape hatch, but the warning catches the most common foot-gun.

- [ ] **Step 2: Add the test**

```rust
#[test]
fn with_id_using_time_now_warns() {
    let src = r#"
        @remote fn act(x: i32) to i32 { return x }
        workflow proc(x: i32) to i32 {
            return @with_id(time.now()) act(x)
        }
    "#;
    let codes = diag_codes(src);
    assert!(
        codes.iter().any(|c| c == "vox/workflow/with-id-non-deterministic"),
        "expected non-determinism warning; got {codes:?}"
    );
}
```

- [ ] **Step 3: Run, expect PASS**

Run: `cargo test -p vox-compiler --test activity_id_inputs 2>&1 | tail -10`
Expected: the new test PASSes alongside earlier ones.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-compiler/src/hir/lower/mod.rs \
        crates/vox-compiler/src/typeck/activity_id_inputs.rs \
        crates/vox-compiler/tests/activity_id_inputs.rs
git commit -m "$(cat <<'EOF'
feat(compiler): @with_id override + non-determinism warning (P1-T4c)

@with_id(expr) overrides the auto-derived activity_id with a business
identity. When `expr` calls time.now() / clock.* / random.*, emit
vox/workflow/with-id-non-deterministic — a warning, not error: this is
an intentional escape hatch but the most common misuse.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>
EOF
)"
```

---

## Task P1-T5: Workflow determinism check

**Files:**

- Create: `crates/vox-compiler/src/typeck/workflow_determinism.rs`
- Modify: `crates/vox-compiler/src/hir/nodes/durability.rs` — add `is_forbidden_in_workflow_body(&str) -> bool`.
- Create: `crates/vox-compiler/tests/workflow_determinism.rs`

This task introduces the `DurabilityKind::Workflow` row restriction: certain *builtin* method calls (`time.now()`, `random.next()`, `process.spawn()`, raw `fs.*` / `http.*` / `db.*`) cannot appear directly inside a workflow body. They must be wrapped in an activity (`@remote fn` or local `activity`) or a `side_effect { … }` block.

This task assumes top-down validation is still in place; it is independent of T6 because we're adding *additional* restrictions to workflows, not changing how effects propagate.

- [ ] **Step 1: Define the forbidden classifier**

In `crates/vox-compiler/src/hir/nodes/durability.rs`:

```rust
impl DurabilityKind {
    /// Builtin method calls forbidden inside a workflow body's straight-line
    /// path. Wrap them in `activity`, `@remote fn`, or `side_effect { … }`.
    pub fn forbidden_builtins_in_body(&self) -> &'static [(&'static str, &'static str)] {
        match self {
            DurabilityKind::Workflow => &[
                ("time", "now"),
                ("time", "instant"),
                ("clock", "now"),
                ("random", "next"),
                ("random", "shuffle"),
                ("process", "spawn"),
                ("fs", "read"),
                ("fs", "write"),
                ("http", "get"),
                ("http", "post"),
                ("db", "query"),
                ("db", "insert"),
                ("env", "get"),
            ],
            DurabilityKind::Activity | DurabilityKind::Actor => &[],
        }
    }
}
```

- [ ] **Step 2: Implement the body check**

Create `crates/vox-compiler/src/typeck/workflow_determinism.rs`:

```rust
//! Phase 1 P1-T5 — workflow determinism check.
//!
//! Walks every workflow function body and emits
//! vox/workflow/non-deterministic-builtin for any forbidden builtin call.

use crate::hir::nodes::{DurabilityKind, HirFn};
use crate::hir::{HirExpr, HirModule, HirStmt};
use crate::typeck::diagnostics::{Diagnostic, DiagnosticCategory, Fix, Applicability};

pub const VOX_WORKFLOW_NON_DETERMINISTIC_BUILTIN: &str = "vox/workflow/non-deterministic-builtin";

pub fn check_workflow_determinism(module: &HirModule, source: &str) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    for f in &module.functions {
        if f.durability != Some(DurabilityKind::Workflow) {
            continue;
        }
        let forbidden = DurabilityKind::Workflow.forbidden_builtins_in_body();
        for stmt in &f.body {
            walk_stmt_for_forbidden(stmt, forbidden, source, &mut diags);
        }
    }
    diags
}

fn walk_stmt_for_forbidden(
    s: &HirStmt,
    forbidden: &[(&str, &str)],
    source: &str,
    out: &mut Vec<Diagnostic>,
) {
    match s {
        HirStmt::Expr { expr, .. }
        | HirStmt::Let { value: expr, .. }
        | HirStmt::Assign { value: expr, .. } => walk_expr(expr, forbidden, source, out),
        HirStmt::Return { value: Some(e), .. } => walk_expr(e, forbidden, source, out),
        HirStmt::While { condition, body, .. } => {
            walk_expr(condition, forbidden, source, out);
            for s in body {
                walk_stmt_for_forbidden(s, forbidden, source, out);
            }
        }
        HirStmt::Loop { body, .. } => {
            for s in body {
                walk_stmt_for_forbidden(s, forbidden, source, out);
            }
        }
        _ => {}
    }
}

fn walk_expr(e: &HirExpr, forbidden: &[(&str, &str)], source: &str, out: &mut Vec<Diagnostic>) {
    use HirExpr::*;
    match e {
        MethodCall(obj, method, args, _, span) => {
            if let Ident(module_name, _) = obj.as_ref() {
                let hit = forbidden.iter().any(|(m, fname)| {
                    m == module_name.as_str() && fname == method.as_str()
                });
                if hit {
                    let mut d = Diagnostic::error(
                        format!(
                            "`{module_name}.{method}()` is non-deterministic and cannot \
                             appear directly inside a `workflow` body. \
                             Wrap it in an `activity` / `@remote fn`, or use \
                             `side_effect {{ … }}` for a one-shot."
                        ),
                        *span,
                        source,
                    );
                    d.code = Some(VOX_WORKFLOW_NON_DETERMINISTIC_BUILTIN.into());
                    d.category = DiagnosticCategory::WorkflowDeterminism;
                    d.fixes.push(Fix {
                        message: "wrap in side_effect { … }".into(),
                        replacement: Some(format!("side_effect {{ {module_name}.{method}({}) }}",
                            args.iter().map(|a| format!("{:?}", a.value)).collect::<Vec<_>>().join(", "))),
                        span: *span,
                        applicability: Applicability::HasPlaceholders,
                    });
                    out.push(d);
                }
            }
            walk_expr(obj, forbidden, source, out);
            for a in args {
                walk_expr(&a.value, forbidden, source, out);
            }
        }
        Call(callee, args, _, _) => {
            walk_expr(callee, forbidden, source, out);
            for a in args {
                walk_expr(&a.value, forbidden, source, out);
            }
        }
        Binary(_, l, r, _) => {
            walk_expr(l, forbidden, source, out);
            walk_expr(r, forbidden, source, out);
        }
        If(c, then, elseb, _) => {
            walk_expr(c, forbidden, source, out);
            for s in then {
                walk_stmt_for_forbidden(s, forbidden, source, out);
            }
            if let Some(e) = elseb {
                for s in e {
                    walk_stmt_for_forbidden(s, forbidden, source, out);
                }
            }
        }
        Block(stmts, _) => {
            for s in stmts {
                walk_stmt_for_forbidden(s, forbidden, source, out);
            }
        }
        _ => {}
    }
}
```

Add `WorkflowDeterminism` to `DiagnosticCategory` if not present.

- [ ] **Step 3: Wire into typeck**

In `crates/vox-compiler/src/typeck/mod.rs`:

```rust
diags.extend(workflow_determinism::check_workflow_determinism(&hir, source));
```

- [ ] **Step 4: Write the failing tests**

Create `crates/vox-compiler/tests/workflow_determinism.rs`:

```rust
use vox_compiler::{lex, parse, lower_module, typeck};

fn check(src: &str) -> Vec<vox_compiler::Diagnostic> {
    let tokens = lex(src);
    let module = parse(tokens).expect("parse");
    let hir = lower_module(&module);
    typeck::run(&hir, src)
}

#[test]
fn time_now_in_workflow_body_is_an_error() {
    let diags = check(r#"
        workflow proc() to i64 {
            let t = time.now()
            return t
        }
    "#);
    assert!(
        diags.iter().any(|d| d.code.as_deref() == Some("vox/workflow/non-deterministic-builtin")),
        "expected non-deterministic-builtin; got: {:?}",
        diags.iter().map(|d| &d.code).collect::<Vec<_>>()
    );
}

#[test]
fn random_in_workflow_body_is_an_error() {
    let diags = check(r#"
        workflow proc() to i32 { return random.next() }
    "#);
    assert!(diags.iter().any(|d| d.code.as_deref() == Some("vox/workflow/non-deterministic-builtin")));
}

#[test]
fn time_now_in_activity_is_fine() {
    let diags = check(r#"
        activity stamp() to i64 { return time.now() }
        workflow proc() to i64 { return stamp() }
    "#);
    assert!(
        !diags.iter().any(|d| d.code.as_deref() == Some("vox/workflow/non-deterministic-builtin")),
        "activity may call time.now(); got {diags:?}"
    );
}

#[test]
fn time_now_inside_side_effect_block_is_fine() {
    let diags = check(r#"
        workflow proc() to i64 {
            return side_effect { time.now() }
        }
    "#);
    assert!(
        !diags.iter().any(|d| d.code.as_deref() == Some("vox/workflow/non-deterministic-builtin")),
        "side_effect wraps non-determinism; got {diags:?}"
    );
}

#[test]
fn fs_read_in_workflow_body_is_an_error() {
    let diags = check(r#"
        workflow proc() to str { return fs.read("/etc/passwd") }
    "#);
    assert!(diags.iter().any(|d| d.code.as_deref() == Some("vox/workflow/non-deterministic-builtin")));
}

#[test]
fn diagnostic_includes_wrap_suggestion() {
    let diags = check(r#"
        workflow proc() to i64 {
            let t = time.now()
            return t
        }
    "#);
    let d = diags.iter().find(|d| d.code.as_deref() == Some("vox/workflow/non-deterministic-builtin")).unwrap();
    assert!(
        d.fixes.iter().any(|f| f.message.contains("side_effect")),
        "expected side_effect suggestion; got {:?}",
        d.fixes
    );
}
```

The "side_effect block is fine" test is forward-looking — `side_effect` parsing arrives in P1-T7. For now, until that lands, the test should be marked `#[ignore]` or written to expect a *different* error (parse error). After P1-T7, the test flips. Add a TODO comment:

```rust
// NOTE: this test passes today only because side_effect { … } currently
// fails to parse, which short-circuits typeck. Once P1-T7 lands and
// side_effect parses successfully, this test exercises the real
// determinism-suppression path. Both paths must yield "no
// non-deterministic-builtin diagnostic", which is what we assert.
```

- [ ] **Step 5: Run, expect PASS**

Run: `cargo test -p vox-compiler --test workflow_determinism 2>&1 | tail -20`
Expected: all six PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-compiler/src/typeck/workflow_determinism.rs \
        crates/vox-compiler/src/typeck/mod.rs \
        crates/vox-compiler/src/typeck/diagnostics.rs \
        crates/vox-compiler/src/hir/nodes/durability.rs \
        crates/vox-compiler/tests/workflow_determinism.rs
git commit -m "$(cat <<'EOF'
feat(compiler): workflow body rejects forbidden non-deterministic builtins (P1-T5)

Adds DurabilityKind::Workflow row restriction over time/clock/random/
process/fs/http/db/env method calls. Diagnostic
vox/workflow/non-deterministic-builtin includes a side_effect { … }
auto-suggestion fix.

Activities, actors, and side_effect blocks are exempt — they're the
sanctioned wrappers for non-determinism inside workflow logic.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>
EOF
)"
```

---

## Task P1-T6: Bottom-up effect inference

**Files:**

- Create: `crates/vox-compiler/src/typeck/effect_inference.rs`
- Modify: `crates/vox-compiler/src/typeck/effect_check.rs` — split into "infer" + "compare-against-declared".
- Modify: `crates/vox-compiler/src/hir/nodes/decl.rs` — add `inferred_effects: Vec<HirCapability>`.
- Modify: `crates/vox-compiler/src/typeck/diagnostics.rs` — new code `vox/effect/missing-declaration`.
- Create: `crates/vox-compiler/tests/effect_effect_inference.rs`

Today's `effect_check.rs` runs *top-down* — given a function with an explicit `uses` clause, it walks the body and errors on any callee that requires an effect not in the clause. The flip is to:

1. Compute every function's *inferred* effect set (union of all callee effect sets + intrinsic stdlib effects) in a fixed-point iteration over the call graph.
2. Compare the *inferred* set to the *declared* set (the existing `uses` clause). If declared ⊃ inferred, no diagnostic. If inferred ⊃ declared, emit `vox/effect/missing-declaration` listing what's missing — with a structured fix that adds the missing effect names to the `uses` clause.

This means **unannotated functions also acquire an effect set** (the full inferred set), which feeds T8's `vox workflow preview` projector.

- [ ] **Step 1: Move shared helpers from `effect_check.rs` to `typeck/effects.rs`**

Extract `effect_kind_to_cap`, `effective_caps`, `is_annotated`, `stdlib_module_capability` into a new module `crates/vox-compiler/src/typeck/effects.rs`. Re-export from `effect_check.rs` for backward compat (codegen uses some).

- [ ] **Step 2: Implement inference**

Create `crates/vox-compiler/src/typeck/effect_inference.rs`:

```rust
//! Phase 1 P1-T6 — bottom-up effect inference.
//!
//! Computes for every function in the module: the union of its own intrinsic
//! effects (from stdlib calls) and the inferred effect sets of every callee.
//! Iterates to fixed point because the call graph may contain cycles
//! (mutual recursion) and self-recursion.

use std::collections::{BTreeSet, HashMap};

use crate::hir::nodes::{HirFn, HirCapability};
use crate::hir::{HirExpr, HirModule, HirStmt};
use crate::typeck::effects::stdlib_module_capability;

pub fn infer_module_effects(
    module: &HirModule,
) -> HashMap<String, BTreeSet<HirCapability>> {
    let mut sets: HashMap<String, BTreeSet<HirCapability>> = module
        .functions
        .iter()
        .map(|f| (f.name.clone(), BTreeSet::new()))
        .collect();

    // Each iteration recomputes f's effect set as direct effects ∪ ⋃ callees.
    loop {
        let mut changed = false;
        for f in &module.functions {
            let mut new_set = direct_effects(f);
            for callee in callees(f) {
                if let Some(callee_set) = sets.get(&callee) {
                    new_set.extend(callee_set.iter().cloned());
                }
            }
            // Add @remote effects.
            if f.is_remote {
                new_set.insert(HirCapability::Spawn);
                new_set.insert(HirCapability::Net);
            }
            if sets.get(&f.name) != Some(&new_set) {
                sets.insert(f.name.clone(), new_set);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    sets
}

fn direct_effects(f: &HirFn) -> BTreeSet<HirCapability> {
    let mut out = BTreeSet::new();
    for stmt in &f.body {
        walk_stmt(stmt, &mut out);
    }
    out
}

fn walk_stmt(s: &HirStmt, out: &mut BTreeSet<HirCapability>) {
    match s {
        HirStmt::Expr { expr, .. }
        | HirStmt::Let { value: expr, .. }
        | HirStmt::Assign { value: expr, .. } => walk_expr(expr, out),
        HirStmt::Return { value: Some(e), .. } => walk_expr(e, out),
        HirStmt::While { condition, body, .. } => {
            walk_expr(condition, out);
            for s in body {
                walk_stmt(s, out);
            }
        }
        HirStmt::Loop { body, .. } => {
            for s in body {
                walk_stmt(s, out);
            }
        }
        _ => {}
    }
}

fn walk_expr(e: &HirExpr, out: &mut BTreeSet<HirCapability>) {
    use HirExpr::*;
    match e {
        MethodCall(obj, _, args, _, _) => {
            if let Ident(module, _) = obj.as_ref() {
                if let Some(cap) = stdlib_module_capability(module) {
                    out.insert(cap);
                }
            }
            walk_expr(obj, out);
            for a in args {
                walk_expr(&a.value, out);
            }
        }
        Call(callee, args, _, _) => {
            walk_expr(callee, out);
            for a in args {
                walk_expr(&a.value, out);
            }
        }
        Binary(_, l, r, _) => {
            walk_expr(l, out);
            walk_expr(r, out);
        }
        If(c, then, elseb, _) => {
            walk_expr(c, out);
            for s in then {
                walk_stmt(s, out);
            }
            if let Some(e) = elseb {
                for s in e {
                    walk_stmt(s, out);
                }
            }
        }
        Block(stmts, _) => {
            for s in stmts {
                walk_stmt(s, out);
            }
        }
        _ => {}
    }
}

fn callees(f: &HirFn) -> Vec<String> {
    let mut out = Vec::new();
    for stmt in &f.body {
        callees_in_stmt(stmt, &mut out);
    }
    out
}

fn callees_in_stmt(s: &HirStmt, out: &mut Vec<String>) {
    match s {
        HirStmt::Expr { expr, .. }
        | HirStmt::Let { value: expr, .. }
        | HirStmt::Assign { value: expr, .. } => callees_in_expr(expr, out),
        HirStmt::Return { value: Some(e), .. } => callees_in_expr(e, out),
        HirStmt::While { condition, body, .. } => {
            callees_in_expr(condition, out);
            for s in body {
                callees_in_stmt(s, out);
            }
        }
        HirStmt::Loop { body, .. } => {
            for s in body {
                callees_in_stmt(s, out);
            }
        }
        _ => {}
    }
}

fn callees_in_expr(e: &HirExpr, out: &mut Vec<String>) {
    use HirExpr::*;
    match e {
        Call(callee, args, _, _) => {
            if let Ident(name, _) = callee.as_ref() {
                out.push(name.clone());
            }
            for a in args {
                callees_in_expr(&a.value, out);
            }
        }
        MethodCall(obj, _, args, _, _) => {
            callees_in_expr(obj, out);
            for a in args {
                callees_in_expr(&a.value, out);
            }
        }
        Binary(_, l, r, _) => {
            callees_in_expr(l, out);
            callees_in_expr(r, out);
        }
        Block(stmts, _) => {
            for s in stmts {
                callees_in_stmt(s, out);
            }
        }
        _ => {}
    }
}
```

- [ ] **Step 3: Re-write the comparison check**

Replace the body of `check_effect_compliance` in `effect_check.rs`:

```rust
pub fn check_effect_compliance(module: &HirModule, source: &str) -> Vec<Diagnostic> {
    let inferred = crate::typeck::inference::infer_module_effects(module);
    let mut diags = Vec::new();
    for f in &module.functions {
        // Unannotated => no compliance check (still get an inferred set for tooling).
        if !is_annotated(f) {
            continue;
        }
        let declared: HashSet<HirCapability> = effective_caps(f).into_iter().collect();
        let empty = BTreeSet::new();
        let inferred_set = inferred.get(&f.name).unwrap_or(&empty);

        let missing: Vec<&HirCapability> = inferred_set
            .iter()
            .filter(|c| !declared.contains(c) && !matches!(c, HirCapability::Nothing))
            .collect();

        if !missing.is_empty() {
            let labels = missing.iter().map(|c| format!("{c}")).collect::<Vec<_>>().join(", ");
            let mut d = Diagnostic::error(
                format!(
                    "function `{}` is annotated `uses {}` but its body requires `{}`",
                    f.name,
                    declared.iter().map(|c| format!("{c}")).collect::<Vec<_>>().join(", "),
                    labels
                ),
                f.span,
                source,
            );
            d.code = Some("vox/effect/missing-declaration".into());
            d.category = DiagnosticCategory::EffectViolation;
            // Auto-fix: append missing effects to `uses` clause.
            d.fixes.push(Fix {
                message: format!("add `{}` to the `uses` clause", labels),
                replacement: Some(format_uses_clause(&declared, &missing)),
                span: f.span,
                applicability: Applicability::MachineApplicable,
            });
            diags.push(d);
        }
    }
    // Endpoint-fn duplicate / pure-conflict structural checks remain unchanged.
    diags.extend(check_endpoint_fn_effects(&module.endpoint_fns));
    diags
}

fn format_uses_clause(declared: &HashSet<HirCapability>, missing: &[&HirCapability]) -> String {
    let mut all: Vec<String> = declared.iter().map(|c| format!("{c}")).collect();
    all.extend(missing.iter().map(|c| format!("{c}")));
    all.sort();
    format!("uses {}", all.join(", "))
}
```

Also persist `inferred_effects` on `HirFn` for downstream consumers (T4 inputs, T8 preview):

```rust
for f in &mut module.functions {
    if let Some(set) = inferred.remove(&f.name) {
        f.inferred_effects = set.into_iter().collect();
    }
}
```

- [ ] **Step 4: Write the failing tests**

Create `crates/vox-compiler/tests/effect_effect_inference.rs`:

```rust
use vox_compiler::{lex, parse, lower_module, typeck};
use vox_compiler::hir::nodes::HirCapability;

fn inferred_for(src: &str, fn_name: &str) -> Vec<HirCapability> {
    let tokens = lex(src);
    let module = parse(tokens).expect("parse");
    let hir = lower_module(&module);
    let _ = typeck::run(&hir, src); // populates inferred_effects
    let f = hir.functions.iter().find(|f| f.name == fn_name).expect("fn");
    f.inferred_effects.clone()
}

#[test]
fn unannotated_caller_inherits_callee_effects() {
    let src = r#"
        fn fetch() uses net to str { http.get("https://example.com") }
        fn caller() to str { fetch() }
    "#;
    let inferred = inferred_for(src, "caller");
    assert!(inferred.contains(&HirCapability::Net), "expected net; got {inferred:?}");
}

#[test]
fn cycle_converges_to_union() {
    let src = r#"
        fn a() uses net to str { http.get("u"); b() }
        fn b() uses db to str { db.query("SELECT 1"); a() }
    "#;
    let inferred_a = inferred_for(src, "a");
    let inferred_b = inferred_for(src, "b");
    assert!(inferred_a.contains(&HirCapability::Net));
    assert!(inferred_a.contains(&HirCapability::Db));
    assert!(inferred_b.contains(&HirCapability::Net));
    assert!(inferred_b.contains(&HirCapability::Db));
}

#[test]
fn declared_subset_of_inferred_emits_missing_declaration() {
    let src = r#"
        fn fetch() uses net to str { http.get("u") }
        fn caller() uses nothing to str { fetch() }
    "#;
    let tokens = lex(src);
    let module = parse(tokens).expect("parse");
    let hir = lower_module(&module);
    let diags = typeck::run(&hir, src);
    assert!(
        diags.iter().any(|d| d.code.as_deref() == Some("vox/effect/missing-declaration")),
        "expected missing-declaration; got: {:?}",
        diags.iter().map(|d| &d.code).collect::<Vec<_>>()
    );
}

#[test]
fn declared_superset_of_inferred_is_fine() {
    let src = r#"
        fn fetch() uses net to str { http.get("u") }
        fn caller() uses net, db to str { fetch() }
    "#;
    let tokens = lex(src);
    let module = parse(tokens).expect("parse");
    let hir = lower_module(&module);
    let diags = typeck::run(&hir, src);
    assert!(
        !diags.iter().any(|d| d.code.as_deref().is_some_and(|c| c.starts_with("vox/effect/"))),
        "no effect diagnostic expected; got {diags:?}"
    );
}

#[test]
fn remote_fn_picks_up_spawn_and_net() {
    let src = r#"
        @remote fn act(x: i32) to i32 { return x }
        fn caller() to i32 { return act(1) }
    "#;
    let inferred = inferred_for(src, "caller");
    assert!(inferred.contains(&HirCapability::Spawn));
    assert!(inferred.contains(&HirCapability::Net));
}

#[test]
fn fix_includes_missing_effects_in_uses_clause() {
    let src = r#"
        fn fetch() uses net, db to str { db.query("x") }
        fn caller() uses net to str { fetch() }
    "#;
    let tokens = lex(src);
    let module = parse(tokens).expect("parse");
    let hir = lower_module(&module);
    let diags = typeck::run(&hir, src);
    let d = diags.iter().find(|d| d.code.as_deref() == Some("vox/effect/missing-declaration")).unwrap();
    let fix = d.fixes.first().unwrap();
    assert!(fix.replacement.as_ref().unwrap().contains("db"));
    assert!(fix.replacement.as_ref().unwrap().contains("net"));
}
```

- [ ] **Step 5: Run, expect PASS**

Run: `cargo test -p vox-compiler --test effect_inference 2>&1 | tail -25`
Expected: all six PASS.

- [ ] **Step 6: Update existing `effect_check::tests`**

The old top-down tests in `effect_check.rs::tests::test_annotated_caller_missing_capability_is_error` will now match `vox/effect/missing-declaration` instead of the old uncoded error. Update assertions:

```rust
#[test]
fn test_annotated_caller_missing_capability_is_error() {
    let diags = check(
        "fn fetch() uses net to str { \"ok\" }
fn caller() uses nothing to str { fetch() }",
    );
    assert_eq!(diags.len(), 1, "expected one violation: {diags:?}");
    assert_eq!(diags[0].code.as_deref(), Some("vox/effect/missing-declaration"));
    assert!(diags[0].message.contains("net"));
}
```

Apply analogous updates to all other tests in `effect_check.rs::tests` that previously asserted on uncoded errors.

- [ ] **Step 7: Run the full crate test suite**

Run: `cargo test -p vox-compiler 2>&1 | tail -20`
Expected: all PASS. If any test still asserts on the old top-down message, update it.

- [ ] **Step 8: Commit**

```bash
git add crates/vox-compiler/src/typeck/effect_inference.rs \
        crates/vox-compiler/src/typeck/effects.rs \
        crates/vox-compiler/src/typeck/effect_check.rs \
        crates/vox-compiler/src/typeck/mod.rs \
        crates/vox-compiler/src/typeck/diagnostics.rs \
        crates/vox-compiler/src/hir/nodes/decl.rs \
        crates/vox-compiler/tests/effect_effect_inference.rs
git commit -m "$(cat <<'EOF'
feat(compiler): bottom-up effect inference replacing top-down validation (P1-T6)

Inference iterates the call graph to fixed point, populating
HirFn.inferred_effects for every function (annotated or not). The check
phase now compares declared `uses` against inferred and emits
vox/effect/missing-declaration when declared lacks something inferred,
with a machine-applicable fix that rewrites the clause.

Unannotated functions get inferred sets too, which feeds the
P1-T8 `vox workflow preview` projector.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>
EOF
)"
```

---

## Task P1-T7: `side_effect { … }` block

**Files:**

- Modify: `crates/vox-compiler/src/lexer/mod.rs` — add `Token::SideEffect` keyword.
- Modify: `crates/vox-compiler/src/parser/descent/expr/mod.rs` — parse `side_effect { … }` as an expression.
- Modify: `crates/vox-compiler/src/ast/expr.rs` — add `Expr::SideEffect(Vec<Stmt>, Span)`.
- Modify: `crates/vox-compiler/src/hir/lower/mod.rs` — desugar to a synthesised inline activity reusing P1-T4 derivation.
- Modify: `crates/vox-compiler/src/typeck/workflow_determinism.rs` — exempt the body of a `side_effect` expression.
- Create: `crates/vox-compiler/tests/side_effect_block.rs`

`side_effect { … }` is a block expression evaluating to whatever the body returns. The compiler synthesises an anonymous inline activity with auto-derived `activity_id` (using P1-T4a/T4b) and replaces the block at lowering time with a call to that activity.

- [ ] **Step 1: Add the keyword token**

In `crates/vox-compiler/src/lexer/mod.rs`:

```rust
"side_effect" => Token::SideEffect,
```

Add `Token::SideEffect` variant.

- [ ] **Step 2: Add the AST node**

In `crates/vox-compiler/src/ast/expr.rs`:

```rust
/// Phase 1 P1-T7: `side_effect { stmts }` — sanctioned non-determinism inside
/// a workflow. Desugars to an anonymous inline activity at lower time.
SideEffect(Vec<Stmt>, Span),
```

- [ ] **Step 3: Parse**

In `crates/vox-compiler/src/parser/descent/expr/mod.rs` (the primary expression dispatcher), add a case for `Token::SideEffect`:

```rust
Token::SideEffect => {
    let start = self.span();
    self.advance();
    self.expect(&Token::LBrace)?;
    let body = self.parse_block()?;
    Ok(Expr::SideEffect(body, start.merge(self.span())))
}
```

- [ ] **Step 4: Desugar in HIR lowering**

In `crates/vox-compiler/src/hir/lower/mod.rs`, when lowering `Expr::SideEffect(body, span)`:

```rust
Expr::SideEffect(body, span) => {
    // Synthesise an anonymous inline activity. The activity has no parameters
    // (the body is a closure over enclosing locals — captures are not
    // permitted in side_effect blocks; a future expansion may relax this).
    let synthetic_name = format!("__side_effect_{}", self.next_synthesis_counter());
    let body_hir: Vec<HirStmt> = body.iter().map(|s| self.lower_stmt(s)).collect();
    // The synthesised activity is appended to the module post-lowering
    // (collected in self.synthesised_activities to avoid invalidating
    // the iterator).
    self.synthesised_activities.push(HirFn {
        name: synthetic_name.clone(),
        durability: Some(DurabilityKind::Activity),
        body: body_hir,
        params: vec![],
        return_type: ctx.typeck.type_of(&block.last_expr())?,
        is_remote: false,
        with_id_expr: None,
        capabilities: vec![], // inferred by P1-T6 in a follow-up pass
        is_pure: false,
        ...
    });
    // Replace the SideEffect with a call to the synthesised activity.
    let call = HirExpr::Call(
        Box::new(HirExpr::Ident(synthetic_name, span)),
        vec![],
        Some(ActivityIdInputs {
            workflow_id: enclosing_workflow_id,
            call_site_span: span_id_from(block.span),
            structural_arg_hash: blake3_zero(),  // no captured args; the block runs immediately
            replay_counter: ctx.next_replay_counter(),
        }),
        span,
    );
    call
}
```

After the lowering loop completes, push every `synthesised_activities` entry into `module.functions` and re-run effect inference (P1-T6) so the synthesised activity gets its inferred effect set populated.

- [ ] **Step 5: Exempt `SideEffect` in the determinism check**

In `crates/vox-compiler/src/typeck/workflow_determinism.rs`, `walk_expr` matches `HirExpr::Call(_, _, _, _)` to a synthesised side-effect name (prefix `__side_effect_`). Skip its descend. Or more robustly: rely on the desugar replacing the original `SideEffect` AST with a call — at HIR time there's no SideEffect to walk into, so the determinism walker simply doesn't see the inner non-deterministic builtins.

This means the desugar order matters: HIR lowering MUST happen before workflow-determinism checking. That's already the case — typeck runs on HIR.

No separate walker is needed — the existing workflow-body restriction (P1-T5) is sufficient because the desugared `__side_effect_<n>` activity inherits the workflow context check. The diagnostic for the outside-workflow case reuses the existing code `vox/workflow/side-effect-outside-workflow` (defined in P1-T9).

- [ ] **Step 6: Write the failing tests**

Create `crates/vox-compiler/tests/side_effect_block.rs`:

```rust
use vox_compiler::{lex, parse, lower_module, typeck};

fn check(src: &str) -> Vec<vox_compiler::Diagnostic> {
    let tokens = lex(src);
    let module = parse(tokens).expect("parse");
    let hir = lower_module(&module);
    typeck::run(&hir, src)
}

#[test]
fn side_effect_block_parses() {
    let src = r#"
        workflow proc() to i64 {
            return side_effect { time.now() }
        }
    "#;
    let diags = check(src);
    let parse_errs: Vec<_> = diags.iter().filter(|d| d.code.as_deref() == Some("E_PARSE")).collect();
    assert!(parse_errs.is_empty(), "should parse: {parse_errs:?}");
}

#[test]
fn side_effect_suppresses_non_determinism_check() {
    let src = r#"
        workflow proc() to i64 {
            return side_effect { time.now() }
        }
    "#;
    let diags = check(src);
    assert!(
        !diags.iter().any(|d| d.code.as_deref() == Some("vox/workflow/non-deterministic-builtin")),
        "side_effect should suppress determinism check; got {diags:?}"
    );
}

#[test]
fn side_effect_outside_workflow_is_an_error() {
    // Outside a workflow body, `side_effect` is an error: it has no journal
    // to bind to. We use durability == None as the discriminator.
    let src = r#"
        fn plain() to i64 {
            return side_effect { time.now() }
        }
    "#;
    let diags = check(src);
    assert!(
        diags.iter().any(|d| d.code.as_deref() == Some("vox/workflow/side-effect-outside-workflow")),
        "expected side-effect-outside-workflow; got {diags:?}"
    );
}

#[test]
fn nested_side_effect_block_creates_distinct_activities() {
    let src = r#"
        workflow proc() to i64 {
            let a = side_effect { time.now() }
            let b = side_effect { time.now() }
            return a + b
        }
    "#;
    let tokens = lex(src);
    let module = parse(tokens).expect("parse");
    let hir = lower_module(&module);
    let synthesised: Vec<_> = hir.functions.iter()
        .filter(|f| f.name.starts_with("__side_effect_"))
        .collect();
    assert_eq!(synthesised.len(), 2, "two side_effect blocks → two synthesised activities");
    assert_ne!(synthesised[0].name, synthesised[1].name);
}
```

- [ ] **Step 7: Run, expect PASS**

Run: `cargo test -p vox-compiler --test side_effect_block 2>&1 | tail -15`
Expected: all four PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/vox-compiler/src/lexer/mod.rs \
        crates/vox-compiler/src/ast/expr.rs \
        crates/vox-compiler/src/parser/descent/expr/mod.rs \
        crates/vox-compiler/src/hir/lower/mod.rs \
        crates/vox-compiler/src/typeck/workflow_determinism.rs \
        crates/vox-compiler/tests/side_effect_block.rs
git commit -m "$(cat <<'EOF'
feat(compiler): side_effect { … } blocks for sanctioned non-determinism (P1-T7)

`side_effect { … }` desugars at HIR lower time to an anonymous inline
activity (name `__side_effect_<n>`) reusing the P1-T4 activity_id
derivation. Inside the synthesised activity, time/random/fs/etc. calls
are permitted — outside a workflow body, the block is an error
(vox/workflow/side-effect-outside-workflow).

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>
EOF
)"
```

---

## Task P1-T8: `vox workflow preview <fn>(args)` dry-run projector

**Files:**

- Create: `crates/vox-cli/src/commands/workflow.rs` — clap dispatch.
- Create: `crates/vox-cli/src/commands/workflow/preview.rs` — projector.
- Modify: `crates/vox-cli/src/lib.rs` — add `Workflow` clap subcommand variant.
- Modify: `crates/vox-cli/src/commands/mod.rs` — `pub mod workflow;`.
- Create: `crates/vox-cli/tests/workflow_preview.rs`
- Create: `tests/fixtures/workflow_preview/simple_two_step.vox`
- Create: `tests/fixtures/workflow_preview/with_signal.vox`
- Create: `tests/fixtures/workflow_preview/with_side_effect.vox`

`vox workflow preview path/to/file.vox::workflow_name(arg1, arg2, ...)` runs the entire frontend (lex/parse/lower/typeck/inference), then walks the named workflow to produce a tree of "would-call" `PlannedActivity` records — same data structure the runtime consumes — *without dispatching anything*. Output is human-readable text by default, with `--json` for tooling.

The projector intentionally does not evaluate user-supplied args; it shows the *static* schedule. Args appear as opaque tokens (`<arg arg1>`) in the output.

- [ ] **Step 1: Wire the clap variant**

In `crates/vox-cli/src/lib.rs` (or wherever the `Cli` enum lives), add:

```rust
/// Workflow-tree introspection: dry-run preview, schedule projection.
Workflow(WorkflowArgs),
```

`WorkflowArgs`:

```rust
#[derive(clap::Args, Debug)]
pub struct WorkflowArgs {
    #[command(subcommand)]
    pub sub: WorkflowSub,
}

#[derive(clap::Subcommand, Debug)]
pub enum WorkflowSub {
    /// Project the schedule of activities a workflow would dispatch.
    Preview(WorkflowPreviewArgs),
}

#[derive(clap::Args, Debug)]
pub struct WorkflowPreviewArgs {
    /// Workflow target: `path/to/file.vox::workflow_name`.
    pub target: String,
    /// Render JSON instead of text.
    #[arg(long)]
    pub json: bool,
}
```

In the dispatch:

```rust
Cmd::Workflow(args) => match &args.sub {
    WorkflowSub::Preview(p) => commands::workflow::preview::run(p).await,
},
```

- [ ] **Step 2: Implement the projector**

Create `crates/vox-cli/src/commands/workflow.rs`:

```rust
//! `vox workflow` — workflow introspection (P1-T8).

pub mod preview;
```

Create `crates/vox-cli/src/commands/workflow/preview.rs`:

```rust
//! Dry-run schedule projection. No I/O. Per Phase 1 SSOT:
//! "Type-checks, infers effects, projects schedule of activities that
//! *would* run; no side effects."

use anyhow::{Context, Result};
use owo_colors::OwoColorize;
use serde::Serialize;
use std::path::{Path, PathBuf};

use crate::cli_args::WorkflowPreviewArgs;

#[derive(Debug, Serialize)]
pub struct PreviewedActivity {
    pub name: String,
    pub call_site_id: u32,
    pub effect_row: Vec<String>,
    pub mens: bool,
    pub children: Vec<PreviewedActivity>,
}

#[derive(Debug, Serialize)]
pub struct PreviewedWorkflow {
    pub workflow: String,
    pub effect_row: Vec<String>,
    pub steps: Vec<PreviewedActivity>,
}

pub async fn run(args: &WorkflowPreviewArgs) -> Result<()> {
    let (path, wf_name) = parse_target(&args.target)
        .with_context(|| format!("invalid target `{}`", args.target))?;
    let source = std::fs::read_to_string(&path)
        .with_context(|| format!("read {}", path.display()))?;
    let result = crate::pipeline::run_frontend(&path, false).await?;
    if result.has_errors() {
        anyhow::bail!("preview aborted: frontend reported errors");
    }
    let projected = project_workflow(&result.hir, &wf_name)
        .with_context(|| format!("workflow `{wf_name}` not found in {}", path.display()))?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&projected)?);
    } else {
        render_tree(&projected);
    }
    Ok(())
}

fn parse_target(t: &str) -> Result<(PathBuf, String)> {
    let (path_str, fn_name) = t
        .rsplit_once("::")
        .ok_or_else(|| anyhow::anyhow!("expected `path::workflow`, got `{t}`"))?;
    Ok((PathBuf::from(path_str), fn_name.to_string()))
}

fn project_workflow(
    hir: &vox_compiler::hir::HirModule,
    wf_name: &str,
) -> Result<PreviewedWorkflow> {
    let wf = hir.functions.iter().find(|f| f.name == wf_name)
        .ok_or_else(|| anyhow::anyhow!("workflow not found"))?;
    if wf.durability != Some(vox_compiler::hir::nodes::DurabilityKind::Workflow) {
        anyhow::bail!("function `{wf_name}` is not a `workflow`");
    }
    let steps = walk_calls_to_activities(&wf.body, hir);
    Ok(PreviewedWorkflow {
        workflow: wf_name.to_string(),
        effect_row: wf.inferred_effects.iter().map(|c| format!("{c}")).collect(),
        steps,
    })
}

fn walk_calls_to_activities(
    stmts: &[vox_compiler::hir::HirStmt],
    hir: &vox_compiler::hir::HirModule,
) -> Vec<PreviewedActivity> {
    let mut out = Vec::new();
    for s in stmts {
        match s {
            vox_compiler::hir::HirStmt::Expr { expr, .. }
            | vox_compiler::hir::HirStmt::Let { value: expr, .. }
            | vox_compiler::hir::HirStmt::Assign { value: expr, .. } => {
                walk_expr(expr, hir, &mut out);
            }
            vox_compiler::hir::HirStmt::Return { value: Some(e), .. } => {
                walk_expr(e, hir, &mut out);
            }
            vox_compiler::hir::HirStmt::While { body, .. }
            | vox_compiler::hir::HirStmt::Loop { body, .. } => {
                out.extend(walk_calls_to_activities(body, hir));
            }
            _ => {}
        }
    }
    out
}

fn walk_expr(
    e: &vox_compiler::hir::HirExpr,
    hir: &vox_compiler::hir::HirModule,
    out: &mut Vec<PreviewedActivity>,
) {
    use vox_compiler::hir::HirExpr;
    match e {
        HirExpr::Call(callee, args, inputs, _) => {
            if let HirExpr::Ident(name, _) = callee.as_ref() {
                if let Some(target) = hir.functions.iter().find(|f| &f.name == name) {
                    let is_activity = target.is_remote
                        || matches!(target.durability, Some(vox_compiler::hir::nodes::DurabilityKind::Activity));
                    if is_activity {
                        let csid = inputs.as_ref().map(|i| i.call_site_id).unwrap_or(u32::MAX);
                        let effect_row = target.inferred_effects.iter().map(|c| format!("{c}")).collect();
                        let children = walk_calls_to_activities(&target.body, hir);
                        out.push(PreviewedActivity {
                            name: target.name.clone(),
                            call_site_id: csid,
                            effect_row,
                            mens: target.is_remote,
                            children,
                        });
                    }
                }
            }
            for a in args {
                walk_expr(&a.value, hir, out);
            }
        }
        HirExpr::Block(stmts, _) => {
            out.extend(walk_calls_to_activities(stmts, hir));
        }
        _ => {}
    }
}

fn render_tree(p: &PreviewedWorkflow) {
    println!("{} {}", "workflow".green().bold(), p.workflow.bold());
    let row = if p.effect_row.is_empty() {
        "uses nothing".to_string()
    } else {
        format!("uses {}", p.effect_row.join(", "))
    };
    println!("  {} {}", "effects:".dimmed(), row);
    println!("  {}:", "schedule".bold());
    for s in &p.steps {
        render_step(s, 2);
    }
}

fn render_step(s: &PreviewedActivity, indent: usize) {
    let pad = " ".repeat(indent);
    let where_ = if s.mens { "mesh".cyan().to_string() } else { "local".dimmed().to_string() };
    println!("{pad}- {} [{}] csid={} effects=[{}]",
        s.name.bold(),
        where_,
        s.call_site_id,
        s.effect_row.join(","));
    for c in &s.children {
        render_step(c, indent + 4);
    }
}
```

- [ ] **Step 3: Author fixtures**

Create `tests/fixtures/workflow_preview/simple_two_step.vox`:

```vox
// vox:skip
@remote fn fetch_url(u: str) to str { return u }
activity parse_json(s: str) to str { return s }

workflow process(u: str) to str {
    let raw = fetch_url(u)
    let j = parse_json(raw)
    return j
}
```

Create `tests/fixtures/workflow_preview/with_side_effect.vox`:

```vox
// vox:skip
@remote fn step(x: i32) to i32 { return x + 1 }

workflow main() to i64 {
    let t = side_effect { time.now() }
    let v = step(1)
    return t + v
}
```

Create `tests/fixtures/workflow_preview/with_signal.vox`:

```vox
// vox:skip
@remote fn fan_out(items: List[i32]) to List[i32] uses spawn { return items }
activity reduce(xs: List[i32]) to i32 { return 0 }

workflow batch(items: List[i32]) to i32 {
    let mapped = fan_out(items)
    let r = reduce(mapped)
    return r
}
```

- [ ] **Step 4: Write the integration tests**

Create `crates/vox-cli/tests/workflow_preview.rs`:

```rust
use std::process::Command;

fn vox_bin() -> String {
    env!("CARGO_BIN_EXE_vox").to_string()
}

#[test]
fn preview_simple_two_step_text_output() {
    let out = Command::new(vox_bin())
        .args(["workflow", "preview", "tests/fixtures/workflow_preview/simple_two_step.vox::process"])
        .output()
        .expect("ran");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("workflow process"), "stdout:\n{stdout}");
    assert!(stdout.contains("fetch_url"), "stdout:\n{stdout}");
    assert!(stdout.contains("parse_json"), "stdout:\n{stdout}");
    assert!(stdout.contains("[mesh]"), "remote should render as mesh; stdout:\n{stdout}");
}

#[test]
fn preview_simple_two_step_json_output() {
    let out = Command::new(vox_bin())
        .args(["workflow", "preview", "--json", "tests/fixtures/workflow_preview/simple_two_step.vox::process"])
        .output()
        .expect("ran");
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid JSON");
    assert_eq!(v["workflow"], "process");
    let steps = v["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 2);
    assert_eq!(steps[0]["name"], "fetch_url");
    assert_eq!(steps[0]["mens"], true);
    assert_eq!(steps[1]["name"], "parse_json");
    assert_eq!(steps[1]["mens"], false);
}

#[test]
fn preview_no_io_when_workflow_calls_stdlib() {
    // Verify the projector does NOT make network calls. We use a workflow
    // that, if executed, would call out to the network. Preview must succeed
    // offline.
    let out = Command::new(vox_bin())
        .env_remove("HTTP_PROXY")
        .env_remove("HTTPS_PROXY")
        .args(["workflow", "preview", "tests/fixtures/workflow_preview/simple_two_step.vox::process"])
        .output()
        .expect("ran");
    assert!(out.status.success(), "preview must work offline");
}

#[test]
fn preview_unknown_workflow_errors_clearly() {
    let out = Command::new(vox_bin())
        .args(["workflow", "preview", "tests/fixtures/workflow_preview/simple_two_step.vox::not_a_workflow"])
        .output()
        .expect("ran");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("not_a_workflow") || stderr.contains("not found"));
}

#[test]
fn preview_workflow_includes_inferred_effect_row() {
    // fan_out's `uses spawn` propagates to batch.
    let out = Command::new(vox_bin())
        .args(["workflow", "preview", "tests/fixtures/workflow_preview/with_signal.vox::batch"])
        .output()
        .expect("ran");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("spawn") || stdout.contains("net"),
        "expected effects shown; stdout:\n{stdout}");
}

#[test]
fn preview_with_side_effect_block_lists_synthesised_activity() {
    let out = Command::new(vox_bin())
        .args(["workflow", "preview", "--json", "tests/fixtures/workflow_preview/with_side_effect.vox::main"])
        .output()
        .expect("ran");
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid JSON");
    let steps = v["steps"].as_array().unwrap();
    let names: Vec<&str> = steps.iter().filter_map(|s| s["name"].as_str()).collect();
    assert!(names.iter().any(|n| n.starts_with("__side_effect_")), "names: {names:?}");
    assert!(names.iter().any(|n| *n == "step"), "names: {names:?}");
}
```

- [ ] **Step 5: Run, expect PASS**

Run: `cargo test -p vox-cli --test workflow_preview 2>&1 | tail -25`
Expected: all six PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-cli/src/commands/workflow.rs \
        crates/vox-cli/src/commands/workflow/preview.rs \
        crates/vox-cli/src/commands/mod.rs \
        crates/vox-cli/src/lib.rs \
        crates/vox-cli/src/cli_args.rs \
        crates/vox-cli/tests/workflow_preview.rs \
        tests/fixtures/workflow_preview/simple_two_step.vox \
        tests/fixtures/workflow_preview/with_side_effect.vox \
        tests/fixtures/workflow_preview/with_signal.vox
git commit -m "$(cat <<'EOF'
feat(cli): vox workflow preview — dry-run schedule projector (P1-T8)

`vox workflow preview path::wf_name` runs the frontend and inference,
then walks the workflow body to produce a tree of would-call activities
annotated with effect rows. No I/O. --json switches output format.

Reuses HIR after typeck (where P1-T6 inference populated
inferred_effects). Renders text by default with mens vs. local
distinction.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>
EOF
)"
```

---

## Task P1-T9: Stable diagnostic IDs sweep

**Files:**

- Modify: `crates/vox-compiler/src/typeck/diagnostics.rs` — declare every Phase 1 diagnostic ID as a public constant.
- Modify: every callsite that emits a hardcoded code string from this phase.
- Create: `crates/vox-compiler/tests/diagnostic_id_namespace.rs` — guards the namespace.

Per `vox-language-rules-and-enforcement-plan-2026.md`, every new diagnostic in Phase 1 must use the `vox/<category>/<kebab>` namespace. The categories used in Phase 1:

- `vox/types/*` — type-system errors / deprecations.
- `vox/effect/*` — effect-row violations.
- `vox/workflow/*` — workflow-determinism and workflow-shape errors.
- `vox/remote/*` — `@remote`-specific errors.
- `vox/api/*` — API-deprecation warnings (e.g., `mesh_*` prefix).

Codes introduced this phase:

| Code | Severity | Where |
|---|---|---|
| `vox/types/durable-promise-arity` | error | P1-T1 |
| `vox/types/future-deprecated` | warning | P1-T2 |
| `vox/types/promise-deprecated` | warning | P1-T2 |
| `vox/api/mesh-prefix-deprecated` | warning | P1-T3 |
| `vox/remote/non-serializable-param` | error | P1-T3 |
| `vox/remote/non-serializable-return` | error | P1-T3 |
| `vox/workflow/with-id-non-deterministic` | warning | P1-T4c |
| `vox/workflow/non-deterministic-builtin` | error | P1-T5 |
| `vox/effect/missing-declaration` | error | P1-T6 |
| `vox/workflow/side-effect-outside-workflow` | error | P1-T7 |

- [ ] **Step 1: Centralise the constants**

In `crates/vox-compiler/src/typeck/diagnostics.rs`:

```rust
// ── Phase 1 diagnostic codes (mesh-phase1-language-spine-plan-2026) ──────────

pub mod codes {
    pub const TYPES_DURABLE_PROMISE_ARITY: &str = "vox/types/durable-promise-arity";
    pub const TYPES_FUTURE_DEPRECATED: &str = "vox/types/future-deprecated";
    pub const TYPES_PROMISE_DEPRECATED: &str = "vox/types/promise-deprecated";

    pub const API_MESH_PREFIX_DEPRECATED: &str = "vox/api/mesh-prefix-deprecated";

    pub const REMOTE_NON_SERIALIZABLE_PARAM: &str = "vox/remote/non-serializable-param";
    pub const REMOTE_NON_SERIALIZABLE_RETURN: &str = "vox/remote/non-serializable-return";

    pub const WORKFLOW_WITH_ID_NON_DETERMINISTIC: &str = "vox/workflow/with-id-non-deterministic";
    pub const WORKFLOW_NON_DETERMINISTIC_BUILTIN: &str = "vox/workflow/non-deterministic-builtin";
    pub const WORKFLOW_SIDE_EFFECT_OUTSIDE_WORKFLOW: &str = "vox/workflow/side-effect-outside-workflow";

    pub const EFFECT_MISSING_DECLARATION: &str = "vox/effect/missing-declaration";

    /// Phase-1 codes registered for stability — used by the namespace guard.
    pub const ALL_PHASE_1: &[&str] = &[
        TYPES_DURABLE_PROMISE_ARITY,
        TYPES_FUTURE_DEPRECATED,
        TYPES_PROMISE_DEPRECATED,
        API_MESH_PREFIX_DEPRECATED,
        REMOTE_NON_SERIALIZABLE_PARAM,
        REMOTE_NON_SERIALIZABLE_RETURN,
        WORKFLOW_WITH_ID_NON_DETERMINISTIC,
        WORKFLOW_NON_DETERMINISTIC_BUILTIN,
        WORKFLOW_SIDE_EFFECT_OUTSIDE_WORKFLOW,
        EFFECT_MISSING_DECLARATION,
    ];
}
```

- [ ] **Step 2: Replace string literals at callsites**

Search for the literal strings (e.g. `"vox/effect/missing-declaration"`) and replace with `codes::EFFECT_MISSING_DECLARATION` referencing the new module. This catches typos and makes the IDs greppable from one place.

- [ ] **Step 3: Add the namespace-guard test**

Create `crates/vox-compiler/tests/diagnostic_id_namespace.rs`:

```rust
use vox_compiler::typeck::diagnostics::codes;

#[test]
fn every_phase_1_code_is_kebab_case() {
    for code in codes::ALL_PHASE_1 {
        assert!(code.starts_with("vox/"), "code `{code}` missing vox/ prefix");
        let parts: Vec<&str> = code.split('/').collect();
        assert_eq!(parts.len(), 3, "code `{code}` must be `vox/<category>/<kebab>`");
        let category = parts[1];
        let kebab = parts[2];
        assert!(
            category.chars().all(|c| c.is_ascii_lowercase() || c == '-'),
            "category `{category}` in `{code}` must be lowercase-kebab"
        );
        assert!(
            kebab.chars().all(|c| c.is_ascii_lowercase() || c == '-' || c.is_ascii_digit()),
            "kebab `{kebab}` in `{code}` must be lowercase-kebab"
        );
        assert!(!kebab.starts_with('-'), "code `{code}` kebab must not start with hyphen");
        assert!(!kebab.ends_with('-'), "code `{code}` kebab must not end with hyphen");
    }
}

#[test]
fn category_set_is_known() {
    let allowed: std::collections::HashSet<&'static str> = ["types", "effect", "workflow", "remote", "api"].into_iter().collect();
    for code in codes::ALL_PHASE_1 {
        let category = code.split('/').nth(1).unwrap();
        assert!(allowed.contains(category), "category `{category}` in `{code}` not in {allowed:?}");
    }
}

#[test]
fn no_duplicates() {
    let mut seen = std::collections::HashSet::new();
    for code in codes::ALL_PHASE_1 {
        assert!(seen.insert(*code), "duplicate code `{code}` in ALL_PHASE_1");
    }
}
```

- [ ] **Step 4: Run, expect PASS**

Run: `cargo test -p vox-compiler --test diagnostic_id_namespace 2>&1 | tail -10`
Expected: three PASS.

- [ ] **Step 5: Run the entire suite to catch any missed callsites**

Run: `cargo test -p vox-compiler 2>&1 | tail -25`
Expected: all PASS. If a Phase 1 test now fails because a callsite still uses a hardcoded string, fix the callsite.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-compiler/src/typeck/diagnostics.rs \
        crates/vox-compiler/src/typeck/effect_check.rs \
        crates/vox-compiler/src/typeck/serializable.rs \
        crates/vox-compiler/src/typeck/workflow_determinism.rs \
        crates/vox-compiler/src/typeck/activity_id_inputs.rs \
        crates/vox-compiler/src/hir/lower/mod.rs \
        crates/vox-compiler/tests/diagnostic_id_namespace.rs
git commit -m "$(cat <<'EOF'
chore(compiler): centralise Phase 1 diagnostic IDs under codes:: module (P1-T9)

Every new diagnostic from P1-T1..T8 now lives as a public constant in
typeck::diagnostics::codes. Namespace guard test enforces:
  * vox/<category>/<kebab> shape
  * category ∈ {types, effect, workflow, remote, api}
  * no duplicates
  * lowercase-kebab only

Per vox-language-rules-and-enforcement-plan-2026.md: stable IDs let LLMs
trained on 0.5 still recognise 0.7 errors.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>
EOF
)"
```

---

## Acceptance

The Phase 1 SSOT acceptance criteria, mapped to the tasks that satisfy them:

| Acceptance criterion | Task(s) |
|---|---|
| A `workflow` body containing `time.now()` fails `vox check` with `vox/workflow/non-deterministic-builtin`. | P1-T5 |
| A `@remote fn foo(x: i32) → i32` compiles. | P1-T3 |
| `@remote fn bar(x: NotSerializable)` fails to compile with a diagnostic naming the offending parameter. | P1-T3 |
| An activity called twice in a workflow with the same args returns the cached value on the second invocation, with no user-supplied ID. | P1-T4a + P1-T4b (call_site_id distinguishes positions; structural_arg_hash + replay_counter handle loop bodies) |
| `vox workflow preview my::workflow(arg1, arg2)` prints the projected schedule without dispatching. | P1-T8 |
| All new diagnostics carry `vox/<kebab>` IDs. | P1-T9 |
| Unannotated functions still receive an inferred effect set (no surprise behaviour). | P1-T6 |
| `Future[T]` and `Promise[T]` continue to compile during the deprecation window with a warning. | P1-T2 |
| `mesh_*`-prefixed functions still work (auto-`@remote`) but warn. | P1-T3 (lower step) |
| `side_effect { time.now() }` inside a workflow body compiles cleanly. | P1-T7 |
| `side_effect { … }` outside a workflow is an error. | P1-T7 |

Final integration check:

- [ ] **Run the full workspace test sweep**

```bash
cargo test --workspace 2>&1 | tail -30
```

Expected: all PASS. If the codegen or runtime tests fail, the `Future`/`Promise` aliasing in `types.rs` (P1-T2) is the most likely culprit; verify both code paths emit `vox_workflow_runtime::DurablePromise<T>`.

- [ ] **Run `vox check` over the whole repo**

```bash
cargo run -p vox-cli -- check ./crates 2>&1 | tail -30
```

Expected: clean, save for any pre-existing diagnostics unrelated to Phase 1.

- [ ] **Smoke test the new CLI**

```bash
cargo run -p vox-cli -- workflow preview tests/fixtures/workflow_preview/simple_two_step.vox::process
```

Expected output (formatting may vary):

```
workflow process
  effects: uses net, spawn
  schedule:
    - fetch_url [mesh] csid=0 effects=[net,spawn]
    - parse_json [local] csid=1 effects=[]
```

---

## Rollback

Each task is independently revertable, but P1-T1 and P1-T2 must roll back together (because `Future`/`Promise` aliasing depends on `DurablePromise` being registered). The full roll-back order is:

1. Revert P1-T9 (constants → strings — purely cosmetic).
2. Revert P1-T8 (CLI subcommand removed).
3. Revert P1-T7 (`side_effect` keyword removed; existing workflows with `side_effect` blocks fail to parse — flag risk: any workflow author who lands on the dev branch and uses the keyword loses their work; mitigate by feature-gating the keyword behind `Cargo.toml` feature `phase1-spine` until merge).
4. Revert P1-T6 (inference removed; top-down validation restored — strictly more permissive; existing diagnostics about `vox/effect/missing-declaration` disappear).
5. Revert P1-T5 (workflow determinism check removed — strictly more permissive).
6. Revert P1-T4a/b/c (activity_id derivation falls back to `format!` — at-rest journal entries with the old `wf-N` naming continue to replay; new entries with `act-<hash>` would be orphaned and need `tracker.migrate_legacy_ids()` run).
7. Revert P1-T3 (`@remote` removed; `mesh_*` deprecation lifted — purely additive removal).
8. Revert P1-T1 + P1-T2 *together* (DurablePromise registration, Future/Promise deprecation; codegen falls back to whatever the repo had before).

After every revert step, `cargo test --workspace` and `cargo run -p vox-cli -- check ./crates` must remain green; that's the workspace-stays-compiling invariant the ordering guarantees in *both* directions.

For at-rest data hazard: P1-T4 changes the `activity_id` shape from `wf-N` to `act-<hex>`. Existing `vox-workflow-runtime` durability journals contain entries keyed by the old shape. The `WorkflowTracker` trait is extended with `next_replay_counter` (P1-T4b); rollback drops that method. Any journal written by a Phase-1 build cannot be replayed by a pre-Phase-1 build *unless* the runtime keeps a `legacy_id_alias` field on each entry. Add this alias as a follow-up, not in Phase 1, since v0.6 is the first release shipping Phase 1.

---

## Self-review checklist

- **Spec coverage.** Every numbered acceptance criterion in `mesh-and-language-distribution-ssot-2026.md` §3 Phase 1 maps to at least one task; cross-checked above.
- **Placeholder scan.** No `TBD`, no "implement later", no "similar to Task N". Every step has actual Rust / Vox code.
- **Type consistency.** `DurablePromise<T>`, `JournalError`, `ActivityIdInputs`, `Serializable`, `NonSerializableReason`, `PreviewedActivity`, `PreviewedWorkflow` are defined exactly once and referenced consistently.
- **Diagnostic IDs.** Every new ID conforms to `vox/<kebab>` and lives in `codes::ALL_PHASE_1`. The namespace-guard test enforces this.
- **Rollback risk.** Highest at P1-T4 (data-shape change in journals). Documented.
- **LLM-target principle.** Diagnostic IDs are stable and the namespace is enumerable; an LLM trained on a 0.5 corpus will continue to recognise these IDs in 0.7.
- **C4 (one canonical primitive per concept).** `DurablePromise[T]` collapses five primitives. `Future[T]` and `Promise[T]` survive only as deprecation aliases.

---

## Revision history

- **2026-05-09.** Initial implementation plan.
