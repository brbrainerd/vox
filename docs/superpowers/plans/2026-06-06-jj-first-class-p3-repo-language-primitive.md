# P3 — `repo.*` Language Primitive + `Vcs` Effect Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `repo.*` namespaced builtin family to the Vox interpreter (snapshot/changes/undo/diff/conflicts), governed by a new `Vcs` effect, executing against an in-memory `RepoStore` — giving every Vox program cheap, language-level time-travel, exactly mirroring the proven `db.*` subsystem.

**Architecture:** `repo.*` is **stateful** (mutates per-interpreter state), so it follows the **`db.*` model** (needs `&mut Interpreter`), NOT the stateless `fs`/`process` model. But its surface is **imperative method calls** (not a query DSL), so dispatch is by **namespace-object detection** (like `fs`) intercepted in the `expr.rs` `MethodCall` arm where `interp` is available — not via `db`'s `opt_plan` query-plan lowering. The interpreter holds `pub repo: RepoStore` beside `pub db: DbStore`. **Pure compiler work** — no dependency on `vox-vcs`/jj-lib; binding the real jj engine is a later integration. Independent of P0–P2.

**Tech Stack:** Rust, `vox-compiler` (lexer→typeck→interp), golden `.vox` `@test` blocks.

**Source spec:** [`2026-06-05-jj-first-class-vcs-design.md`](../specs/2026-06-05-jj-first-class-vcs-design.md) §5. **Grounding:** verbatim `db.*`/`fs.*` anchors captured 2026-06-06.

**Verified anchors (file:line, current code):**
- `EffectAnnotation` + `from_keyword` + `as_str` — `crates/vox-compiler/src/ast/decl/effect.rs:8`
- `HirEffectKind` + `label` — `crates/vox-compiler/src/hir/nodes/effect.rs:8`
- `HirCapability` + `as_str` — `crates/vox-compiler/src/hir/nodes/decl.rs:700`
- `effect_kind_to_cap` — `crates/vox-compiler/src/typeck/effect_check.rs:76`; `stdlib_module_capability` — `:506` (note the existing `"Scrape"|"Browser" => Net` arm)
- `Interpreter` struct (`pub db: DbStore`) — `crates/vox-compiler/src/eval/mod.rs:31`; constructor seeds namespaces (e.g. `fs` as `Object[("__namespace__", Str("fs"))]`) and sets `db: DbStore::default()` (~`:239`)
- `DbStore` — `crates/vox-compiler/src/eval/db.rs:28`
- `call_builtin_method` (namespace match on `__namespace__`) — `crates/vox-compiler/src/eval/builtins.rs:108` (the `fs`/`time`/`env` arms)
- `HirExpr::MethodCall` arm (where the receiver `o` is evaluated and `call_builtin_method` is called with `interp.caps`) — `crates/vox-compiler/src/eval/expr.rs:449`
- `HirExpr::Ident` eval (scope lookup; seeded namespaces resolve here) — `crates/vox-compiler/src/eval/expr.rs:32`
- Golden test format — `examples/golden/db_operations.vox:22` (`@test fn ... { assert(db.Widget.insert(...).unwrap() is 0) }`)

---

## File Structure

| File | Responsibility |
|---|---|
| Modify `crates/vox-compiler/src/ast/decl/effect.rs` | `EffectAnnotation::Vcs` + `from_keyword("vcs")` + `as_str` |
| Modify `crates/vox-compiler/src/hir/nodes/effect.rs` | `HirEffectKind::Vcs` + `label` |
| Modify `crates/vox-compiler/src/hir/nodes/decl.rs` | `HirCapability::Vcs` + `as_str` |
| Modify `crates/vox-compiler/src/typeck/effect_check.rs` | `effect_kind_to_cap` Vcs arm + `stdlib_module_capability` `"repo"\|"vcs"` arm |
| Modify the AST→HIR effect lowering site | map `EffectAnnotation::Vcs → HirEffectKind::Vcs` (Task 1 locates it) |
| Create `crates/vox-compiler/src/eval/repo.rs` | `RepoStore` + `execute_repo_op(interp, method, args)` |
| Modify `crates/vox-compiler/src/eval/mod.rs` | `pub repo: RepoStore` field; seed `repo` namespace + init `RepoStore::default()` |
| Modify `crates/vox-compiler/src/eval/expr.rs` | intercept `repo` namespace in `MethodCall` → `execute_repo_op` |
| Create `examples/golden/repo_operations.vox` | `@test` blocks exercising `repo.*` |

