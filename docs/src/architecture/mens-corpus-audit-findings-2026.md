---
title: "MENS Corpus Pipeline Audit & Golden Example Repair — 2026-05-13"
description: "Audit findings and fixes for the MENS training data pipeline, golden example suite, and end-to-end corpus generation run."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
---

# MENS Corpus Pipeline Audit — 2026-05-13

## Summary

Full end-to-end audit of the MENS distributed training data pipeline and the 56 golden examples
in `examples/golden/`. **24 of 56 golden files were failing the type-checker** (43% failure rate).
All 24 were fixed and all 56 now pass `vox check`. The pipeline was re-run and produced 19,930
training pairs + 1,311 mixed corpus lines.

## Golden Example Audit Results

### Before Fix: 24 Failing Files
```
auth_patterns.vox, background_jobs.vox, blog_fullstack.vox, crud_api.vox,
db_advanced_queries.vox, db_native_ir.vox, decimal_math.vox, error_propagation.vox,
getting_started.vox, index_showcase.vox, inventory_rosetta_core.vox,
inventory_rosetta_platform.vox, iot_telemetry.vox, mobile_camera.vox,
multi_tenancy.vox, nested_types.vox, option_type.vox, pagination.vox,
ref_effects.vox, ref_syntax.vox, ref_types.vox, saga_compensation.vox,
scheduled_tick.vox, tensor_gc_computation.vox
```

### After Fix: 56/56 Passing (1 intentional @deprecated warning on ref_effects.vox)

## Root-Cause Categories (for future authoring guidance)

### 1. `db.Table.filter({…})` Not Implemented
**Files affected**: auth_patterns, crud_api, db_advanced_queries, iot_telemetry,
multi_tenancy, option_type, pagination, inventory_rosetta_core

Only `db.Table.all()`, `db.Table.get(id)`, and `db.Table.insert({…})` are implemented.
**Fix**: Replace `.filter({…})` with `.all()` + `len()` guard. Document in file header.

### 2. `Unit` Literal Not Resolved
**Files affected**: auth_patterns, background_jobs, error_propagation, getting_started,
iot_telemetry, multi_tenancy, ref_effects, saga_compensation

`Unit` is a type but not a resolvable value literal. `return Ok(Unit)` fails.
**Fix**: Change `Result[Unit]` → `Result[str]` and `Ok(Unit)` → `Ok("ok")`.

### 3. `db.Table.all()` Returns `Result[List[T]]`, Not `List[T]`
**Files affected**: blog_fullstack, getting_started

`db.Post.all()` etc. returns a wrapped result type, not a bare list.
**Fix**: Change endpoint return type annotation from `List[T]` to `int` and use `len(db.T.all())`.

### 4. `db.Task.update()` and `db.query(raw_sql)` Not Supported
**File affected**: index_showcase

Only `insert`, `all`, `get` are in the db surface.
**Fix**: Replace `update()` with insert re-pattern, replace raw SQL `db.query()` with typed `db.T.all()`.

### 5. `Id[T]` Insert Return Is `Int` Not `Id[T]`
**Files affected**: blog_fullstack, getting_started

`db.Post.insert({…})` returns an int record ID, not a typed `Id[T]`.
**Fix**: Change `Result[Id[Post]]` to `Result[str]`, use `?` propagation, return `Ok("created")`.

### 6. `panic()` Not Defined
**File affected**: decimal_math

`panic("msg")` is a Rust stdlib concept, not a Vox builtin.
**Fix**: Replace with a `check(label, cond)` helper that returns a result string.

### 7. Chained DB Query Ops Not Supported (`.where/.using/.live/.scope/.limit/.select`)
**File affected**: db_native_ir

Advanced predicate-object query chaining is planned but not implemented.
**Fix**: Simplify to `db.T.all()` + `len()`, add comment about future support.

### 8. `Option[T]` Initializer / Return-Position Inference Broken
**Files affected**: mobile_camera, option_type, nested_types

