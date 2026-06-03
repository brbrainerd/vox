# Widen interpreter `Result` Err side from `String` to `VoxValue` — design

**Date:** 2026-06-03
**Status:** Design (implementable). Companion to the typecheck-side plan in
[`2026-06-03-result-two-param-reconciliation.md`](2026-06-03-result-two-param-reconciliation.md)
and the canonical [`2026-05-18-result-two-param-design.md`](2026-05-18-result-two-param-design.md).
**Track:** C1 (golden-corpus & compiler-reality). This is item 5 ("Optional, runtime")
of the reconciliation addendum's implementation order — made concrete.

## Problem

`Result[T, E]` is now real at **typecheck** time (two-param `Ty::Result(ok, err)` landed
on 2026-06-03; the `Err`/`Error` pattern payload can be typed as a user ADT `E`). But the
**interpreter** still throws away the error type: `VoxValue::Result` holds
`core::result::Result<Box<VoxValue>, String>`, so the Err side is always a `String`.

Consequences in `--mode interp`:

- `Error(MyErr::NotFound)` is coerced to its `Debug` string at construction
  (`eval/expr.rs:381-386`), so `match f() { Error(e) => e.code }` cannot get a field back
  out of `e` — `e` rematerializes as `VoxValue::Str("NotFound")`, not the ADT value.
- The typechecker promises `e: MyErr` on the `Error(e)` arm, but the runtime delivers
  `e: str`. Typed-error programs that pass typecheck **misbehave** at runtime.
- `map_err` is forced to require the closure return a `Str` (`eval/expr.rs:826-828`),
  contradicting the typed-error model.

This is the last layer keeping `Result[T, E]` from being real end-to-end.

## Verified current state (file:line, 2026-06-03)

### The representation

`crates/vox-compiler/src/eval/value.rs:30`:
```rust
Result(core::result::Result<Box<VoxValue>, String>),
```
`PartialEq` for the variant (`value.rs:77`) delegates to the inner `Result`'s `==`, which
already works for any `Box<VoxValue>` Err once the type changes — no manual arm edit needed.

### The single string-coercing constructor

`crates/vox-compiler/src/eval/expr.rs:381-386` — the `"Err" | "Error"` constructor arm is
the **only** place that flattens an arbitrary argument into a `String`:
```rust
"Err" | "Error" if eval_args.len() == 1 => {
    let msg = match eval_args.into_iter().next().unwrap() {
        VoxValue::Str(s) => s,
        other => format!("{other:?}"),   // <-- ADT values lost here
    };
    Ok(VoxValue::Result(Err(msg)))
}
```
`Ok` (`expr.rs:378-380`) already boxes an arbitrary `VoxValue` — only the Err side is lossy.

### The `?` operator

`crates/vox-compiler/src/eval/expr.rs:601-617` (`HirExpr::Try`). On `Result(Err(e))` it
re-wraps `e` unchanged into `_Return(Result(Err(e)))` (`expr.rs:608-609`). Because it moves
`e` through untouched, **this code is type-agnostic** — it compiles unchanged whether `e` is
`String` or `Box<VoxValue>`. No logic change needed; it just starts propagating typed errors
for free.

### Pattern matching the Err arm

`crates/vox-compiler/src/eval/stmt.rs:91-109` (`eval_pattern`, `VoxValue::Result` arm). The
`Err`/`Error` branch (lines 99-105) currently rewraps the string into a `Str`:
```rust
} else if (name == "Err" || name == "Error") && args.len() == 1 {
    if let Err(msg) = res {
        eval_pattern(interp, &args[0], VoxValue::Str(msg))?;   // <-- forces Str
```
With the wider type, `res` is `Err(Box<VoxValue>)`; bind `*boxed` directly.

### Result methods (`call_builtin_method`)

`crates/vox-compiler/src/eval/builtins.rs:678-728`, the `VoxValue::Result(res) => match method`
block. String-dependent arms:
- `err` (lines 686-690): wraps the error in `Box::new(VoxValue::Str(e.clone()))`.
- `unwrap` (693-697): `format!(... {e})` — needs `e: Display`; today `e: &String`.
- `unwrap_err` (701-705): returns `VoxValue::Str(e.clone())`.
- `expect` (718-727): `format!("Result.expect: {ctx} ({e})")`.

