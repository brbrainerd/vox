---
title: "Vox Core-Surface Taxonomy + Source-Token Budget"
date: 2026-06-29
status: design
revision: 2
program: core-surface-taxonomy
supersedes: []
related:
  - docs/superpowers/specs/2026-06-20-vox-native-frontend-ssot-design.md
  - docs/superpowers/specs/2026-06-20-vox-native-frontend-ssot-subproject-b-design.md
audit: "Adversarially audited against the codebase 2026-06-29 (27-agent workflow). Revision 2 incorporates the findings; the original 'hard keyword / front-of-pipe rename' framing was falsified and is replaced by a soft-keyword design with a tier split."
---

# Vox Core-Surface Taxonomy + Source-Token Budget

## Problem

The Vox surface has drifted into "everything is a decorator." `feature_matrix.rs`
declares **56** `DecoratorFeature` variants (`feature_matrix.rs:697`), each with a
dedicated `Token::At*`. Many are not modifiers — they *define a kind*. `@table type
User {…}` says "User **is** a table"; `@query fn f()` says "f **is** an endpoint."
Deleting those annotations changes what the construct *is* — the signature of a
keyword, not a decorator.

Two consequences:

1. **No semantic law.** No written rule for keyword vs builtin vs decorator, so
   every new capability defaults to "add decorator #57." Authoring the GUI in Vox
   will keep inflating this surface unless a law governs it.
2. **No source-token budget.** `vox ci k-complexity-budget` measures *WebIR bytes*
   (`syntax_k.rs`), not *source tokens a model emits*. The redundancy this program
   removes (`@table type`, `@query fn`) is invisible to it.

The precedent: `@component` was a decorator, was retired, and is now the
`component` keyword (`language_surface.rs:166`; golden `dashboard_ui.vox` uses bare
`component`). This program generalizes that into a law and applies it.

## The Rule (normative)

> **Delete the annotation. Is it still the same *kind* of thing?**

- **Yes — same kind, different behavior** → legitimate **decorator** (removable
  cross-cutting modifier): `@pure`, `@auth`, `@cors`, `@rate_limit`, `@pii`,
  `@deprecated`, `@json_as`, `@layer`, `@reactive`.
- **No — its identity changes** → **keyword** (kind-defining declaration) → demote.
- **Value-level, no special grammar** → **builtin**: `len`, `now`, `print`.

### Category charters

| Category | Charter | Registered via |
|---|---|---|
| **Keyword** | Defines a *kind* of declaration or a control-flow form; dedicated grammar; lowers to a kind-specific AST node. | **Soft keyword** (see below) + a `parse_*` head. |
| **Builtin** | Value-level function, pure call syntax. | `SURFACE_BUILTIN_NAMES` + runtime. |
| **Decorator** | Removable modifier on an existing construct that does not change its kind. | `LEXER_AT_DECORATORS` + `DecoratorFeature` + `Token::At*`. |

### Declared exception

`@test` / `@example` are constitutive by the literal test but remain **decorators**
by universal convention (tooling tags, no runtime surface). The single named
exception, so the rule is not silently violated.

## Soft keywords — the central design decision (revised)

> **The demoted words are SOFT (contextual) keywords, not hard reserved keywords.**

The first revision proposed giving each word a `logos #[token]`. The audit
falsified this: Vox lexes identifiers to `Token::Ident` at priority 1
(`token.rs:510`), and `parse_ident_name` (`head.rs:391-460`) is a hand-maintained
allowlist admitting only 13 specific tokens. Promoting these 11 words to dedicated
tokens would make them illegal as field names, parameter names, method names, and
variables — which **the live corpus relies on**:

- `Search { query: str }` — field name (`json_as_typed.vox:64`)
- `fn search_docs(query: str)` — param name (`index_showcase.vox:41`)
- `fn provision_resource(tenant: str, resource: str)` — param name (`multi_tenancy.vox:36`)
- `db.query("…")` — method call (`contracts/eval/repair-corpus/projects/020-pure-calls-db/src/main.vox:7`)

Hard-tokenizing breaks all of these — including the program's own migration corpus.

The correct mechanism already exists in Vox: **`get`/`post`/`put`/`delete` are in
`LEXER_KEYWORDS` yet have NO `logos #[token]`** — they lex as `Token::Ident` and are
recognized **positionally** by the parser (which is why `db.Widget.delete(0)` and
`http { get "/x" … }` both work). We follow that precedent exactly:

