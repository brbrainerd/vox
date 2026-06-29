---
title: "Core-Surface Taxonomy — Tier 2 (@form demotion + reclassification)"
date: 2026-06-29
status: design
program: core-surface-taxonomy
amends: docs/superpowers/specs/2026-06-29-core-surface-taxonomy-design.md
---

# Core-Surface Taxonomy — Tier 2

## Why this exists

The P0 spec (`2026-06-29-core-surface-taxonomy-design.md`) deferred four decorators
— `@webhook @subagent @form @search` — as "Tier 2: real AST work." Applying The
Rule rigorously to them, with a sharpened test, **collapses that scope**: three are
not kind-defining at all, and only one is.

## The sharpened litmus (code-grounded)

The P0 rule asked "delete the annotation — same *kind*?" That is correct but
under-specified. The decidable, implementable form:

> **A decorator is kind-defining (→ keyword) iff it produces a distinct `Decl`
> variant. If it only sets fields on an existing `Decl` node, it is a modifier
> (→ keep as decorator).**

This is exactly why P0's Tier-1 worked: `@table`→`Decl::Table`, `@query`→`Decl::Endpoint`,
`@tool`→`Decl::McpTool` each yield a distinct node. Verified against the four:

| Decorator | Produces | Distinct `Decl`? | Verdict |
|---|---|---|---|
| `@form` | `Decl::Form(FormDecl)` (`head_form.rs:10→89`) | **Yes** — own block grammar | **Demote → `form` keyword** |
| `@webhook` | `Decl::Function` + `FnDecl.webhook: Option<AstWebhookSpec>` (`head_fn.rs:675`) | No — flag on a fn | **Keep as decorator** |
| `@subagent` | `Decl::Function` + `FnDecl.subagent_*` fields (`head_fn.rs:285`) | No — flags on a fn | **Keep as decorator** |
| `@search` | `Decl::Function` + `is_llm`/corpus args (`head_fn.rs:364`) | No — modifier on a fn | **Keep as decorator** |

`@webhook @subagent @search` sit in the decorator-list dispatch (`mod.rs:283-296`,
`721-767`), all falling through to `Ok(Decl::Function(parse_fn_decl()))`. They make a
function *behave* differently (webhook-routed, subagent-backed, search-backed) but it
is still a function — the signature of a **modifier**, like `@ai`/`@traced`/`@cors`.
They were mis-bucketed. Correcting them is the right outcome, not a workaround.

## Decision

1. **Reclassify `@webhook @subagent @search` as keep-decorators** (correct, no code
   change — they already are `@`-decorators producing `Decl::Function`). This amends
   the P0 Appendix A: `T2 → K`.
2. **Demote `@form` → `form` soft keyword** — the only genuine Tier-2 kind. Same
   soft-keyword mechanism as P0: no `logos #[token]`, positional dispatch on
   `Ident("form")` at declaration-head, reusing `parse_form_decl` to produce the
   identical `Decl::Form(FormDecl)`.

### Amended counts (supersedes P0 Appendix A totals)

P0 said T1=7, T2=4, X=5, K=40. Corrected: **demotions = 8** (7 P0 Tier-1 + `@form`),
**X = 5**, **K = 43** (40 + webhook/subagent/search). 8 + 5 + 43 = 56. `ALL.len()`
unchanged.

## `@form` demotion surface

| Decorator form | Soft-keyword form |
|---|---|
| `@form Signup { field email: str ... on_submit: register }` | `form Signup { field email: str ... on_submit: register }` |

`@form` already uses internal soft keywords (`field`, `on_submit`, `success_redirect`
are `Ident`-matched at `head_form.rs:25-34`), so the only change is dropping the `@`
and recognizing the leading `form` positionally. `Decl::Form(FormDecl)` is produced
identically → HIR/codegen unchanged, golden output byte-identical (the P0
byte-identical invariant holds for this one cleanly, unlike the three modifiers).

## Migration

- Add `form` to the P0 positional dispatch + a `parse_form_kw` head reusing
  `parse_form_decl`'s body.
- Tombstone `@form` (`mod.rs:810`) with the machine-readable `replacement` payload
  from P0 (`{from:"@form", to:"form", code:"vox/decorator/form-retired"}`).
- Codemod `@form ` → `form ` across `.vox` + Rust string literals (same harness as
  P0 Task 7). `@form` has no `fn`/`type` to subsume and no parenthesized args, so the
  rewrite is a literal `@form`→`form` at declaration head.
- **No code change for `@webhook @subagent @search`** — only the spec Appendix A row
  and any doc that called them "kind-defining" / "Tier-2 demotion."

## Out of scope

The three reclassified decorators keep their exact current behavior. If a future
design wants genuine `webhook`/`subagent` *declaration kinds* (distinct `Decl`
nodes), that is new language design — not this program.

## Testing

- **`form` equivalence:** serde AST-equality `@form Signup { field email: str }` ≡
  `form Signup { field email: str }` (reuse the P0 harness + `strip_spans`).
- **`form` as identifier preserved:** `fn f(form: str)` and a field `form: str` still
  parse (soft keyword).
- **`@form` tombstoned:** `class == Tombstoned`, `replacement.to == "form"`.
- **Reclassification regression:** `@webhook`/`@subagent`/`@search` still parse
  unchanged to `Decl::Function` (a guard that this spec changed nothing for them).
- **Appendix-A consistency:** a doc/test asserting demotions=8, X=5, K=43, sum=56.
