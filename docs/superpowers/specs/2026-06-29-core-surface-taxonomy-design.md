---
title: "Vox Core-Surface Taxonomy + Source-Token Budget"
date: 2026-06-29
status: design
program: core-surface-taxonomy
supersedes: []
related:
  - docs/superpowers/specs/2026-06-20-vox-native-frontend-ssot-design.md
  - docs/superpowers/specs/2026-06-20-vox-native-frontend-ssot-subproject-b-design.md
---

# Vox Core-Surface Taxonomy + Source-Token Budget

## Problem

The Vox surface has drifted into "everything is a decorator." `feature_matrix.rs`
declares **56** `DecoratorFeature` variants (asserted at `feature_matrix.rs:697`),
each with a dedicated `Token::At*` lexer variant. Many are not modifiers at all —
they *define a kind*. `@table type User { … }` says "User **is** a table," and
`@query fn f()` says "f **is** an endpoint." Deleting those annotations changes
what the construct *is*, which is the signature of a keyword, not a decorator.

Two consequences:

1. **No semantic law.** There is no written rule for what belongs as a keyword vs
   a builtin vs a decorator, so every new capability defaults to "add decorator
   #57." The march toward authoring the GUI in Vox (the Tauri-surface goal) will
   keep inflating this surface unless a law governs it.
2. **No source-token budget.** The existing `vox ci k-complexity-budget` gate
   measures **WebIR bytes** (`syntax_k.rs`), not the **source tokens a coding model
   must emit**. The redundancy this spec removes (`@table type`, `@query fn`) is
   invisible to that gate. We want fewer tokens per construct *and* a gate that
   proves the reduction and prevents regression.

The precedent already exists: `@component` was a decorator, was retired into
`LEXER_DEPRECATED_DECORATORS`, and is now the `component` **keyword**
(`language_surface.rs:166`; golden `dashboard_ui.vox` uses bare `component`).
This spec generalizes that one-off into a law and applies it.

## The Rule (normative)

Classify every surface element by one test:

> **Delete the annotation. Is it still the same *kind* of thing?**

- **Yes — same kind, different behavior** → legitimate **decorator**: a removable,
  cross-cutting modifier. `@pure`, `@auth`, `@cors`, `@rate_limit`, `@pii`,
  `@deprecated`, `@json_as`, `@layer`, `@reactive`.
- **No — its identity changes** → it is a **keyword** (kind-defining declaration)
  miscategorized as a decorator → **demote**. `@table`, `@query`, `@mutation`,
  `@tool`, `@resource`, `@server`, `@webhook`, `@form`, `@search`, `@subagent`,
  `@index`.
- **Value-level, no special grammar** → **builtin** (an ordinary callable):
  `len`, `now`, `print`, `uuid`, `assert`.

### Category charters

| Category | Charter | How it is registered |
|---|---|---|
| **Keyword** | Introduces or defines a *kind* of declaration, or a control-flow form. Has dedicated grammar. Lowers to a kind-specific AST node. | `LEXER_KEYWORDS` + a `Token` variant + a `parse_*` head in `descent/decl/`. |
| **Builtin** | A value-level function with no special grammar. Pure call syntax `name(args)`. | `SURFACE_BUILTIN_NAMES` + runtime registration. No lexer token. |
| **Decorator** | A *removable cross-cutting modifier* attached to an existing construct that does not change its kind. | `LEXER_AT_DECORATORS` + `DecoratorFeature` + `Token::At*`. |

### The one declared exception

`@test` / `@example` are *constitutive* by the literal test (delete `@test` and a
test becomes an ordinary fn), but remain **decorators** by universal convention
(tooling tags that emit no runtime surface, as in every test framework). The spec
records this as the single named exception so the rule is not silently violated.

## Triage of all 56 decorators

The full per-decorator classification is the normative table in
`Appendix A` below. Summary:

| Bucket | Count | Members | Action |
|---|---|---|---|
| **Demote → keyword (this program)** | 11 | `@table @index @query @mutation @tool @resource @server @webhook @form @search @subagent` | New keyword subsumes the `fn`/`type` it sat on; old `@` spelling → **hard error**. |
| **Kill (already dead)** | 5 | `@component @mcp.tool @mcp.resource @v0 @place` | Already in `LEXER_DEPRECATED_DECORATORS` / retired. Confirm hard-error path; remove from active docs/LSP/snippets. |
| **Keep as decorator (correct)** | 40 | `@pure @auth @cors @rate_limit @pii @deprecated @require @ensure @invariant @forall @fuzz @test @example @json_as @field_name @default @skip_if_none @ai @prompt @hole @layer @reactive @versioned @tracked @scheduled @uses @embed @cancellable @loading @back_button @deep_link @push @tokens @offline_capable @collaborative @public @remote @inference @training_step @distributed_train` | Untouched. (`@traced` has a lexer token but is not a `DecoratorFeature`, so it is outside the 56 and untouched too.) |

