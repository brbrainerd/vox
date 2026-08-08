---
title: "Vox Core-Syntax Convergence"
date: 2026-08-08
status: design
revision: 2
program: core-syntax-convergence
supersedes: []
related:
  - docs/src/architecture/vox-language-syntax-audit-2026-08-08.md
  - docs/superpowers/specs/2026-06-29-core-surface-taxonomy-design.md
  - docs/superpowers/specs/2026-06-20-vox-native-frontend-ssot-design.md
audit: "Evidence base: docs/src/architecture/vox-language-syntax-audit-2026-08-08.md (4-track audit of grammar, corpus, history, frontend at 759ad898b7). Adversarially reviewed 2026-08-08 (4 blind critique agents + direct code verification against the same commit): 4 blockers, ~10 majors confirmed and resolved in revision 2 — see 'Revision 2 changes' below."
---

# Vox Core-Syntax Convergence

## Problem

The grammar audit found a clean expression/statement core wrapped in (a) a lexer
that silently drops unrecognized bytes, (b) coexisting corpus dialects for
equality, casing, arguments, imports, capabilities, and separators (12 concepts
audited, spanning ~5 largely-disjoint corpus populations), (c) 56–57
per-decorator lexer tokens, and (d) description surfaces (MENS system prompt,
AGENTS.md, EBNF export, tree-sitter) that all describe dead dialects. The two
main training corpora (goldens vs HumanEval-Vox) contradict each other on the
most frequent *equality* operator in the language (`==` vs `is`). This blocks
the CR-L2 ≥95 % on-distribution goal: a model cannot converge on a corpus that
disagrees with itself.

## Decisions (operator-approved 2026-08-08)

1. **Break freely + auto-migrate.** Pre-1.0; every change ships with a
   `vox fmt --fix` rewrite that migrates the whole corpus in the same PR.
2. **Tolerant reader, strict writer.** Mainstream/legacy spellings are accepted
   as input with a warning + machine-readable `Replacement`; `vox fmt`
   canonicalizes. The committed corpus is 100 % canonical (enforced by CI
   fmt-idempotency), so training data never contains aliases.
3. **Scope: language core only.** The frontend/interop track (audit §6) gets
   its own spec. No endpoint re-litigation, no `struct/enum/trait/impl`, no
   indentation sensitivity, no new bare keywords. A small, explicitly-scoped
   typeck touch is unavoidable for two parser bugfixes (S3b) — this does not
   reopen the boundary, see S3b's own note.
4. Canonical picks: negation is **`is not`**; stdlib modules are **short-named**
   (`import fs`, `fs.read(...)`; `std.*` is a rewritten alias); full-expression
   interpolation is **in scope** (its regex-deletion half; see S5.2 for the
   split the review surfaced).
5. **Capability-enforcement parity is a hard gate, not a style choice.**
   Adversarial review found `// vox:caps` is, today, the *only* capability
   notation the interpreter actually reads (`crates/vox-compiler/src/eval/
   builtins.rs:953-967` gates `fs`/`io`/`process`/`env`/`secrets` only when
   `Interpreter.caps` is `Some(...)`, and that field is populated *only* from
   a literal `// vox:caps ` first-line pragma in `crates/vox-cli/src/commands/
   run.rs:44-66`; `@uses(...)`/`uses` clauses parse into `EffectAnnotation`
   but are never read by `eval/`). S3's capability-vocabulary row must not
   ship as a cosmetic fmt rewrite — see S3's Capabilities row and S7.

All rules below argue from `LANGUAGE_DESIGN_PRIORITIES.md` (P0 unrepresentable >
P1 fewest decisions > P2 distinctive surface > P3 locality > P4 ergonomics > P5
familiarity-as-tiebreaker).

## S2 — Lexer hardening (P0)

**Two `Err(_) => None` sites, both must change.** `lexer/cursor.rs:58` (`lex`,
the compiler's read path) and `lexer/cursor.rs:25` (`lex_preserving`, the
byte-preserving path `vox fmt`/`vox migrate` build on — its own doc comment
calls this "gap-fill"). Deleting only `:58` leaves the silent drop live on the
formatting path; deleting `:25` changes the byte-preservation contract that
`crates/vox-cli/src/commands/migrate/mod.rs:205-208` explicitly depends on
("bytes … land here as part of the inter-token gap") — that comment and any
`_`-arm logic downstream of it must be updated in the same change. Both sites
emit a spanned `Token::Unknown(char)`; the parser emits a diagnostic with a
`Replacement` payload where a known mapping exists.

- `;` → delete (message: "Vox statements end at end of line; no semicolon
  needed"), **warning** severity — this is a true lexer-only change (no AST
  shape change; the parser already just stops at end-of-statement).
- `->` → `to` **only in return-type position** (`parser/descent/mod.rs:182-193`
  already emits this as a `Warning` today — retained, not new work). `->` in
  **match-arm** position is a *separate, already-existing, default-severity
  error* (`parser/descent/expr/pratt_ops.rs:673-681`, canonical is `=>`) with
  no alias mapping — `->` is not globally tolerated, only in the one position
  that already tolerates it. Do not conflate the two in implementation.
