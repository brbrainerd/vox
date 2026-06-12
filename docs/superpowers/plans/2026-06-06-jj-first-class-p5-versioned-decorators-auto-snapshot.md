# P5 — `@versioned` / `@tracked` Decorators + Auto-Snapshot-on-Effect Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. TDD-first: every behavioral task starts with a failing test before any implementation code.

**Goal:** Add a `@versioned` / `@tracked` function decorator that auto-checkpoints the interpreter's
`RepoStore` at the function's effect boundary — i.e. a `repo.snapshot(<auto-label>)` is recorded
automatically when a `@versioned` function returns successfully — so that time-travel/undo becomes a
*language guarantee* rather than a hand-written `repo.snapshot()` discipline. This is the
decorator/auto-snapshot layer of the VCS-as-language-feature initiative, building directly on the
already-landed P3 `repo.*` primitive (`eval/repo.rs`) and `Vcs` effect.

**Architecture:** Mirror the **existing bare-token decorator mechanism** end-to-end. `@pure` and
`@reactive` are the exact precedent: a lexer token (`AtPure`/`AtReactive`,
`crates/vox-compiler/src/lexer/token.rs:193-199`), a parse-loop arm in `parse_fn_decl`
(`parser/descent/decl/head.rs:1194-1201`) that flips a `bool` local (`is_pure`/`is_reactive`), a
`bool` field on `FnDecl` (`vox-ast/src/decl/fundecl.rs:43-51`), an identical field on `HirFn`
(`hir/nodes/decl.rs:313-320`), and a one-line copy in lowering (`hir/lower/decl.rs:56-57`). P5 adds
`is_versioned` along that same spine.

The **one genuinely new capability** P5 must add is at the *interpreter value* layer:
`VoxValue::Fn { params, body, env }` (`eval/value.rs:24-28`) carries no decorator metadata and no
name, and the two call sites that invoke a user function (`eval/expr.rs:392-419` and
`eval/mod.rs:483-521`) destructure exactly those three fields. The decorator therefore **cannot be
observed at call time today** — so Task 3 extends `VoxValue::Fn` with an `is_versioned: bool` (and the
function `name`, used to build the auto-label), threaded from `run_module` registration
(`eval/mod.rs:299-307`). This is the "if the mechanism can't express something needed, ADD that
capability (with a test)" task the scope demands; it is flagged honestly rather than faked.

Trigger semantics (per design §4.3, "auto-`repo.snapshot()` at the effect boundary"): **on
successful return of a `@versioned` function**, the interpreter calls
`interp.repo.snapshot(Some("@versioned <fn_name>"))` exactly once. Snapshot-on-success (not on entry,
not before each mutation) is chosen because (a) the `RepoStore` snapshot is a logical change marker,
not a pre-image, and (b) a function that errors should not leave a spurious checkpoint. Errors
short-circuit (the `?` in the call loop returns before the snapshot hook), so no checkpoint is
recorded for a failed `@versioned` call — asserted by test.

**`@versioned` implies `uses vcs`.** Because the auto-snapshot performs a `Vcs`-effecting operation,
a `@versioned` function is given the `Vcs` capability at lowering time (Task 4) so the effect checker
(`typeck/effect_check.rs`) treats it consistently with an explicit `uses vcs` clause and propagation
stays sound. Specified and tested.

**Tech Stack:** Rust; the Vox compiler crate (`vox-compiler`: lexer/parser/AST/HIR/typeck/eval); the
existing `RepoStore` (`eval/repo.rs`). No new crates, no `jj-lib`, no async — P5 is entirely within the
`--mode interp` tree-walking interpreter and the compiler front-end.

**Source spec:** [`vcs-as-vox-language-feature-jujutsu-2026.md`](../../src/architecture/vcs-as-vox-language-feature-jujutsu-2026.md) §4.3 (automatic snapshotting via decorators + effect boundaries), §4.2 (the `Vcs` effect), §4.4 (the interpreter `RepoStore`), §6 (P4 row, here delivered as P5).
**Depends on:** P3 (the `repo.*` primitive + `RepoStore` at `eval/repo.rs`; the `Vcs`
`EffectAnnotation`/`HirCapability`/`HirEffectKind`; `stdlib_module_capability("repo") => Vcs`). All
landed and merged. Independent of P4 (orchestrator isolation/GUI).