### `ALL.len()` stays 56 — surface shrinks by *deprecation*, not deletion

Demoted and killed decorators **keep** their `DecoratorFeature` variant, their
`Token::At*` variant, and their `LEXER_AT_DECORATORS` entry — purely to carry the
hard-error diagnostic, exactly as `@component` does today. They are added to
`LEXER_DEPRECATED_DECORATORS`. Therefore:

- `assert_eq!(DecoratorFeature::ALL.len(), 56)` stays true.
- `decorator_feature_lexer_parity_mismatch()` stays `None` (both lists keep every
  entry).

What shrinks is the **active** surface: LSP docs/snippets, grammar SSOT, training
corpus, and the *source-token cost* of each construct.

## New keyword surface

Each demoted keyword **subsumes** the `fn`/`type` it used to annotate — you never
write both. That subsumption is where the token reduction comes from, not the
deletion of the `@` alone.

| Today | Tomorrow | Notes |
|---|---|---|
| `@table type User { … }` | `table User { … }` | type-kind keyword; drops `type`. |
| `@index type … { … }` | `index … { … }` | sibling of `table`. |
| `@query fn user_count() to int { … }` | `query user_count() to int { … }` | drops `fn`; lowers to `EndpointDecl{ kind: Query }`. |
| `@mutation fn add_note(b: str) to int { … }` | `mutation add_note(b: str) to int { … }` | `EndpointDecl{ kind: Mutation }`. |
| `@server fn handler() { … }` | `server handler() { … }` | `EndpointDecl{ kind: Server }`. |
| `@webhook fn on_push() { … }` | `webhook on_push() { … }` | endpoint kind. |
| `@form fn submit(…) { … }` | `form submit(…) { … }` | the form *handler* keyword (distinct from form *elements*, see §GUI). |
| `@search fn q(…) { … }` | `search q(…) { … }` | endpoint kind. |
| `@subagent fn worker(…) { … }` | `subagent worker(…) { … }` | agentic handler kind. |
| `@tool("search") fn search(…) { … }` | `tool search(…) { … }` | name **defaults to the identifier**; explicit only when different: `tool "web.search" search(…)`. |
| `@resource("uri") fn load(…) { … }` | `resource "uri" load(…) { … }` | keeps the URI string (genuinely needed). |

### The byte-identical-HIR invariant

`@query`/`@mutation`/`@server` already parse into `EndpointDecl { kind: EndpointKind }`
(`descent/decl/head_types.rs`); `@tool` into `McpToolDecl`; `@table` into a table
type decl. The decorator is *pure surface* over an AST node that **already encodes
the kind**. The new keyword MUST produce the **same AST node** the decorator did.

Consequence: HIR, typeck, placement, and all codegen backends
(`emit_tsx`, rust, interp) require **zero changes**. This is a front-of-pipe
rename. Every golden's *emitted output* is byte-identical before and after; only
its *source* shrinks. This invariant is the program's primary safety property and
is enforced by a parser equivalence test (Appendix B).

## Migration: hard error + codemod

1. **Lexer/parser:** add the 11 keywords to `LEXER_KEYWORDS` + `Token`; add
   `parse_*` heads that produce the existing AST nodes. Add the 11 old spellings
   to `LEXER_DEPRECATED_DECORATORS` with a `ParseErrorClass::Tombstoned` diagnostic
   ("`@table` is retired; use `table`").
2. **SSOT lockstep:** update `language_surface.rs` (`LSP_KEYWORD_SNIPPETS`,
   `LSP_DECORATOR_DOCS`, `LSP_DECORATOR_SNIPPETS`, `LEXER_DEPRECATED_DECORATORS`),
   `feature_matrix.rs` (no count change), LSP `grammar.rs`, and any grammar-SSOT
   parity gate. The `decorator_feature_lexer_parity_mismatch()` helper must stay
   `None`.
3. **Codemod (one-shot, this program):** a `.vox` script
   (`scripts/migrate-decorator-keywords.vox`) rewrites every affected `.vox` —
   goldens, `contracts/eval/` corpus, doc anchors — from `@kw [fn|type]` to `kw`.
   No dual-surface period; one spelling exists after this lands.