---

### Task 1: The `Vcs` effect + capability wiring

**Files:** the four effect/typeck files above + the AST→HIR lowering site.

- [ ] **Step 1: Write the failing test**

Add to the test module in `crates/vox-compiler/src/typeck/effect_check.rs` (mirror an existing effect test):

```rust
#[test]
fn repo_call_requires_vcs_capability() {
    // A function that calls `repo.*` but declares no `uses vcs` must fail effect-check.
    let src = r#"
        @pure
        fn bad() {
            repo.snapshot("x")
        }
    "#;
    let diags = check_src_effects(src); // use this file's existing test harness helper
    assert!(
        diags.iter().any(|d| d.message.contains("vcs")),
        "expected a missing-vcs-capability diagnostic, got: {diags:?}"
    );
}
```
(Use whatever helper the existing effect tests use to compile a source string to diagnostics; match their pattern exactly. If `@pure` isn't the right way to express "declares no effects", mirror how an existing test asserts a missing-capability diagnostic for `db`/`net`.)

- [ ] **Step 2: Run → FAIL**

Run: `cargo test -p vox-compiler repo_call_requires_vcs_capability`
Expected: FAIL — `repo` is currently ungoverned (`stdlib_module_capability` returns `None`), so no diagnostic is produced.

- [ ] **Step 3: Add the `Vcs` variant across the three enums + conversions**

In `ast/decl/effect.rs`: add `Vcs,` to `EffectAnnotation` (after `Mutate`); add `"vcs" => Some(Self::Vcs),` to `from_keyword`; add `Self::Vcs => "vcs",` to `as_str`.

In `hir/nodes/effect.rs`: add `Vcs,` to `HirEffectKind`; add `HirEffectKind::Vcs => "vcs".into(),` to `label`.

In `hir/nodes/decl.rs`: add `Vcs,` to `HirCapability`; add `Self::Vcs => "vcs",` to `as_str`.

In `typeck/effect_check.rs`: add `HirEffectKind::Vcs => HirCapability::Vcs,` to `effect_kind_to_cap`; and to `stdlib_module_capability` add (next to the `Browser`/`Scrape` arm):
```rust
        "repo" | "Repo" | "vcs" | "Vcs" => Some(HirCapability::Vcs),
```

- [ ] **Step 4: Fix the AST→HIR lowering (locate it)**

Grep for where `EffectAnnotation` lowers to `HirEffectKind` (e.g. `rg "EffectAnnotation::Mutate" crates/vox-compiler/src/hir`). Add the `EffectAnnotation::Vcs => HirEffectKind::Vcs` arm there. Also handle any OTHER exhaustive `match` over these enums the compiler now flags as non-exhaustive (the compiler will list them — fix each by adding the `Vcs` arm with the obvious mapping). Build to find them all: `cargo build -p vox-compiler`.

- [ ] **Step 5: Run → PASS**

Run: `cargo test -p vox-compiler repo_call_requires_vcs_capability` → PASS.
Run: `cargo build -p vox-compiler` → compiles (all exhaustive matches handled).

- [ ] **Step 6: Commit**

```bash
cargo fmt -p vox-compiler
git add -A
git commit -m "feat(compiler): add Vcs effect — repo.*/vcs.* require 'uses vcs'"
```

---

### Task 2: `RepoStore` + interpreter execution of `repo.*`

**Files:** Create `eval/repo.rs`; Modify `eval/mod.rs`, `eval/expr.rs`.

- [ ] **Step 1: Write the failing test**

Create `crates/vox-compiler/src/eval/repo.rs` with the test first:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::value::VoxValue;

    #[test]
    fn snapshot_then_changes_and_undo() {
        let mut store = RepoStore::default();
        let id0 = store.snapshot(Some("first"));
        let id1 = store.snapshot(Some("second"));
        assert_eq!(id0, 0);
        assert_eq!(id1, 1);
        assert_eq!(store.changes().len(), 2);
        let undone = store.undo();
        assert_eq!(undone, Some(1));
        assert_eq!(store.changes().len(), 1);
    }

    #[test]
    fn execute_repo_op_snapshot_returns_change_id() {
        // execute_repo_op is the interpreter entry; here we test the store-facing core.
        let mut store = RepoStore::default();
        let _ = VoxValue::Null; // keep the import used
        assert_eq!(store.snapshot(None), 0);
    }
}
```

- [ ] **Step 2: Run → FAIL.** Run: `cargo test -p vox-compiler eval::repo`. Expected: `RepoStore` undefined.

- [ ] **Step 3: Implement `RepoStore` + `execute_repo_op`**

Prepend to `eval/repo.rs` (mirror `eval/db.rs`'s structure):
```rust
//! In-memory `repo.*` (VCS) store for `--mode interp`, mirroring [`crate::eval::db`].
//! Stateful, so dispatched with `&mut Interpreter` (see `eval::expr` MethodCall).

