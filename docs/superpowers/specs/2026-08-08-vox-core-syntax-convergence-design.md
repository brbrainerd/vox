---
title: "Vox Core-Syntax Convergence"
date: 2026-08-08
status: design
revision: 1
program: core-syntax-convergence
supersedes: []
related:
  - docs/src/architecture/vox-language-syntax-audit-2026-08-08.md
  - docs/superpowers/specs/2026-06-29-core-surface-taxonomy-design.md
  - docs/superpowers/specs/2026-06-20-vox-native-frontend-ssot-design.md
audit: "Evidence base: docs/src/architecture/vox-language-syntax-audit-2026-08-08.md (4-track audit of grammar, corpus, history, frontend at 759ad898b7)."
---

# Vox Core-Syntax Convergence

## Problem

The grammar audit found a clean expression/statement core wrapped in (a) a lexer
that silently drops unrecognized bytes, (b) ~5 coexisting corpus dialects for
equality, casing, arguments, imports, capabilities, and separators, (c) ~60
per-decorator lexer tokens, and (d) description surfaces (MENS system prompt,
AGENTS.md, EBNF export, tree-sitter) that all describe dead dialects. The two
main training corpora (goldens vs HumanEval-Vox) contradict each other on the
most frequent operator in the language. This blocks the CR-L2 ≥95 %
on-distribution goal: a model cannot converge on a corpus that disagrees with
itself.

## Decisions (operator-approved 2026-08-08)

1. **Break freely + auto-migrate.** Pre-1.0; every change ships with a
   `vox fmt --fix` rewrite that migrates the whole corpus in the same PR.
2. **Tolerant reader, strict writer.** Mainstream/legacy spellings are accepted
   as input with a warning + machine-readable `Replacement`; `vox fmt`
   canonicalizes. The committed corpus is 100 % canonical (enforced by CI
   fmt-idempotency), so training data never contains aliases.
3. **Scope: language core only.** The frontend/interop track (audit §6) gets
   its own spec. No endpoint re-litigation, no `struct/enum/trait/impl`, no
   indentation sensitivity, no new bare keywords.
4. Canonical picks: negation is **`is not`**; stdlib modules are **short-named**
   (`import fs`, `fs.read(...)`; `std.*` is a rewritten alias); full-expression
   interpolation is **in scope**.

All rules below argue from `LANGUAGE_DESIGN_PRIORITIES.md` (P0 unrepresentable >
P1 fewest decisions > P2 distinctive surface > P3 locality > P4 ergonomics > P5
familiarity-as-tiebreaker).

## S2 — Lexer hardening (P0)

- Delete `Err(_) => None` in `lexer/cursor.rs`. Unlexable bytes produce a
  spanned `Token::Unknown(char)`; the parser emits a diagnostic with a
  `Replacement` payload where a known mapping exists.
- Alias map (tolerant reader, **warning** severity): `;` → delete (message:
  "Vox statements end at end of line; no semicolon needed"), `&&`→`and`,
  `||`→`or`, `!`→`not` (existing `BangInvalid` folds into this path, severity
  drops from error to warning), `==`→`is`, `!=`→`is not`, `->`→`to` (existing
  warning retained), `=>` in match arms stays canonical.
- Genuinely unknown bytes (`^ ~ $ \` @unknown` etc.) are **errors** with
  nearest-token suggestions. A bare `@name` not in the decorator registry (S4)
  is "unknown decorator `@name`, did you mean `@…`" — never a silent identifier.
- Delete dead `parser/indent.rs`.

## S3 — One spelling per concept

Format: **canonical** / tolerated→rewritten (warn + fmt fix) / banned (fix-it error).

| Concept | Canonical | Tolerated → rewritten | Banned |
|---|---|---|---|
| Equality | `is`, `is not` | `==`, `!=`, `isnt` | — |
| Return type | `to` | `->` | — |
| Generics | `Name[T]` in all positions; `fn foo[T](…)`; **`type Foo[T] { … }` becomes parseable** | — | `<T>` on fn decls (0 corpus uses) |
| Type casing | containers lowercase `list/map/set`; ADTs capitalized `Option/Result/Unit/Json/Element` | `List[`→`list[`, `unit`→`Unit`, `Int/Str`→`int/str` | — |
| Type aliases | `type Meters = int` **works** (fix the silent-empty-TypeDef bug) | — | — |
| Lambdas | `fn(a: T) to U { … }`, `fn(a) { … }` | `x => e` → `fn(x) { e }` | — |
| Arguments vs fields | `=` binds arguments (calls **and** decorator args); `:` declares fields (type/object members) | decorator `key: value` → `key = value` | — |
| Separators | newline (multiline); comma (single-line) | trailing commas stripped | — |
| Length | `.len()` | `len(x)` → `x.len()` | `.length()` |
| List mutation | `x.push(y)` statement; mutating methods return `Unit` | — | `x = x.push(y)` (fix-it; semantic change, loud) |
| Error ctor | `Error(msg)` | — | `Err(` (fix-it) |
| Option | `.is_none()` / `.is_some()` | `x is null` → `.is_none()` (typeck-aware lint autofix) | new `null` in non-FFI surface |
| Capabilities | `@uses(fs, process, net)` on decls; file-level `@uses(…)` header for scripts | `uses` clause, `// vox:caps` pragma → header; `subprocess`→`process` | — |
| Comments | `//`, `///` | `#` → `//` | — |
| Stdlib modules | `import fs`; call as `fs.read(…)` | `std.fs.*` → `fs.*` (all modules incl. `http`, `time`) | — |
| Imports | `import fs` (stdlib) · `import a.b.C [as x]` / `import a.b as { C, D as E }` (project) · `import "./f.vox" as m` (path, **alias required**) · `import react … from "…"` · `import rust:crate(…)` | `/`-as-`.` → dots | CommonJS `module.exports`; alias-less path import (global injection) |