---

## File Structure

| File | Responsibility |
|---|---|
| Modify `crates/vox-compiler/src/lexer/token.rs` | Add `#[token("@versioned")] AtVersioned` + `#[token("@tracked")] AtTracked` (alias) + their `Display` arms |
| Modify `crates/vox-compiler/src/parser/descent/decl/head.rs` | `is_versioned` local + parse-loop arm (mirror the `AtPure`/`AtReactive` arms); set field in the `FnDecl {…}` constructor |
| Modify `crates/vox-ast/src/decl/fundecl.rs` | Add `#[serde(default)] pub is_versioned: bool` to `FnDecl` |
| Modify `crates/vox-compiler/src/hir/nodes/decl.rs` | Add `#[serde(default)] pub is_versioned: bool` to `HirFn` |
| Modify `crates/vox-compiler/src/hir/lower/decl.rs` | Copy `is_versioned: f.is_versioned`; inject `HirCapability::Vcs` into `capabilities` when `is_versioned` |
| Modify `crates/vox-compiler/src/eval/value.rs` | Extend `VoxValue::Fn` with `name: String` + `is_versioned: bool` |
| Modify `crates/vox-compiler/src/eval/mod.rs` | Populate the two new `Fn` fields at registration (`run_module`); fix the `call()` destructure + add the snapshot-on-success hook |
| Modify `crates/vox-compiler/src/eval/expr.rs` | Fix every `VoxValue::Fn { … }` destructure (Lambda build, Call, method dispatch, helpers) + add the snapshot-on-success hook to the `Call` arm |
| Create `examples/golden/repo_versioned_decorator.vox` | Golden exercising `@versioned` auto-snapshot, runnable under `--mode interp` |
| Modify any other `VoxValue::Fn { … }` / `FnDecl {…}` / `HirFn {…}` literal | Mechanical field-add fallout (test harness builders, json_as lowering, fixtures) — found by `cargo build` |

> **Field-fallout note (read before Task 3):** `VoxValue::Fn { params, body, env }` is pattern-matched
> and constructed in several places — confirmed sites: `eval/expr.rs:56`, `:367`, `:392`, `:741`,
> `eval/mod.rs:300`, `:310`, `:326`, `:426`, `:483`. Adding fields is a **breaking change to the
> variant**; the compiler will list every site. `FnDecl`/`HirFn` literal sites that need the new
> `is_versioned` field (each defaulting to `false`) include the test-harness builder
> (`vox-test-harness/src/hir_builders.rs:36` neighborhood), `typeck/cuda_gate.rs:78`,
> `hir/lower/json_as.rs`, `hir/lower/expr.rs:248`, `parser/descent/mod.rs:333`, and the metamorphic
> test builder — all surfaced by `cargo build -p vox-compiler` / `-p vox-test-harness`. Use
> `#[serde(default)]` on the struct fields so existing serialized fixtures keep deserializing.

---

### Task 1: `@versioned` / `@tracked` lexer tokens (AST decorator surface, step 1)

**Files:** Modify `crates/vox-compiler/src/lexer/token.rs`.

- [ ] **Step 1: Write the failing lexer test.** Add to the lexer test module (mirror the existing
  `@pure`/`@reactive` token tests):
```rust
#[test]
fn lexes_versioned_and_tracked_decorators() {
    use crate::lexer::{lex, token::Token};
    let toks: Vec<Token> = lex("@versioned @tracked").into_iter().map(|t| t.token).collect();
    assert!(toks.contains(&Token::AtVersioned), "expected AtVersioned, got {toks:?}");
    assert!(toks.contains(&Token::AtTracked),   "expected AtTracked, got {toks:?}");
}
```
  (Adjust the `lex(...)` element-access shape to match how the existing lexer tests read the token —
  confirm against a neighbouring `@pure` test in `token.rs` / `lexer/`.)

