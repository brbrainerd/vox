---
title: "Plan — Core-Surface Taxonomy (P0 foundation)"
date: 2026-06-29
status: ready
spec: docs/superpowers/specs/2026-06-29-core-surface-taxonomy-design.md
program: core-surface-taxonomy
---

# Plan — Core-Surface Taxonomy (P0 foundation)

Lands: The Rule, 11 decorator→keyword demotions, 5 confirmed kills, one-shot
codemod, and the `vox ci source-token-budget` gate. **Safety property:** every new
keyword parses to the *same AST node* its decorator did (byte-identical-HIR), so
codegen does not change and golden outputs do not move.

TDD throughout: write the failing test, then the code. Subagents are read-only in
this sandbox — author and commit inline in the main session.

## Pre-flight

- [ ] `cargo test -p vox-compiler` green at HEAD (baseline).
- [ ] `vox run scripts/ci ... k-complexity-budget` green (confirm gate harness works before mirroring it).
- [ ] Confirm AST node names: `EndpointDecl`/`EndpointKind`, `McpToolDecl`, `McpResourceDecl`, table type decl (`descent/decl/head_types.rs`).

## Task 1 — Parser equivalence harness (failing first)

**Files:** `crates/vox-compiler/tests/keyword_decorator_equivalence.rs` (new).

- [ ] Write `ast_eq_modulo_span(old, new)` per Appendix B.
- [ ] One `#[test]` per construct (11): `table`, `index`, `query`, `mutation`,
      `server`, `webhook`, `form`, `search`, `subagent`, `tool`, `resource`.
