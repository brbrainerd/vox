---
title: "RFC: Closures in Vox (Phase G — Bucket-A v0.6)"
description: "Grammar, type rules, lowering, and corpus-impact analysis for first-class closures. The single highest-leverage Bucket-A feature per the 2026-05-23 stdlib-gap audit."
category: "Architecture SSOTs"
status: "research"
last_updated: "2026-05-23"
training_eligible: false
training_rationale: "RFC in design phase; promote to training_eligible once status reaches 'current' (after implementation lands and corpus stabilizes)."

schema_type: "TechArticle"
---

# RFC: Closures in Vox

> **Status:** draft (2026-05-23). Reviews welcome from `vox-compiler`,
> `vox-codegen`, and `mens-corpus` owners.
> **Authorship:** scaffolded as part of the 2026-05-23 stdlib-gap session
> (see [`vox-stdlib-gap-audit-2026-05-23.md`](./vox-stdlib-gap-audit-2026-05-23.md) §12 Phase G).
> **Bar to land:** approved by the council; tests in
> `crates/vox-compiler/tests/closures_*.rs`; corpus migration to use
> closure-based `.map`/`.filter`/`.and_then`.

## §1 — Motivation

The stdlib-gap audit found that **~25 of the 35 failing `scripts/`
entries** fail because they want to write:

```vox
// vox:skip
let names = files.map(|f| f.path)
let big = items.filter(|x| x.size > threshold)
let m = result.map(|v| transform(v))
```

Without closures, every collection transformation needs either a named
top-level `fn` (verbose), a manual `for` loop (loses functional style),
or a workaround that loses static-checkability. This is the single
largest **lexical K-complexity** drag on AI-written Vox today: a generated
`.map(|x| x.foo)` is a 4-token line; the workaround is a multi-line loop.

Closures also unblock the Option/Result method completion (Phase G §15
of the audit doc): `.map`, `.and_then`, `.map_err`, `.filter` — every one
of these takes a closure. Without closures, the function-set is stuck at
`unwrap`/`unwrap_or`/`is_some`/`is_none`/`is_ok`/`is_err`. With closures,
it becomes the full Rust-ish API surface.

## §2 — Grammar

### §2.1 — Closure literal

```bnf
closure-expr := "|" param-list "|" closure-body
param-list   := ε
             |  param ("," param)*
param        := IDENT (":" type-annotation)?     // type optional; inferred from call site
closure-body := expr                              // single-expression form
             |  "{" stmt* expr? "}"              // block form
```

Examples:

```vox
// vox:skip
|x| x * 2
|x: int| x.to_string()
|x, y| x + y
|line| {
    let trimmed = line.trim()
    trimmed.starts_with("//")
}
```

**Disambiguation from `|` boolean-or:** the lexer already lexes `|` as
`Token::Pipe` (used in or-patterns). The parser distinguishes a closure
literal from a boolean-or expression by **position** — a leading `|` in
expression position starts a closure; a `|` between two expressions in
arithmetic position is boolean-or. The Pratt parser handles this with a
prefix-position check on the lookahead token after `|`: if it's an
identifier (closure param) or another `|` (zero-param closure), it's a
closure; otherwise (a literal, an open-paren, etc.) it's an or.

### §2.2 — Zero-argument closures

```vox
// vox:skip
let lazy = || expensive_compute()
```

The empty-param form `||` (no whitespace) is parsed as a single token
`Token::PipePipe` today (used in some contexts for or-patterns) — we'll
need to either split that or special-case the closure parser to accept
`||` as `<empty params>`.

### §2.3 — Type annotation on closure params

Optional but supported, matching Rust:

```vox
// vox:skip
xs.filter(|x: User| x.is_active)
```

When omitted, the type is inferred from the receiver's expected
parameter type (most commonly via the method-receiver context).

## §3 — Type system

### §3.1 — Function type already exists

`Ty::Fn(Vec<Ty>, Box<Ty>)` is in
[`crates/vox-compiler/src/typeck/ty.rs`](../../../crates/vox-compiler/src/typeck/ty.rs)
and is used by named functions today. Closures reuse this type — a
closure literal has type `Ty::Fn(param_tys, return_ty)`.