The `is_ok`/`is_err`/`ok`/`unwrap_or`/`unwrap_or_default` arms are error-payload-agnostic and
need no change.

### Closure-method arms (`apply_closure_method`)

`crates/vox-compiler/src/eval/expr.rs:816-843`:
- `map` (816-821): Err arm re-wraps `Err(e.clone())` — type-agnostic once `e: &Box<VoxValue>`.
- `map_err` (823-836): **calls the closure with `VoxValue::Str(e.clone())`** (line 826) and
  requires the result be a `Str` (827-834). This is the one arm whose *semantics* change.
- `and_then` (837-843): Err arm re-wraps `Err(e.clone())` — type-agnostic.

### Display

`crates/vox-compiler/src/eval/builtins.rs:2273-2299` (`vox_value_display`). `VoxValue::Result`
is **not** an explicit arm — it falls through to `_ => format!("{v:?}")` (line 2297), i.e. the
derived `Debug`. Type-name lookups at `builtins.rs:2142` and `builtins.rs:2250` return the
literal `"Result"` and are payload-agnostic.

### Stdlib construction sites

The stdlib builtins (file I/O, json/csv/toml/yaml parse, regex compile, process spawn, etc.)
build `VoxValue::Result(...)` from real Rust errors. Grep for the Err-producing forms in
`builtins.rs` (`Result(Err(`, `Err(e.to_string())`, `Err(e)`, `Err("...")`) returns **38**
matches; deduping the `Some(VoxValue::Result(res))` re-wraps from the actual `Err(...)`
*producers*, there are **≈24 distinct Err-value producers** in `builtins.rs` (e.g. lines
885, 902, 913, 931, 944, 972, 1003, 1019, 1043, 1070, 1089, 1100, 1344, 1371, 1389, 1416,
1425, 1485, 1512, 1521, 1567, 1577, 1588, 1595, 1608, 1619, 1632, 1643, 1656, 1672, 1685,
1692, 1704, 1734, 1748, 1756, 1875, 1884). All of these produce a `String` Err today. Two
more `Err`-producers live in `eval/shell_stdlib.rs` (lines 261-265, 302, 336) that flow into
`VoxValue::Result` through these call sites.

**These are all intrinsic, machine-origin errors.** They should remain *string* errors — wrap
each at its single fan-in point, not at all 24 sites (see design §"Migration").

## Design decision

### 1. Representation

Change `eval/value.rs:30` to:
```rust
Result(core::result::Result<Box<VoxValue>, Box<VoxValue>>),
```
Symmetric with `Option(Option<Box<VoxValue>>)` already above it. `PartialEq` (value.rs:77)
needs no edit.

### 2. `Error("string")` (current corpus) AND `Error(adtValue)` (new) both work

The constructor arm (`expr.rs:381-386`) stops flattening. It boxes whatever it is given,
exactly like the `Ok` arm:
```rust
"Err" | "Error" if eval_args.len() == 1 => Ok(VoxValue::Result(Err(Box::new(
    eval_args.into_iter().next().unwrap(),
)))),
```
- `Error("not found")` → `Err(Box::new(VoxValue::Str("not found")))`. Display, `err()`,
  `unwrap`'s message, and `match Error(e) => e` all see a `Str` — **byte-for-byte the same
  observable behavior** the corpus relies on today (the string is still a string).
- `Error(MyErr::NotFound { code: 7 })` → `Err(Box::new(VoxValue::Tagged{..}))`. The ADT
  survives; `match Error(e) => e.code` now works.

This collapses the `"Err" | "Error"` arm into the same one-liner as `Ok`; they could even be
merged, but keeping them separate preserves the readable diff.

### 3. `?` propagates a typed Err

No change to `expr.rs:601-617`. It already moves the boxed Err through untouched. After the
type change it carries `Box<VoxValue>` instead of `String`, so a `?` on a `Result[T, MyErr]`
in a function returning `Result[U, MyErr]` propagates the real `MyErr` value.

### 4. Pattern match binds the real value