- `==` → `is`, `!=` → `is not`: **no new parsing.** `pratt_ops.rs:56-59`
  already collapses `Token::EqEq`/`Token::NotEq` into the same `BinOp::Is`/
  `BinOp::Isnt` as `is`/`isnt` today. The only new work is emitting the
  reader-warning diagnostic at the point those tokens are consumed; `vox fmt`
  already has the AST-level information to print the canonical spelling for
  free.
- `&&` → `and`, `||` → `or`: **this is lexer *and* parser work, not lexer-only.**
  No `&`, `&&`, or `||` token exists today (`|` alone already lexes as
  `Token::Bar`, used for ADT variant separators — `&&`/`||` need their own
  Logos patterns to avoid a longest-match conflict with `Bar`/`Bar Bar`). A
  tolerant reader must also add `Token::AmpAmp`/`Token::BarBar` arms to the
  Pratt infix table (`pratt_ops.rs:44-59`) producing real `BinOp::And`/
  `BinOp::Or` nodes — a diagnostic alone does not make `a && b` compile.
- `!` → `not`: **keep as a fix-it *error*, do not downgrade to warning.**
  Reversing the original plan here: the audit's own headline finding is that
  `!` was already the project's one prior P0 near-miss (`if !x` silently
  parsing as `if x` before `BangInvalid` existed). `BangInvalid` today does
  `self.advance(); return Err(())` with **no AST node synthesized**
  (`pratt_match.rs:74-89`) — a true "tolerant" `!` would require building a
  `UnOp::Not` node, which reintroduces exactly the ambiguity the dedicated
  error token exists to prevent. The fix-it (fmt rewrites `!x` → `not x`)
  still fires; only the *reader* stays strict here, as an explicit exception
  to the general tolerant-reader policy.