### §3.2 — Inference

Closures appear primarily as method arguments. The method signature
already names the expected closure shape — for example
`List<T>::filter(predicate: fn(T) -> bool) -> List<T>` (already in
typeck/builtins.rs line ~482). Inference flow:

1. Type-check the method receiver to get `T`.
2. Look up the method signature; param[0] is `Ty::Fn(vec![T], Box::new(Bool))`.
3. Type-check the closure body with the inferred param-type binding.
4. Unify the closure's return type with the expected return.

This is unification-driven and doesn't require new HIR machinery — the
existing `Ty::Fn` and unification machinery suffice.

### §3.3 — Closures over local bindings (captures)

A closure captures the lexical scope's bindings by reference (interp) or
by clone (eval). The interp's `Interpreter` already has a `Scope` that
forms a lexical chain; closures package up the scope at the point of
creation. Existing `VoxValue::Fn { params, body, env: Scope }` already
captures this shape — closures reuse it.

## §4 — HIR lowering

```rust
// AST → HIR
Expr::Closure { params, body, span }
    → HirExpr::Lambda(params: Vec<(String, Ty)>, body: Box<HirExpr>, span: Span)
```

A new `HirExpr::Lambda` variant. Existing `eval::value::VoxValue::Fn`
shape already aligns with what lowering produces — eval just constructs
a `Fn` with the captured scope.

## §5 — Eval / interp

```rust
// eval/expr.rs — new arm
HirExpr::Lambda(params, body, _) => {
    Ok(VoxValue::Fn {
        params: params.iter().map(|(n, _)| n.clone()).collect(),
        body: vec![/* convert body */],
        env: interp.scope.clone(),  // capture-by-clone for interp
    })
}
```

Method dispatch (`call_builtin_method`) already accepts function
arguments — `xs.map(closure)` works when `xs` is a `List` and `closure`
is `VoxValue::Fn`. The List.map dispatch in
[`crates/vox-compiler/src/eval/builtins.rs`](../../../crates/vox-compiler/src/eval/builtins.rs)
needs a small extension: when arg[0] is a `VoxValue::Fn`, apply it to
each element.

```rust
// List.map(f) — pseudocode
VoxValue::List(items) => match method {
    "map" => {
        let closure = args.into_iter().next()?;
        let mapped: Vec<VoxValue> = items.iter().map(|item| {
            apply_closure(&closure, vec![item.clone()])
        }).collect();
        Some(VoxValue::List(mapped))
    }
    ...
}
```

Where `apply_closure` looks up the `Fn` variant, extends its captured
env with the param binding, and runs the body.

## §6 — Codegen (TS / Rust emit)

### §6.1 — TS emit

Closures become arrow functions:

```vox
// vox:skip
|x| x * 2     →    (x: T) => x * 2
|x| { ... }   →    (x: T) => { ... }
```

The TS emitter already produces arrow functions for named local
functions; closures reuse that path.

### §6.2 — Rust emit (vox-codegen Rust path)

Closures become Rust closures:

```vox
// vox:skip
|x| x * 2     →    |x: T| x * 2
```

The Rust emitter needs to determine the closure's move/borrow shape —
the simplest approach is `move` closures (capture-by-value), matching
the interp's clone semantics. Move closures avoid lifetime parameters
in generated code.

## §7 — Migration impact

### §7.1 — Corpus expected to unblock

Per the 2026-05-23 audit, ~25 scripts use closure-needing patterns:

- `scripts/check_dashboard_ssot.vox` — `.filter_map` chain with closure
- `scripts/quality/doc-policy-lint.vox` — `.map`/`.filter` over file lists
- `scripts/extract_table_names.vox` — `if let Some(m) = re.find(line)` (closure-adjacent)
- `scripts/mens-corpus/jsonl_writer.vox` — closure-based file iteration
- `scripts/ci-proximity-drift.vox` — `.map(|x| format!(...))` patterns
- ... and ~20 more

After closures land, pass rate should jump from 20/53 to ~40-45/53. The
final scripts blocked are heavy Rust-syntax authoring (full
`std::process::Command` chains) — these are Bucket C (corpus rewrite,
not language feature).