`stmt.rs:100-101` becomes:
```rust
if let Err(boxed) = res {
    eval_pattern(interp, &args[0], *boxed)?;
```
`res` is already moved (it's `VoxValue::Result(res)` by value), so `*boxed` moves the inner
`VoxValue` out without a clone.

### 5. Methods

In `builtins.rs:678-728`:
- `err` (686-690): drop the `Box::new(VoxValue::Str(...))` wrap — the error is already a
  boxed `VoxValue`: `res.as_ref().err().map(|e| e.clone())`.
- `unwrap` (693-697): `format!("called \`Result.unwrap()\` on an Err value: {}",
  vox_value_display(e))` — route through `vox_value_display` so ADT errors render readably
  instead of `Debug`.
- `unwrap_err` (701-705): return `(**e).clone()` (the real error value) instead of
  `VoxValue::Str(e.clone())`. This is a behavior upgrade: `unwrap_err()` now yields the typed
  error, matching Rust.
- `expect` (718-727): `format!("Result.expect: {ctx} ({})", vox_value_display(e))`.

### 6. `map_err` semantics

`expr.rs:823-836`: pass the **real** error value to the closure and accept **any** return
value (it is the new error, of the closure's output type):
```rust
(VoxValue::Result(res), "map_err") => match res.as_ref() {
    Ok(v) => Ok(Some(VoxValue::Result(Ok(v.clone())))),
    Err(e) => {
        let mapped = apply_closure(interp, &closure, vec![(**e).clone()])?;
        Ok(Some(VoxValue::Result(Err(Box::new(mapped)))))
    }
},
```
This removes the `Str`-in/`Str`-out constraint and the `TypeError` arm at 830-834. `map`
(816-821) and `and_then` (837-843) need only `e.clone()` → kept as-is (they re-wrap the boxed
Err unchanged; they already compile against `&Box<VoxValue>`).

### 7. Display

Add an explicit `VoxValue::Result` arm to `vox_value_display` (builtins.rs:2273) so typed
errors print sensibly rather than via `Debug`:
```rust
VoxValue::Result(Ok(v))  => format!("Ok({})",  vox_value_display(v)),
VoxValue::Result(Err(e)) => format!("Error({})", vox_value_display(e)),
```
(Mirror the Option case if/when one is added; today Option also falls through to `Debug`, so
matching that is acceptable — but adding Result display is cheap and improves print output of
typed errors. Use `Error(` to match the surface keyword the corpus emits.)

### 8. Migration of the ≈24 intrinsic stdlib Err producers

These produce machine-origin `String` errors and should *stay* string-valued. Rather than
editing 24+ call sites, introduce one helper in `builtins.rs`:
```rust
#[inline]
fn err_str(s: impl Into<String>) -> Box<VoxValue> { Box::new(VoxValue::Str(s.into())) }
```
and change the **type** the stdlib match-blocks build. Each block currently shapes a
`core::result::Result<Box<VoxValue>, String>` and wraps it in `VoxValue::Result(res)`. The
mechanical migration is: at each producer, `Err(e.to_string())` → `Err(err_str(e.to_string()))`,
`Err(e)` (already-`String`) → `Err(err_str(e))`, `Err("literal".to_string())` →
`Err(err_str("literal"))`. The compiler drives this: after the `value.rs` type change, every
one of the 24 sites is a type error (`expected Box<VoxValue>, found String`), so none can be
missed. `shell_stdlib.rs` returns `Result<_, String>` internally (e.g. lines 261-265); those
inner signatures stay `String` and are wrapped with `err_str` only where they cross into
`VoxValue::Result` (the builtins.rs call site), so shell_stdlib needs no edit.

## Implementation steps (TDD sequence)

Each step: write the failing `.vox` repro / Rust unit test first, then make it pass.

1. **Repro test (red).** Add a `.vox` golden under the interpreter test corpus:
   ```vox
   enum MyErr { NotFound(int), Bad }
   fn f() to Result[int, MyErr] { return Error(MyErr::NotFound(7)) }
   match f() { Ok(v) => print(v), Error(e) => match e { MyErr::NotFound(c) => print(c), MyErr::Bad => print(-1) } }
   ```
   Run with `target/debug/vox.exe run`. Today it prints the `Debug` of a `Str`, not `7`.
   Lock the expected stdout `7`.

2. **Type change (value.rs:30).** Flip Err side to `Box<VoxValue>`. The build now fails at
   exactly the sites below — use the compiler as the worklist.

3. **Constructor (expr.rs:381-386).** Box the arg like `Ok`. Re-run the corpus that uses
   `Error("string")` — must stay green (string round-trips as `Str`).

4. **Pattern (stmt.rs:100-101).** Bind `*boxed`. Step-1 repro now binds the ADT.

5. **Methods (builtins.rs:686, 693, 701, 718).** Fix `err`, `unwrap`, `unwrap_err`, `expect`
   per §5. Add unit tests: `unwrap_err()` on `Error(MyErr::Bad)` returns the ADT; `unwrap()`
   panic message renders via `vox_value_display`.

6. **map_err (expr.rs:823-836).** Rewrite per §6; delete the `TypeError` arm. Test:
   `f().map_err(|e| OtherErr::Wrap(e))` yields `Error(OtherErr::Wrap(MyErr::Bad))`.

7. **Stdlib (builtins.rs ≈24 sites + shell_stdlib boundary).** Add `err_str`; wrap each
   remaining type error. Keep existing stdlib golden output identical (string errors
   unchanged). Re-run file/json/csv/regex golden suite.

8. **Display (builtins.rs:2273).** Add the `Result` arm. Update any golden whose expected
   stdout was the old `Debug` form (`Result(Err("..."))`) to the new `Error(...)` form.

9. **`?` propagation test.** Add a `.vox` test: a typed-error helper threaded through `?` in a
   caller returning the same `Result[T, MyErr]`; assert the propagated `Error(e)` keeps the
   ADT. (No code change in step — proves expr.rs:601-617 carries typed errors.)

10. **Corpus + golden.** Per the reconciliation addendum §"Corpus rule": once this lands, add
    one golden pairing a named error enum with `to Result[T, MyErr]`, an exhaustive `match` on
    the `Error` arm, and a `?`-propagation case. Run HumanEval + golden gates.

## Test strategy

- **Rust unit tests** (in `eval/builtins.rs`/`expr.rs` test modules): `err`, `unwrap`,
  `unwrap_err`, `expect`, `map`, `map_err`, `and_then` against an ADT-valued `Err`.
- **`.vox` behavioral goldens** run through `target/debug/vox.exe run`, asserting stdout:
  (a) `Error("string")` backward-compat, (b) ADT round-trip through `match Error(e)`,
  (c) `?` propagation, (d) `map_err` retype, (e) `unwrap` panic message rendering.
- **Regression**: full existing interpreter golden suite (the 24 stdlib sites must keep
  identical string-error output).
- **No cargo runs here** — author-time grounding only; CI runs the gates.

## Risks

- **Golden churn from display.** Any golden that printed a raw `Result` value gets the new
  `Error(...)` rendering (step 8). Mitigate by grepping goldens for `Result(Err` / `Result(Ok`
  in expected-output files before flipping display, and updating them in the same commit.
- **`unwrap_err` behavior change.** It now returns the typed value, not a `Str`. Any code
  relying on `unwrap_err()` being a string concatenable value could change. Search the corpus
  for `unwrap_err`; today it is rare. The new behavior matches Rust and the typed model.
- **Clone cost.** `(**e).clone()` deep-clones the error ADT on `err`/`unwrap_err`/`map`. Same
  cost profile as the existing `Ok` path (`ok()`/`unwrap` already clone `(**v)`), so no new
  regression class.
- **shell_stdlib boundary.** Easy to over-edit. Its internal `Result<_, String>` signatures
  must stay `String`; only the builtins.rs call sites wrap with `err_str`. The compiler will
  *not* flag a missed internal site (those are still `String`), so review the
  shell_stdlib→builtins handoff lines manually.

## Done when

- `match f() { Error(e) => e.field }` in `--mode interp` yields the **ADT field**, not a
  `Str`, for `f() to Result[T, MyErr]`.
- `Error("string")` corpus behavior is byte-for-byte unchanged.
- `?` propagates the typed `Error(...)` value through a caller of the same `Result[T, MyErr]`.
- `map_err(|e| ...)` receives the real error and may return any new error type.
- `unwrap`/`expect` panic messages render the error via `vox_value_display`.
- All 24 stdlib Err producers compile (string errors preserved) and their goldens are green.
- One `Result[T, MyErr]` behavioral golden added; HumanEval + golden gates pass.