- [ ] **Step 2: Run → FAIL.** `cargo test -p vox-compiler lexes_versioned` → `AtVersioned` undefined.

- [ ] **Step 3: Add the tokens.** In `token.rs`, after `AtReactive` (line 199), add:
```rust
/// `@versioned` — marks a function whose successful return auto-records a
/// `repo.snapshot()` checkpoint in the interpreter's RepoStore (design §4.3).
/// Implies `uses vcs`. `@tracked` is an accepted spelling alias.
#[token("@versioned")]
AtVersioned,
#[token("@tracked")]
AtTracked,
```
  Add the matching `Display` arms next to `Token::AtReactive => write!(f, "@reactive")` (token.rs:597):
```rust
Token::AtVersioned => write!(f, "@versioned"),
Token::AtTracked => write!(f, "@tracked"),
```

- [ ] **Step 4: Run → PASS.** `cargo test -p vox-compiler lexes_versioned`.

- [ ] **Step 5: Commit** `feat(lexer): @versioned/@tracked decorator tokens`.

---

### Task 2: Parse `@versioned`/`@tracked` into `FnDecl.is_versioned` (AST)

**Files:** Modify `crates/vox-ast/src/decl/fundecl.rs`; Modify
`crates/vox-compiler/src/parser/descent/decl/head.rs`.

- [ ] **Step 1: Add the AST field (no behavior yet).** In `vox-ast/src/decl/fundecl.rs`, in `FnDecl`,
  next to `is_reactive` (line 51):
```rust
/// `@versioned` / `@tracked` — auto-checkpoint this function's successful
/// return into the interpreter RepoStore (design §4.3). Implies `uses vcs`.
#[serde(default)]
pub is_versioned: bool,
```
  This breaks every `FnDecl { … }` literal; add `is_versioned: false` to each (build will list them —
  see the Field-fallout note). Build `-p vox-ast -p vox-compiler` to green the struct change first.

- [ ] **Step 2: Failing parser test.** Add to the `head.rs` parser tests (mirror an existing
  `@pure`/`@reactive` parse test):