### §7.2 — `vox check`-side method completion

With closures in the language, typecheck registers the closure-taking
variants:

```rust
// crates/vox-compiler/src/typeck/builtins.rs — additions
result_methods.insert("map".into(), Ty::Fn(
    vec![Ty::Fn(vec![Ty::GenericParam(0)], Box::new(Ty::GenericParam(1)))],
    Box::new(Ty::Result(Box::new(Ty::GenericParam(1)))),
));
result_methods.insert("and_then".into(), Ty::Fn(
    vec![Ty::Fn(
        vec![Ty::GenericParam(0)],
        Box::new(Ty::Result(Box::new(Ty::GenericParam(1)))),
    )],
    Box::new(Ty::Result(Box::new(Ty::GenericParam(1)))),
));
result_methods.insert("map_err".into(), Ty::Fn(
    vec![Ty::Fn(vec![Ty::Str], Box::new(Ty::Str))],
    Box::new(Ty::Result(Box::new(Ty::GenericParam(0)))),
));
// ... and same shape for Option, List
```

The parity test (`eval_typeck_parity_test.rs`, added 2026-05-23) will
gate that each new typeck signature has matching eval dispatch.

## §8 — Risks / open questions

1. **`|` parser ambiguity** — needs lookahead-driven prefix-position
   handling. Risk of regressing or-pattern parsing in match arms. **Mitigation:** add a parser test matrix covering `match x { 1 | 2 => …, … }`, `let f = |x| x`, `let r = a | b`, `let n = match x { y | z => y }`.

2. **Capture mode** — interp clones (cheap, correct, no lifetimes);
   Rust-emit needs to pick `move` vs `Fn`/`FnMut`. **Open question:**
   if a closure mutates a captured `let mut`, does it FnMut-capture, or
   is mutation forbidden? **Proposal:** forbid in v0.6; revisit if
   corpus demands it (currently none of the 25 closure-using scripts
   need capture-mut).

3. **Generic param inference in higher-order positions** — `xs.map(|x| ...)`
   needs the typechecker to thread `T` from the receiver to the closure
   param. The existing generic-param infrastructure
   ([`crates/vox-compiler/src/typeck/unify.rs`](../../../crates/vox-compiler/src/typeck/unify.rs))
   handles this for named functions today; closures should fall out
   without new machinery.

4. **Closure → `Ty::Fn` vs typed-but-anonymous newtype** — keep the type
   as plain `Ty::Fn(params, ret)`. No subtyping, no opaque-existential
   surface. Trade-off: a function pointer and a captured-closure look
   identical at the type level, which simplifies the type system but
   loses the "this closure captures `x`" diagnostic. Acceptable for v0.6.

5. **What about `fn` as a closure expression?** — eg. `xs.map(fn(x) { x * 2 })`.
   This is supported syntactically as an anonymous function. **Proposal:**
   support it as a synonym for the `||` form — both produce
   `HirExpr::Lambda`. The `||` form is canonical; `fn(...)` is for
   when the syntax helps a reader.

## §9 — Implementation plan

| Step | Scope | Cost |
|---|---|---|
| **9.1** | Lexer: confirm `|` and `||` tokens work; add tests for closure-position recognition | half day |
| **9.2** | Parser: `parse_closure_expr` prefix-position handler; or-pattern tests stay green | 1 day |
| **9.3** | HIR: `HirExpr::Lambda` + lowering | half day |
| **9.4** | Typeck: closure inference + receiver-driven param-type binding; new tests in `tests/closures_typecheck.rs` | 1.5 days |
| **9.5** | Eval: `HirExpr::Lambda → VoxValue::Fn` + `apply_closure` helper; extend `List.map/filter/and_then`, `Option.map/and_then`, `Result.map/and_then/map_err` | 2 days |
| **9.6** | Typeck stdlib: add closure-taking method signatures matched by parity test | 1 day |
| **9.7** | Corpus migration: rewrite scripts that have closure workarounds | 1 day |
| **9.8** | Codegen: TS + Rust emit | 2 days |
| **9.9** | Doc updates: tutorial, ref-builtins-stdlib.md, audit doc §11/§12 | half day |