- The 11 words get **no** `Token::*` variant and **no** `#[token]`. `cursor.rs`
  needs zero changes.
- The top-level declaration dispatcher (`descent/mod.rs` `parse_decl`) peeks: when
  the next token is `Token::Ident(s)` **at declaration-head position** and `s`
  matches a demoted keyword **and** the following token shape confirms a
  declaration (e.g. `table` + `Ident` + `{`), it routes to the keyword head.
  Otherwise the word stays an ordinary identifier.
- Consequence: `let query = 1`, `fn f(table: str)`, a field `search: str`, and
  `db.query(...)` all keep parsing. **This is a normative requirement** with a
  dedicated regression test (see Testing).

`LEXER_KEYWORDS` membership is **introspection/LSP metadata only** — it has no
lexing effect (proven by `get`/`delete`). The spec no longer claims otherwise.

## Triage of all 56 — split into two tiers

The litmus classifies 11 as kind-defining. But **demoting them is not uniform**:
the audit proved the AST shape differs. We split by implementation reality.

### Tier 1 — clean head-swap demotions (this program, P0)

These already lower to a kind-specific `Decl` node; the soft-keyword head reuses
the existing parser and produces the **same** node.

| Decorator | Existing node | Soft-keyword form |
|---|---|---|
| `@table` | `Decl::Table(TableDecl)` (`mid.rs:392→521`) | `table User { … }` / `table(pk: uid) User { … }` |
| `@index` | `Decl::Index(IndexDecl)` (`tail.rs:11-36`) | `index User.by_name on (name)` |
| `@query` | `Decl::Endpoint{kind: Query}` (`head.rs:249`) | `query user_count() to int { … }` |
| `@mutation` | `Decl::Endpoint{kind: Mutation}` (`head.rs:260`) | `mutation add_note(b: str) to int { … }` |
| `@server` | `Decl::Endpoint{kind: Server}` (`head.rs:271`) | `server handler() to int { … }` |
| `@tool` | `Decl::McpTool(McpToolDecl)` (`head.rs:40`) | `tool search(q: str) to str { … }` |
| `@resource` | `Decl::McpResource(McpResourceDecl)` (`head.rs:65`) | `resource "uri" "desc" load() to str { … }` |

### Tier 2 — real AST work, NOT a head-swap (separate spec/plan, out of P0)

The audit proved these do **not** share a lowering target and are **not** front-of-
pipe renames:

- `@webhook`, `@subagent` — parsed as **field-setting modifiers inside
  `parse_fn_decl`** (`head_fn.rs:675`, `:285`); they set `Option`/flag fields on a
  generic `FnDecl` and return `Decl::Function`. A leading keyword must reconstruct
  those fields (`webhook: Some(AstWebhookSpec)`, `subagent_policy/max_depth/…`).
- `@form` — a wholly distinct `Decl::Form(FormDecl)` with a `{ field…; on_submit }`
  block (`head_form.rs:89`). It does **not** wrap a `fn`; the rev-1 row
  `@form fn submit(…) → form submit(…)` described a construct that does not exist.
- `@search` — **not a fn endpoint decorator at all.** It is a standalone directive
  with named args: `@search(corpus=docs, query="…", into=Hit, top_k=3)`
  (`end_to_end_demo.vox:28`), read as a modifier at `head_fn.rs:364`. Demoting it
  to `search q(…)` would silently drop the payload. It is arguably **mis-classified**
  — flagged for re-derivation in the Tier-2 spec.

Tier 2 keeps its taxonomy classification (kind-defining) but its *implementation* is
deferred to `2026-06-29-core-surface-taxonomy-tier2-design.md`, because cramming it
into P0 would ship incorrect lowering. This is honest decomposition, not deferral of
the law.

### Kill bucket — verify they are actually dead (they are not)

Rev-1 claimed `@component @mcp.tool @mcp.resource @v0 @place` are "already retired."
The audit falsified this: `@v0` still dispatches live (`mod.rs:613
Token::AtV0 => parse_v0_component`), `@place` still lexes+parses, `@mcp.tool` emits
only a **warning** (`head.rs:44`). List membership ≠ tombstone. P0 must add genuine
`Tombstoned`-error dispatch arms for each before claiming them dead.