```rust
#[test]
fn parses_versioned_decorator_sets_flag() {
    let src = "@versioned fn save() uses vcs { repo.snapshot(\"x\") }";
    let module = crate::parser::parse_script(crate::lexer::lex(src)).expect("parse");
    let f = module.functions.iter().find(|f| f.name == "save").expect("fn save");
    assert!(f.is_versioned, "@versioned must set FnDecl.is_versioned");
}

#[test]
fn parses_tracked_alias_sets_same_flag() {
    let src = "@tracked fn save() uses vcs { repo.snapshot(\"x\") }";
    let module = crate::parser::parse_script(crate::lexer::lex(src)).expect("parse");
    assert!(module.functions.iter().find(|f| f.name == "save").unwrap().is_versioned);
}
```
  (Confirm `parse_script` + `module.functions` accessors against the existing parser tests in
  `head.rs`; the golden runner uses `parse` while `run_interp` uses `parse_script` — use whichever the
  neighbouring tests use.)

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-compiler parses_versioned` → field is always `false`.

- [ ] **Step 4: Implement the parse arm.** In `parse_fn_decl` (`head.rs`): add the local next to
  `let mut is_reactive = false;` (line 1118):
```rust
let mut is_versioned = false;
```
  Add the parse-loop arm next to the `Token::AtReactive` arm (lines 1198-1201):
```rust
Token::AtVersioned | Token::AtTracked => {
    self.advance();
    is_versioned = true;
}
```
  Set the field in the `FnDecl { … }` constructor next to `is_reactive,` (line 2101):
```rust
is_versioned,
```

- [ ] **Step 5: Run → PASS.** `cargo test -p vox-compiler parses_versioned parses_tracked`.

- [ ] **Step 6: Commit** `feat(parser): parse @versioned/@tracked into FnDecl.is_versioned`.

---

### Task 3: Lower `is_versioned` into `HirFn`; thread it onto `VoxValue::Fn`

**This is the new-capability task** — `VoxValue::Fn` currently cannot carry decorator metadata, so the
auto-snapshot hook (Task 5) has nothing to observe. Here we extend the value and the HIR.

**Files:** Modify `crates/vox-compiler/src/hir/nodes/decl.rs`; Modify
`crates/vox-compiler/src/hir/lower/decl.rs`; Modify `crates/vox-compiler/src/eval/value.rs`; Modify
`crates/vox-compiler/src/eval/mod.rs` (registration only); plus mechanical fallout.

- [ ] **Step 1: Add the HIR field.** In `hir/nodes/decl.rs`, next to `is_reactive` (line 320):
```rust
/// `@versioned` / `@tracked` — see `FnDecl::is_versioned`. Interpreter
/// auto-snapshots on this function's successful return.
#[serde(default)]
pub is_versioned: bool,
```
  Copy it in lowering: in `hir/lower/decl.rs` next to `is_reactive: f.is_reactive,` (line 57):
```rust
is_versioned: f.is_versioned,
```
  Build `-p vox-compiler` and fix `HirFn { … }` literal fallout (default `false`).

- [ ] **Step 2: Failing test for the value carrying the flag.** Add to the `eval` tests (e.g. a new
  test near `run_module`) — assert that after `run_module`, a `@versioned fn` is registered as a
  `VoxValue::Fn` whose `is_versioned` is `true`:
```rust
#[test]
fn versioned_fn_value_carries_flag() {
    let src = "@versioned fn save() uses vcs { repo.snapshot(\"x\") }";
    let module = crate::hir::lower::lower_module(
        &crate::parser::parse_script(crate::lexer::lex(src)).unwrap());
    let mut interp = Interpreter::new(10_000);
    interp.run_module(&module).unwrap();
    match interp.scope.get("save") {
        Some(VoxValue::Fn { is_versioned, name, .. }) => {
            assert!(*is_versioned, "registered fn must carry is_versioned");
            assert_eq!(name, "save");
        }
        other => panic!("expected versioned Fn, got {other:?}"),
    }
}
```

- [ ] **Step 3: Run → FAIL.** `VoxValue::Fn` has no `is_versioned`/`name` fields.

- [ ] **Step 4: Extend `VoxValue::Fn`.** In `eval/value.rs` (lines 24-28):
```rust
Fn {
    params: Vec<String>,
    body: Vec<HirStmt>,
    env: crate::eval::env::Scope,
    /// Function name (for the auto-snapshot label); "" for anonymous lambdas.
    name: String,
    /// `@versioned`/`@tracked` — auto-snapshot on successful return.
    is_versioned: bool,
},
```
  Now `cargo build -p vox-compiler` lists every construct/destructure site. Fix each:
  - **Constructors** (`eval/mod.rs:300,310,326,426`, `eval/expr.rs:56,367`): set
    `name: f.name.clone()` (or `String::new()` for the Lambda literal at `expr.rs:56`/`:367`) and
    `is_versioned: f.is_versioned` (or `false` for lambdas — anonymous closures are never
    `@versioned`). The two registration loops in `run_module` (functions + endpoint_fns) read from
    `&HirFn`, so `f.name`/`f.is_versioned` are in scope.
  - **Destructures** (`eval/expr.rs:392`, `:741`, `eval/mod.rs:483`): add `, ..` or bind the two new
    fields as needed (Task 5 binds them in the call arms).
  - Type-name helpers (`builtins.rs:2441,2545`) use `Fn { .. }` already — no change.

- [ ] **Step 5: Run → PASS.** `cargo test -p vox-compiler versioned_fn_value_carries_flag` and
  `cargo build -p vox-compiler -p vox-test-harness` (fix any harness `Fn`/`HirFn`/`FnDecl` builder
  fallout, defaulting the new fields).

- [ ] **Step 6: Commit** `feat(eval/hir): thread is_versioned onto HirFn + VoxValue::Fn`.

---

### Task 4: `@versioned` implies `uses vcs` (effect interaction) + typeck

**Files:** Modify `crates/vox-compiler/src/hir/lower/decl.rs` (capability injection).

- [ ] **Step 1: Failing effect-check test.** A `@versioned` function that does NOT declare `uses vcs`
  but performs a `repo.*` call must still type-check cleanly (the decorator grants the capability),
  AND a function that is `@versioned` must carry `HirCapability::Vcs`. Add to the `effect_check` /
  lowering tests:
```rust
#[test]
fn versioned_decorator_grants_vcs_capability() {
    // No explicit `uses vcs` clause — the decorator supplies it.
    let src = "@versioned fn save() { repo.snapshot(\"x\") }";
    let hir = crate::hir::lower::lower_module(
        &crate::parser::parse_script(crate::lexer::lex(src)).unwrap());
    let f = hir.functions.iter().find(|f| f.name == "save").unwrap();
    assert!(f.capabilities.contains(&crate::hir::HirCapability::Vcs),
        "@versioned must imply uses vcs; caps = {:?}", f.capabilities);
    // And the effect checker must report no E_EFFECT violation for the bare repo.* call.
    let diags = crate::typeck::effect_check::check_effect_compliance(&hir, src);
    assert!(diags.iter().all(|d| !format!("{d:?}").contains("E_EFFECT")),
        "no effect violation expected, got {diags:?}");
}
```

- [ ] **Step 2: Run → FAIL.** Lowering does not inject `Vcs` from the decorator.

- [ ] **Step 3: Inject the capability in lowering.** In `hir/lower/decl.rs`, where `capabilities` is
  built from `f.effects` (the `EffectAnnotation → HirCapability` map at lines 24-41), append `Vcs`
  when `f.is_versioned` and it is not already present. Concretely, after the `.collect()` that
  produces the capability vec passed into `HirFn`, add a guarded `push`:
```rust
// @versioned/@tracked implies `uses vcs` so the auto-snapshot hook's
// Vcs effect is governed consistently with an explicit clause (design §4.2).
if f.is_versioned && !capabilities.contains(&crate::hir::HirCapability::Vcs) {
    capabilities.push(crate::hir::HirCapability::Vcs);
}
```
  (Bind the collected vec to a `let mut capabilities = …;` local first if it is currently inlined into
  the `HirFn { … }` literal.) **Important interaction:** the effect checker only enforces propagation
  for *annotated* callers (`is_annotated` = non-empty `capabilities` or `is_pure`,
  `effect_check.rs:104`). Injecting `Vcs` makes a bare `@versioned fn` annotated, so it now requires
  its *own* callees' effects to be covered — verify in Step 1's test that a pure-body `@versioned fn`
  does not spuriously fail. If a `@versioned fn` legitimately needs other effects (e.g. `uses fs`),
  the author still declares them; the decorator only adds `Vcs`.

- [ ] **Step 4: Run → PASS.** `cargo test -p vox-compiler versioned_decorator_grants_vcs`.

- [ ] **Step 5: Commit** `feat(hir): @versioned implies uses vcs (capability injection)`.

---

### Task 5: Interpreter auto-snapshot hook on `@versioned` successful return

**The heart of P5.** When a `@versioned` user function returns successfully, the interpreter records
one `repo.snapshot(Some("@versioned <name>"))` via the existing `RepoStore`. Errors short-circuit and
record nothing.

**Files:** Modify `crates/vox-compiler/src/eval/expr.rs` (the `Call` arm, lines 391-419); Modify
`crates/vox-compiler/src/eval/mod.rs` (the `call()` method, lines 483-521).

- [ ] **Step 1: Failing behavioral tests** (the core acceptance tests). Add to the `eval` test module:
```rust
fn interp_for(src: &str) -> Interpreter {
    let module = crate::hir::lower::lower_module(
        &crate::parser::parse_script(crate::lexer::lex(src)).unwrap());
    let mut interp = Interpreter::new(1_000_000);
    interp.run_module(&module).unwrap();
    interp
}