Casing law (one sentence, teachable): *lowercase words are things you use
(`fn let fs list str`); Capitalized words are shapes you define or match
(`Option Result Unit MyType Some Ok`).*

## S4 — Decorator surface collapse

- Replace ~60 `Token::At*` variants with one lexeme: `@` + identifier
  (dotted allowed) — `Token::Decorator(String)`.
- `DecoratorRegistry` (evolves `feature_matrix::DecoratorFeature`, stays in
  `vox-language-surface` as SSOT): name, arg schema (positional/named/none),
  applicability (fn / type / component / field / file), status
  (stable/deprecated/retired + `Replacement`), and doc line. Parser validates
  against the registry; arg parsing becomes one generic schema-driven path,
  deleting the 1,080-line per-decorator loop in `head_fn.rs`.
- Unknown decorator = registry error + nearest-name suggestion (P0: today it
  silently degrades to an identifier).
- The taxonomy law is restated as the acceptance rule for future surface: *a
  construct is a bare (soft) keyword iff it produces a distinct `Decl` variant;
  otherwise it is a decorator; value-level → builtin.* New bare keywords still
  require an ADR (closed-keyword-table rule).

## S5 — B-lite ergonomics (ceremony deletions)

1. **`else if` chains** — grammar: `if … { } else if … { } else { }`. Removes
   the 74 nested `else { if` pyramids; unblocks the 10 existing (currently
   silently-mis-lexed) script uses.
2. **Full-expression interpolation** — `"{user.name}"`, `"{a + b}"`,
   `"{f(x)}"`. Replaces the regex-classified `TemplateStringLit` with
   brace-aware scanning in the lexer (mode with nesting depth); `\{` escapes a
   literal brace; the JSON-in-string misclassification class dies with the
   regex. This is the largest implementation item in the spec (operator opted
   to keep it in).
3. **`?`-first Result idiom** — no grammar change (`?` exists). Goldens and
   tutorials rewritten to use `?`; a lint with autofix flags the
   match-and-rewrap pattern; `.unwrap()` outside `@test`/`@example` gets a
   warning steering to `?` / `.unwrap_or(…)`.

## S6 — Description surfaces regenerate or die

- Generate from `vox-grammar-export::grammar_ir` (single derivation root, each
  with `@generated-hash` provenance): the EBNF export, compact LLM prompt,
  `tree-sitter-vox` grammar (or retire it explicitly), GRAMMAR_SSOT.md,
  `docs/agents/vox-language-surface.v1.json`, **`mens/config/system_prompt.txt`**,
  and the AGENTS.md §Grammar Unification block (generated include or
  drift-checked).
- Build **CR-F3**: `contracts/spec/language-surface-coverage.v1.yaml` mapping
  every grammar production + decorator + builtin to ≥1 behavioral fixture; CI
  fails any grammar change without a coverage row. This is the gate that makes
  §4-of-the-audit drift structurally impossible.
- CHANGELOG entries: retroactive 0.6.x taxonomy-flip entry + this program's
  0.7.0 entry.

## S7 — Migration mechanics

- `vox fmt --fix` implements every S3 rewrite (parse tolerant → print
  canonical). Typeck-dependent rewrites (`is null`, push-reassignment) live in
  `vox check --fix` lints instead.
- **One atomic corpus PR**: all 743 `.vox` files, HumanEval-Vox regenerated to
  canonical dialect, golden frontmatter `constructs:` regenerated from actual
  parse, `examples/forbidden/` gains one negative fixture per banned form,
  stale twins deleted (`aspirational/intra-project-imports/`, root
  `test_syntax.vox`, orphan `tests/snip*.vox`), stdlib `builtin/std/mobile.vox`
  migrated.
- Deprecation ladder stays per existing policy: aliases warn for ≥1 minor
  version, flip to error no earlier than 0.8, deletions of alias *lexing* only
  at 1.0.

## S8 — Testing & acceptance

- Property: parse→fmt→parse fixpoint on the full corpus (fmt idempotency in CI).
- Paired fixtures per S3 row: alias input → canonical output (fmt), plus a
  warning-diagnostic assertion (reader) — both directions gated.
- `vox ci source-token-budget` must be non-increasing on the ladder fixtures;
  `syntax_k` telemetry recorded before/after migration.
- CR-L2 minimum-viable probe: common-word identifiers (`query`, `form`,
  `search`, `table` as variables/params/fields) keep parsing — regression
  fixture, plus the offline regeneration eval from the taxonomy spec
  (normative addition #3) run once on the migrated goldens.
- Grammar diff gate: any PR touching `lexer/`/`parser/descent/` without a
  CR-F3 coverage-ledger row fails.

## Sequencing (for the implementation plan)

1. S2 lexer hardening + alias map (unblocks everything; smallest diff, biggest P0 win)
2. S4 decorator collapse (parser simplification pays for the rest)
3. S3 canonicalization rules in fmt + reader warnings
4. S5.1 `else if`, S5.3 `?`-idiom lint; S5.2 interpolation last (largest item)
5. S6 regeneration + CR-F3
6. S7 atomic corpus migration
7. S8 gates flipped on

## Non-goals

Frontend/interop changes (own spec); endpoint shape changes; new declaration
kinds; indentation significance; match guards; range literals; `struct/enum/
trait/impl` keywords; any change to the durable subset (ADR-019/021/041).
