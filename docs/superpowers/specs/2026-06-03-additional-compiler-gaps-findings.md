# Additional interp/typeck/codegen correctness gaps — ranked findings

**Date:** 2026-06-03
**Status:** Findings (each item is an implementable mini-fix-spec)
**Track:** C1 (golden-corpus & compiler-reality). Companion to
[`2026-06-03-result-runtime-error-value-design.md`](2026-06-03-result-runtime-error-value-design.md).

## Problem

The just-landed `opt is None` fix closed one Constructor-vs-canonical equality
divergence (`eval/expr.rs:170-175` now calls `normalize_constructor` on both
sides of `is`/`isnt`). This is a fresh scan for *additional* correctness bugs of
the same family: programs that **pass `vox check` but crash or misbehave under
`vox run --mode interp`**, and interp-vs-codegen-intent divergences.

All repros below were run against the prebuilt binary
`target/debug/vox.exe` on 2026-06-03. `--mode script` is not compiled into this
binary (`script-execution` feature off), so divergences are stated against the
**typechecker's declared semantics** (the SSOT the codegen path also honors),
not against a live script run.

The findings are ranked by value (typecheck-passes-then-crashes outranks
already-error cases; broad-surface outranks narrow).

---

## Finding 1 — `for` over a map or string typechecks but crashes at runtime (HIGH)

**Files:** interp `crates/vox-compiler/src/eval/expr.rs:503-533`; typeck
`crates/vox-compiler/src/typeck/checker/expr_ops.rs:10-22` and
`crates/vox-compiler/src/typeck/checker/expr.rs:580-601`.

**Repro (map):**
```vox
fn main() {
  let m = {"a": 1, "b": 2}
  for k, v in m { print(k) print(v) }
}
```
```
$ vox check     → Check passed with 0 warning(s)
$ vox run --mode interp → Error: Eval failed calling main: TypeError { expected: "List", found: "other" }
```

**Repro (string):**
```vox
fn main() { for c in "abc" { print(c) } }
```
`vox check` passes; interp errors with the same `TypeError { expected: "List" }`.

**Why:** `extract_iterable_element` (expr_ops.rs:13-17) explicitly declares
`Map(k,v)` iterable (yields `Tuple(k,v)`), `Set`, `Stream`, and `Str` (yields
`Char`). The interpreter's `HirExpr::For` arm only matches `VoxValue::List` and
falls to the `else` `TypeError` for every other receiver (expr.rs:528-532).

**Severity:** HIGH. Map and string iteration are bread-and-butter loops; both
typecheck clean and both crash. This is the single most likely silent failure
in real `.vox` programs.

**Fix sketch:** Extend the `HirExpr::For` arm to convert the iterable into a
`Vec<VoxValue>` before the loop, mirroring the typechecker's element types:
`VoxValue::List(ls) => ls`; `VoxValue::Object(pairs) =>` one
`VoxValue::Tuple(vec![Str(k), v])` per entry (so the single-binding form binds
the pair and `for k, v` can destructure — see Finding 1b); `VoxValue::Str(s) =>`
one `VoxValue::Str(c.to_string())` per `char`; `VoxValue::Set`/`Stream` if those
runtime values exist. Keep the existing `_Return/_Break/_Continue` propagation.
Add an explicit error only for genuinely non-iterable receivers (Int, Bool).

### Finding 1b — two-binding `for k, v in map` binds `v` to an Int index, not the value (HIGH, rides on 1)

**File:** typeck `expr.rs:589-597`; interp `expr.rs:510-512`.

The `index` (second binding) is **always** bound to `Ty::Int` in the
typechecker (expr.rs:592-596) and to `VoxValue::Int(i)` (the loop counter) in
the interpreter (expr.rs:511). For a `List` that is the intended
"value, index" enumerate form. But for a `Map`, the element type is the
`Tuple(k,v)` and the natural `for key, value in map` reading wants `value` =
the map value, **not** an integer counter. So even once Finding 1 makes maps
iterate, `for k, v in m` would bind `v` to `0,1,2,...`. **Decision required:**
either (a) keep enumerate semantics and document `for pair in map` +
`pair[0]/pair[1]`, or (b) special-case the two-binding form over a `Map` to
destructure the `(k,v)` tuple in both typeck and interp. Recommend (b) for
ergonomics; gate it on the iterable being a `Map` so list-enumerate is
unchanged. Fix both the typeck binding (bind 2nd name to the value type for
maps) and the interp loop (set 2nd binding to the value, not `i`).

---

## Finding 2 — mixed Int/Float arithmetic typechecks but crashes, with no workaround (HIGH)

