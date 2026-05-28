# Two-Parameter `Result[T, E]` — Implementation Plan

**Date:** 2026-05-18
**Status:** Proposed, awaiting approval before implementation begins.
**Closes:** the "Result is single-parameter in Vox v0.5" footnote that recurs across this session's commits (marquee Slot 2, marquee Slot 3, examples/golden/*, the audit doc's omission list).

## Why now

Every `Result[T]`-returning fn in the corpus loses error-type information at the typecheck level. The Err side stores a raw `String` per the interpreter's `VoxValue::Result(core::result::Result<Box<VoxValue>, String>)`, which forces all errors through one untyped channel. AGENTS.md `vox/types/anonymous-error-type` flags `Result[T, str]` as a boundary smell but the language has no way to express the alternative — until we ship `Result[T, E]`.

This blocks:
- Real ADT error types for `@endpoint` fns (currently `Result[str]` everywhere).
- The `Error(TitleEmpty(...))` shape I wanted to ship in the marquee todo-auth fixture (had to fall back to string errors per the commit fb811651b).
- Exhaustive `match` on user-declared error ADTs across the Err arm.

## Surface examples

What this enables:

```vox
type TodoError =
    | TitleEmpty(text: str)
    | NotFound(id: int)
    | NotAuthorized

@endpoint(kind: mutation) @auth(scheme: bearer)
fn add_todo(title: str) to Result[Id[Todo], TodoError] {
    if len(title) is 0 {
        return Error(TitleEmpty("title must not be empty"))
    }
    let id = db.Todo.insert({ title: title })?
    return Ok(id)
}

fn handle(t: Result[Id[Todo], TodoError]) to str {
    match t {
        Ok(id)                  => "created:" + str(id)
        Error(TitleEmpty(msg))  => "bad-title:" + msg
        Error(NotFound(id))     => "missing:" + str(id)
        Error(NotAuthorized)    => "forbidden"
    }
}
```

Today the AST drops the `E` argument and `Result[T, E]` resolves to `Result[T]` with `E` silently discarded.

## Where the surface lives today

The single-arg Result path is implemented in **four** layers; all need to grow a second slot:

| Layer | File | Current shape |
|---|---|---|
| AST/parser | [parser/descent/types.rs](crates/vox-compiler/src/parser/descent/types.rs) | `Result[T]` parses; `Result[T, E]` parses but the second arg is dropped during lowering |
| AST → Ty (3 paths) | [typeck/ast_decl_lints.rs:96](crates/vox-compiler/src/typeck/ast_decl_lints.rs:96), [typeck/infer.rs:21](crates/vox-compiler/src/typeck/infer.rs:21), [typeck/registration.rs:233](crates/vox-compiler/src/typeck/registration.rs:233) | All three lower `"Result"` → `Ty::Result(Box<Ty>)` (single param) |
| Internal `Ty` enum | [typeck/ty.rs:17](crates/vox-compiler/src/typeck/ty.rs:17) | `Ty::Result(Box<Ty>)` |
| Unify | [typeck/unify.rs](crates/vox-compiler/src/typeck/unify.rs) | `(Result(a), Result(b))` arm threads through one inner type |
| Runtime value | [eval/value.rs:19](crates/vox-compiler/src/eval/value.rs:19) | `Result(core::result::Result<Box<VoxValue>, String>)` — Err side is String, not VoxValue |
| Pattern match (eval) | [eval/stmt.rs:69](crates/vox-compiler/src/eval/stmt.rs:69) | `Err`/`Error` arm only extracts a string |
| Pattern match (typeck) | typeck/checker pattern paths | mirrors the runtime shape |
| Built-in constructors | [eval/expr.rs Ok/Err/Error case](crates/vox-compiler/src/eval/expr.rs) (added in c497b73b8) | Err/Error coerces the payload to String via `vox_value_display` |

A grep for `Ty::Result(` returns ~40 hits across the workspace; a grep for `VoxValue::Result(` returns ~10. Every one of them needs review for "does this pin the Err side to String, and if so does this change break it?"

