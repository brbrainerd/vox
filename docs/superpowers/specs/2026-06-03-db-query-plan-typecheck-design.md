# Typed db query-plan chaining (`.where/.filter/.order_by/.limit/.select`) — Design

**Date:** 2026-06-03
**Status:** Proposed, awaiting approval before implementation.
**Track:** B5 (golden-corpus & compiler-reality plan). Reclassified from "20-min guard removal" to a real type-system feature after empirical verification (below).

## Why this is NOT a one-line fix (verified 2026-06-03)

The scout claimed: "remove the typeck guard at `checker/expr.rs:381` and the working order/limit codegen becomes reachable." **That is false on the typecheck side**, verified empirically against the prebuilt binary:

```
$ vox check <snippet with db.Task.where({done:{eq:false}}).order_by("title").limit(10)>
error[E0001]: Method `where` not found on Table("Task", [("title", Str), ("done", Bool)]).
error[E0002]: db query chaining via '.limit(...)' is not supported yet; use typed db.Table operations directly
```

So **`.where()` itself does not typecheck** — not just `.order_by/.limit`. Removing the guard only deletes the E0002 and leaves the E0001 "method not found".

### Current state (file:line, verified)

| Fact | Location | Verified |
|---|---|---|
| Guard hard-rejects `.limit`/`.order_by` when `opt_plan.is_some()` | `crates/vox-compiler/src/typeck/checker/expr.rs:381` | ✅ read + reproduced |
| `Ty::Table` method table advertises only `insert/get/delete/query/all/count/find` — no `where/filter/order_by/limit/select` | `crates/vox-compiler/src/typeck/builtins.rs:1546-1593` | ✅ read |
| `opt_plan` is referenced **only** by the guard in the whole checker | `grep opt_plan crates/vox-compiler/src/typeck/checker/` → 1 hit (expr.rs:380-381) | ✅ grep |
| A predicate validator (`validate_db_predicate`) and projection checker (`check_db_select_projection`) exist | `checker/expr.rs:56`, `:15` | ✅ read |
| …but they are wired to `check_db_scoped_handler` (`checker/mod.rs:223`), a **different** db construct — NOT the `.where()` method chain | `checker/mod.rs:223` | ✅ grep |
| HIR lowering builds `order_by`/`limit`/predicate into a `HirDbQueryPlan` | `crates/vox-compiler/src/hir/lower/expr_db.rs` (`apply_order_by`/`apply_limit`) | ⚠️ scout-claimed — **VERIFY FIRST** |
| Rust codegen emits `all_order_limit` / `filter_where_order_limit` SQL | `codegen_rust/emit/tables/codegen.rs`, `emit/method_emit.rs:293-324` | ⚠️ scout-claimed — **VERIFY FIRST** |

**Step 0 of implementation is to verify the two ⚠️ rows.** If codegen does NOT actually emit correct SQL for chained queries, this becomes a codegen task too, not just typecheck.

## Goal

Make the idiomatic chained query typecheck and round-trip to working SQL under both `--mode script` (codegen) and `--mode interp`:

```vox
@query fn active_tasks(limit: int) to list[Task] {
    return db.Task.where({ done: { eq: false } }).order_by("title").limit(10)
}
```

Strong-typing requirement (per maintainer directive): predicates must be field- and type-checked against the table at the admission boundary — never loosened to anonymous records.

## Design

### The query type

Introduce a dedicated **query type** so chaining composes and the terminal type is well-defined. Two options:

- **Option A — `Ty::Query(Box<Ty /* Table */>)`** (recommended). `db.Table` → `Ty::Table`; the first chain method (`.where`/`.order_by`/`.limit`/`.select`) returns `Ty::Query(table)`; further chain methods accept `Ty::Query` and return `Ty::Query`; the query is *coerced* to `Result[List[Record]]` when assigned/returned (and `.all()`/`.first()` are explicit terminals). This keeps "is this still chainable?" in the type, and predicate validation has the table in hand at each step.
- **Option B — collapse to `Result[List[Record]]` immediately.** Each chain method returns `Result[List[Record]]` and re-accepts chaining via `opt_plan`. Simpler types but loses the "table is still in scope for the next predicate" property, which the predicate validator needs. **Not recommended** — predicate field-checks need the originating table type.