#[test]
fn versioned_fn_auto_snapshots_on_success() {
    // The body does NO explicit repo.snapshot — the decorator supplies it.
    let mut interp = interp_for("@versioned fn save() { let x = 1 }\nfn main() { save() }");
    interp.call("main", vec![]).unwrap();
    let changes = interp.repo.changes();
    assert_eq!(changes.len(), 1, "exactly one auto-snapshot per @versioned call");
    assert_eq!(changes[0].label.as_deref(), Some("@versioned save"));
}

#[test]
fn non_versioned_fn_does_not_auto_snapshot() {
    let mut interp = interp_for("fn save() { let x = 1 }\nfn main() { save() }");
    interp.call("main", vec![]).unwrap();
    assert!(interp.repo.changes().is_empty(), "no decorator → no auto-snapshot");
}

#[test]
fn versioned_fn_error_records_no_snapshot() {
    // assert(false) raises; the snapshot hook is after the body loop and must be skipped.
    let mut interp = interp_for("@versioned fn save() { assert(false) }\nfn main() { save() }");
    let _ = interp.call("main", vec![]); // expected Err
    assert!(interp.repo.changes().is_empty(),
        "a failing @versioned call must not leave a checkpoint");
}

#[test]
fn nested_versioned_calls_each_snapshot_once() {
    let mut interp = interp_for(
        "@versioned fn inner() { let x = 1 }\n\
         @versioned fn outer() { inner() }\n\
         fn main() { outer() }");
    interp.call("main", vec![]).unwrap();
    let labels: Vec<_> = interp.repo.changes().iter()
        .map(|c| c.label.clone().unwrap_or_default()).collect();
    assert_eq!(labels, vec!["@versioned inner".to_string(), "@versioned outer".to_string()],
        "inner snapshots before outer (snapshot-on-success ordering)");
}
```

- [ ] **Step 2: Run → FAIL.** No hook exists; `changes()` is empty in all four.

- [ ] **Step 3: Implement the hook in the `Call` arm.** In `eval/expr.rs`, the `VoxValue::Fn` match arm
  (lines 392-419) currently destructures `{ params, body, mut env }`. Bind the two new fields and snap
  after the body completes **without error** (the `?` on `eval_stmt` already returns early on error, so
  reaching past the loop means success):
```rust
VoxValue::Fn { params, body, mut env, name, is_versioned } => {
    env.push_frame();
    for (p, arg) in params.iter().zip(eval_args) {
        env.set(p.clone(), arg);
    }
    let old_scope = interp.scope.clone();
    interp.scope = env;

    let mut val = VoxValue::Null;
    for stmt in body {
        val = super::stmt::eval_stmt(interp, &stmt)?; // error → early return, no snapshot
        if let VoxValue::_Return(v) = val { val = *v; break; }
        if matches!(val, VoxValue::_Break | VoxValue::_Continue) { break; }
    }
    interp.scope = old_scope;

    // P5: auto-checkpoint on successful return of a @versioned function.
    if is_versioned {
        interp.repo.snapshot(Some(&format!("@versioned {name}")));
    }
    Ok(val)
}
```
  Apply the **same hook** to `Interpreter::call()` in `eval/mod.rs` (lines 483-521): bind
  `name`/`is_versioned` in the destructure and add the identical `if is_versioned { … }` block after
  `self.scope = old_scope;` and before `Ok(res)`. (Both call paths matter: `call()` is the entry the
  CLI uses for `main`/`@test`, and `expr.rs`'s arm handles ordinary in-body calls — the nested-call
  test exercises both.)

- [ ] **Step 4: Run → PASS.** `cargo test -p vox-compiler versioned_fn_auto`,
  `non_versioned_fn_does_not`, `versioned_fn_error_records_no`, `nested_versioned`.

- [ ] **Step 5: Capability-gate honesty check.** If `interp.caps` is `Some(set)` (a `// vox:caps`
  directive present) and does NOT contain `vcs`, decide the policy: the auto-snapshot is a `Vcs`
  effect, so it should be **suppressed (no snapshot) rather than panic** when the caps gate forbids
  `vcs`, consistent with how `repo.*` is governed. Add a test
  (`versioned_fn_suppressed_when_vcs_cap_absent`) only if `repo.*` calls themselves consult
  `interp.caps` today — **grep `execute_repo_op` / `repo.rs` for a `caps` check first.** If `repo.*`
  does NOT currently gate on `interp.caps` (it appears not to, per `eval/repo.rs`), then the
  auto-snapshot inherits that same ungated behavior — do NOT invent a new gate here; note it as a
  follow-up and keep P5 consistent with the landed `repo.*` semantics. Record the decision in a code
  comment at the hook.

