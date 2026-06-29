---
title: "Plan — Render Control-Flow Lowering (P3)"
date: 2026-06-29
status: ready
spec: docs/superpowers/specs/2026-06-29-core-surface-taxonomy-design.md
program: core-surface-taxonomy
depends_on: docs/superpowers/plans/2026-06-29-core-surface-taxonomy.md
---

# Plan — Render Control-Flow Lowering (P3)

**Goal:** stop render-body control flow from escaping to unvalidated raw TS
(`DomNode::Expr{ts}`). Per The Rule, `if`/`match` are already core keywords — this
is *lowering* work, not new surface.

**Reality check (verified 2026-06-29, not from stale memory):**
- `if cond { … } else { … }` → `DomNode::Conditional` is **already lowered**
  (`crates/vox-codegen/src/web_ir/lower.rs:181-237`). It falls back to
  `DomNode::Expr{ts}` only when a branch contains non-`Expr` statements, e.g. a
  `let` binding (`lower.rs:185-203, 231-235`).
- `match` → **entirely** falls to `DomNode::Expr{ts}` (`lower.rs:247`, comment:
  "Full DomNode::Conditional lowering is deferred (arm patterns may not be JSX)").

So two concrete gaps remain. Existing coverage lives in
`crates/vox-codegen/tests/web_ir_control_flow_test.rs` — extend it.

## Task 1 — `match` → validated DomNode (failing test first)

**Files:** `web_ir/mod.rs` (node def), `web_ir/lower.rs:247`,
`web_ir/validate.rs`, `web_ir/emit_tsx.rs`, `tests/web_ir_control_flow_test.rs`.

- [ ] Test: a `match` whose arms are all JSX/element expressions lowers to a new
      `DomNode::Switch { scrutinee, arms: Vec<(pattern_ts, Vec<DomNode>)>, default }`
      (or reuse `Conditional` chained) — **not** `DomNode::Expr`.
- [ ] Lower simple arms (literal/enum-variant patterns with element bodies). Arms
      with non-Expr statements keep the documented `Expr` fallback for now (note it
      with a `ponytail:` comment naming the ceiling).
- [ ] Emit to a TS ternary/IIFE-switch that the existing validators can see.
- [ ] Validate: the lowered node participates in `validate_web_ir` (palette/a11y/
      layer) instead of bypassing it.

## Task 2 — `if` branches with `let` bindings

**Files:** `web_ir/lower.rs:185-235`, `tests/web_ir_control_flow_test.rs`.

- [ ] Test: `if c { let x = f(); <T>{x}</T> } else { <U/> }` no longer yields raw
      `DomNode::Expr`.
- [ ] Lower by wrapping the branch in a validated IIFE node (bindings hoisted into
      the closure) so the element subtree is still a real `DomNode`, or reject with
      a clear diagnostic if the binding can't be expressed — **decide which in the
      test**; do not silently fall back.

## Task 3 — Close the raw-TS escape (coordinates with P6)

- [ ] Add a test asserting the golden render fixtures produce **zero**
      `DomNode::Expr` nodes for `if`/`match` constructs (a count assertion over the
      lowered tree). This is the measurable "control flow no longer bypasses
      validators" property. The general fallback-emission gate is P6.

## Verify

- [ ] `cargo test -p vox-codegen` green; new control-flow tests pass.
- [ ] Golden TS outputs for affected fixtures reviewed — emitted UI unchanged in
      behavior, now validated.
- [ ] `cargo clippy -p vox-codegen -- -D warnings`.