- Genuinely unknown bytes (`^ ~ $ \` etc., no mapping above) are **errors**
  with nearest-token suggestions, **bounded**: cap surfaced diagnostics (e.g.
  first ~100, then "…and N more"), and use a static byte→suggestion lookup
  for single-character unknowns instead of a Levenshtein search per byte
  (`tokens/mod.rs:219-224`'s existing distance-≤2 sweep is the wrong tool for
  O(N) adversarial input — reserve it for multi-character unknown decorator
  names). Add a pathological-input fixture (long run of one unknown byte) to
  S8. A bare `@name` not in the decorator registry (S4) is "unknown decorator
  `@name`, did you mean `@…`" — never a silent identifier — and *is* an
  appropriate use of the registry's fuzzy-match search, since decorator names
  are a small, bounded set.
- Delete dead `parser/indent.rs` — verified not even a declared module in
  `parser/mod.rs` today; this is a zero-risk, zero-benefit cleanup, not a
  K-complexity win (it was never compiled).

## S3 — One spelling per concept

Format: **canonical** / tolerated→rewritten (warn + fmt fix) / banned (fix-it error).
Pure-syntax rows only — see **S3b** for the two rows the review found hiding a
typeck change behind a "spelling" framing.

| Concept | Canonical | Tolerated → rewritten | Banned |
|---|---|---|---|
| Equality | `is`, `is not` | `==`, `!=`, `isnt` | — |
| Boolean ops | `and`, `or`, `not` | `&&`, `\|\|`, `!` (see S2 — `!` stays a reader error, fmt still auto-fixes) | — |
| Return type | `to` (return-type position only) | `->` (return-type position only) | `->` in match-arm position (already an error today; no change) |
| Generics on `fn` | `fn foo[T](…)` | — | `fn foo<T>(…)` (0 corpus uses; deletes a live but unused parser path at zero migration cost) |
| Type casing | containers lowercase `list/map/set`; ADTs capitalized `Option/Result/Unit/Json/Element` | `List[`→`list[`, `unit`→`Unit`, `Int/Str`→`int/str` | — |
| Lambdas | `fn(a: T) to U { … }`, `fn(a) { … }` | `x => e` → `fn(x) { e }` | — |
| Arguments vs fields | `=` binds arguments (calls **and** decorator args); `:` declares fields (type/object members) | decorator `key: value` → `key = value` (both accepted during S4's deprecation window; see S4) | — |
| Separators | newline (multiline); comma (single-line) | trailing commas stripped | — |
| Length | `.len()` (`len(x)` free-function form removed from lint-preferred surface but **not deleted** — it stays a registered builtin per S4's taxonomy law, so the concept still has a legacy alias but a single canonical spelling) | `len(x)` → `x.len()` | — (`.length()`, 3 corpus sites, left to decline naturally — a dedicated ban is disproportionate to the evidence) |
| List mutation | `x.push(y)` statement; mutating methods return `Unit` | — | `x = x.push(y)` (fix-it; semantic change, loud) |
| Option | `.is_none()` / `.is_some()` | — | `x is null` (fix-it, **loud** — moved out of the quiet tolerated column: this population is exactly the one the audit flags as already confused, and only ships once a proven-Option receiver check exists; no rewrite under type uncertainty) |
| Capabilities | `@uses(fs, process)` on decls; file-level `@uses(…)` header for scripts. **`net` is not yet a gated namespace at runtime — do not imply enforcement that doesn't exist; document as declared-but-unenforced until a follow-up wires it into `eval/builtins.rs`'s gate.** | `uses` clause, `// vox:caps` pragma → header (**gated on S7's capability-parity prerequisite — see there; this row must not ship before it**); `subprocess`→`process` | — |
| Comments | `//`, `///` | `#` → `//` (**via the existing token-preserving `migrate` rewriter, not `vox fmt`** — see S7, `vox fmt` has no comment/trivia representation at all today) | — |
| Stdlib modules | `import fs`; call as `fs.read(…)` | `std.fs.*` → `fs.*` (all modules incl. `http`, `time`); rewrite **inserts `import fs` when absent** and **deduplicates** when both `fs.` and `std.fs.` already appear in one file (audit: 12 such files exist) | — |
| Imports | `import fs` (stdlib) · `import a.b.C [as x]` / `import a.b as { C, D as E }` (project) · `import react … from "…"` · `import rust:crate(…)` | `/`-as-`.` → dots | CommonJS `module.exports` (4 corpus files) — **path-import alias policy deferred**: the audit records bare relative `import "./f.vox"` with no alias as a live corpus form (global name injection); banning it is a name-resolution semantics change with no corpus count backing it, out of scope for this pass |

Casing law (one sentence, teachable): *lowercase words are things you use
(`fn let fs list str`); Capitalized words are shapes you define or match
(`Option Result Unit MyType Some Ok`).*

## S3b — Parser/typeck bugfixes (adjacent, not pure-syntax)

Two real bugs the audit found while cataloguing "spellings" are actually unset
typeck surface, not alternate spellings — S8's fmt-paired-fixture scheme
cannot test either as written, and both were originally folded into S3's
table where they don't belong. Both require a small, explicitly-scoped typeck
touch (Decisions §3 already carves this out — it is not a scope violation).

- **`type Foo[T] { … }` becomes parseable.** Today `TypeDefDecl.generics` is
  hardcoded `vec![]` in both parse branches (`parser/descent/decl/mid.rs:213`
  struct branch, `:288` ADT branch). Parsing `[T]` after the type name is the
  easy half. The hard half: `typeck/env.rs:72-78`'s `AdtDef` has no arity or
  parameter list at all, and `register_hir_typedef`
  (`typeck/registration.rs:320-343`) never reads `HirTypeDef.generics` even
  though the HIR field exists — contrast `register_hir_function`
  (`registration.rs:366-370`), which does bind `Ty::GenericParam` per generic.
  Landing this means: add arity to `AdtDef`, scope `T` during field
  resolution, and instantiate `Ty::GenericParam` at constructor/field-access/
  use sites. Acceptance test: `type Box[T] { value: T }` + a use site with two
  different concrete `T`s typechecks distinctly (not just "parses").
- **`type Meters = int` works.** `type_alias` exists on `TypeDefDecl` but is
  parser-set to `None` unconditionally (`mid.rs:216, 291`) and is read in
  exactly one place, `fmt/printer.rs:491` — there is no HIR lowering, no
  registration, no typeck resolution. Before implementing, decide (this spec
  does not, and should not, decide it unilaterally): does `Meters` become a
  **transparent alias** (unifies with `int` everywhere) or a **nominal
  newtype** (distinct type, needs explicit conversion)? This is a semantic
  design decision, not a bugfix, and belongs in its own short design note
  before implementation. Acceptance test differs by answer and must be
  written after that note lands.

## S4 — Decorator surface collapse

- Replace 57 `Token::At*` variants (`lexer/token.rs`; `DecoratorFeature::ALL`
  is 56 — the taxonomy spec already documents this 1-off, `@traced` has a
  token and no feature) with one lexeme: `@` + identifier (dotted allowed) —
  `Token::Decorator(String)`.
- **This is a token-representation collapse, not a spelling removal**, and
  must be described that way: the sibling taxonomy spec's explicit invariant
  is *"do not remove any spelling from `DecoratorFeature::ALL` or
  `LEXER_AT_DECORATORS`"* — those lists exist to carry retired-form tombstone
  lookups. The collapse changes *how* each name is represented (one generic
  token instead of one dedicated enum variant per name); the registry below
  still recognizes every current and retired spelling, so no name is
  actually removed. `decorator_feature_lexer_parity_mismatch()`
  (`language_surface.rs`) compares two enums and must be retired/replaced
  once decorators are a single generic token — add as an explicit task.
- **Initial collapse scope** (defer the rest): `DecoratorRegistry` (evolves
  `feature_matrix::DecoratorFeature` — which lives in
  `crates/vox-compiler/src/feature_matrix.rs` **today, not**
  `vox-language-surface`; moving it is itself unbudgeted work with a
  `vox ci crate-edges` implication and should be its own task) carries name,
  arg schema (positional/named/none — **must accept both `:` and `=`
  separators during S3's deprecation window**, since 8 existing per-decorator
  parsers hard-require `:` today: `@ensure`/`@webhook`/`@cors`/
  `@rate_limit`/`@pii`/`@layer`/`@auth`/`@embed`), and status
  (stable/deprecated/retired + `Replacement`). Parser validates against the
  registry; arg parsing becomes one generic schema-driven path, replacing the
  1,230-line `head_fn.rs`. **Defer to a follow-up:** the `applicability`
  (fn/type/component/field/file) axis and the `doc line` field — both are new
  validation/documentation surface with no test coverage described here, not
  needed for the collapse itself.
- **The dominant migration cost is not `head_fn.rs`.** `Token::At*` is
  referenced 193 times in `parser/descent/mod.rs` alone (script-mode
  decl-position heuristics and tombstone detection, e.g. `descent/mod.rs:
  318-322`'s `matches!(self.peek(), Token::Http | Token::AtComponent | …)`),
  vs 30 in `head_fn.rs` and 57 in the lexer itself. Every one of these
  becomes a string compare against the registry. This is exactly the code
  class the audit's own history section names as historically fragile
  ("script-mode heuristics destabilized by each soft-keyword addition") —
  budget accordingly, and re-run the full script-mode test suite, not just
  decorator-parsing tests. `lexer/compact.rs:92-100` is a third consumer (the
  `source-token-budget` metric S8 gates on) not covered by the above two.
- Unknown decorator = registry error + nearest-name suggestion (P0: today it
  silently degrades to an identifier). This is the one place a fuzzy
  Levenshtein search over the registry (`tokens/mod.rs:219-224`) is the right
  tool — see S2's bounded-cost note for why it is *not* the right tool for
  single-byte unknowns.
- The taxonomy law is restated as the acceptance rule for future surface: *a
  construct is a bare (soft) keyword iff it produces a distinct `Decl` variant;
  otherwise it is a decorator; value-level → builtin.* New bare keywords still
  require an ADR (closed-keyword-table rule).
- **Existing test migration** (not new coverage, just relocation): six lexer
  tests assert exact `Token::At*` variants (`lexer/cursor.rs`:
  `lexes_versioned_and_tracked_decorators`, `lexes_at_traced`,
  `test_query_and_mutation_lex_as_distinct_tokens`, `test_decorators`,
  `test_pure_scheduled_deprecated_tokens`, `test_component_decorator`,
  `test_chatbot_tokenizes`) become `Token::Decorator("…")` assertions.

### S4b — Retire `@v0` and the two other inert declaration kinds

Operator-directed 2026-08-08, verified against current code (not commit log
or docs) during the review pass. `@v0` is a **full declaration kind that
lowers to nothing**:

| Surface | Location |
|---|---|
| Lexer token `AtV0` | `lexer/token.rs:218-219` |
| Parser dispatch + parser | `parser/descent/mod.rs:654`, `parser/descent/decl/tail.rs:40-93` |
| AST variant `Decl::V0Component` | `vox-ast/src/decl/types.rs:145` (1 of 41) |
| `feature_matrix` entry | `feature_matrix.rs:280, 356, 515` |
| fmt printer arm | `fmt/printer.rs:176, 267` |
| Script-mode decl-head heuristic | `parser/descent/mod.rs:255` |
| **HIR lowering** | `hir/lower/mod.rs:417-419` — `Decl::V0Component(_) \| Decl::Page(_) \| Decl::Loading(_) => { /* Retired/legacy UI declarations: silently dropped from HIR. */ }` |

Corpus uses: **0**. Already listed in `LEXER_DEPRECATED_DECORATORS`
(`vox-language-surface/src/lib.rs:341`) — yet still advertised as canonical
to every agent session in `AGENTS.md:236`, and still present in the
VS Code extension README, `tree-sitter-vox/grammar.js`, and
`contracts/speech-to-code/vox_grammar_artifact.json`.

**Action:** retire the whole chain — token, parser fn, `Decl::V0Component`,
`V0ComponentDecl`, printer arm, `feature_matrix` entry, script-mode
heuristic arm, and the HIR drop-arm — with a `Replacement` tombstone
pointing at the tooling pathway in S4c. Remove from `AGENTS.md:236` and the
other description surfaces as part of S6.

`Decl::Page` and `Decl::Loading` share the same drop-arm and warrant the
same scrutiny; the review did not establish whether either has live corpus
or roadmap use, so they are **flagged, not scheduled** — confirm before
retiring, and do not bundle them into the `@v0` change without that check.

### S4c — Design-tool import is tooling, not language surface (normative)

`@v0` is the empirical cost of importing a design tool *as syntax*: six
compiler surfaces, zero output, zero adoption. The successful precedent in
this repo is `vox component <name>`
(`crates/vox-cli/src/commands/add_component.rs`), which vendors shadcn/ui by
fetching the registry, resolving `registryDependencies`, rewriting aliases,
and writing `.tsx` to disk — after which it is consumed through an ordinary
`import react Button from "./components/ui/button"`. Its own module doc
records the reasoning: shadcn is not npm-resolvable, so treating it as a
*dependency* is a category error. Treating a rendered design artifact as a
*language construct* is the same category error one level up.

**Normative rule:** design-tool and AI-artifact ingest (v0.dev, Claude
Design bundles, Figma exports, or any future equivalent) is implemented as a
CLI command with a source adapter — `vox design import <bundle> [--from
claude|v0|…]` — and **never** as a decorator, keyword, or `Decl` variant.
Any future proposal to add syntax for this class must first explain why it
is not `@v0` again.

Sketch (not scheduled by this spec; recorded so the decision is not
re-litigated). A Claude Design export contains `artifact.html`, `assets/`,
`manifest.json` (name/created/last-opened), and `comments.json` (inline
comments bound to elements):

- **Tier 1 — vendor as-is** (the shadcn shape): unzip, copy `assets/`, emit a
  thin `.tsx` wrapper plus a provenance header from `manifest.json`, consume
  via the existing React-interop import path (verified working today:
  default/named/namespace forms, CSS auto-injection, JSX-tag rendering).
  Zero new compiler surface. This is the version to build if it is ever
  built.
- **Tier 2 — optional, later:** `artifact.html` → WebIR → `.vox` component
  source. This is the *reverse* of an already-tested path (`web_ir/
  emit_tsx.rs` emits TSX from WebIR today, and WebIR already models
  `DomNode::Element/Text/Fragment/Conditional/Loop` plus a 90-tag HTML
  allowlist), so it is a `vox-codegen` feature, not a grammar change.
  `comments.json` lowers to ordinary comments, not annotations.

## S5 — B-lite ergonomics (ceremony deletions)

1. **`else if` chains** — grammar: `if … { } else if … { } else { }`. Removes
   the 74 nested `else { if` pyramids; unblocks the 10 existing script uses
   (today a **parse error**, not a lex error — `else`/`if` are both real
   tokens; the sequence just isn't a recognized production yet).
2. **Full-expression interpolation** — `"{user.name}"`, `"{a + b}"`,
   `"{f(x)}"`. Replaces the regex-classified `TemplateStringLit` with
   brace-aware scanning in the lexer (mode with nesting depth); `\{` escapes a
   literal brace; the JSON-in-string misclassification class dies with the
   regex. This is the largest implementation item in the spec (operator opted
   to keep it in). Three specific gaps the review found, all must be resolved
   in implementation, not left to discovery:
   - **Escape processing is a behavior change, not just a grammar change.**
     `TemplateStringLit` today does *zero* escape decoding (`token.rs:
     476-479` returns the raw inner slice) — `StringLit` does full `\n`/`\t`/
     `\\`/`\"` decoding but templates don't. Adding `\{` forces adding escape
     processing to the template path, which changes the *runtime value* of
     every existing corpus template string containing a backslash. This is a
     breaking change and must be in S7's migration list, not implicit.
   - **Sub-expression spans need real byte offsets.**
     `parse_expression_from_string` (`pratt_match.rs:1049-1056`) returns
     `self.span()` — the span of the *whole string token* — for every
     identifier inside. Full expressions need offset-mapped sub-spans
     (literal offset + the escape-decoding shift from the point above), or
     every diagnostic and the WebIR `SourceSpanTable` (audit §6) point at the
     enclosing string instead of the actual sub-expression.
   - **Only double-quoted templates interpolate.** State explicitly: raw
     strings (`r"..."`, all 4 hash-depth forms) and single-quoted strings do
     not gain expression interpolation — `TemplateStringLit`'s regex today
     sits at the same priority tier as `RawStringLit`'s bare form and the
     comment at `token.rs:462-467` documents a prior misclassification
     incident from loosening this exact boundary. A lexer *mode* replacing
     the regex must preserve the same non-participation for raw/single-quoted
     forms, not just replicate the regex's accidental behavior.
3. **`?`-first Result idiom** — no grammar change (`?` exists). Goldens and
   tutorials rewritten to use `?`; a lint with autofix flags the
   match-and-rewrap pattern; `.unwrap()` outside `@test`/`@example` gets a
   warning steering to `?` / `.unwrap_or(…)`.

## S6 — Description surfaces regenerate or die

- **Correction to the original framing:** `vox-grammar-export::grammar_ir` is
  **not** an existing derivation root to generate *from*. Verified:
  `grammar_ir.rs:45`'s only entry point is `from_ebnf(ebnf: &str)` — it
  *parses* the hand-written EBNF string, it does not produce one. `ebnf.rs`'s
  `emit_ebnf()` is 210 lines of hand-written `g.push_str(...)` calls whose own
  header falsely claims `"(* auto-generated from parser/descent/ *)"` and
  `"(* 57 production rules — do not hand-edit *)"` — this is precisely the
  defect the audit indicts in §4, one level removed. `grammar_ir::Grammar`'s
  symbol alphabet (`NonTerminal | Terminal | IdentClass | DigitClass`) also
  has no precedence, conflict, lexer-mode, or decorator-applicability
  representation — insufficient for `tree-sitter-vox` (needs precedence/
  conflicts) or the MENS prompt (needs prose + examples) even if the
  direction were fixed. **Corrected scope:** (1) build a real grammar IR
  derived from the parser itself (new work — this does not exist today,
  budget it as such, not as "regenerate from an existing root"); (2)
  regenerate the EBNF export, compact LLM prompt, GRAMMAR_SSOT.md, and
  `docs/agents/vox-language-surface.v1.json` from it, each with
  `@generated-hash` provenance; (3) **explicitly retire** `tree-sitter-vox`
  (mark unmaintained/superseded in its own README) rather than regenerate it
  — the spec already offered this as an alternative and the IR's symbol
  alphabet doesn't support it anyway.
- **Pull forward, no dependency on the above:** regenerating
  `mens/config/system_prompt.txt` (verified: 105 lines, last touched
  2026-06-13, still teaches `ret`, colon-block syntax, `@table type`,
  `@query fn`, `@component fn`, `@mcp.tool(...)` — the audit's own
  "highest-leverage training defect") does not depend on the grammar-IR work
  landing first. Do this as step 0, alongside AGENTS.md §Grammar Unification
  (also independently stale today, verified live during this review).
- Build **CR-F3** in **warn mode starting at step 0**, not at the end of
  Sequencing: `contracts/spec/language-surface-coverage.v1.yaml` mapping
  every grammar production + decorator + builtin to ≥1 behavioral fixture.
  Landing the ledger *before* S2/S4/S3/S5 means each of those steps adds its
  own coverage rows as it ships; landing it after means CR-F3 must be
  back-filled retroactively for the program's four largest grammar changes —
  exactly the kind of after-the-fact documentation task whose failure mode
  produced the audit's §4 drift in the first place. CI failing on missing
  rows (the hard gate) flips on only once the ledger is populated, at the end
  of Sequencing, alongside S8.
- CHANGELOG entries: retroactive 0.6.x taxonomy-flip entry + this program's
  0.7.0 entry (confirm the actual current `Cargo.toml [workspace.package]
  version` before pinning "0.7.0" — the deprecation ladder in S7 is
  version-anchored and must not be retroactive on arrival).

## S7 — Migration mechanics

- **Prerequisite, must land before any corpus-wide `vox fmt` run:** the
  formatter's `print_decl` currently handles exactly 20 of the language's 41
  `Decl` variants and silently drops the other 21 via a catch-all `_ => {}`
  (`fmt/printer.rs:179`, verified against the full enum in
  `vox-ast/src/decl/types.rs`) — including `Workflow`, `Activity`, `Actor`,
  `StateMachine`, `Url`, `Endpoint`, `Form`, `Tokens`, `Message`, `Agent`,
  `Loading`, `Theme`, `Page`, and every index/collection kind. Running `vox
  fmt --fix` over the corpus today would silently erase every durable
  workflow, actor, and state-machine declaration it touches, and S8's
  parse→fmt→parse fixpoint check **would not catch it** — a module with
  fewer declarations still re-parses cleanly. Completing the printer for all
  41 variants is a hard prerequisite, tracked as its own task, before S7's
  corpus pass runs.
- **`vox fmt` has no comment or trivia representation at all** — it formats
  via `lex()`, which strips `Token::Comment` unconditionally, and
  `fmt/printer.rs` has zero comment-emission code (verified: no match for
  "comment" anywhere under `crates/vox-compiler/src/fmt/`). Any rewrite
  involving a comment token — S3's `#`→`//` row, and critically the
  `// vox:caps` pragma migration — **cannot go through `vox fmt`** as
  architected. Route those specific rewrites through the existing
  token-preserving rewriter (`crates/vox-cli/src/commands/migrate/mod.rs`,
  today scoped to identifier renames only) instead, extended to cover
  comment-token rewrites. `vox fmt --fix` implements every other, purely
  AST-shaped S3 rewrite. Typeck-dependent rewrites (`is null`,
  push-reassignment) live in `vox check --fix` lints.
- **Capability-parity prerequisite** (Decisions §5): before the
  `// vox:caps` → `@uses(…)` header rewrite ships in any form, land an
  `@uses`-header runtime reader in `eval/` that the interpreter actually
  consults, and add the S8 fixture asserting `grant(new @uses form) ==
  grant(old vox:caps form)` for every one of the 56 pragma sites. Until then,
  the S3 Capabilities row's tolerated-alias direction does not run — `vox
  fmt` must leave `// vox:caps` pragmas untouched rather than silently
  disarm the sandbox on 56 scripts. This gate is fail-closed: if the reader
  isn't ready, the corpus pass skips this one row, it does not proceed
  anyway.
- **Three commits, not one atomic PR** (bisectability): (1) tooling lands,
  every new gate off, corpus untouched; (2) `vox fmt --fix` output only,
  machine-reproducible — CI asserts re-running the tool at that commit
  produces an empty diff, which keeps this commit re-derivable so a bisect
  hit here points at the tool (commit 1), not at 743 files; (3) hand-edits —
  typeck-dependent rewrites, deletions, frontmatter regeneration — small and
  actually reviewable, the real bisect target. All three still land in one
  PR/window; the split is about revert and bisect granularity, not sequencing
  delay.
- **`@endpoint(kind:)` sites are explicitly carved out, not silently
  included.** The audit records 51 unmigrated `@endpoint` sites, all in
  `contracts/eval/plan-fidelity/plans/*/base.vox` — reaching canonical there
  requires an endpoint-shape rewrite, which Decisions §3 and Non-goals both
  exclude from this program. State this directly rather than let "all 743
  files" imply full coverage: this migration reaches 692 of 743 files; the
  51 plan-fidelity fixtures are pre-existing tracked debt, out of scope here.
- **Preserve the pre-migration HumanEval-Vox corpus** (tag or
  `evals/legacy/`) before regenerating it to canonical dialect. It is
  simultaneously the rewrite target and the CR-L2 on-distribution
  measurement instrument (S8) — rewriting it in place with no baseline
  destroys the ability to measure improvement, not just the corpus.
- Golden frontmatter `constructs:` regenerated from actual parse; stale twins
  deleted (`aspirational/intra-project-imports/`, root `test_syntax.vox`,
  orphan `tests/snip*.vox`); stdlib `builtin/std/mobile.vox` migrated.
- **Concurrent-work coordination:** state a `.vox` freeze/rebase window for
  the duration of commit 2–3, and flip `vox fmt --check` to a hard
  corpus-wide CI gate at the *same* commit the migration lands (not a
  separate follow-up) — otherwise a cleanly-merging branch from a parallel
  worktree can silently reintroduce alias forms and void Decisions §2's "the
  committed corpus is 100 % canonical" claim. This is the exact
  parallel-agent-fmt-drift failure mode AGENTS.md's Perennial Bug Patterns
  section already names, at 743-file scale.
- Deprecation ladder stays per existing policy: aliases warn for ≥1 minor
  version, flip to error no earlier than 0.8, deletions of alias *lexing* only
  at 1.0. Confirm the current workspace version before pinning "0.8" (see S6).

## S8 — Testing & acceptance

- **Extend `crates/vox-compiler/tests/forbidden_corpus_test.rs` first.** It
  currently collapses every parse failure to the generic string
  `"parse-error"` — no banned-form fixture can assert *which* diagnostic
  fired, and nothing can assert a `Replacement` payload was actually emitted,
  which is the entire point of the tolerant-reader/fix-it mechanism. Extend
  the harness to surface real diagnostic codes + `Replacement` before adding
  any S3/S3b `examples/forbidden/` fixtures against it.
- Property: parse→fmt→parse fixpoint on the full corpus (fmt idempotency in
  CI) — **plus, not instead of:** `vox check` (typecheck) over the full
  migrated corpus, and a golden-emission-snapshot diff reviewed before merge.
  The fixpoint property is purely syntactic; several S3/S3b rows are
  semantics-changing (`is null` rewrite, push-reassignment, stdlib-prefix
  import injection, generics/alias typeck work) and a syntactically-clean but
  semantically-broken file passes the fixpoint check trivially.
- Paired fixtures per S3 row: alias input → canonical output (fmt), plus a
  warning-diagnostic assertion (reader) — both directions gated. Rows with no
  possible fmt pairing get their own named test instead of silent omission:
  boolean-operator aliases (new S3 row, was previously untested), the
  stdlib-module-prefix rewrite (assert no dangling reference and correct
  dedup on the 12 already-mixed-prefix files), the capabilities-header
  rewrite (per-capability grant-parity assertion, see S7), S3b's two
  bugfixes (positive-parse + typecheck-distinction tests, not fmt pairs).
- `vox ci source-token-budget` must be **non-increasing** (gate, not just
  recorded) on the ladder fixtures; `syntax_k` telemetry recorded **before
  and after** migration (a pre-migration baseline, not a single post-hoc
  number) — ties to S7's HumanEval-Vox preservation requirement.
- CR-L2 minimum-viable probe: common-word identifiers (`query`, `form`,
  `search`, `table` as variables/params/fields) keep parsing — regression
  fixture, plus the offline regeneration eval from the taxonomy spec
  (normative addition #3) run **before and after** the migration, not once
  post-hoc, against a stated ≥95 % on-distribution target.
- Pathological-input fixture (S2): a long run of one unknown byte does not
  produce unbounded diagnostics or unbounded parse time.
- Grammar diff gate: any PR touching `lexer/`/`parser/descent/` without a
  CR-F3 coverage-ledger row fails — active once CR-F3 is in warn mode from
  Sequencing step 0 onward (see S6); flips to hard-fail at the end of
  Sequencing.

## Sequencing (for the implementation plan)

Revised from revision 1 to resolve two confirmed ordering violations: S2's
own unknown-decorator diagnostic references S4's registry (couldn't ship
first as originally ordered), and S4's decorator-arg schema was ordered
before S3 locks the `:`-vs-`=` canonical rule it's built against.

0. **CR-F3 ledger scaffold (warn mode) + `mens/config/system_prompt.txt` +
   AGENTS.md §Grammar Unification regeneration** — no dependencies on
   anything else in this program; pulled forward because it's the
   single highest-leverage, lowest-risk fix available.
1. **S2 lexer hardening** — `;`/`->`/`==`/`!=` (no new parsing per the
   corrected framing), plus `&&`/`||` (real lexer+parser work), plus keeping
   `!` a reader error. Bounded unknown-byte diagnostics.
2. **S3 canonicalization rules locked** — in particular the `:`-vs-`=`
   decorator-arg decision, needed before S4 can build its schema against a
   stable target. `vox fmt`/reader-warning implementation for the rows that
   don't depend on S4 or S7's prerequisites can start here; the capabilities
   and comment rows wait on their stated prerequisites regardless of this
   step's position.
3. **S4 decorator collapse** — built against the now-locked separator rule,
   tolerant of the legacy separator through the deprecation window.
   **S4b `@v0` retirement** lands here (it removes surface the collapse would
   otherwise have to carry forward); S4c is a normative rule, not an
   implementation step.
4. **`vox fmt` `Decl`-printer completion** (all 41 variants) — disjoint from
   steps 1–3, can run in parallel with them; hard prerequisite for S7.
5. **S5.1 `else if`, S5.3 `?`-idiom lint** (small); **S5.2 interpolation**
   last within S5 (largest single item, three sub-gaps must all be closed
   per S5.2's list).
6. **S6 remaining regeneration** — grammar IR (new work, corrected scope),
   EBNF/compact-prompt/GRAMMAR_SSOT regeneration, tree-sitter retirement.
7. **S7 three-commit corpus migration** — gated on step 4 (printer) and the
   capability-parity prerequisite; explicitly excludes the 51
   `@endpoint`-bearing plan-fidelity files.
8. **S8 gates flip from warn to hard-fail**, including CR-F3.

## Non-goals

Frontend/interop changes (own spec); endpoint shape changes (including the
51 unmigrated `@endpoint(kind:)` plan-fidelity fixtures — tracked separately,
not touched by S7); new declaration kinds; indentation significance; match
guards; range literals; `struct/enum/trait/impl` keywords; any change to the
durable subset (ADR-019/021/041). S3b's two bugfixes are a narrow,
explicitly-scoped exception to "no typeck changes" — see Decisions §3 and
S3b's own note; nothing else in this program touches typeck.

**Explicitly not built here, and deliberately not language surface:** the
`vox design import` tooling pathway sketched in S4c. This spec retires
`@v0` (S4b) and records the normative rule that its replacement is a CLI
command; building that command is separate, unscheduled work.

## Revision 2 changes (adversarial review, 2026-08-08)

Four blind critique agents (feasibility-vs-code, spec/audit self-consistency,
scope/YAGNI + test coverage, security/operational readiness) plus direct
code verification of every high-severity claim. Confirmed and resolved in
this revision:

- **4 blockers**: (1) capability enforcement is `vox:caps`-pragma-only and
  fail-open — `@uses`/`uses` clauses are parsed but never read by `eval/`
  (Decisions §5, S3 Capabilities row, S7); (2) `vox fmt`'s printer silently
  drops 21 of 41 `Decl` variants including `Workflow`/`Activity`/`Actor`/
  `StateMachine` (S7 prerequisite); (3) `vox fmt` has zero comment/trivia
  handling, breaking the comments and capability-header rows as originally
  scoped (S3, S7); (4) S6's claimed `grammar_ir` derivation direction was
  backwards — it consumes hand-written EBNF, it does not generate it (S6,
  corrected scope).
- **~10 majors resolved**: `&&`/`||` reclassified as lexer+parser work, not
  lexer-only (S2); `!` kept as a reader error rather than downgraded (S2);
  `->` split into its two actually-independent positions with different
  severities (S2/S3); `type Foo[T]`/`type Meters = int` moved out of the
  spelling table into S3b with their real typeck scope stated; S4's registry
  location and taxonomy-invariant framing corrected, decorator-arg separator
  tolerance added, true migration-cost center identified (`descent/mod.rs`,
  not `head_fn.rs` alone); S5.2 interpolation's escape/span/priority gaps
  named explicitly; S7 split into three commits for bisectability, gated on
  the printer-completion and capability-parity prerequisites, `@endpoint`
  sites explicitly carved out, HumanEval-Vox baseline preservation added;
  S8 gained a compile-after-migration gate and extended the
  error-code-collapsing test harness.
- **Rejected with reason**: `.length()` and `Err(` bans cut from S3 (YAGNI —
  3 corpus sites and zero corpus sites respectively; natural decline, not
  worth a dedicated rule+fixture+ledger-row+description-surface footprint).
- **Added by operator direction after the review** (verified against current
  code, not docs): **S4b** retires `@v0` — confirmed to be a complete
  declaration kind (token, parser, AST variant, printer arm,
  `feature_matrix` entry, script-mode heuristic) that lowers to nothing
  (`hir/lower/mod.rs:417-419`), has zero corpus uses, is already in
  `LEXER_DEPRECATED_DECORATORS`, and is nonetheless still advertised as
  canonical in `AGENTS.md:236`. `Decl::Page` / `Decl::Loading` share its
  drop-arm and are flagged for the same check but not scheduled.
  **S4c** records the normative rule that design-tool / AI-artifact ingest
  (v0.dev, Claude Design bundles) is CLI tooling with a source adapter, never
  language surface — grounded in the `vox component <name>` shadcn precedent
  and in `@v0` as the measured cost of the alternative.
- **Corrected figures** (personally re-derived against the audited commit,
  not just re-cited): `Decl` = 41 variants (not 44; audit doc also
  corrected), decorator tokens = 57 lexer / 56 `DecoratorFeature` (not "~60"),
  `head_fn.rs` = 1,230 lines total.
- **Parallelism/orchestration finding** (Phase 5 of the review): this
  program's Sequencing is a strict dependency chain — every step after 0
  either reads state a prior step establishes (S3's separator rule before
  S4's schema, S2's registry reference on S4, the printer prerequisite before
  S7) or touches the same lexer/parser files a prior step just changed. No
  step pair is safely parallelizable as independent background agents, and
  the corrected sequencing above is deliberately more serial than revision 1,
  not less. **Workflow-tool orchestration does not fit any part of this
  program**: there is no bulk independent fan-out (S7's 743-file migration is
  one mechanical tool run, not 743 independent authoring tasks) and no
  multi-stage discovery→transform barrier of the shape Workflow earns its
  overhead on. A human (or a single agent) working through Sequencing steps
  0–8 in order, verifying each before starting the next, is the right shape
  for this plan — recommending Workflow here would itself be a scope/YAGNI
  violation.
- **Remains genuinely unverifiable without further work**: the precise
  runtime cost of the S2 unknown-byte diagnostic path under adversarial
  input (bounded by design in this revision, but not benchmarked); whether
  `type Meters = int` should be a transparent alias or nominal newtype
  (deferred to its own design note per S3b); the exact current
  `Cargo.toml [workspace.package] version` to anchor the deprecation ladder
  (not read during this review — confirm before implementation).