**Total: ~10 days focused work.** Real elapsed could be 2–3 weeks.

## §10 — Out of scope for v0.6

- **Async closures** — wait for v0.7 once async is firmed up.
- **`impl Trait` / opaque return types** — Vox doesn't have these today;
  closures return `Ty::Fn` always.
- **Closure traits hierarchy** (`Fn`/`FnMut`/`FnOnce`) — single tier in
  v0.6; everything is effectively `move | clone`.

## §11 — Decision needed before implementation (re-done 2026-05-23 for long-term language health)

The first pass of these questions optimized for "least implementation
surface". The user pushed back: pick what's best for the language
long-term, even if it costs more upfront. This section is the
health-first re-do.

### Q1 + Q2 (combined) — anonymous function syntax

**Decision: anonymous functions use the `fn(params) { body }` syntax
ONLY. No `|x| ...` pipe form. No `||` zero-param form.**

Rationale (health > ergonomics):

1. **One canonical form per concept.** Vox's session-long discipline has
   been "lower K-complexity by removing alternate spellings"
   (`println`→`print`, `!`→`not`, `@endpoint(kind: query)`→`@query`).
   Two anonymous-fn syntaxes (`|x|` and `fn(x)`) re-introduces the
   same K-complexity tax we removed elsewhere.

2. **Match the named-fn shape.** Vox already has `fn name(params) { body }`
   for named functions. An anonymous fn is "the same minus the name":
   `fn(params) { body }`. One mental model. The `|x|` syntax forces an
   AI (or human) to learn a separate construction.

3. **No `||` parser ambiguity.** Rust's `||` zero-param closure shares
   a token with boolean-or, requiring position-aware Pratt parser
   logic. Every parser test on closures becomes a precedence test on
   `or`. C++ has the same papercut. Vox has the runway to avoid it
   entirely.

4. **AI-first weighting.** AI generators handle character count well;
   they handle ambiguity poorly. `fn(x) { x * 2 }` is 5 characters
   longer than `|x| x * 2` but zero parser ambiguity. The trade is
   correct for an AI-first language.

5. **Pedagogical clarity.** A new reader sees `fn(x) { x * 2 }` and
   immediately reads "anonymous function taking x". Reading `|x| x * 2`
   requires knowing the closure syntax exists at all.

Example shapes:

```vox
// vox:skip
xs.map(fn(x) { x * 2 })
xs.filter(fn(x) { x.is_active })
xs.fold(0, fn(acc, x) { acc + x.cost })
let lazy = fn() { expensive_compute() }
```

Tradeoff acknowledged: per call site this is 3–5 more characters than
the pipe form. The audit values this loss as < 1% of corpus token
count and well below the K-complexity cost of two syntaxes for one
concept.

### Q3 — Capture-mut

**Decision: closures CANNOT mutate captures. v0.6 AND v1.0.**

Rationale:

- Captured state in closures is the #1 footgun in JS, Python, and Rust
  (Rust solves it via FnMut tier; JS/Python via "is `i` `let` or `var`?").
  All three solutions surface in confused-LLM-output reports.
- An AI-first language should make accumulator/state patterns explicit
  via `fold`/`scan`/`reduce`-style operations, not implicit capture-mut.
- If state IS needed, users define a named `fn` with explicit `mut`
  parameter passing or use a stateful struct. Slightly more code; far
  fewer surprises.
- No `FnMut`/`Fn`/`FnOnce` tier in Vox. Single closure trait.

### Q4 — Generic-param inference + diagnostic quality bar

**Decision: HM inference is the default, BUT the diagnostic quality bar
for closure inference failures is a release blocker.**

If inference fails, the error message MUST:

- Name the closure param whose type couldn't be inferred.
- Name the receiver/context that should have provided the type.
- Suggest the explicit annotation that would fix it.

Acceptance example:

```
error: couldn't infer type of `x` in closure
  --> foo.vox:12:14
   |
12 |     xs.map(fn(x) { x.length() })
   |              ^
   |
   = note: receiver `xs` has type `list[Item]`; expected closure
           argument type is `Item`.
   = help: add an explicit annotation:  fn(x: Item) { x.length() }
```