### Keep as decorator — 40 (correct)

`@pure @auth @cors @rate_limit @pii @deprecated @require @ensure @invariant @forall
@fuzz @test @example @json_as @field_name @default @skip_if_none @ai @prompt @hole
@layer @reactive @versioned @tracked @scheduled @uses @embed @cancellable @loading
@back_button @deep_link @push @tokens @offline_capable @collaborative @public @remote
@inference @training_step @distributed_train`. (`@traced` has a lexer token but is
not a `DecoratorFeature`, so it is outside the 56.) Full per-decorator table:
Appendix A.

### `ALL.len()` stays 56 — and the real parity lever

Demoted/killed decorators **keep** their `DecoratorFeature` variant, `Token::At*`,
and `LEXER_AT_DECORATORS` entry — purely to carry the hard-error diagnostic (exactly
how `@mcp.tool` works today). The audit corrected the *mechanism*:
`decorator_feature_lexer_parity_mismatch()` (`language_surface.rs:273-295`) compares
**only** `DecoratorFeature::ALL` spellings against `LEXER_AT_DECORATORS`; it never
reads `LEXER_DEPRECATED_DECORATORS`. So:

> **The invariant lever is: do not remove any spelling from `DecoratorFeature::ALL`
> or `LEXER_AT_DECORATORS`.** `LEXER_DEPRECATED_DECORATORS` is orthogonal — it drives
> LSP/introspection suppression and the hard-error path, not parity.

`ALL.len() == 56` holds; parity stays `None`.

## New keyword surface (corrected)

Each Tier-1 keyword **subsumes** the `fn`/`type` it used to annotate. Corrected
details from the audit:

| Decorator form | Soft-keyword form | Notes |
|---|---|---|
| `@table type User { … }` | `table User { … }` | drops `type` (`Token::TypeKw`, not `Token::Type`). |
| `@table(pk: uid) type User {…}` | `table(pk: uid) User { … }` | **args come BEFORE the name** (`mid.rs:404-495` runs before name); `table User(pk:…)` would NOT parse. `extern`/`source:` args likewise. |
| `@index User.by_name on (name)` | `index User.by_name on (name)` | `@index` has **no `type`, no `{}` body** (`tail.rs:11`). Rev-1's `@index type {…}` was fabricated. No `fn`/`type` to subsume. |
| `@query fn user_count() to int` | `query user_count() to int` | `Decl::Endpoint{Query}`. |
| `@mutation fn add(b: str) to int` | `mutation add(b: str) to int` | `Decl::Endpoint{Mutation}`. |
| `@server fn handler() to int` | `server handler() to int` | `Decl::Endpoint{Server}`. |
| `@tool fn search(…)` (no string) | `tool search(…)` | both → `description=""`. |
| `@tool("desc") fn search(…)` | `tool "desc" search(…)` | the string is the **description**, not the name; the name is always `func.name`. `McpToolDecl` has **no `name` field** (`fundecl.rs:312`). |
| `@resource("uri","desc") fn load(…)` | `resource "uri" "desc" load(…)` | `@resource` **requires both** uri AND description (`head.rs:65-139`); a single-string form does not parse today. `McpResourceDecl{uri, description, func}`. |

### The corrected safety invariant

For Tier-1, the soft-keyword head produces the **same `Decl` node with the same
field values** the decorator did → HIR, typeck, placement, and all codegen backends
are unchanged, and golden *emitted output* is byte-identical (only *source* shrinks).
This is enforced by a serde-based AST-equality test (Appendix B), **and** by an
app-contract assertion that `description`/`uri`/`name` in the emitted MCP manifest
are byte-identical (the audit showed a naive `tool` head corrupts `description`).

## Migration: hard error + codemod (corrected blast radius)

1. **Parser:** add Tier-1 soft-keyword heads (positional dispatch). Replace the
   Tier-1 + kill `@` dispatch arms with `ParseErrorClass::Tombstoned` errors.