use crate::eval::mod_interp::Interpreter; // adjust path: the module that defines Interpreter
use crate::eval::value::VoxValue;
use crate::eval::EvalError;

/// One recorded change/snapshot.
#[derive(Debug, Clone)]
pub struct RepoChange {
    pub id: i64,
    pub label: Option<String>,
}

/// In-memory operation log for one interpreter run.
#[derive(Debug, Clone, Default)]
pub struct RepoStore {
    changes: Vec<RepoChange>,
    next_id: i64,
}

impl RepoStore {
    pub fn snapshot(&mut self, label: Option<&str>) -> i64 {
        let id = self.next_id;
        self.next_id += 1;
        self.changes.push(RepoChange { id, label: label.map(str::to_owned) });
        id
    }
    pub fn changes(&self) -> &[RepoChange] {
        &self.changes
    }
    pub fn undo(&mut self) -> Option<i64> {
        self.changes.pop().map(|c| c.id)
    }
}

/// Interpreter entry point for `repo.<method>(...)`. Mirrors `execute_db_plan`.
pub fn execute_repo_op(
    interp: &mut Interpreter,
    method: &str,
    args: Vec<VoxValue>,
) -> Result<VoxValue, EvalError> {
    match method {
        "snapshot" => {
            let label = args.first().and_then(|v| if let VoxValue::Str(s) = v { Some(s.as_str()) } else { None });
            Ok(VoxValue::Int(interp.repo.snapshot(label)))
        }
        "changes" => {
            let list = interp.repo.changes().iter().map(|c| VoxValue::Int(c.id)).collect();
            Ok(VoxValue::List(list))
        }
        "undo" => Ok(match interp.repo.undo() {
            Some(id) => VoxValue::Int(id),
            None => VoxValue::Null,
        }),
        other => Err(EvalError::AssertionFailed(format!("repo.{other}: unknown method"))),
    }
}
```
**Adjust to reality:** the exact `Interpreter` import path, `VoxValue` variants (`Int`/`Str`/`List`/`Null`), and `EvalError` variant names must match the crate — fix imports until `cargo build -p vox-compiler` is clean. `repo.snapshot` returns the change id as `Int`; `repo.changes()` returns a `List` of ids; `repo.undo()` returns the undone id or `Null`. (Diff/conflicts can be added later; keep Task 2 to snapshot/changes/undo.)

- [ ] **Step 4: Wire the store into the interpreter**

In `eval/mod.rs`: add `pub mod repo;` (near `pub mod db;`); add field `pub repo: crate::eval::repo::RepoStore,` to `Interpreter` (after `pub db`); in the constructor set `repo: crate::eval::repo::RepoStore::default(),`; and SEED the `repo` namespace in `scope` exactly like `fs`:
```rust
scope.set(
    "repo".to_string(),
    VoxValue::Object(vec![("__namespace__".to_string(), VoxValue::Str("repo".to_string()))]),
);
```

- [ ] **Step 5: Intercept `repo` in the `MethodCall` arm**

In `eval/expr.rs`, in the `HirExpr::MethodCall` arm, AFTER the receiver `o` is evaluated and args are evaluated, BEFORE the generic `call_builtin_method` call, add:
```rust
if let VoxValue::Object(fields) = &o
    && fields.iter().any(|(k, v)| k == "__namespace__" && matches!(v, VoxValue::Str(s) if s == "repo"))
{
    return super::repo::execute_repo_op(interp, method, eval_args);
}
```
(Place it adjacent to the existing namespace-field dispatch block so the control flow matches. Confirm the variable names `o`, `eval_args`, `method` match the surrounding code.)

- [ ] **Step 6: Run → PASS.** Run: `cargo test -p vox-compiler eval::repo` and `cargo build -p vox-compiler`.

- [ ] **Step 7: Commit**

```bash
cargo fmt -p vox-compiler
git add -A
git commit -m "feat(compiler): RepoStore + interpreter execution of repo.snapshot/changes/undo"
```

---

### Task 3: Golden `.vox` end-to-end test

**Files:** Create `examples/golden/repo_operations.vox`.

- [ ] **Step 1: Write the golden with `@test` blocks** (mirror `examples/golden/db_operations.vox`):
```vox
// repo.* — language-level version control (in-memory, --mode interp).
// vox:skip-reason none — this must compile and its @test must pass.

