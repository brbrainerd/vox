---
title: "Vox Language & Syntax Audit (2026-08-08)"
description: "Empirical audit of the Vox grammar, the 743-file .vox corpus, description-surface drift, and frontend emission quality — evidence base for the core-syntax convergence spec."
category: "architecture"
status: "current"
---

# Vox Language & Syntax Audit (2026-08-08)

**Method.** Four parallel deep-reads over the repository at `759ad898b7`: (1) the full `.vox` corpus (743 tracked files, 24,324 lines, ~60 files read in full), (2) the implemented grammar in `crates/vox-compiler` (lexer + 22 parser files), (3) design history (ADRs, specs, commit archaeology since 2026-01), (4) the frontend emission pipeline (`vox-codegen`, `vox-codegen-ts`, `vox-gui`). Knowledge-graph coverage was rebuilt first (graphify: 39.5k → 70k nodes; `.vox` sources and docs are now indexed for search — they previously were not).

**Companion spec.** Recommendations are normative in
[`docs/superpowers/specs/2026-08-08-vox-core-syntax-convergence-design.md`](../../superpowers/specs/2026-08-08-vox-core-syntax-convergence-design.md).
This document is the evidence base only.

---

## 1. Headline findings

1. **Semicolons are already absent from the grammar** — and always were a no-op. The lexer has no `;` token (`crates/vox-compiler/src/lexer/token.rs`); newline is the statement separator, braces carry structure, indentation is cosmetic. The user-facing question "can we drop semicolons without significant whitespace" is answered *yes, shipped*. What remains is a hazard (finding 2) and a cleanup (1,145 stray `;` lines, §3).
2. **The lexer silently drops every unrecognized character** (`lexer/cursor.rs:56`, `Err(_) => None`). `let x = 5;` parses because the `;` vanishes — but so do `&`, `^`, `$`, backticks, and a bare `@`, so `a && b` lexes as `a b` and `@unknown_decorator` degrades to an identifier. The project was already burned by exactly this class once: `if !x` parsed as `if x` until `!` got a dedicated error token (`Token::BangInvalid`, `token.rs:107-113`). This is the single worst violation of design priority P0 ("make wrong programs unrepresentable") in the language.
3. **The corpus speaks ~5 dialects at once.** Equality alone: `==` 1,399 / `is` 703 / `!=` 111 / `isnt` 68 / `is not` 26 — and the populations are nearly disjoint (HumanEval benchmark corpus: 100 % `==`; goldens: `is`-canonical). The two main training corpora contradict each other. Full inventory in §3.
4. **Complexity is concentrated in declarations, not the core.** 8 statement kinds and 28 expression kinds are a small, clean core; but 44 `Decl` variants, ~60 per-decorator lexer tokens, a 55-field `FnDecl`, and a 1,080-line decorator-parsing loop carry the surface area. 67 % of parser LoC lives under `parser/descent/decl/`.
5. **Every description of the language is stale**; the only true grammar SSOT is the Rust source. Worst: `mens/config/system_prompt.txt` (the artifact that teaches MENS what Vox *is*) still teaches a colon-block, `ret`-using, `@table type`/`@query fn` dialect dead since April 2026. AGENTS.md §Grammar Unification contradicts shipped code (post-2026-06-30 taxonomy flip). The "authoritative" EBNF export and the tree-sitter grammar describe dead languages. CR-F3 — the v1 gate that would catch all of this mechanically — is unbuilt.
6. **Frontend emission is deterministic and clean but provenance-blind**, and React interop genuinely works today (§6).

---

## 2. Grammar reality (implemented, with citations)