2. **The codemod must cover more than `.vox`:**
   - **Rust string literals** (~57 files embed `@query fn …` / `@table type …` and
     call `parse`/`lex` at test time, e.g.
     `crates/vox-compiler/tests/{interpreter_db_test,db_query_safety_test,…}.rs`,
     `src/app_contract.rs`, `src/hir/lower/mod.rs`, `src/parser/descent/tests.rs`).
     They break the instant the tombstone lands and the `.vox`-only codemod misses
     them. **Either** extend the codemod to `*.rs` literals **or** ship the tombstone
     as a **deprecation warning first**, migrate the Rust corpus, then flip to error.
   - **Multiline decorators:** goldens predominantly use newline-separated form
     (`crud_api.vox:12-14`). The codemod must match `@kw\s+(fn|type)` across
     newline+whitespace, and assert **zero** residual `@(table|index|query|…)` tokens
     after `--apply`.
   - **Frontmatter/comments:** goldens carry spellings in `constructs:` lists and
     `@training_prompt` prose. Migrate those deliberately (training signal must match
     the keyword body) but do not let a blind rewrite corrupt metadata.
   - **`@table(pk:)` args:** the codemod must carry `(pk:…)`/`(extern)`/`(source:)`
     into the keyword form `table(pk:…) User`.
3. **SSOT lockstep** (expanded — see §SSOT below).

## Source-token budget gate (honest framing)

New `vox ci source-token-budget`, mirroring `KComplexityBudget`. Two corrections:

1. **What it measures.** `lex(source).len()` counts **structural lexer tokens** — a
   *lower bound on grammar verbosity*, **not** the BPE tokens a coding model emits.
   `lex()` collapses each identifier/string to one token and strips comments
   (`cursor.rs:48`), so it cannot see identifier-length or comment cost. It is
   **exact for the one thing this program changes** (removing a fixed `fn`/`type`
   token) and is used only to assert that delta. A vendor tokenizer is YAGNI, but to
   make identifier/comment regressions at least *visible* the gate records a
   **secondary measure: raw source byte count** per fixture (`bytes/4` is the
   standard BPE rule-of-thumb). Budget file carries both.
2. **Which fixtures witness the shrink.** The gate is ladder-scoped. Of the rev-1
   examples, only `crud_api` qualifies: `db_operations` is **not in the ladder**
   (`continue` skips it) and `dashboard_ui` uses **zero** demoted decorators. The
   shrink-proof uses the ladder fixtures that actually contain Tier-1 decorators:
   **`crud_api` (@table/@query/@mutation), `db_native_ir` (@table/@query),
   `mcp_tools` (@tool), `web_routing_fullstack` (@query)**. `db_operations` is either
   added to `canonical-ladder.v1.yaml` or dropped from the proof.

## AI-first requirements (the north star, made measurable)

The program's purpose is a leaner language for *model* authorship. The audit found
the AI-first claims were asserted, not designed. Three normative additions:

1. **Machine-readable tombstone payload.** A retired-decorator error must be
   auto-fixable by an LLM/codemod *as data*, not prose. Today `ParseError` renders
   `expected`/`found` as a human string (`error.rs:87`). Add a typed, serializable
   `replacement: Option<{ from: String, to: String, code: String }>` to the
   diagnostic (e.g. `{from:"@table type", to:"table", code:"vox/decorator/table-retired"}`),
   surfaced on the diagnostic-JSON channel. **Normative for every Tier-1 + kill
   tombstone.**
2. **Reserved-word hazard, explicitly bounded.** Soft keywords are the mitigation:
   the 11 common nouns (`search`, `form`, `index`, `query`, `tool`, `resource`,
   `server`…) stay usable in the value namespace, so a model writing `let query = …`
   does not hit a new parse error. The spec states the disambiguation rule (keyword
   only at declaration-head position) explicitly.
3. **A measurable AI-first acceptance criterion.** Add an offline eval (MENS or
   OpenRouter, both wired): prompt the project model to regenerate N migrated goldens
   from a fixed NL spec and assert (a) it emits the new keyword form, (b) output
   compiles, (c) median **BPE**-token count did not increase, and (d) a parse-error-
   rate probe on common-word identifiers (`query`/`search`/`form` as vars) stays at
   zero. This converts "AI-first" from assertion to evidence and is the gate that
   would catch a reserved-word regression. Minimum viable: the probe in (d).

## SSOT lockstep (expanded — audit-found gaps)

Updating `language_surface.rs` is **not** sufficient. The following also assert the
surface and must move in lockstep or their gates break:

- `crates/vox-grammar-export/src/ssot_markdown.rs` — **hardcodes** its own
  `LEXER_KEYWORDS`/`LEXER_DECORATORS` copies and slices them with fixed indices
  `[..19]/[19..36]/[36..]` (`ssot_markdown.rs:78-84`). Adding keywords mis-buckets or
  panics; `vox ci grammar-ssot-parity` then fails. Update the copies **and** the
  slice boundaries, then regenerate `GRAMMAR_SSOT.md` via the generator (do not hand-
  edit).
- `crates/vox-compiler/tests/language_surface_ssot_test.rs` — parity + LSP/lexer
  coherence asserts.
- `crates/vox-orchestrator-mcp/src/introspection_tools.rs` — builds the MCP decorator
  list from `LEXER_DECORATORS` (test at `:247` asserts `@tool`/`@resource` present);
  and the keyword list from `LEXER_KEYWORDS`.
- Verification runs: `cargo test -p vox-compiler -p vox-cli -p vox-orchestrator-mcp
  -p vox-grammar-export -p vox-lsp -p vox-codegen`.

## Decomposition (program)

1. **P0 — Foundation** (this spec): The Rule, soft-keyword infrastructure, **Tier-1
   demotions (7)** + **5 kills** (with real tombstones), codemod (Rust+vox+frontmatter),
   source-token gate (+byte secondary), tombstone payload, AI-first probe. Plan:
   `plans/2026-06-29-core-surface-taxonomy.md`.
2. **Tier-2** (`webhook/subagent/form`, and re-derive `@search`): own spec+plan —
   real AST/lowering work, not a rename.
3. **P1 reactive streams** (`on stream`) — existing Sub-project-B plan.
4. **P2 form primitives**, **P3 render control-flow lowering** — existing plans.
5. **P4 interop / P5 mobile-PWA / P6 fallback gate** — roadmap; need own brainstorm.

## Testing strategy