**Files:** typeck `crates/vox-compiler/src/typeck/checker/expr_ops.rs:43-75`;
interp `crates/vox-compiler/src/eval/expr.rs:242-252`.

**Repro:**
```vox
fn main() { let x = 1  let y = 2.0  print(x + y) }
```
```
$ vox check → Check passed with 0 warning(s)
$ vox run --mode interp → Error: ... AssertionFailed("unsupported binary op `Add` for operands Int and Float")
```

**Why:** The typechecker promotes mixed Int/Float to `Float` for `+`
(expr_ops.rs:54) and for `- * / %` (expr_ops.rs:70). The interpreter has arms
only for `Int op Int` and `Float op Float`; every mixed pair falls into the
catch-all error at expr.rs:248-252.

**The workaround named in the code does not exist.** The interp comment
(expr.rs:245) tells the user to call `to_float()` to opt into mixed arithmetic,
but `to_float` is registered **only on `str`** (typeck/builtins.rs:1244-1248),
not on `int`. `vox check` on `let y = x.to_float()` where `x: int` errors
`Method to_float not found on Int`. So a typecheck-clean `1 + 2.0` has **no
runtime-safe rewrite** in interp.

**Severity:** HIGH. Any arithmetic mixing an int literal/var with a float (very
common) passes the type gate and dies. Includes the comparison operators: `1 <
2.0` also crashes (`AssertionFailed("unsupported binary op `Lt` ...")`) while
typeck unconditionally returns `Bool` for `< > <= >=` (expr_ops.rs:76).

**Fix sketch:** Align interp with the declared promotion. Add mixed arms before
the catch-all in `HirExpr::Binary`: for `Add/Sub/Mul/Div`, when one operand is
`Int(i)` and the other `Float(f)`, promote the Int to `f64` and compute in
`Float` (matching expr_ops.rs:54/70 → `Ty::Float`). Do the same for
`Lt/Gt/Lte/Gte` (compare as `f64`). Preserve the existing pure-Int integer
division-by-zero / overflow checks for the Int/Int path; the mixed path is
float division so it follows IEEE semantics (matches codegen). Leave Decimal
mixing as an error (typeck does not promote Decimal with Int/Float). Independent
follow-up: either implement `int.to_float()`/`int.to_int()` or scrub the
misleading comment at expr.rs:244-247.

---

## Finding 3 — cross-numeric-variant `is` equality silently returns false (MED)

**File:** `crates/vox-compiler/src/eval/value.rs:60-92` (`PartialEq`); reached
via `eval/expr.rs:170-171`.

**Repro:**
```vox
fn main() { print(1 is 1.0) }   // prints: false
```
`vox check` passes (`is` always types to `Bool`, expr_ops.rs:77).

**Why:** `PartialEq` has no `(Int, Float)` / `(Float, Int)` arm; the catch-all
`_ => false` at value.rs:89 makes any value of numerically-equal-but-different
variant compare unequal. This is the **same shape** as the original
`None`/`Constructor` bug — a missing variant-pair arm dropping into `_ => false`
— except `normalize_constructor` does not help here because neither side is a
constructor.