## Proposed shape

### Ty changes

```rust
// Before
Result(Box<Ty>)

// After
Result(Box<Ty>, Box<Ty>)
```

Default `E` when omitted in source (single-arg `Result[T]`) lowers to `Ty::Str` to preserve current behavior. Migration: every existing `Result[T]` annotation continues to work; explicit two-arg form gains type-safety on the Err side.

### Runtime value changes

```rust
// Before
Result(core::result::Result<Box<VoxValue>, String>)

// After
Result(core::result::Result<Box<VoxValue>, Box<VoxValue>>)
```

This is the biggest blast radius — every match-and-string-format call site that does `if let Err(msg) = res` needs to handle `Box<VoxValue>` instead of `String`. Most coerce via `vox_value_display` already, but they'd need to walk the value rather than receive a String.

### Constructor coercion (eval/expr.rs)

Today the Err/Error constructor coerces non-string args via `vox_value_display`. After: it stores the VoxValue directly so pattern-matching can inspect it.

```rust
// Today
("Err", 1) | ("Error", 1) => {
    let v = eval_args.into_iter().next().unwrap();
    let msg = match v {
        VoxValue::Str(s) => s,
        other => super::builtins::vox_value_display(&other),
    };
    Ok(VoxValue::Result(Err(msg)))
}

// After
("Err", 1) | ("Error", 1) => {
    let v = eval_args.into_iter().next().unwrap();
    Ok(VoxValue::Result(Err(Box::new(v))))
}
```

### Pattern match changes (eval/stmt.rs:69)

```rust
// Today
} else if (name == "Err" || name == "Error") && args.len() == 1 {
    if let Err(msg) = res {
        eval_pattern(interp, &args[0], VoxValue::Str(msg))?;
        ...
    }
}

// After
} else if (name == "Err" || name == "Error") && args.len() == 1 {
    if let Err(payload) = res {
        eval_pattern(interp, &args[0], *payload)?;
        ...
    }
}
```

This is where the user-facing benefit lands: `Error(TitleEmpty(msg))` now matches the constructor pattern through to the payload variant, not just to a string.

## Migration shim — keeping the corpus green

The single biggest risk is breaking the ~60 places in the workspace that work with `Result[T]` as a single-param type. Mitigations:

1. **Source-level migration:** `Result[T]` in source resolves to `Result[T, Str]` automatically — no source-side breakage.
2. **Display path:** when an `Err` arm wants to print the payload, the existing `vox_value_display` helper already handles VoxValue → String. Replace `Err(msg)` in print/log sites with `Err(p) => vox_value_display(&p)`.
3. **Snapshot tests:** `cargo insta` will flag any test that pinned the old Err-side-is-String shape in serialized output. Each snapshot drift gets reviewed for "is the new shape correct, or did I break something?"
4. **Stages:** land the AST/HIR/Ty changes first (Phase 1), confirm `cargo test -p vox-compiler --lib` stays green, then the eval/runtime changes (Phase 2), then the constructor coercion (Phase 3).

## Phased implementation plan

### Phase 1 — Type-level wiring (no runtime change yet)

- Extend `Ty::Result(Box<Ty>, Box<Ty>)`; bump signature serialization.
- Update the 3 lowering paths to thread the second arg (default to `Ty::Str` when absent in source).
- Update `unify.rs` Result arm to unify both sides.
- Update display/signature: `Result[T, E]` round-trips correctly through `Ty::signature()`.
- Test: every existing `Result[T]` continues to compile. New `Result[T, E]` annotation in a new fixture compiles.

**Acceptance:** `cargo test -p vox-compiler --lib` 285→285 (no regressions). Marquee + audit suites still green.

### Phase 2 — Runtime value carrier

