# Core-Surface Taxonomy — Tier 2 (`form` keyword) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Demote `@form` to the `form` soft keyword (the one genuine kind-defining Tier-2 decorator), and reclassify `@webhook @subagent @search` as keep-decorators (a doc/Appendix correction — they produce `Decl::Function`, so they are modifiers, not kinds).

**Architecture:** Reuse the P0 soft-keyword machinery exactly. `form` gets no `logos #[token]`; the P0 positional dispatcher recognizes `Ident("form")` at declaration-head and routes to a `parse_form_kw` head that reuses `parse_form_decl`'s body to produce the identical `Decl::Form(FormDecl)`. `@form` is tombstoned with the P0 machine-readable payload. The three reclassified decorators get **zero code changes** — they already are `@`-decorators producing `Decl::Function`.

**Tech Stack:** Rust (`vox-compiler` parser), VoxScript (codemod extension), the P0 harness + tombstone payload.

**Spec:** `docs/superpowers/specs/2026-06-29-core-surface-taxonomy-tier2-design.md` (amends the P0 spec).

**Depends on:** `docs/superpowers/plans/2026-06-29-core-surface-taxonomy.md` (P0) — needs P0's positional dispatcher (`parse_decl` soft-keyword match), the `Replacement`/`ParseError::tombstone` payload, and the codemod harness. Execute P0 first.

---

## File Structure

| File | Responsibility | Change |
|---|---|---|
| `crates/vox-compiler/tests/keyword_decorator_equivalence.rs` | `form` equivalence + ident-preservation + reclassification guard | Modify (extend P0 harness) |
| `crates/vox-compiler/src/parser/descent/mod.rs` | add `form` to positional dispatch; tombstone `@form` (`:810`) | Modify |
| `crates/vox-compiler/src/parser/descent/decl/head_form.rs:10` | `parse_form_kw` wrapper (eat soft `form` instead of `@form`) | Modify |
| `crates/vox-compiler/src/language_surface.rs` | add `form` keyword; add `@form` to deprecated | Modify |
| `scripts/migrate-decorator-keywords.vox` | add `@form ` → `form ` rule | Modify |
| `docs/superpowers/specs/2026-06-29-core-surface-taxonomy-design.md` | Appendix A: 3 rows T2→K, `@form` T2→demote; totals 8/5/43 | Modify |

---

## Pre-flight

- [ ] **P0 landed.** Run: `cargo test -p vox-compiler --test keyword_decorator_equivalence` — Expected: PASS (P0 harness green, positional dispatch + tombstone payload exist).
- [ ] **Confirm the shapes.** Run: `grep -n 'Decl::Form\|Decl::Function' crates/vox-compiler/src/parser/descent/decl/head_form.rs crates/vox-compiler/src/parser/descent/mod.rs | head` — Expected: `head_form.rs` returns `Decl::Form`; the `@webhook/@search/@subagent` dispatch arms (`mod.rs:721-772`) return `Decl::Function`.

## Task 1: Reclassification guard (webhook/subagent/search stay decorators)

This locks in that the spec change is doc-only for the three — they must keep parsing exactly as today.

**Files:** Modify `crates/vox-compiler/tests/keyword_decorator_equivalence.rs`.

- [ ] **Step 1: Write the guard test**

```rust
// The three reclassified decorators are MODIFIERS on a function (Decl::Function),
// not kinds. This program changes nothing for them — assert they still parse.
#[test] fn reclassified_decorators_unchanged() {
    for src in [
        "@webhook fn on_push() to int { return 0 }",
        "@subagent(policy = strict) fn worker() to int { return 0 }",
        "@search(corpus = docs, query = \"x\", into = str, top_k = 3) fn q() to str { return \"\" }",
    ] {
        vox_compiler::parser::parse(vox_compiler::lexer::lex(src)).expect(src);
    }
}
```

- [ ] **Step 2: Run, verify it passes today**

Run: `cargo test -p vox-compiler --test keyword_decorator_equivalence reclassified_decorators_unchanged`
Expected: PASS (no code changed; this is a regression lock). If it fails, the chosen `@subagent`/`@search` arg syntax is wrong — re-derive from `head_fn.rs:285`/`:364` and `examples/golden`/`ai_fixtures` before continuing.