This is non-negotiable. The audit doc §6 P0 list adds a CR-L gate for
closure-error message quality.

### Q5 — `return` inside a closure body

**Decision: `return` returns from the CLOSURE, not the enclosing fn.**

Lua/JS/Python semantics. Rust's "return from outer scope" through
labeled blocks is a known confusion point — even seasoned Rust authors
get it wrong with `try {}`-style blocks. AI-first: pick the obvious
behavior.

### Q6 — Last-expression semantics

**Decision: a closure's block ends with an expression-statement that is
the return value, matching named-`fn` behavior.**

```vox
// vox:skip
fn(x) {
    let doubled = x * 2
    doubled + 1            // ← return value
}
```

No `return` needed for the final expression. Consistent with named
fns; consistent with Rust/Ruby/OCaml.

### Q7 — Self-recursion in anonymous functions

**Decision: anonymous functions CANNOT refer to themselves. Use a named
`fn` for recursion.**

Self-reference requires `let rec` syntax (OCaml) or explicit binding
shape that complicates type inference. Vox v0.6 keeps anonymous fns
strictly anonymous; recursive functions are named.

### Q8 — Currying / partial application

**Decision: NO automatic currying. Use explicit closures for partial application.**

```vox
// vox:skip
let add_one = fn(y) { add(1, y) }    // ✓ canonical
let add_one = add(1, _)              // ✗ not supported
```

Currying is clever but adds inference complexity ("does `f(1)` mean
'call with one arg' or 'partially apply'?"). One canonical form;
zero ambiguity.

### Q9 — Pattern matching in closure parameters

**Decision: closure params are simple identifiers in v0.6. Pattern
matching in params (destructuring `|{a, b}|`, variant patterns
`|Some(x)|`) is deferred to v0.7.**

Pattern-matching params are a real win for ergonomics but conflict with
type inference in the closure-call-site context. v0.7 can revisit once
v0.6 closures land and we have real usage data.

### Q10 — Capture mode under codegen

**Decision: Rust-emit produces `move` closures (capture-by-value).**

Matches the interp's clone semantics — behavior is identical across
modes. `move` avoids lifetime parameters in generated code, which would
otherwise leak into every function signature that takes a closure.

### Q11 — Closure trait hierarchy

**Decision: single tier. No `Fn`/`FnMut`/`FnOnce` distinction. All
closures are `Ty::Fn`. All callsites accept by-value.**

Vox doesn't carry Rust's ownership system; it doesn't need Rust's
closure trait hierarchy. Simpler type system, fewer corner cases.

### Summary of v0.6 closure design

| Aspect | Decision |
|---|---|
| Syntax | `fn(params) { body }` — anonymous variant of named-fn syntax |
| Zero-param | `fn() { body }` |
| Body | block with last-expr-is-return; matches named fn |
| Capture | by-value (clone in interp; `move` in Rust-emit) |
| Mutate-capture | forbidden |
| Recursion | not supported; use named fn |
| Currying | not supported; use explicit fn wrapper |
| Pattern params | not supported in v0.6; defer to v0.7 |
| `return` | returns from closure |
| Closure trait | single tier (`Ty::Fn`) |
| Inference | HM with release-blocking diagnostic quality |

The audit accepts the K-complexity tax of `fn(x) { … }` over `|x| ...`
because Vox is an **AI-first language where ambiguity is the enemy**.
Every extra character in a call site is paid back many times over by
the absence of a "two ways to write this" decision the AI has to make.

## Related plans

- [`vox-stdlib-gap-audit-2026-05-23.md`](./vox-stdlib-gap-audit-2026-05-23.md) §11.4 / §12 Phase G — the demand signal
- [`external-frontend-interop-plan-2026.md`](./external-frontend-interop-plan-2026.md) — TS-emit ↔ closure alignment
- [`v1-llm-target-implementation-plan-2026.md`](./v1-llm-target-implementation-plan-2026.md) — Bucket-A scheduling within the broader v1 roadmap