@test fn snapshot_undo_lifecycle() uses vcs {
    let c0 = repo.snapshot("first")
    let c1 = repo.snapshot("second")
    assert(c0 is 0)
    assert(c1 is 1)
    assert(len(repo.changes()) is 2)
    let undone = repo.undo()
    assert(undone is 1)
    assert(len(repo.changes()) is 1)
}
```
(Confirm the exact `@test` + `uses vcs` syntax against `db_operations.vox` and the grammar — if `@test fn ... uses vcs` isn't valid placement, match the working form. The point: the test fn must declare `uses vcs` and exercise snapshot/changes/undo.)

- [ ] **Step 2: Run the golden** with the repo's golden-test runner (find how `db_operations.vox` is executed — likely `cargo test -p vox-compiler` golden harness, or `vox test examples/golden/repo_operations.vox`, or a `--mode interp` run). Confirm the `@test` passes and `uses vcs` type-checks (no missing-capability diagnostic since it's declared).

- [ ] **Step 3: Commit**

```bash
git add examples/golden/repo_operations.vox
git commit -m "test(golden): repo.* snapshot/undo lifecycle with uses vcs"
```

---

## Self-Review

- **Spec coverage (§5):** `repo.*` builtins (snapshot/changes/undo) ✓ (T2); `Vcs` effect governs them via `stdlib_module_capability` ✓ (T1); interpreter `RepoStore` mirroring `DbStore` ✓ (T2); golden `@test` ✓ (T3). Packaging follows the `Db` precedent (no `runtime-capabilities.v1.yaml` row needed — `hir_capability_to_packaging_id` returns `None`). diff/conflicts and the parameterized `Vcs(repo)` are deferred (YAGNI for the language MVP).
- **Dispatch choice justified:** namespace-object interception (not `db`'s `opt_plan`) because `repo.*` is imperative, not a query DSL — but routed through `interp` (not stateless `call_builtin_method`) because it's stateful.
- **No vox-vcs/jj-lib dependency** — pure compiler; real-engine binding is later.
- **Placeholders:** the "adjust import path / variant names to reality" notes are compile-time confirmations against existing types, not missing logic.
