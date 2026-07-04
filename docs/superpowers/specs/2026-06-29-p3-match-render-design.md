---
title: "P3 — match-in-render lowering (design)"
date: 2026-06-29
status: design
program: core-surface-taxonomy
related:
  - docs/superpowers/plans/2026-06-29-render-control-flow-lowering.md
---

# P3 — Lowering `match` in render bodies to a validated DomNode

## Problem

`match` in a render body falls entirely to `DomNode::Expr { ts }` (raw TS string) at
`crates/vox-codegen/src/web_ir/lower.rs:247`. That string IS a working TS `switch` IIFE
(`hir_emit/mod.rs:710` — dispatches on the `_tag` ADT discriminator), but because the arm
bodies live inside an opaque TS blob, the WebIR validators (palette / a11y / layer /
overlay) **never walk them**. So a `match` that picks between UI views bypasses exactly
the checks that make "bad UI doesn't compile" true. The goal: lower `match` so each arm
body is a real child `DomNode` the validators traverse.

## Non-functional constraints

- **No behavior change to emitted output for the cases we lower** — the generated TS must
  remain equivalent (same view chosen per scrutinee value).
- **No new validation gaps** — every arm body must be reachable by the existing
  validator traversal.
- **Honest fallback metric** — anything we can't lower keeps incrementing
  `expr_fallback_count` (the WebIR parity metric).
- **Small, single-crate change** — this is `vox-codegen` lowering, no parser/HIR churn.

## Key facts (verified against code)

- `DomNode::Conditional { predicate, then_children, else_children, span }` already exists,
  already emits (ternary/IIFE), and **its children are already validated** — proven by the
  `if`→`Conditional` lowering at `lower.rs:181-243` (the validators walk `then/else_children`).
- `HirPattern` = `Ident(String) | Tuple(..) | Constructor(name, fields) | Wildcard | Literal(expr)`
  (`hir/nodes/stmt_expr.rs:283`).
- ADT values carry a `_tag` discriminator; the existing TS match emits `case "Ok": …` on
  `_val._tag`, binding constructor fields via `(_val as any)._p<i> ?? (_val as any).value`
  (`hir_emit/mod.rs:740-769`).

## Decision 1 — reuse nested `Conditional`, NOT a new `DomNode::Switch`

A `match` lowers to a right-nested `Conditional` chain:

```
match s { A => <A/>  B => <B/>  _ => <C/> }
  ⇒ Conditional( pred(A), [<A/>],
       [ Conditional( pred(B), [<B/>], [<C/>] ) ] )
```

| | New `Switch` variant | **Nested `Conditional` (chosen)** |
|---|---|---|
| `DomNode` enum + serde | new variant | none |
| `emit_tsx` | new arm (nested ternary/IIFE) | none — `Conditional` emit already nests |
| `validate.rs` (collect_child_ids + work-queue) | must add traversal for the new node | none — `Conditional` already walked |
| OP-0049 schema doc | update | none |
| Net new code | ~4 sites | **1 site (`lower.rs:247`) + a predicate helper** |

The only theoretical edge a dedicated `Switch` would buy is a single-evaluation of the
scrutinee (a chain re-emits the scrutinee in each predicate). In render, scrutinees are
near-always cheap state reads (`state.status`), so this is immaterial; noted under
"revisit" below.

## Decision 2 — v1 lowers NON-BINDING patterns only

The blocker is **field bindings**: `Some(name) => <Text>{name}</Text>` needs `name` in
scope. A `Conditional` child is emitted as an *expression* (ternary branch), where you
can't introduce a `const name = …` binding without wrapping it in an IIFE — which puts the
arm body back inside an opaque TS blob, defeating the whole point (the validators couldn't
see it).

So v1 lowers a `match` to nested `Conditional` **iff every arm pattern is one of**:

- `Literal(lit)` → predicate `‹scrut› === ‹lit›`
- `Constructor(name, fields)` **with no binding** (all fields are `Wildcard`, or zero
  fields) → predicate `(‹scrut› as any)._tag === "‹name›"`
- `Wildcard` → the `else` (default) arm

…and every arm body reduces to `Expr` statements (same rule the `if`-lowering already uses
at `lower.rs:189-204`). Otherwise → **today's raw-`Expr` fallback, unchanged**, with
`expr_fallback_count += 1`. Binding patterns (`Some(x)`) are rare in render and keep
working exactly as now.