- [ ] **Step 6: Commit** `feat(eval): auto-snapshot RepoStore on @versioned successful return`.

---

### Task 6: Golden `.vox` example — `@versioned` auto-snapshot under `--mode interp`

**Files:** Create `examples/golden/repo_versioned_decorator.vox`.

- [ ] **Step 1: Write the golden** (mirrors `examples/golden/repo_operations.vox`'s frontmatter +
  `@test` shape; `@test` bodies are registered as callables and run by the corpus harness, and a
  `main` makes it runnable end-to-end via `vox run --mode interp`):
```
// ---
// title: "Versioned Decorator Auto-Snapshot Golden"
// description: "@versioned auto-records a repo.snapshot on successful return; verified by @test and runnable under --mode interp."
// syntax_version: "0.5.0"
// status: golden
// category: example
// constructs: [@versioned, @test, repo, fn, uses vcs]
// last_validated: 2026-06-06
// training_eligible: true
// training_weight: 1.0
// ---
// @training_prompt: Demonstrate the Vox @versioned decorator — a function marked @versioned auto-records one repo.snapshot() checkpoint on each successful return, with no explicit snapshot call in the body, observable via repo.changes().

// A @versioned function performs NO explicit repo.snapshot — the decorator
// supplies the checkpoint automatically on successful return.
@versioned fn record_payment(amount: int) {
    let _total = amount + 1
}

@test fn versioned_auto_snapshots_once_per_call() uses vcs {
    // No checkpoints yet.
    assert(len(repo.changes()) is 0)

    // Each @versioned call records exactly one checkpoint.
    record_payment(10)
    assert(len(repo.changes()) is 1)

    record_payment(20)
    assert(len(repo.changes()) is 2)

    // undo pops the latest auto-checkpoint.
    repo.undo()
    assert(len(repo.changes()) is 1)
}

fn main() {
    record_payment(100)
    record_payment(200)
    // Two @versioned calls → two auto-snapshots.
    print(len(repo.changes()))
}
```
  (Confirm `print`/`len` are the corpus-canonical builtins used by `repo_operations.vox`'s neighbours;
  `repo_operations.vox` uses `len(...)` and `assert(... is N)`, so reuse those exact forms.)