- **AST equivalence (Tier-1 safety):** serialize old `@kw` and new `kw` forms to
  `serde_json::Value`, run the existing `strip_spans` pattern (`hir/lower/mod.rs:733`
  — detects `{start,end}` objects), assert the Values are equal. Type-driven; immune
  to `Debug`-format drift. (Replaces rev-1's fragile `{:#?}` string-munging.)
- **Identifier preservation (the highest-value test):** assert every demoted word
  still parses as field name (`Search { query: str }`), enum binder (`Search(query)`),
  param (`fn f(query: str)`), method call (`db.query(…)`), and named-arg key. This is
  the proof the soft-keyword design preserves the corpus.
- **App-contract byte-identity:** emitted MCP manifest `description`/`uri`/`name` are
  byte-identical pre/post (catches the `tool`/`resource` field bugs).
- **Tombstone fires (typed):** assert `class == Tombstoned` and the `replacement`
  payload names the keyword — not a substring of the message. Added **after** the live
  dispatch arms are replaced (`@v0`/`@place` still parse today).
- **Codemod completeness:** after `--apply`, grep `.vox` **and** `.rs` literals,
  assert zero residual `@(table|index|query|mutation|server)` tokens incl. multiline.
- **Shrink witness:** ladder fixtures with demoted decorators only
  (`crud_api`/`db_native_ir`/`mcp_tools`/`web_routing_fullstack`); auto-baseline PRE
  counts from git, not hardcoded constants.
- **SSOT parity:** `decorator_feature_lexer_parity_mismatch()` is `None`;
  `ALL.len()==56`; grammar-ssot-parity green.

## Appendix A — full 56-decorator classification

`T1` = demote, tier-1 (this program). `T2` = demote, tier-2 (separate). `X` = kill.
`K` = keep decorator.

| Decorator | Verdict | Node today / rationale |
|---|---|---|
| `@table` | T1 | `Decl::Table(TableDecl)` |
| `@index` | T1 | `Decl::Index(IndexDecl)` — `index T.name on (cols)`, no `type`/body |
| `@query` | T1 | `Decl::Endpoint{Query}` |
| `@mutation` | T1 | `Decl::Endpoint{Mutation}` |
| `@server` | T1 | `Decl::Endpoint{Server}` |
| `@tool` | T1 | `Decl::McpTool{description, func}` (no name field) |
| `@resource` | T1 | `Decl::McpResource{uri, description, func}` (both strings required) |
| `@webhook` | T2 | `FnDecl.webhook: Option<AstWebhookSpec>` modifier → `Decl::Function` |
| `@subagent` | T2 | `FnDecl.subagent_*` modifiers → `Decl::Function` |
| `@form` | T2 | `Decl::Form(FormDecl)` block (not fn-shaped) |
| `@search` | T2 | standalone `@search(corpus=,query=,into=,top_k=)` directive — re-derive; possibly mis-classified |
| `@component` | X | already keyword; live `@v0`-style component path — add tombstone |
| `@mcp.tool` | X | superseded by `@tool`/`tool`; today only warns — upgrade to error |
| `@mcp.resource` | X | superseded; today only warns — upgrade to error |
| `@v0` | X | **still parses live** (`mod.rs:613`) — add tombstone |
| `@place` | X | **still parses live** — add tombstone; rev-1 "retired" was false |
| `@pure` `@deprecated` `@require` `@ensure` `@invariant` `@forall` `@fuzz` `@test` `@example` `@json_as` `@field_name` `@default` `@skip_if_none` `@ai` `@prompt` `@hole` `@reactive` `@versioned` `@tracked` `@scheduled` `@uses` `@embed` `@cancellable` `@loading` `@back_button` `@deep_link` `@push` `@tokens` `@cors` `@rate_limit` `@pii` `@public` `@auth` `@offline_capable` `@collaborative` `@layer` `@remote` `@inference` `@training_step` `@distributed_train` | K | true cross-cutting modifiers |

Totals: **T1 = 7, T2 = 4, X = 5, K = 40 → 56.** (Demoted/killed remain zombie
variants for the hard-error path; `ALL.len()` unchanged.)

## Appendix B — AST-equivalence harness (serde-based)

```rust
// vox-compiler/tests/keyword_decorator_equivalence.rs (sketch)
use vox_compiler::lexer::lex;          // re-export, matches syntax_k.rs
use vox_compiler::parser::parse;

fn strip_spans(v: &mut serde_json::Value) {
    // mirror hir/lower/mod.rs:733 — null any {start,end} object.
    match v {
        serde_json::Value::Object(m) => {
            if m.len() == 2 && m.contains_key("start") && m.contains_key("end") {
                *v = serde_json::Value::Null; return;
            }
            for val in m.values_mut() { strip_spans(val); }
        }
        serde_json::Value::Array(a) => for val in a { strip_spans(val); },
        _ => {}
    }
}

fn ast_eq(old_src: &str, new_src: &str) {
    let a = parse(lex(old_src)).expect("old parses");
    let b = parse(lex(new_src)).expect("new parses");
    let mut va = serde_json::to_value(&a).unwrap();
    let mut vb = serde_json::to_value(&b).unwrap();
    strip_spans(&mut va); strip_spans(&mut vb);
    assert_eq!(va, vb);
}

#[test] fn table()  { ast_eq("@table type User { name: str }", "table User { name: str }"); }
#[test] fn table_pk(){ ast_eq("@table(pk: uid) type User { uid: int }", "table(pk: uid) User { uid: int }"); }
#[test] fn index()  { ast_eq("@index User.by_name on (name)", "index User.by_name on (name)"); }
#[test] fn query()  { ast_eq("@query fn c() to int { return 0 }", "query c() to int { return 0 }"); }
#[test] fn tool_empty(){ ast_eq("@tool fn search(q: str) to str { return q }", "tool search(q: str) to str { return q }"); } // both description=""
#[test] fn tool_desc(){ ast_eq("@tool(\"web\") fn search(q: str) to str { return q }", "tool \"web\" search(q: str) to str { return q }"); }
#[test] fn resource(){ ast_eq("@resource(\"u\",\"d\") fn load() to str { return \"\" }", "resource \"u\" \"d\" load() to str { return \"\" }"); }

// Identifier preservation — must STILL parse (no equivalence, just success):
#[test] fn ident_uses_preserved() {
    for src in [
        "type Search { query: str }",
        "fn f(query: str, resource: str, table: str) to int { return 0 }",
        "@query fn g() to int { return len(db.query()) }",  // method name
    ] { parse(lex(src)).expect(src); }
}
```
