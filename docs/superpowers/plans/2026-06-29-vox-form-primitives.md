---
title: "Plan — Form Primitives (P2)"
date: 2026-06-29
status: ready
spec: docs/superpowers/specs/2026-06-29-core-surface-taxonomy-design.md
program: core-surface-taxonomy
depends_on: docs/superpowers/plans/2026-06-29-core-surface-taxonomy.md
---

# Plan — Form Primitives (P2)

**Goal:** make real forms expressible in `.vox` — input/select/textarea elements
plus working two-way binding. Per The Rule, form **elements** are view-tree
keywords/builtins (peers of `text()`, `column()`, `heading()`), **not** decorators.
The `@form` → `form` *handler* keyword is delivered by P0; this plan delivers the
*elements* it submits.

**Reality check (verified 2026-06-29):**
- `bind={…}` already expands to `value` + `onChange`
  (`crates/vox-codegen/src/web_ir/lower.rs:492`, `expand_bind_hir_attribute`). So
  two-way binding *machinery* exists — the gap is the element vocabulary that uses
  it, and confirming `bind` round-trips through a real input.
- Element primitives live in `crates/vox-codegen/src/web_ir/primitives/mod.rs`.

## Task 0 — Audit current element vocabulary

- [ ] Read `web_ir/primitives/mod.rs`: list the registered elements. Confirm
      `input`/`select`/`textarea` are absent (expected) and note how `text`/
      `column`/`heading` are registered — the new elements follow that exact shape.
- [ ] Confirm whether the parser already accepts arbitrary element-call syntax in
      `view:` bodies or needs the three names whitelisted.

## Task 1 — `input` element (failing test first)

**Files:** `web_ir/primitives/mod.rs`, `web_ir/lower.rs`, `web_ir/emit_tsx.rs`,
`web_ir/validate_a11y.rs`, `crates/vox-codegen/tests/` (new
`form_primitives_test.rs`).

- [ ] Test: `input(bind=state.name, placeholder="Name")` lowers to a DOM input
      whose `value`/`onChange` come from the existing `bind` expansion, and emits
      valid TSX.
- [ ] Register `input` with its attributes (`bind`, `placeholder`, `kind`/type
      mapped carefully — note `type` is a reserved Vox keyword, so the attribute is
      spelled `kind=` and maps to HTML `type`).
- [ ] a11y validator: an `input` without an associated label or `aria-label`
      produces a blocking diagnostic ("bad UI doesn't compile").

## Task 2 — `select` and `textarea`

- [ ] Test + register `select(bind=…) { option(value=…) { "Label" } }` and
      `textarea(bind=…)`.
- [ ] `select` lowers options as child DOM nodes; `bind` drives `value`+`onChange`.
- [ ] a11y: same label requirement.

## Task 3 — End-to-end form golden

**Files:** `examples/golden/forms_basic.vox` (new), wired into the ladder if the
canonical ladder governs new fixtures (check `canonical-ladder.v1.yaml`).

- [ ] A `form submit(...)` handler (P0 keyword) with `input`/`select`/`textarea`
      bound to component `state`, submitting to the handler.
- [ ] `@test` exercising the handler under `--mode interp` (behavioral coverage,
      matching the golden-corpus convention).
- [ ] Add its budgets to `complexity-budget.v1.json` and
      `source-token-budget.v1.json` (`--update`).

## Verify

- [ ] `cargo test -p vox-codegen` green; form tests pass.
- [ ] The new golden passes parse→lower→typecheck→interp.
- [ ] a11y validator rejects an unlabeled input (negative test present).
- [ ] `cargo clippy -p vox-codegen -- -D warnings`.

## Note

This unblocks form-bearing GUI surfaces in the migration (Sub-project G). It does
**not** cover file-upload or multi-step wizards — scope those separately if needed
(YAGNI until a surface requires them).