- [ ] **Step 3: Commit**

```bash
git add crates/vox-compiler/tests/keyword_decorator_equivalence.rs
git commit -m "test: lock @webhook/@subagent/@search as unchanged decorators (reclassification guard)"
```

## Task 2: `form` equivalence + ident-preservation (failing first)

**Files:** Modify `crates/vox-compiler/tests/keyword_decorator_equivalence.rs`.

- [ ] **Step 1: Write the failing tests** (reuse the P0 `ast_eq`/`strip_spans` helpers already in this file)

```rust
#[test] fn form_equivalence() {
    ast_eq(
        "@form Signup { field email: str\n on_submit: register }",
        "form Signup { field email: str\n on_submit: register }",
    );
}
#[test] fn form_as_identifier_preserved() {
    // soft keyword: `form` must still be usable as a param/field name.
    for src in ["fn f(form: str) to int { return 0 }", "type T { form: str }"] {
        vox_compiler::parser::parse(vox_compiler::lexer::lex(src)).expect(src);
    }
}
```

- [ ] **Step 2: Run, verify the split**

Run: `cargo test -p vox-compiler --test keyword_decorator_equivalence form_`
Expected: `form_equivalence` FAILS (`form Signup …` doesn't parse yet); `form_as_identifier_preserved` PASSES already (still `Ident`).

- [ ] **Step 3: Commit the failing test**

```bash
git add crates/vox-compiler/tests/keyword_decorator_equivalence.rs
git commit -m "test: form keyword equivalence + ident-preservation (failing)"
```

## Task 3: `parse_form_kw` head + positional dispatch

**Files:** Modify `crates/vox-compiler/src/parser/descent/decl/head_form.rs:10`, `crates/vox-compiler/src/parser/descent/mod.rs`.

- [ ] **Step 1: Add the keyword head.** `parse_form_decl` (`head_form.rs:10`) starts with `self.advance(); // eat @form` then `parse_ident_name` + `{ … }`. Add a sibling that eats the soft keyword and shares the body. Minimal form — extract the post-advance body into a private `parse_form_body(start)` both call, OR add a head that re-enters after the advance:

```rust
/// `form Name { field ... }` — soft-keyword form of the retired `@form`.
pub(crate) fn parse_form_kw(&mut self) -> Result<Decl, ()> {
    // identical to parse_form_decl except the leading token is the soft `form`
    // ident, not `@form`. self.advance() eats it, then the same name + `{...}` body.
    self.parse_form_decl_after_head()
}
```

Refactor `parse_form_decl` so its line-12 `self.advance()` is the only difference; both call `parse_form_decl_after_head(start)` which begins at `parse_ident_name` (`head_form.rs:13`).

- [ ] **Step 2: Add `form` to the P0 positional dispatcher.** In `descent/mod.rs` `parse_decl`, in the soft-keyword `match s.as_str()` block P0 added, insert:

```rust
        "form" => return self.parse_form_kw(),
```

- [ ] **Step 3: Run the equivalence test**

Run: `cargo test -p vox-compiler --test keyword_decorator_equivalence form_equivalence`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-compiler/src/parser/descent/decl/head_form.rs crates/vox-compiler/src/parser/descent/mod.rs
git commit -m "feat(parser): form soft-keyword head (reuses parse_form_decl -> Decl::Form)"
```

## Task 4: Tombstone `@form`

**Files:** Modify `crates/vox-compiler/src/parser/descent/mod.rs:810`, `crates/vox-compiler/src/language_surface.rs`.

- [ ] **Step 1: Failing test**

```rust
use vox_compiler::parser::error::ParseErrorClass;
#[test] fn form_decorator_tombstoned() {
    let errs = vox_compiler::parser::parse(
        vox_compiler::lexer::lex("@form Signup { field email: str }")).unwrap_err();
    assert!(errs.iter().any(|e| e.class == ParseErrorClass::Tombstoned
        && e.replacement.as_ref().map(|r| r.to == "form").unwrap_or(false)));
}
```

- [ ] **Step 2: Run, verify fail.** Run: `cargo test -p vox-compiler --test keyword_decorator_equivalence form_decorator_tombstoned` — Expected: FAIL (`@form` still parses live at `mod.rs:810`).

- [ ] **Step 3: Replace the dispatch arm.** At `descent/mod.rs:810`, change `Token::AtForm => self.parse_form_decl()` to the P0 tombstone:

```rust
            Token::AtForm => {
                let span = self.span();
                self.errors.push(ParseError::tombstone(
                    span, "@form", "form", "vox/decorator/form-retired",
                ));
                Err(())
            }