- **Statement termination:** `parse_block` loops `skip_newlines → parse_stmt`; the Pratt infix loop simply breaks on `Newline` (`parser/descent/expr/pratt_ops.rs:61`). Consequences: leading-operator continuation works only on a binding RHS (`let x =\n 5` ✓, `let x = 1\n + 2` ✗ splits), and **method chains cannot span lines** (`foo\n .bar()` is two statements).
- **Tokens:** 57 hard keywords; soft/contextual keywords recognized positionally at decl-head (`routes url state_machine partial query mutation server table index tool resource form`, `descent/mod.rs:903-950`); ~60 decorator tokens, each a distinct lexer variant — the decorator set is **closed at the lexer level** with no `@ident` fallback.
- **Generics are inconsistent:** type expressions use `Name[T]` (`descent/types.rs:79-92`); `fn foo<T>()` uses angle brackets (`decl/head_fn.rs:1122-1133`); `type` declarations parse **no generics at all** (`TypeDefDecl.generics` hardcoded `vec![]`, `decl/mid.rs:216,284`) despite the AST documenting `type Response[T]`.
- **`type Alias = int` silently produces an empty TypeDef** (`mid.rs:281-290`); `type_alias` exists in the AST but no parser path sets it.
- **No ranges** (`..` doesn't lex; idiom is `range(a, b)`), **no match guards** (`MatchArm.guard` hardcoded `None`), **no hex/binary/exponent literals**, no block comments.
- **Five string forms** (plain, single-quoted, raw ×4 hash depths, template) where template-vs-plain is regex-disambiguated with a documented misclassification history (`token.rs:462-479`); interpolation accepts **bare identifiers only** ("Complex expressions in template strings not yet supported", `pratt_match.rs:1053-1071`).
- **Notable hacks:** named-arg detection backtracks (`pratt_match.rs:510-525`); view-call vs call disambiguation needs a triple guard plus a 90-entry hardcoded HTML tag allowlist (`pratt_match.rs:399-458, 912-940`); `{` is 3-way overloaded (block / object / empty-object) with 2-token lookahead; `@layer` needs bespoke lookahead (`descent/mod.rs:531-562`); `parser/indent.rs` is dead code with a factually wrong doc comment.
- **Error machinery is good:** accumulating errors, 2 recovery routines, machine-readable `Replacement { from, to, code }` payloads on retired-form tombstones (`parser/error.rs`). Weakest link: the generic `Expected X, found Y` with no construct context — the most common error path.
- **Size (K-complexity proxies):** lexer+parser ≈ 9,263 production LoC; `Token` ≈ 145 variants; `Decl` 44; `Expr` 28; `Stmt` 8; `TypeExpr` 7; `Pattern` 5.

## 3. Corpus reality (743 files, 24,324 lines)

Populations: HumanEval-Vox eval corpus (328 files), repair/plan-fidelity evals (130), goldens (79 + 12 TS + 15 forbidden), **scripts/ (83 files, 8,375 lines — the largest authored body)**, apps (10), fixtures (~68).

**Divergent spellings for the same concept** (counts are corpus-wide):

| Concept | Forms in use |
|---|---|
| Equality / negation | `==` 1,399 · `is` 703 · `!=` 111 · `isnt` 68 · `is not` 26 (dialects per population; both in single files, e.g. `scripts/quality/audit-workspace-health.vox:68` vs `:93`) |
| Semicolons | 1,145 `;`-terminated lines; 96 % of them in `scripts/` (13.1 % of script lines) — all silently dropped by the lexer. Goldens/evals ≈ 0 % |
| Generic casing | `list[` 674 vs `List[` 34 — the *types golden* uses `List`, the *interpolation golden* uses `list` |
| Return arrow | `to` 1,389 vs retired `->` ~103 — including the compiler's own stdlib `builtin/std/mobile.vox` |
| Named args | `key: value` (decorators) · `key = value` (AI-fixture decorators) · `key=value` (view calls, 168) · block-bodied (`@tokens { … }`) |
| Imports | 6 shapes; `import fs` vs `import fs;` vs dotted vs JS-style vs `rust:` scheme vs CommonJS `module.exports` (4 files). 12 files use both `fs.` and `std.fs.` prefix families; several scripts call `process.run` with **no import at all** |
| Capabilities | `// vox:caps` comment pragma (56) · `@uses(...)` (23) · `uses` clause (5); `process`/`subprocess` interchangeable within one pragma family |
| Field/arm separators | commas vs newlines, mixed within single files (`apps/vox-mental-tracker/src/main.vox:5-23`) |
| Length | `len(x)` 515 · `.len()` 353 · `.length()` 3 |
| List mutation | `x = x.push(y)` 96 vs statement `x.push(y)` 175 — both in goldens |
| Null | scripts run a parallel `is null`/`isnt null` model (89 sites) against values then `.unwrap()`ed as Options |
| Comments | `//` 3,317 · `///` 71 · shell `#` 19 |
| Lambdas | 5 forms (typed fn, untyped fn, nullary, one arrow-lambda site, bare closure types) |

**Verbosity hotspots (training-data quality):** `?` exists but appears 13× against 328 `.unwrap()`; the dominant Result idiom is a full match-and-rewrap (goldens teach it); JSON access is a 50-site `.get(k).and_then(...).unwrap_or(...)` chain the goldens label "CANONICAL FORM"; identifier-only interpolation forces temp-variable ceremony; `arr[i]` returning `Option` makes element loops 4–6 lines. `else if` does not exist → 74 nested `else { if` pyramids (10 `else if` occurrences exist *only* in `scripts/`, i.e. inside the silently-lenient population).

**Dead/legacy syntax still present:** `@endpoint(kind:)` ×51 in plan-fidelity bases (unmigrated); `@component fn` in 2 stray test files; stdlib `mobile.vox` entirely on `->`/`@mobile.native`/`Result[unit]`; `examples/aspirational/intra-project-imports/` duplicated by rewritten helpers but never deleted; fixtures named `fix_04_enum`/`fix_06_impl`/`fix_07_trait` for constructs that don't exist; goldens named for constructs they no longer contain (`ref_actors.vox`); one tracked scratch file at repo root (`test_syntax.vox`).

## 4. Description-surface drift (all stale against shipped code)

| Artifact | State |
|---|---|
| `mens/config/system_prompt.txt` (2026-06-13) | Teaches colon-block indentation syntax, `ret`, `@table type`, `@query fn`, `@component fn`, `@mcp.tool` — a dialect that has not existed since April. Highest-leverage training defect in the repo |
| `AGENTS.md` §Grammar Unification | Lists `@table @query @mutation @server` as canonical decorators; they are hard parse errors since 2026-06-30 (`cd7cc96874`); soft keywords `table/query/mutation/server/tool/resource/form/index` are canonical. `module`/`state_machine` listing also drifted |
| `vox-grammar-export/src/ebnf.rs` | Self-described "authoritative", actually hand-rolled and wrong: contains `ret`, `@endpoint`, retired decorators, `->` match arms; missing `when`, `fragment`, `state_machine`, `?`, `is/isnt`, raw/template strings, ~50 decorators |
| `tree-sitter-vox/grammar.js` | Describes the pre-brace colon-and-indent language, including features (match guards) the real parser lacks |
| `docs/agents/vox-language-surface.v1.json` | 4 keywords, 6 decorators, `updated_at 2026-04-19` — a stub |
| `docs/src/reference/parser-feature-matrix.md`, `ref-decorators.md` | Claim `@tool` "still parses" (hard error), `@pure`/`@scheduled` "not yet parseable" (both shipped) |
| `CHANGELOG.md` | No entry for the 2026-06-30 hard-error taxonomy flip (a breaking grammar change) |
| ADR index | Omits ADR-038–040/042/043; three files claim number 037 |

The designed antidote — **CR-F3**, a language-surface coverage ledger gating every grammar production to ≥1 behavioral fixture — is Unbuilt per `v1-readiness-status-2026-07.md` (4/19 criteria verified).

## 5. Design history signal

- The constitution (`LANGUAGE_DESIGN_PRIORITIES.md`) ranks: P0 unrepresentable > P1 fewest independent decisions > P2 distinctive surface (anti-Python drift) > P3 locality > P4 human ergonomics > P5 mainstream familiarity (tiebreaker only). Corollary C4: one primitive per concept; anti-pattern: "two ways to express the same data shape."
- The dominant historical motion is **removal**: `ret`, `@island`, `@component fn`, `@endpoint` (after oscillating through three shapes in eight weeks), `@native`, `http` routing, then the Tier-1/Tier-2 decorator demotions to soft keywords (PR #411 → flip `cd7cc96874`). The durable selection rule: *"Delete the annotation. Is it still the same kind of thing? Yes → decorator. No → keyword"* — sharpened to "kind-defining iff it produces a distinct `Decl` variant."
- Recurring fix themes: silent drops (decorator parsed, field set, nothing reads it), interp↔codegen↔codegen-ts arm parity ("four implementations of the same language"), SSOT mirror drift (structurally improved by the `vox-language-surface` leaf crate), script-mode heuristics destabilized by each soft-keyword addition.
- Trainability constraints already normative: append-only diagnostic IDs; single-token decorators; soft keywords chosen so `let query = …` never errors; machine-readable tombstone `Replacement` payloads; source-token budget gate (`vox ci source-token-budget`); K-complexity telemetry (`syntax_k.rs`); CR-L2 target ≥95 % on-distribution MENS emissions (measurement absent).

## 6. Frontend emission & React interop (evidence for the follow-up track)

- **Pipeline:** `.vox → HIR → WebIR (lower.rs) → validate (5 validators, one pass) → EmissionProfile/Target → React 19 TSX` with Tailwind classes; `useState`/`useMemo`/`useEffect`; full generated bootstrap (entry, router, error boundary, SW). No Solid/web-components/custom runtime. `Target` = `interp | rust-axum | rust-tauri | typescript`; the emission-side seam (`frontend_backend.rs`) is used only by `--target client` — the default fullstack path bypasses it.
- **Interop that works today:** `import react` default/named/namespace forms with deterministic import emission; JSX-tag rendering of imported components; known-library CSS auto-injection (12-package SSOT in `external_libs.rs`); React dedupe in Vite scaffold; TS-source FFI (`extern fn … = "./module"`); **shadcn/ui via `vox component <name>`** (registry fetch + alias rewrite — correct, since shadcn is not an npm dependency); Tauri event streams via `on stream(ch) as x:` with transport that degrades in a bare browser.
- **Gaps:** no prop typechecking of imported components (consumer `tsc` is the authority); namespace-member tags (`Dialog.Root`) unsupported; imported packages not auto-added to `package.json`; provider requirements are comments, not validators.
- **LLM-target quality:** strong determinism (sorted imports/attrs, no mangling, provenance headers on shared files, explanatory dep-inference comments) but **no sourcemaps and no per-component generated-from header** despite a complete `SourceSpanTable` in WebIR; invalid TS escapes the gate (`list[int]` → `list<number>` in 2 committed snapshots) because the `tsc --noEmit` gate covers only `examples/golden-ts/` (12 files), not the 79-file main corpus; behavioral render gate is a 2-row stub not in CI; 12 orphaned snapshots after a harness retarget; ragged indentation from string-splicing `inject_key_into_jsx`; a fail-open path emits components that compile and render nothing.
- **Tauri self-host ledger:** 13/26 GUI surfaces `.vox`-expressible today, 10 blocked on reactive streams (primitive landed; migration pending), 2 blocked-other (CDP mirror, PTY/xterm). Sub-projects A–G sequence exists; F (CI gate failing new hand-written `.tsx` surfaces) is not built.

## 7. Recommendations

Normative version in the [core-syntax convergence spec](../../superpowers/specs/2026-08-08-vox-core-syntax-convergence-design.md). Summary: lexer hardening (kill silent drop; fix-it diagnostics; `;` warn+strip), one-spelling-per-concept canonicalization under a tolerant-reader/strict-writer policy (`vox fmt --fix` rewrites), generic `@ident` decorator lexing + registry, three ergonomic deletions (`else if`, full-expression interpolation, `?`-first idiom), regeneration of all description surfaces from `grammar_ir` with CR-F3 built, and one atomic corpus migration. The frontend track (provenance headers + sourcemaps, tsc gate over the full corpus, behavioral gate in CI, stream-surface migration) is deliberately out of scope here and should get its own spec seeded by §6.