**Severity:** MED. Less common than Findings 1-2 (users rarely compare across
numeric types), but it is a silent wrong-answer (no error), which is worse per
unit than a crash, and it directly contradicts Finding 2's promotion rule: `1 +
2.0` is meant to be float-ish, yet `1 is 1.0` says false.

**Fix sketch:** Decide the contract first. If Vox wants value equality across
numeric types (consistent with arithmetic promotion in Finding 2), add
`(Int(a), Float(b)) | (Float(a), Int(b)) => *a as f64 == *b` arms to `PartialEq`
(and the Decimal/Int, Decimal/Float pairs if desired). If Vox wants strict
same-representation equality, then **`is` across distinct numeric types should
be a typecheck error**, not silently `false` — add a check in `check_binary_op`
for `Is/Isnt` that rejects unrelated operand types. Either way the current
"typechecks, returns false" behavior is a footgun. Recommend the typecheck-error
route: it is the lower-risk change and matches the strict-Option philosophy
elsewhere in the checker.

---

## Finding 4 — `Constructor` value leaks past equality for unapplied multi-arg ADT constructors (LOW)

**File:** `eval/expr.rs:18-23` (`normalize_constructor`), value.rs:78.

**Why:** `normalize_constructor` only canonicalizes the **`None`** constructor
(expr.rs:20). A bare unapplied user constructor used as a value (e.g. comparing
a zero-arg ADT variant `Color::Red` against another) stays
`VoxValue::Constructor(name)` and compares via value.rs:78
(`Constructor(a) == Constructor(b)` → name equality). That arm exists, so
same-name bare constructors compare correctly — but a bare zero-arg ADT variant
will **not** equal its applied `Tagged { name, fields: [] }` form if one side
was constructed via a call and the other referenced bare. `None` is the only
constructor for which the two forms are unified.

**Severity:** LOW. Requires a zero-arg user ADT variant compared in mixed
bare/applied form, which is rare and not exercised by the corpus today.

**Fix sketch:** Generalize `normalize_constructor` to also fold a bare zero-arg
ADT `Constructor(name)` into `Tagged { name, fields: vec![] }` when the
constructor's declared arity is zero (the interpreter has the ADT decl table in
`module_scope`/`run_module`). Lower risk: leave as-is and document that bare
ADT constructors must be applied (`Color::Red()`); only revisit if a golden
file needs it.

---

## Finding 5 — interp arms that are reachable only via typecheck escape hatches (LOW / audit-only)

**Files:** `eval/expr.rs:393-396`, `eval/expr.rs:535-550`.

The function-call catch-all (`expr.rs:393` `_ => Err(TypeError { expected:
"function" })`) and the field-access non-Object catch-all (`expr.rs:546`
`expected: "Object"`) are defensive arms that the typechecker should make
unreachable for well-typed programs. They are **not** bugs on their own, but
they emit the low-information `found: "other"` string (the same opaque
diagnostic that masked Findings 1-2 above). When any of Findings 1-3 fires
through these paths, the user sees `found: "other"` instead of the real type.

**Severity:** LOW (diagnostics quality, not correctness).

**Fix sketch:** Replace the literal `"other".into()` in the `found` field at
expr.rs:531, 548, 395 with `vox_value_type_name(&v)` (already used at
expr.rs:250 for the binary-op error) so the runtime diagnostic names the actual
offending type. Cheap, improves every future divergence report.

---

## Test strategy (TDD sequence)

Write the failing tests first, in the interpreter integration suite that drives
`run_module`/`eval_expr` (same harness the existing `eval` tests use). For each
finding, the test asserts **typecheck-clean + correct interp result**, which is
exactly the invariant that is currently broken:

1. **Finding 1:** `tests` — assert `for k, v in {"a":1}` and `for c in "ab"`
   each run without error and produce the expected printed/collected values.
   Add a `vox check` assertion that they already pass (regression guard that the
   gap was typecheck-passes).
2. **Finding 1b:** assert two-binding map iteration binds value (not index);
   gate on the Finding-1b decision.
3. **Finding 2:** assert `1 + 2.0 == 3.0`, `2.0 + 1 == 3.0`, `1 < 2.0 == true`,
   `2.0 * 2 == 4.0`; assert Decimal-mixed still errors.
4. **Finding 3:** assert the chosen contract — either `1 is 1.0 == true`, or
   `vox check` rejects `1 is 1.0`. Pin whichever is chosen with a test so it
   can't silently regress.
5. **Finding 5:** assert the runtime error message for a forced non-function
   call names the real type, not `"other"`.

Run the full interpreter suite plus the golden corpus after each fix to confirm
no previously-passing golden regresses (the corpus has parse-only goldens that
may start *executing* once iteration works — watch for new behavioral diffs).

## Risks

- **Finding 2 / Finding 3 interaction:** promoting mixed arithmetic to Float but
  leaving `1 is 1.0` false is internally inconsistent. Decide both together.
- **Finding 1b is a semantics decision**, not a pure bug fix — landing
  enumerate-vs-destructure changes the meaning of existing `for k, v in map`
  code (currently none, since it crashes). Choose before implementing.
- **Float promotion changes division semantics**: `1 / 2` (Int/Int) stays `0`
  (integer), but `1 / 2.0` becomes `0.5`. This matches the typechecker
  (`Int/Int → Int`, mixed `→ Float`) and codegen intent, but is a sharp edge to
  document.
- Generalizing `normalize_constructor` (Finding 4) touches the `?`-operator and
  `return` paths that also call it (expr.rs:605); regression-test those.

## Done when

- `vox check` and `vox run --mode interp` agree on: map iteration, string
  iteration, mixed Int/Float arithmetic and comparison, and cross-numeric `is`
  (per the chosen contract).
- No typecheck-clean snippet in Findings 1-3 produces a runtime
  `TypeError`/`AssertionFailed`.
- Runtime divergence diagnostics name the real value type, not `"other"`.
- New regression tests for each finding pass; full interp + golden suites green.