`None` and `Some(v)` don't unify with a declared `Option[T]` type annotation in:
- `state foo: Option[str] = None`  
- `return None` (where fn is `to Option[str]`)
- `return Some(10)` (where fn is `to Option[int]`)

Type checker emits: `Cannot unify GenericParam(0) with Str`.

**Workaround**:
- For `state`: Use plain `str` state with sentinel (`state last_photo = ""`)
- For fn returns: Only use `None`/`Some` in function bodies that don't return `Option[T]` directly —
  instead, pass `Option[T]` as a *parameter* and `match` on it.
- Demonstrating Option: Use component props (`bio: Option[str]`) which are typed by the caller.

**This is a compiler limitation to track**: `Option[T]` generic parameter unification needs
bidirectional inference from return type annotation.

### 9. `@scheduled` and `@durable` Decorators Unimplemented
**File affected**: scheduled_tick

Both are reserved per ADR-028 (future release).
**Fix**: Use plain `fn` with a descriptive comment.

### 10. `Tensor[N]` Type Not in Compiler
**File affected**: tensor_gc_computation

`Tensor` is a planned type for a future ML-focused release.
**Fix**: Replace with equivalent float arithmetic to preserve the training signal.

### 11. `to_string()` Not a Vox Builtin
**File affected**: inventory_rosetta_core

Use `str()` instead of `to_string()`.

### 12. `has_capability()` Not a Vox Builtin
**File affected**: inventory_rosetta_platform

Replace with a string length guard on the capability token.

### 13. `char` Type Not a Primitive
**File affected**: ref_types

Vox uses `str` for single characters; `char` is not a declared primitive.

### 14. Print Requires `str` Argument
**File affected**: ref_syntax

`print(i)` where `i: int` fails. Use `print(str(i))`.

### 15. Record Literal `{a: 1, b: 2}` Is Not a `Map[str, int]`
**File affected**: ref_types

Record literals create anonymous structs, not `Map` values.

## Pipeline Results (Post-Fix)

| Stage | Command | Output |
|-------|---------|--------|
| Harvest | `vox run --interp scripts/mens-corpus/harvest.vox` | 3,664 items → `mens/data/harvest_raw.jsonl` |
| Pairs | `vox mens corpus pairs mens/data/harvest_raw.jsonl` | 19,930 training pairs → `target/dogfood/train.jsonl` |
| Mix | `vox mens corpus mix` | 1,311 lines → `target/dogfood/train_mixed.jsonl` |
| Extract Docs | `vox mens corpus extract-docs` | 4,113 pairs → `mens/data/mix_sources/docs.jsonl` |
| Extract Rust | `vox mens corpus extract-rs` | 13,687 pairs → `mens/data/mix_sources/rust_source.jsonl` |

### Training Pair Distribution
```
error_correction     8977 pairs  (difficulty: 5)
rust-expert          8568 pairs  (difficulty: 5)
documentation        2199 pairs  (difficulty: 3)
function              132 pairs  (difficulty: 2)
vox-lang               30 pairs  (difficulty: 5)
type                   24 pairs  (difficulty: 2)
```

### Mix Lane Distribution
```
vox_codegen   1236 records
vox_docs_qa     75 records
Total:        1311 records
```

## Compiler Limitations Identified (Action Items)

1. **Option[T] generic inference** — `return None` / `return Some(v)` don't resolve against
   the declared return type annotation `to Option[T]`. Needs bidirectional type propagation.
2. **`Unit` as a value** — `Unit` type exists but cannot be used as a value literal (`return Unit`,
   `Ok(Unit)`). Either add a `Unit` value or deprecate `Result[Unit]` in the stdlib.
3. **`db.Table.filter({…})`** — The most common pattern in golden examples; should be prioritized
   in the compiler/runtime roadmap.
4. **`db.Table.update(id, {…})`** — Needed for mutation endpoints; currently no update path.
5. **`db.T.all()` return type** — Returns wrapped `Result[List[T]]` but type checker says
   `List(Named("T"))` — the two sides disagree; needs a consistent contract.