- [ ] These FAIL now (new keywords don't lex). That failure is the spec for Tasks 2–4.

Need: a span-stripping helper. If none exists, add a minimal `strip_spans` test
helper local to this file (do not add a production API for test convenience).

## Task 2 — Lexer: keyword tokens

**Files:** `crates/vox-compiler/src/lexer/token.rs`, `lexer/cursor.rs`,
`language_surface.rs`.

- [ ] Add the 11 keyword spellings to `LEXER_KEYWORDS`.
- [ ] Add `Token` variants (or route through the existing keyword path if the lexer
      keys keywords off a table — follow `component`'s exact mechanism).
- [ ] Add the 11 old `@` spellings to `LEXER_DEPRECATED_DECORATORS`.
- [ ] Keep every `Token::At*` variant and `LEXER_AT_DECORATORS` entry (zombie path).
- [ ] Test: `lex("table")` yields the keyword token; `lex("@table")` still yields
      `Token::AtTable`.

## Task 3 — Parser: keyword heads producing existing AST nodes

**Files:** `crates/vox-compiler/src/parser/descent/decl/head.rs`,
`head_types.rs`, `head_fn.rs`, `head_component.rs`.

For each construct, add a `parse_<kw>` head that produces the **same** `Decl`
variant the decorator path produces:

- [ ] `table` / `index` → table/index type decl (mirror `parse` path of `@table`).
- [ ] `query`/`mutation`/`server`/`webhook`/`form`/`search`/`subagent` →
      `EndpointDecl { kind: <existing EndpointKind> }`. Reuse `parse_fn_decl` body
      parsing; the keyword replaces the `fn` token.
- [ ] `tool` → `McpToolDecl`; default `description`/name to the fn identifier when
      no string literal follows; accept an optional leading `StringLit` for an
      explicit name.
- [ ] `resource` → `McpResourceDecl`; require the URI string, then the identifier.
- [ ] Wire each into the top-level decl dispatch (where `@table` etc. dispatch now).
- [ ] Run Task 1 tests → they pass. **Do not touch HIR/typeck/codegen.**

## Task 4 — Deprecation diagnostics

**Files:** the decorator `parse_*` heads (e.g. `parse_mcp_tool` pattern at
`head.rs:40`), `parser/error.rs`.

- [ ] Each demoted/killed `@` spelling pushes a `ParseErrorClass::Tombstoned`
      error: "`@table` is retired; use `table`" (and the 10 others + the 5 kills).
- [ ] Decision: demoted forms **hard-error** (not warn) per the approved spec.
      Match the existing tombstone severity used for `@v0`/`@component`.
- [ ] Test: parsing `@table type User {}` yields the tombstone diagnostic naming
      `table`.

## Task 5 — SSOT lockstep + parity

**Files:** `language_surface.rs`, `feature_matrix.rs`, `vox-lsp/src/grammar.rs`,
any `grammar_ssot_parity.rs` gate.

- [ ] Move the 11 demoted spellings out of `LSP_DECORATOR_DOCS`/`LSP_DECORATOR_SNIPPETS`;
      add the 11 keywords to `LSP_KEYWORD_SNIPPETS`.
- [ ] Confirm `DecoratorFeature::ALL.len() == 56` still holds (no enum edits).
- [ ] `decorator_feature_lexer_parity_mismatch()` returns `None` — add/keep a test.
- [ ] Run `cargo test -p vox-compiler -p vox-lsp`; fix any grammar-parity drift.

## Task 6 — Codemod (one-shot)

**Files:** `scripts/migrate-decorator-keywords.vox` (new, VoxScript — no `.ps1`/`.sh`/`.py`).

- [ ] Rewrite `@kw type` → `kw` and `@kw fn` → `kw` (and `@tool("x") fn x` →
      `tool x`, `@resource("u") fn f` → `resource "u" f`) across:
      `examples/golden/**.vox`, `contracts/eval/**.vox`, doc anchors under `docs/`.
- [ ] Idempotent + dry-run flag. Run it; review the diff.
- [ ] Re-run the full golden suite (`golden_ts_test.rs`, interp, rust) → **outputs
      unchanged**. This is the live proof of the byte-identical-HIR invariant.

## Task 7 — Source-token budget gate

**Files:** `crates/vox-cli/src/commands/ci/cmd_enums.rs`,
`run_body.rs`, `run_body_helpers/mod.rs`,
`run_body_helpers/source_tokens.rs` (new, mirror `syntax_k.rs`),
`pipeline_parity.rs`, `contracts/eval/source-token-budget.v1.json` (new).

- [ ] Add `CiCmd::SourceTokenBudget { tolerance_percent, update }` (mirror
      `KComplexityBudget` at `cmd_enums.rs:512`).
- [ ] `run_source_token_budget(root, tol, update)`: for each ladder fixture in
      `examples/golden`, `count = lex(read(path)).len()`; compare to budget;
      ratchet-down; `--update` rebaselines. Copy the structure of `syntax_k.rs`
      including its two unit tests.
- [ ] Baseline the JSON via `--update` **after** Task 6 (so it records the shrunk
      counts).
- [ ] Wire the call into `pipeline_parity.rs` next to the k-complexity call.
- [ ] Test: a fixture exceeding budget fails; a missing-budget fixture fails when
      `!update`.

## Task 8 — Shrink-proof test

**Files:** `crates/vox-cli/tests/source_token_shrink_test.rs` (new) or fold into
Task 7's helper tests.

- [ ] For the 11 migrated golden fixtures, assert post-migration `lex().len()` is
      strictly less than the recorded pre-migration count (capture the pre-counts
      as constants from a one-time measurement noted in the test).

## Task 9 — Verify + commit

- [ ] `cargo test -p vox-compiler -p vox-lsp -p vox-cli -p vox-codegen` green.
- [ ] `vox ci pipeline-parity` (k-complexity + source-token) green.
- [ ] Clippy on touched crates: `cargo clippy -p vox-compiler -- -D warnings`
      (and per-crate for others touched). Do **not** `cargo fmt --all` (Windows
      arg-limit) — use `vox run scripts/fmt.vox` or `cargo fmt -p <crate>`.
- [ ] Commit. If pushing to main and admin-bypass is needed, verify clippy by hand
      first (admin-merge skips CI).

## Out of scope (governed sub-projects, separate plans)

P1 reactive streams (existing plan), P2 form primitives, P3 render control-flow
lowering, P4 interop hardening, P5 mobile/PWA, P6 fallback validation gate. The
keyword heads from Task 3 are the foundation those build on.