4. **Docs:** regenerate any tool-generated index (do **not** hand-edit generated
   files — rerun the generator).

## Source-token budget gate

New gate `vox ci source-token-budget`, mirroring `KComplexityBudget` exactly:

- **Measure:** `lex(source).len()` per golden **ladder** fixture. Deterministic;
  `lex()` already exists. A real model tokenizer is YAGNI — lexer-token count is a
  faithful, backend-agnostic proxy and never drifts with a vendor's BPE table.
- **Budget file:** `contracts/eval/source-token-budget.v1.json`, same
  `{ "fixtures": { id: count } }` shape as `complexity-budget.v1.json`.
- **Policy:** ratchet-down only. Fail when a fixture exceeds
  `allowed * (1 + tolerance/100)`. `--update` rebaselines. Wire into
  `pipeline_parity.rs` next to the existing k-complexity call.
- **First assertion:** after the codemod, every migrated fixture's token count
  MUST drop. That delta is the proof the shrink is real, captured as a test that
  diffs pre/post counts for the 11-construct fixtures.

## What is lacking for the Tauri surface (the categorization law in action)

The Vox-native GUI program (`2026-06-20-vox-native-frontend-ssot-design.md`,
Sub-projects A–G; A merged) needs capabilities Vox does not yet have. This spec's
job is **not** to build them — it is to **pre-classify** each so the surface stays
lean as they land. Each is a downstream sub-project governed by The Rule:

| Missing capability | The Rule classifies it as | Owner sub-project | Status |
|---|---|---|---|
| Reactive streams `on stream(ch) as s: { … }` (blocks 10/25 surfaces) | **`on`-family keyword member** — NOT a decorator (follows `on mount`/`depends_on` precedent) | Sub-project B (spec+plan exist) | designed, unexecuted |
| Form **elements** `input/select/textarea` + real two-way `bind:` | **view-tree keywords/builtins** (peers of `text()`, `column()`, `heading()`) — NOT decorators | "Form primitives" (plan in this program) | not built; `bind:` is a placeholder |
| `if` / `match` inside render bodies | **core control-flow keywords** (already exist) — need *lowering*, not new surface | "Render control-flow lowering" (plan in this program) | falls to raw `DomNode::Expr{ts}` today |
| React interop dedupe + `package.json` auto-manifest | **toolchain/codegen fix** — no surface element | "Interop hardening" (roadmap; needs own brainstorm) | live runtime bug ("Invalid hook call") |
| Mobile: PWA `Target` arm + mobile-first validator | **new `Target` variant + validator** — surface unchanged; `@offline_capable/@deep_link/@push` stay correct decorators | "Mobile/PWA" (roadmap; needs own brainstorm) | no PWA today |
| Fallback-emission validation gate | **CI gate** — no surface element | "Fallback validation gate" (roadmap) | raw-TS escapes are unvalidated |

The payoff: every future GUI/mobile construct has a *pre-decided home*. The
source-token gate then enforces leanness numerically, so reaching 95–99% Tauri
parity cannot silently re-bloat the decorator surface.

## Decomposition (program)

This spec is the **foundation**. It lands the law, the 11 migrations + 5 kills,
the codemod, and the token gate. The GUI/mobile gaps are governed-but-separate
sub-projects, sequenced by dependency:

1. **P0 — Foundation** (this spec). Plan: `plans/2026-06-29-core-surface-taxonomy.md`.
2. **P1 — Reactive streams** (`on stream`). Existing: `plans/2026-06-20-vox-native-frontend-ssot-subproject-b.md`. Make-or-break; +10 surfaces.
3. **P2 — Form primitives** (`input/select/textarea`, `bind:`). Plan: `plans/2026-06-29-vox-form-primitives.md`.
4. **P3 — Render control-flow lowering** (`if`/`match` → WebIR). Plan: `plans/2026-06-29-render-control-flow-lowering.md`.
5. **P4 — Interop hardening** (dedupe + manifest). Needs own brainstorm.
6. **P5 — Mobile/PWA** (Target arm + validator). Needs own brainstorm.
7. **P6 — Fallback validation gate**. Needs own brainstorm.

P1–P3 are unblocked by P0 once the keyword heads exist. P4–P6 are scoped here but
require their own design pass before a TDD plan — fabricating one now would ship
stubs, which this program forbids.

## Testing strategy

- **Parser equivalence (the safety property):** for each of the 11 constructs,
  parse the old `@kw` form and the new `kw` form and assert **identical AST**
  (modulo span). This proves the byte-identical-HIR invariant at the cheapest
  layer. (Appendix B.)