- [ ] **Step 2: Run it end-to-end.** `cargo run -p vox-cli -- run --mode interp examples/golden/repo_versioned_decorator.vox` → must print `2`. (Use `vox run --mode interp …` if a fresh `vox` binary is on PATH.)

- [ ] **Step 3: Run the golden guardrail.** `cargo test -p vox-compiler --test golden_vox_examples_test`
  (the parse-and-lower guardrail at `crates/vox-compiler/tests/golden_vox_examples_test.rs` enumerates
  `examples/golden/**/*.vox`; the new file must pass with no `legacy_ast_nodes`). Also run the strict
  parse + SSOT tests if they enumerate the golden dir (`golden_examples_strict_parse.rs`,
  `examples_ssot_test.rs`) — regenerate any examples index via its generator, never by hand (per the
  no-hand-edit-generated-docs rule).

- [ ] **Step 4: Commit** `test(golden): @versioned auto-snapshot example (repo_versioned_decorator.vox)`.

---

### Task 7: Cross-arm honesty — `script`/`ts` behavior for `@versioned`

`@versioned` auto-snapshot is an **interpreter (`--mode interp`) feature** in P5 — the `RepoStore` is
an interpreter-only in-memory store (`eval/repo.rs`), exactly like `db.*`'s `DbStore`. The compiled
arms (`--mode script` / TS emit) have no `RepoStore`. Per CR-F4 (an arm that cannot support a
construct must say so clearly, never silently drop it), confirm the decorator does not *break* the
other arms and is at worst inert.

**Files:** none expected (verification task); a one-line doc note if a gap is found.

