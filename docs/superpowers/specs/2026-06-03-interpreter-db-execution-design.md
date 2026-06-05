# Interpreter DB Execution — design

Status: design → implementing (2026-06-03)
Author: compiler-reality track
Related: `2026-06-03-db-query-plan-typecheck-design.md`, golden-corpus-and-compiler-reality plan B5.

## Problem

`db.*` operations typecheck and codegen to real SQL under `--mode script`, but
the tree-walking interpreter (`--mode interp`, the **default** run mode) has no
db support at all: `db` is an undefined variable, so any data-layer program
fails at runtime with `UndefinedVariable("db")`. This is the keystone
tier-divergence — a flagship Vox feature is dead in the default mode.

It went unnoticed because the only golden that uses `db.*` (`crud_api.vox`) has
no `@test` functions, so the behavioral golden runner never exercised db in
interp.

## Approach

Every db operation lowers to `HirExpr::MethodCall(receiver, method, args,
Some(HirDbQueryPlan), span)`. The plan carries everything needed to execute:
`table`, `op` (Insert/Get/Delete/All/FilterRecord/Count/UnsafeQueryRawClause),
`predicate`, `projection`, `order_by`, `has_limit`. So the interpreter executes
db by intercepting `MethodCall` when `opt_plan.is_some()` — **before** it
evaluates the `db.Table` receiver (which would fail as undefined) — and running
the plan against a per-`Interpreter` in-memory store.

### In-memory store

```
DbStore { tables: BTreeMap<String, DbTable> }
DbTable { rows: Vec<Vec<(String, VoxValue)>>, next_id: i64 }
```

Each row is an object's field list (matching `VoxValue::Object`). Rows carry an
auto-assigned `_id: Int` field, matching the `getting_started.vox` auto-`_id`
idiom. The store lives on `Interpreter` (a new `pub db: DbStore` field) so it
persists across calls within a run.

### Op semantics & return types (must match `typeck/builtins.rs`)

| op | method(s) | returns |
|----|-----------|---------|
| Insert | `insert` | `Result[Int]` — the new `_id` |
| Get | `get`, `find` | `Result[Option[Record]]` |
| Delete | `delete` | `Result[Unit]` (Ok = Null) |
| All | `all` | `Result[List[Record]]` |
| Count | `count` | `Result[Int]` |
| FilterRecord / query chain | `filter`, `where`, `order_by`, `limit`, `select` | `Result[List[Record]]` |

All return a `VoxValue::Result(Ok(...))` (errors reserved for future capability
violations). The Err side uses the widened `Box<VoxValue>` (`err_str`).

### Predicate evaluation

`HirDbPredicate` is evaluated against a row by threading the call args
positionally with an `arg_ix` cursor — the exact mirror of the typecheck-side
`validate_db_predicate`, so value ordering agrees. Covers
Eq/Neq/Lt/Lte/Gt/Gte/Contains/In/And/Or/Not. `FilterRecord` matches each `Eq`
field to the named arg of the same name. Comparisons reuse `VoxValue`'s existing
ordering (incl. cross-numeric Int/Float).

### Projection / order_by / limit

After predicate filtering: `projection` keeps only the named columns (always
retaining `_id`); `order_by` sorts by the named field asc/desc; `has_limit`
truncates (limit value is the trailing arg).

## Verification

- New `crates/vox-compiler/tests/interpreter_db_test.rs`: insert→all→count,
  get/find by id, delete, filter-equality, where-gte, projection, order_by+limit
  — all run end-to-end in interp and assert concrete values.
- Add `@test` fns to `crud_api.vox` (or a new golden) exercising db in interp,
  closing the "no behavioral test caught this" gap.
- Full vox-compiler suite + golden sweep 71/71 + HumanEval unchanged.

## Fused query chains (lifted 2026-06-03)

A *fused* chain that puts a predicate behind a later modifier —
`.where({age:{gte:18}}).select("name")` — splits the comparison value onto an
inner chain node, while only the outer node executes. This was originally a
loud `Err`. It is now **fully supported**: `HirDbQueryPlan` carries
`predicate_args` (the flattened comparison values in predicate DFS order) and
`limit_value`, populated by `make_db_plan_from_chain` from `DbQueryChain`. The
interpreter threads `predicate_args` positionally against `predicate`
(`And`/`Or` evaluate every branch so the cursor stays aligned), so `where`
composes correctly with `select` / `order_by` / `limit`. Codegen ignores the new
fields (it has its own arg path). `HirDbQueryPlan` consequently drops its
`PartialEq`/`Eq` derive (`HirExpr` is not comparable); nothing compared plans by
value.

**UnsafeQueryRawClause** (raw SQL fragment) has no interp analogue — it degrades
to an unfiltered scan. Capability modifiers (`using`/`live`/`scope`/`sync`) are
interp no-ops.