```

- [ ] **Step 4: SSOT.** In `language_surface.rs`: add `"form"` to `LEXER_KEYWORDS` + a `LSP_KEYWORD_SNIPPETS` entry; add `"@form"` to `LEXER_DEPRECATED_DECORATORS`; remove `@form` from `LSP_DECORATOR_DOCS`/`LSP_DECORATOR_SNIPPETS` if present. **Do not** touch `DecoratorFeature::ALL` or `LEXER_AT_DECORATORS` (parity lever).

- [ ] **Step 5: Run.** Run: `cargo test -p vox-compiler --test keyword_decorator_equivalence form_ && cargo test -p vox-compiler decorator_feature_lexer_parity` — Expected: PASS; parity `None`; `ALL.len()==56`.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-compiler/src/parser/descent/mod.rs crates/vox-compiler/src/language_surface.rs crates/vox-compiler/tests/keyword_decorator_equivalence.rs
git commit -m "feat(parser): tombstone @form -> form keyword (machine-readable payload)"
```

## Task 5: Codemod `@form` + Appendix-A reconciliation

**Files:** Modify `scripts/migrate-decorator-keywords.vox`, `docs/superpowers/specs/2026-06-29-core-surface-taxonomy-design.md`.

- [ ] **Step 1: Extend the codemod.** Add to `scripts/migrate-decorator-keywords.vox` a rule `@form ` → `form ` at declaration head (no `fn`/`type`/args to handle). Idempotent; covers `.vox` + Rust string literals (same target set as P0 Task 7).

- [ ] **Step 2: Dry-run + apply.**

Run: `vox run scripts/migrate-decorator-keywords.vox` then `vox run scripts/migrate-decorator-keywords.vox --apply`
Expected: every `@form` declaration head rewritten to `form`; no other changes.

- [ ] **Step 3: Completeness + output-stability.**

Run: `grep -rnE '@form\b' examples/golden contracts/eval crates --include=*.vox --include=*.rs` (Expected: zero code-position hits) then `cargo test -p vox-codegen golden && cargo test -p vox-compiler` (Expected: PASS; emitted output byte-identical).

- [ ] **Step 4: Reconcile the P0 spec Appendix A.** Edit `2026-06-29-core-surface-taxonomy-design.md`: change the `@webhook @subagent @search` rows from `T2` to `K` (rationale: modifier on `Decl::Function`); change `@form` from `T2` to a demotion handled here; update the totals line to **demotions = 8 (7 P0 + form), X = 5, K = 43 → 56**.

- [ ] **Step 5: Commit**

```bash
git add scripts/migrate-decorator-keywords.vox examples/golden contracts/eval crates docs/superpowers/specs/2026-06-29-core-surface-taxonomy-design.md
git commit -m "refactor: codemod @form -> form; reconcile Appendix A (3 reclassified, counts 8/5/43)"
```

## Task 6: Full verification

- [ ] `cargo test -p vox-compiler -p vox-cli -p vox-orchestrator-mcp -p vox-grammar-export -p vox-lsp -p vox-codegen` — Expected: PASS.
- [ ] `vox ci grammar-ssot-parity && vox ci source-token-budget` — Expected: PASS (the `form` keyword is in the grammar SSOT; `form`-bearing ladder fixtures, if any, ratcheted).
- [ ] `cargo clippy -p vox-compiler -- -D warnings`. Do **not** `cargo fmt --all` — use `cargo fmt -p vox-compiler`.
- [ ] `git log --oneline -7` — one commit per task.

---

## Out of scope

`@webhook @subagent @search` keep their exact current behavior (this plan only
corrects their classification). Genuine `webhook`/`subagent` *declaration kinds*
(distinct `Decl` nodes) would be new language design, not this program.