- [ ] **Step 1:** Build/lower the golden through codegen: `cargo test -p vox-codegen` and confirm
  `@versioned` is ignored (inert) rather than producing wrong code — grep codegen for `is_reactive`
  handling (`vox-codegen-ts/src/reactive/mod.rs:1036`) to see how a decorator flag is consumed or
  skipped, and confirm `is_versioned` is simply not read (acceptable: inert in compiled arms for P5).

- [ ] **Step 2:** If codegen would silently miscompile a `@versioned fn`, do NOT fake support — instead
  add the construct to the codegen "unsupported, diagnose" path (mirror how a native-only construct is
  reported) and note it. If it is harmlessly inert (expected), record that in a one-line comment at the
  `HirFn.is_versioned` field doc and STOP.

- [ ] **Step 3:** Update `docs/src/architecture/where-things-live.md` with a row mapping
  `@versioned`/`@tracked` → `vox-compiler` (lexer/parser/eval) if no VCS-decorator row exists yet (per
  the CLAUDE.md "add the row in the same PR" rule). Commit
  `docs(arch): where-things-live row for @versioned/@tracked`.

---

## Self-Review

- **Spec coverage (design §4.3, P4/P5 row):** `@versioned`/`@tracked` decorator parsed (T1-T2) ✓;
  lowered to HIR (T3) ✓; interpreter hook auto-calls `repo.snapshot(<auto-label>)` via the existing
  `RepoStore`/`execute_repo_op` substrate at the **successful-return effect boundary**, with explicit
  trigger semantics (on-success, once, errors skip) asserted by four behavioral tests (T5) ✓;
  `@versioned` implies `uses vcs` via capability injection, specified and tested (T4) ✓; golden
  `.vox` runnable under `--mode interp` (T6) ✓.
- **Grounded in real code, not invented:** every decorator step mirrors the proven
  `@pure`/`@reactive` bare-token spine — token (`token.rs:193-199`), parse arm (`head.rs:1194-1201`),
  `FnDecl` bool (`fundecl.rs:51`), `HirFn` bool (`decl.rs:320`), lowering copy (`lower/decl.rs:57`).
  The snapshot uses the existing `RepoStore::snapshot` (`eval/repo.rs:20`) and the landed `Vcs`
  capability (`effect.rs:28`, `effect_check.rs:532`). No new IR, no new effect kind, no new store.
- **The one added capability, flagged honestly:** `VoxValue::Fn` (`value.rs:24-28`) carried no
  decorator metadata or name, and the call sites destructure exactly `{params, body, env}`
  (`expr.rs:392`, `mod.rs:483`) — so the decorator was unobservable at call time. Task 3 ADDS
  `name` + `is_versioned` to the variant (with a test), which is a breaking variant change handled by
  fixing every construct/destructure site the compiler lists (Field-fallout note enumerates them).
  This is the prompt's "if the mechanism can't express it, add the capability with a test" path, not
  a stub.
- **Trigger-semantics rationale:** snapshot-on-success (not on-entry, not pre-mutation) because the
  `RepoStore` change is a logical checkpoint and a failed call should leave no spurious history; error
  short-circuit via `?` is the mechanism, asserted by `versioned_fn_error_records_no_snapshot`.
- **Honest scope limits:** auto-snapshot is interpreter-only in P5 (T7 confirms compiled arms are
  inert, not miscompiled); the broader "auto-snapshot at *any* inferred `fs`/`db` effect boundary
  even without a decorator" half of design §4.3 is **out of scope** for P5 (it needs a per-effect
  coalescing/debounce policy — design §7 risk) and is left for a follow-up; the caps-gate question for
  the auto-snapshot is resolved to match the landed `repo.*` ungated behavior (T5 Step 5) rather than
  inventing a new gate. No `todo!()`/stub is introduced; genuinely-deferred work is named, not faked.
- **Type/field consistency:** `is_versioned` is added with `#[serde(default)]` on both `FnDecl` and
  `HirFn` so existing serialized fixtures still deserialize; the new `VoxValue::Fn` fields are
  populated at all four registration sites and bound at both call sites.