Pick **Option A**. Add `Ty::Query(Box<Ty>)` to `typeck/ty.rs` alongside `Ty::Table`.

### Where the chain is typechecked

In `checker/expr.rs` `HirExpr::MethodCall`, **before** the generic `lookup_method` path, add a branch:

```
if opt_plan.is_some() {
    // db query-plan method. object resolves to Ty::Table or Ty::Query(table).
    let table_ty = resolve to the underlying Ty::Table (peel Query if present)
    match method {
        "where" | "filter" => { validate_db_predicate(arg0, table_fields, ...); Ty::Query(table) }
        "order_by"         => { check order_by column exists on table; Ty::Query(table) }
        "limit"            => { check arg is int; Ty::Query(table) }
        "select"           => { check_db_select_projection(...); Ty::Query(projected) }
        "using"|"live"|"scope"|"sync" => { /* capability modifiers, Ty::Query(table) */ }
        _ => fall through
    }
}
```

Then `Ty::Query(T)` unifies with / coerces to `Result[List[Record(T fields)]]` at use sites (return-type check, `let` binding, `len()` arg). Add the coercion in `unify.rs` (a `Query(t)` ~ `Result(List(Record))` rule) or resolve `Ty::Query` to that shape at the end of `check_expr`.

### Reconcile the method table

Add `where/filter/order_by/limit/select` to the `Ty::Table` (and `Ty::Query`) method advertisement in `typeck/builtins.rs:1546-1593` so error messages for typos are correct ("no such predicate field" not "no such method").

### Remove the guard

Delete the `expr.rs:381` guard once order_by/limit are handled by the new branch.

## Implementation steps (TDD)

1. **Verify codegen** (Step 0): write a Rust test that lowers `db.T.where(..).order_by(..).limit(..)` HIR to Rust and asserts the emitted SQL contains `WHERE`/`ORDER BY`/`LIMIT`. If it fails, expand scope to codegen.
2. Failing typecheck test: `db.T.where({f:{eq:v}}).order_by("f").limit(10)` produces **no** diagnostics (currently E0001+E0002).
3. Add `Ty::Query`; add the `opt_plan` branch; wire `validate_db_predicate`/`check_db_select_projection`.
4. Add the `Query → Result[List[Record]]` coercion.
5. Reconcile the method table; delete the guard.
6. Un-skip nothing new (db_native_ir already un-skipped), but **add a new golden** `db_query_plan.vox` exercising `.where/.order_by/.limit/.select` + `@test`, and verify it runs in interp (the interp must also execute the plan — confirm `eval` handles `opt_plan` chains, or this becomes interp work too).
7. Full `vox check` golden sweep + HumanEval gate green; `cargo test -p vox-compiler`.

## Risks / open questions

- **Interp execution.** This spec covers typecheck. The interpreter must also *execute* `.where/.order_by/.limit` (does `eval` honor `opt_plan`, or does it run `all()` then ignore the chain?). Verify; may need an interp task to filter/sort/limit the in-memory list so the golden runs in both modes (consistent with the B1–B4 dual-mode principle).
- **`.using(fts|vector|hybrid)/.live/.scope`** capability modifiers are lower priority and runtime-incomplete; type them as `Ty::Query` passthroughs but do **not** add corpus coverage until the runtime is verified (scout: scaffolded, not complete).
- **`select` projection type.** `.select([...])` should narrow the Record to the projected columns; `check_db_select_projection` exists — confirm it returns a usable projected type.

## Done when

- The chained-query golden typechecks, runs in interp, and emits correct SQL in codegen.
- `where/order_by/limit/select` are in the method table; the `expr.rs:381` guard is gone.
- Predicates are still strongly field/type-checked (no loosening).
- Full corpus + HumanEval gates green.