This single restriction is what lets us reuse `Conditional` with zero new machinery, and
it covers the dominant render idiom — dispatching a view on a status/variant enum.

## Lowering algorithm (replaces `lower.rs:247-252`)

```
fn lower_match(scrut, arms):
    # guard — bail to the existing raw-Expr fallback if any arm is unsupported
    if not arms.all(arm => is_non_binding(arm.pattern) and body_is_all_expr(arm.body)):
        expr_fallback_count += 1
        return push(DomNode::Expr { ts: emit_hir_expr(whole_match) })

    scrut_ts = emit_hir_expr(scrut)           # cheap state read in practice
    # split: the wildcard arm (if any) is the base else; the rest fold right
    base_else = wildcard_arm ? lower_bodies(wildcard_arm.body) : []
    node = fold arms-without-wildcard from right:
        acc = base_else
        for arm in reverse(non_wildcard_arms):
            pred = predicate(scrut_ts, arm.pattern)
            then = lower_bodies(arm.body)      # real child DomNodes → validated
            acc  = [ push(DomNode::Conditional { predicate: pred, then_children: then,
                                                 else_children: acc, span: None }) ]
        acc[0]
    return node

fn predicate(scrut_ts, pat):
    Literal(lit)            => format!("{scrut_ts} === {}", emit_hir_expr(lit))
    Constructor(name, _)    => format!("({scrut_ts} as any)._tag === \"{name}\"")

fn is_non_binding(pat):
    Literal(_) | Wildcard(_) => true
    Constructor(_, fields)   => fields.all(f => matches Wildcard)   # no Ident binders
    Ident(_) | Tuple(_)      => false
```

`lower_bodies` mirrors the `if` branch handling (`lower.rs:206-230`): each `HirStmt::Expr`
arm body becomes a child via `self.lower_expr(...)`. Predicate uses the SAME `_tag` test
the existing TS emission relies on, so behavior is unchanged.

## What changes / what does NOT

- **Changes:** only `crates/vox-codegen/src/web_ir/lower.rs` (the `HirExpr::Match` arm +
  a private `predicate`/`is_non_binding` helper).
- **Unchanged:** `web_ir/mod.rs` (no new `DomNode`), `emit_tsx.rs`, `validate.rs`,
  `hir_emit` (the raw-fallback path for binding/unsupported arms still uses it).

## GAP B (secondary) — `if` + `let` fallback

The plan's secondary gap (`if c { let x = …; <T/> }` falls to raw `Expr` at
`lower.rs:189-204`) is the **same binding problem** in `if` clothing and gets the **same
v1 answer**: leave the documented raw-`Expr` fallback. A real fix (binding-aware child
nodes) is the natural follow-up that unlocks bindings for *both* `if` and `match` at once —
see "revisit".

## Testing (TDD, extend `tests/web_ir_control_flow_test.rs`)

1. all-nullary-variant `match` (e.g. `Loading`/`Ready`/`Error`) lowers to a `Conditional`
   chain — assert **zero** `DomNode::Expr` for the arm subtree, and the chain depth.
2. `match` with a `Literal` arm + `_` lowers (predicate `=== lit`).
3. `match` with a binding arm (`Some(x)`) **still** falls to `DomNode::Expr` (guard holds)
   and increments `expr_fallback_count`.
4. Validator reach: an arm body that violates a11y (e.g. unlabeled `input`) inside a
   lowered `match` now produces the diagnostic (proving the validators walk it) — this is
   the whole point of the change.

## Trade-offs & what to revisit as it grows

- **Scrutinee re-evaluation** in the chain — fine for state reads; if a non-trivial
  scrutinee ever appears in render, hoist it (a `let` child node, or revisit `Switch`).
- **No bindings in v1** — the explicit ceiling. The upgrade path is a single
  "binding-scope" `DomNode` (an IIFE that still exposes its returned child for validation),
  which would lift the restriction for `if`+`let` AND `match`-with-bindings together. Build
  it only when a real surface needs payload binding in render.
- **Exhaustiveness** is the typechecker's job (it already runs on `match`); the lowering
  trusts it and emits an empty `else` when there's no wildcard, matching the existing TS
  emission's `default: return undefined`.