- **Deprecation:** `@table` (and the other 10) now produce a `Tombstoned`
  diagnostic naming the replacement.
- **Golden output stability:** the existing golden TS/rust/interp suites pass
  unchanged after the codemod — emitted artifacts do not move.
- **SSOT parity:** `decorator_feature_lexer_parity_mismatch()` is `None`;
  `DecoratorFeature::ALL.len() == 56`.
- **Token gate:** `vox ci source-token-budget` passes; a focused test asserts each
  migrated fixture's `lex().len()` strictly decreased vs. its pre-migration value.

## Appendix A — full 56-decorator classification

`K` = keep decorator, `D` = demote to keyword (this program), `X` = kill/already-dead.

| Decorator | Verdict | Rationale |
|---|---|---|
| `@table` | D | defines a table *type kind* |
| `@index` | D | defines a db index *kind* |
| `@query` | D | endpoint kind (`EndpointKind::Query`) |
| `@mutation` | D | endpoint kind |
| `@server` | D | endpoint kind |
| `@webhook` | D | endpoint kind |
| `@form` | D | form-handler kind |
| `@search` | D | endpoint kind |
| `@subagent` | D | agentic-handler kind |
| `@tool` | D | MCP tool kind (name defaults to ident) |
| `@resource` | D | MCP resource kind (keeps URI) |
| `@component` | X | already a keyword; dead decorator |
| `@mcp.tool` | X | superseded by `@tool`/`tool`; dead |
| `@mcp.resource` | X | superseded by `@resource`/`resource`; dead |
| `@v0` | X | retired |
| `@place` | X | retired (placement model; `@place`/`@native` gone) |
| `@pure` | K | modifier: still a fn without it |
| `@deprecated` | K | modifier: diagnostics tag |
| `@require` | K | modifier: precondition guard |
| `@ensure` | K | modifier: postcondition guard |
| `@invariant` | K | modifier: bound guard |
| `@forall` | K | modifier: property-test tag |
| `@fuzz` | K | modifier: fuzz tag |
| `@test` | K | **declared exception** (constitutive but conventional tag) |
| `@example` | K | **declared exception** (corpus/doc tag) |
| `@json_as` | K | modifier: serde representation |
| `@field_name` | K | modifier: serde rename |
| `@default` | K | modifier: field default |
| `@skip_if_none` | K | modifier: serde skip |
| `@ai` | K | modifier: still a fn, body is LLM-implemented |
| `@prompt` | K | modifier: prompt metadata |
| `@hole` | K | modifier: synthesis hole |
| `@reactive` | K | modifier: reactivity hint |
| `@versioned` | K | modifier: versioning |
| `@tracked` | K | modifier: change-tracking |
| `@scheduled` | K | modifier: adds periodicity to a fn |
| `@uses` | K | modifier: capability declaration |
| `@embed` | K | modifier: embedding directive |
| `@cancellable` | K | modifier: cancellation |
| `@loading` | K | modifier: suspense UI for a route fn |
| `@back_button` | K | modifier: mobile nav |
| `@deep_link` | K | modifier: mobile nav |
| `@push` | K | modifier: mobile push |
| `@tokens` | K | modifier: token/theme directive |
| `@cors` | K | modifier: endpoint CORS |
| `@rate_limit` | K | modifier: endpoint throttle |
| `@pii` | K | modifier: data-classification tag |
| `@public` | K | modifier: visibility |
| `@auth` | K | modifier: auth requirement |
| `@offline_capable` | K | modifier: PWA capability |
| `@collaborative` | K | modifier: collaboration |
| `@layer` | K | modifier: UI layer/overlay |
| `@remote` | K | modifier: remote execution |
| `@inference` | K | modifier: ML inference |
| `@training_step` | K | modifier: ML training |
| `@distributed_train` | K | modifier: ML distribution |

Totals: **D = 11, X = 5, K = 40** → 56. (Demoted/killed remain as zombie variants
for the hard-error path; `ALL.len()` unchanged.)

## Appendix B — parser equivalence harness sketch

```rust
// vox-compiler/tests/keyword_decorator_equivalence.rs  (sketch, not final)
fn ast_eq_modulo_span(old_src: &str, new_src: &str) {
    let a = parse(lex(old_src)).expect("old form parses");
    let b = parse(lex(new_src)).expect("new form parses");
    assert_eq!(strip_spans(&a), strip_spans(&b));
}

#[test] fn table_equivalence() {
    ast_eq_modulo_span(
        "@table type User { name: str }",
        "table User { name: str }",
    );
}
// … one per migrated construct.
```