- `VoxValue::Result(core::result::Result<Box<VoxValue>, Box<VoxValue>>)`.
- Update every match-on-Err site (~10) to handle `Box<VoxValue>` instead of `String`. Most sites already pipe through `vox_value_display`; the change is one-shot mechanical.
- Pattern-match in `eval/stmt.rs:69` extracts the payload as VoxValue, not Str.
- Constructor in `eval/expr.rs` Err/Error case stores VoxValue directly.

**Acceptance:** the in-process @test runner against the seed corpus still reports 56/56 passing. The custom Err-ADT test from the proposal example actually pattern-matches end-to-end.

### Phase 3 — Pattern-match completeness

- Update typeck pattern paths to thread the E type through `Err(pat)`.
- Exhaustiveness checker (already has `missing_cases`): if E is a user-declared ADT, the Err-arm constructor patterns get exhaustiveness coverage.
- Update `vox-code-audit/anonymous_error.rs` detector — `Result[T, str]` is now an explicit choice; the detector should still flag it as a smell but with updated diagnostic text.

**Acceptance:** the proposal's `match` example with `Error(TitleEmpty(...))`, `Error(NotFound(...))`, `Error(NotAuthorized)` round-trips through both typecheck (exhaustiveness) and the in-process interpreter.

### Phase 4 — Corpus + Marquee updates

- Replace `Result[T]` → `Result[T, E]` in the marquee todo-auth and chat fixtures where the `E` is meaningfully ADT-shaped.
- Add 2-3 new humaneval-vox fixtures that exercise typed Err variants.
- Update the `Result is single-parameter in Vox v0.5` comments left in c5742f1cc, fb811651b, a0cd76255 to honor the new shape.

**Acceptance:** marquee fixtures still doctor-green with the upgraded error types. Seed corpus grows by 2-3 fixtures, all execute clean.

## Estimated effort

- Phase 1: ~3 hours focused work.
- Phase 2: ~4 hours (runtime/eval is the largest blast radius).
- Phase 3: ~2 hours (typeck pattern wiring).
- Phase 4: ~1 hour (fixture polish).

Total ~10 hours, comfortably distributed across two session blocks. Phase 1 and 2 are NOT independent — Phase 2 needs the typecheck to allow the new shape so the runtime can populate it. Phase 3 can land after Phase 2 even if the interpreter only sees Box<VoxValue> on Err.

## Risks

| Risk | Mitigation |
|---|---|
| Snapshot test drift across the workspace | `cargo insta review` per phase; each snapshot diff inspected for "is this correct?" |
| Some VoxValue::Result consumer I missed in the grep | The Rust compiler catches this at build time — adding a second field to the tuple variant forces every match arm to update. |
| Performance regression on the Err path (Box<VoxValue> vs String) | Negligible — Err paths are rare and already heap-allocated. |
| Single-arg `Result[T]` no longer parses | Mitigated by making the second arg defaulted (Ty::Str) at lowering time. |
| The `?` propagation operator (used everywhere in the corpus) needs to know the E type | Already typed — `?` propagates whatever the inner Result's Err side is. Just needs to thread the E parameter through. |
| Diagnostic shape changes — error_code consumers see new fields | The diagnostic envelope has serde-default-skipped optionals (per the minimal_repro precedent in cd2d130b6). Forward-compat works. |

## Out of scope for this plan

- Custom Try/? operator semantics for non-Result types (Option already auto-propagates per Phase-2 of the original Vox language spec).
- Cross-language emit changes (TS/Rust codegen). The codegen layer reads from HIR; once HIR carries `Result(T, E)`, emit can take its own follow-on.
- Two-parameter `Option[T, E]` or other ADT extensions — Result is the only one widely used as a return-type wrapper today.

## Approval criteria

- The shape (`Ty::Result(Box<Ty>, Box<Ty>)`, default `E = Str`) is acceptable.
- The phased rollout (type → runtime → pattern → fixtures) is acceptable.
- The 10-hour effort estimate is within budget.

If you say yes, I start Phase 1 immediately and check in between phases. If you say no, this design doc stays as a parking lot for whenever the issue is worth tackling.
