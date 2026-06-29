# Core-Surface Taxonomy (P0 Foundation) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Demote the 7 **Tier-1** kind-defining decorators (`@table @index @query @mutation @server @tool @resource`) to **soft (contextual) keywords** that subsume the `fn`/`type` they sat on, hard-error the old `@` spellings (and 5 dead decorators) with a machine-readable replacement payload, codemod the `.vox` **and Rust** corpus, and add a source-token budget CI gate — all without changing HIR, typeck, or any codegen backend, and without breaking any existing identifier use of those words.

**Architecture:** Soft keywords, per the `get/post/put/delete` precedent: the words get **no `logos #[token]`** — they stay `Token::Ident` and are recognized **positionally** by the top-level decl dispatcher. The existing parser heads (`parse_table`/`parse_query`/…) already produce the correct `Decl` node; the soft-keyword head reuses them via a thin `parse_fn_decl_headless` wrapper that makes `fn` optional **only on the keyword path**. Tier-2 (`@webhook @subagent @form @search`) is OUT of scope — it is real AST work, not a rename (separate spec).

**Tech Stack:** Rust (`vox-compiler` parser/lexer-metadata, `vox-cli` CI, `vox-grammar-export`, `vox-orchestrator-mcp`), VoxScript (codemod), JSON budget files.

**Spec:** `docs/superpowers/specs/2026-06-29-core-surface-taxonomy-design.md` (revision 2 — post-audit).

---

## File Structure

| File | Responsibility | Change |
|---|---|---|
| `crates/vox-compiler/tests/keyword_decorator_equivalence.rs` | serde AST-equality + identifier-preservation | Create |
| `crates/vox-compiler/src/parser/descent/decl/head_fn.rs:1097` | `parse_fn_decl_headless` wrapper (optional `fn`) | Modify |
| `crates/vox-compiler/src/parser/descent/mod.rs` | positional soft-keyword dispatch in `parse_decl`; tombstone `@` arms | Modify |
| `crates/vox-compiler/src/parser/descent/decl/head.rs` | `tool`/`resource` keyword heads (correct fields) | Modify |
| `crates/vox-compiler/src/parser/descent/decl/mid.rs:392` | `table` keyword head (`TypeKw` optional; pk-before-name) | Modify |
| `crates/vox-compiler/src/parser/descent/decl/tail.rs:11` | `index` keyword head (reuse `parse_index`) | Modify |
| `crates/vox-compiler/src/parser/error.rs` | typed `replacement` payload on `Tombstoned` | Modify |
| `crates/vox-compiler/src/language_surface.rs` | keyword/deprecated SSOT lists | Modify |
| `crates/vox-grammar-export/src/ssot_markdown.rs:78` | hardcoded keyword copy + slice boundaries | Modify |
| `crates/vox-orchestrator-mcp/src/introspection_tools.rs` | MCP keyword/decorator lists | Modify (verify tests) |
| `scripts/migrate-decorator-keywords.vox` | one-shot corpus rewrite (vox + rust + multiline) | Create |
| `crates/vox-cli/src/commands/ci/run_body_helpers/source_tokens.rs` | source-token + byte gate | Create |
| `crates/vox-cli/src/commands/ci/{cmd_enums.rs:512,run_body.rs:356,run_body_helpers/mod.rs,pipeline_parity.rs:43}` | wire the gate | Modify |
| `contracts/eval/source-token-budget.v1.json` | per-fixture token + byte budget | Create |
| ~57 Rust test/source files embedding `@kw` string literals | codemod targets | Modify (via codemod) |

---

## Pre-flight

- [ ] **Baseline green.** Run: `cargo test -p vox-compiler` — Expected: PASS.
- [ ] **Gate template works.** Run: `cargo test -p vox-cli k_complexity_budget` — Expected: PASS.
- [ ] **Confirm the soft-keyword precedent.** Run: `grep -n '#\[token("get")\]\|#\[token("query")\]\|#\[token("table")\]' crates/vox-compiler/src/lexer/token.rs` — Expected: NO matches (these are NOT lexer tokens). This is why the design uses positional dispatch, not `#[token]`.

## Task 0: Corpus collision scan (gating — do not skip)

**Files:** none (analysis). Output: a checklist of identifier uses to preserve.

- [ ] **Step 1:** Grep the 7 Tier-1 words as identifiers across the corpus:

Run: `grep -rnE '\b(table|index|query|mutation|server|tool|resource)\b\s*:' examples/golden contracts/eval` (field/param uses) and `grep -rnE '\.(query|index)\(' examples/golden contracts/eval` (method uses).
Expected: hits incl. `json_as_typed.vox` (`query:` field), `index_showcase.vox` (`query` param), `multi_tenancy.vox` (`resource` param), `020-pure-calls-db` (`db.query()`).

- [ ] **Step 2:** Record each as a required identifier-preservation case for Task 1. These MUST still parse after the change. If any of the 7 words is used in **declaration-head position** as a plain identifier (it should not be), flag it — that is the only true ambiguity.

## Task 1: AST-equivalence + identifier-preservation harness (failing first)

**Files:** Create `crates/vox-compiler/tests/keyword_decorator_equivalence.rs`.

- [ ] **Step 1: Write the harness** (serde-based, per spec Appendix B — copy that code verbatim, including `strip_spans`, `ast_eq`, the 7 equivalence tests, and `ident_uses_preserved`).

- [ ] **Step 2: Run, verify it fails the right way**

Run: `cargo test -p vox-compiler --test keyword_decorator_equivalence`
Expected: the `ast_eq` tests FAIL (new `table`/`query`/… forms don't parse yet); **`ident_uses_preserved` PASSES already** (the words are still `Ident` today). If `ident_uses_preserved` fails now, stop — the corpus is already broken.

- [ ] **Step 3: Commit the failing harness**

```bash
git add crates/vox-compiler/tests/keyword_decorator_equivalence.rs
git commit -m "test: serde AST-equivalence + identifier-preservation harness (failing)"
```

## Task 2: `parse_fn_decl_headless` wrapper (optional `fn`, keyword path only)

The `fn` token is consumed at `head_fn.rs:1097` (`self.expect(&Token::Fn)?`), **after** a ~1025-line decorator loop (69-1095). Do NOT weaken the shared `expect` — that would silently legalize `@pure foo()` for all 13 callers. Add a wrapper.

**Files:** Modify `crates/vox-compiler/src/parser/descent/decl/head_fn.rs`.

- [ ] **Step 1: Failing test** (append to harness)

```rust
#[test] fn headless_query_parses() {
    parse(lex("query f() to int { return 1 }")).expect("headless query parses");
}
#[test] fn bare_call_still_rejected_at_toplevel() {
    // optional-fn must NOT widen the grammar: a bare `foo() {}` with no keyword/fn errors.
    assert!(parse(lex("foo() { }")).is_err(), "bare headless decl must still error");
}
```

- [ ] **Step 2: Run, verify fail.** Run: `cargo test -p vox-compiler --test keyword_decorator_equivalence headless_query_parses` — Expected: FAIL.

- [ ] **Step 3: Add the wrapper.** Refactor `parse_fn_decl` so its body after the `fn`-consume is reusable, then:

```rust
/// Keyword-headed declarations (query/mutation/server/tool) call this: the kind
/// keyword has already subsumed `fn`, so the `fn` token is optional here ONLY.
pub(crate) fn parse_fn_decl_headless(&mut self, is_pub: bool) -> Result<FnDecl, ()> {
    // identical to parse_fn_decl except the fn-token consume is `eat` not `expect`:
    //   self.eat(&Token::Fn);  // optional on the keyword path
    // Implement by extracting the shared tail (decorator loop already ran) into a
    // helper both call, OR duplicate the one differing line. Keep `expect` in
    // parse_fn_decl unchanged so plain `fn`/`@decorator`/`pub fn` still require `fn`.
}
```

- [ ] **Step 4: Run.** Run: `cargo test -p vox-compiler` — Expected: PASS (existing `fn` callers unchanged; wrapper compiles). `bare_call_still_rejected_at_toplevel` passes because the top-level dispatcher never routes a bare `Ident(` into the headless path (Task 3).

- [ ] **Step 5: Commit.**

```bash
git add crates/vox-compiler/src/parser/descent/decl/head_fn.rs crates/vox-compiler/tests/keyword_decorator_equivalence.rs
git commit -m "feat(parser): parse_fn_decl_headless wrapper (optional fn on keyword path)"
```

## Task 3: Positional soft-keyword dispatch (the 7 Tier-1 heads)

**Files:** Modify `crates/vox-compiler/src/parser/descent/mod.rs` (`parse_decl`), `head.rs`, `mid.rs`, `tail.rs`.

- [ ] **Step 1:** In `parse_decl`, before the fallback that treats a leading `Ident` as an expression/error, add positional recognition:

```rust
// Soft keywords (no logos token; recognized only at declaration-head position).
if let Token::Ident(s) = self.peek() {
    match s.as_str() {
        "table"    => return self.parse_table_kw(),
        "index"    => return self.parse_index_kw(),
        "query"    => return self.parse_endpoint_kw(EndpointKind::Query),
        "mutation" => return self.parse_endpoint_kw(EndpointKind::Mutation),
        "server"   => return self.parse_endpoint_kw(EndpointKind::Server),
        "tool"     => return self.parse_tool_kw(),
        "resource" => return self.parse_resource_kw(),
        _ => {} // any other ident falls through unchanged
    }
}
```

> Guard against false positives: only enter this match where `parse_decl` is parsing
> a *top-level declaration*, never in expression/statement position. Confirm the
> dispatch site is the top-level decl loop (the same place `Token::AtTable` dispatches
> at `mod.rs:855`), so `let query = 1` (statement) and `db.query()` (expr) never reach
> it.

- [ ] **Step 2:** Implement the endpoint head (reuses the existing body):

```rust
fn parse_endpoint_kw(&mut self, kind: EndpointKind) -> Result<Decl, ()> {
    self.advance(); // eat the soft keyword (query/mutation/server)
    self.skip_newlines();
    let f = self.parse_fn_decl_headless(false)?;
    Ok(Decl::Endpoint(EndpointDecl { kind, func: f }))
}
```

- [ ] **Step 3: Run the endpoint equivalence tests.** Run: `cargo test -p vox-compiler --test keyword_decorator_equivalence query` — Expected: `query` PASSES.

- [ ] **Step 4: Commit.**

```bash
git add crates/vox-compiler/src/parser/descent/mod.rs crates/vox-compiler/src/parser/descent/decl/head_fn.rs
git commit -m "feat(parser): positional soft-keyword dispatch + endpoint heads (query/mutation/server)"
```

## Task 4: `table` / `index` / `tool` / `resource` heads (correct nodes)

**Files:** `mid.rs:392`, `tail.rs:11`, `head.rs:40-150`.

- [ ] **Step 1: `table` head.** Add `parse_table_kw`: copy `parse_table` (`mid.rs:392`) but eat the soft keyword instead of `@table`, and relax the type consume — note the token is **`Token::TypeKw`**, not `Token::Type`:

```rust
fn parse_table_kw(&mut self) -> Result<Decl, ()> {
    self.advance(); // eat `table`
    // ... existing (pk:)/(extern)/(source:) arg loop (mid.rs:404-495) UNCHANGED —
    //     args come BEFORE the name, so `table(pk: uid) User { ... }` ...
    self.eat(&Token::TypeKw); // was expect(&Token::TypeKw) at mid.rs:497 — now optional
    // ... existing name + `{` body parsing UNCHANGED, producing Decl::Table(TableDecl)
}
```

- [ ] **Step 2: `index` head.** `@index` is `index Table.name on (cols)` → `Decl::Index` (`tail.rs:11`), structurally unlike `table` — **do not clone `parse_table`**. Reuse the existing body:

```rust
fn parse_index_kw(&mut self) -> Result<Decl, ()> {
    self.advance(); // eat `index` (the rest of tail.rs:11 parse_index body verbatim)
    // ... Table.name on (cols) -> Decl::Index(IndexDecl{...})
}
```

- [ ] **Step 3: `tool` head.** Map the optional leading string to **`description`** (empty default), never to a name:

```rust
fn parse_tool_kw(&mut self) -> Result<Decl, ()> {
    self.advance(); // eat `tool`
    let description = if let Token::StringLit(s) = self.peek().clone() { self.advance(); s }
                      else { String::new() };
    self.skip_newlines();
    let f = self.parse_fn_decl_headless(false)?;
    Ok(Decl::McpTool(McpToolDecl { description, func: f })) // name is func.name downstream
}
```

- [ ] **Step 4: `resource` head.** `McpResourceDecl{uri, description, func}` — both strings required (mirror `head.rs:65-139`):

```rust
fn parse_resource_kw(&mut self) -> Result<Decl, ()> {
    self.advance(); // eat `resource`
    let uri = self.expect_string("resource URI")?;
    let description = self.expect_string("resource description")?;
    self.skip_newlines();
    let f = self.parse_fn_decl_headless(false)?;
    Ok(Decl::McpResource(McpResourceDecl { uri, description, func: f }))
}
```

- [ ] **Step 5: Run all Tier-1 equivalence + ident tests.** Run: `cargo test -p vox-compiler --test keyword_decorator_equivalence` — Expected: PASS (all 7 equivalence + `table_pk` ordering + `ident_uses_preserved`).

- [ ] **Step 6: Commit.**

```bash
git add crates/vox-compiler/src/parser/descent/decl/
git commit -m "feat(parser): table/index/tool/resource soft-keyword heads (correct AST nodes)"
```

## Task 5: Tombstones with machine-readable replacement payload

**Files:** `error.rs`, `descent/mod.rs`, `language_surface.rs`.

- [ ] **Step 1: Typed payload.** Add to `ParseError` (`error.rs`):

```rust
pub struct Replacement { pub from: String, pub to: String, pub code: String }
// field: pub replacement: Option<Replacement>
// constructor: ParseError::tombstone(span, from, to, code)
```

- [ ] **Step 2: Failing test** (harness):

```rust
use vox_compiler::parser::error::ParseErrorClass;
#[test] fn table_decorator_tombstoned() {
    let errs = parse(lex("@table type User { name: str }")).unwrap_err();
    assert!(errs.iter().any(|e| e.class == ParseErrorClass::Tombstoned
        && e.replacement.as_ref().map(|r| r.to == "table").unwrap_or(false)));
}
#[test] fn v0_and_place_killed() {
    for src in ["@v0 fn C() to Element {}", "@place fn f() {}"] {
        assert!(parse(lex(src)).is_err(), "{src} must be tombstoned");
    }
}
```

- [ ] **Step 3: Replace dispatch arms.** In `descent/mod.rs`, change the Tier-1 `@` arms (`AtTable`/`AtQuery`/…/`AtResource`) AND the 5 kill arms — **including the still-live `Token::AtV0 => parse_v0_component` (`mod.rs:613`) and `@place`** — to push `ParseError::tombstone(...)` and return `Err(())`. Upgrade `@mcp.tool`/`@mcp.resource` from warning to tombstone. Each names its replacement (`@table`→`table`, `@v0`→none/removed).

- [ ] **Step 4: SSOT.** Add the 7 Tier-1 spellings + `@place` to `LEXER_DEPRECATED_DECORATORS`; add the 7 soft keywords to `LEXER_KEYWORDS` + `LSP_KEYWORD_SNIPPETS`; remove the 7 from `LSP_DECORATOR_DOCS`/`LSP_DECORATOR_SNIPPETS`. **Do NOT remove anything from `DecoratorFeature::ALL` or `LEXER_AT_DECORATORS`** (the real parity lever).

- [ ] **Step 5: Run.** Run: `cargo test -p vox-compiler --test keyword_decorator_equivalence table_decorator_tombstoned v0_and_place_killed && cargo test -p vox-compiler decorator_feature_lexer_parity` — Expected: PASS; `ALL.len()==56`; parity `None`.

- [ ] **Step 6: Commit.**

```bash
git add crates/vox-compiler/src/parser/error.rs crates/vox-compiler/src/parser/descent/mod.rs crates/vox-compiler/src/language_surface.rs crates/vox-compiler/tests/keyword_decorator_equivalence.rs
git commit -m "feat(parser): tombstone retired decorators with machine-readable replacement payload"
```

## Task 6: SSOT lockstep — grammar export + MCP introspection

**Files:** `vox-grammar-export/src/ssot_markdown.rs:78`, `vox-orchestrator-mcp/src/introspection_tools.rs`, `tree-sitter-vox/GRAMMAR_SSOT.md` (generated).

- [ ] **Step 1:** Update the **hardcoded** `LEXER_KEYWORDS`/`LEXER_DECORATORS` copies in `ssot_markdown.rs` AND fix the fixed slice boundaries `[..19]/[19..36]/[36..]` (`ssot_markdown.rs:78-84`) for the 7 new keywords. Regenerate `GRAMMAR_SSOT.md` via the generator (do not hand-edit).
- [ ] **Step 2:** Confirm `introspection_tools.rs` still lists `@tool`/`@resource` (its test at `:247`) — demoted decorators stay in `LEXER_DECORATORS` (zombie), so the MCP list is unaffected; add the 7 keywords to its keyword surface if it enumerates them.
- [ ] **Step 3: Run.** Run: `cargo test -p vox-grammar-export -p vox-orchestrator-mcp && vox ci grammar-ssot-parity` — Expected: PASS.
- [ ] **Step 4: Commit.**

```bash
git add crates/vox-grammar-export crates/vox-orchestrator-mcp tree-sitter-vox/GRAMMAR_SSOT.md
git commit -m "chore(ssot): grammar-export + MCP introspection lockstep for soft keywords"
```

## Task 7: Codemod (vox + Rust + multiline + frontmatter)

**Files:** Create `scripts/migrate-decorator-keywords.vox`.

- [ ] **Step 1: Write the codemod.** Targets: `examples/golden/**.vox`, `contracts/eval/**.vox`, doc anchors, **and Rust string literals in ~57 `.rs` files** (e.g. `crates/vox-compiler/tests/{interpreter_db_test,db_query_safety_test,projection_parity_test}.rs`, `src/app_contract.rs`, `src/hir/lower/mod.rs`, `src/parser/descent/tests.rs`). Rules:
  - `@(table|index|query|mutation|server)\s+(fn|type)` → keyword (match **across newlines+whitespace** — most goldens are multiline, `crud_api.vox:12-14`).
  - `@table(<args>)\s+type` → `table(<args>)` (carry `pk:`/`extern`/`source:`).
  - `@tool("x")\s+fn` → `tool "x"` ; `@tool\s+fn` → `tool` ; `@resource("u","d")\s+fn` → `resource "u" "d"`.
  - `@index ... on (...)` → drop only the `@`.
  - Update `constructs:` frontmatter + `@training_prompt` prose deliberately to keyword spelling.
  - Idempotent; `--apply` writes, default dry-run.

- [ ] **Step 2: Dry-run + review.** Run: `vox run scripts/migrate-decorator-keywords.vox` — Expected: prints rewrites incl. multiline and Rust literals; no writes.
- [ ] **Step 3: Apply.** Run: `vox run scripts/migrate-decorator-keywords.vox --apply`.
- [ ] **Step 4: Completeness assertion.** Run: `grep -rnE '@(table|index|query|mutation|server)\b' examples/golden contracts/eval crates --include=*.vox --include=*.rs` — Expected: **zero** code-position hits (only allowed inside the codemod script's own pattern strings).
- [ ] **Step 5: Prove outputs unchanged.** Run: `cargo test -p vox-codegen golden && cargo test -p vox-compiler` — Expected: PASS; emitted artifacts byte-identical; the ~57 Rust fixtures now parse the keyword form.
- [ ] **Step 6: Commit.**

```bash
git add scripts/migrate-decorator-keywords.vox examples/golden contracts/eval docs crates
git commit -m "refactor: codemod Tier-1 decorators to soft keywords (vox + rust + multiline)"
```

## Task 8: Source-token + byte budget gate

**Files:** Create `run_body_helpers/source_tokens.rs`; modify `cmd_enums.rs:512`, `run_body.rs:356`, `run_body_helpers/mod.rs`, `pipeline_parity.rs:43`; create `contracts/eval/source-token-budget.v1.json`.

- [ ] **Step 1: Write the gate** mirroring `syntax_k.rs`, with **two measures** per ladder fixture: `tokens = lex(src).len()` (import `vox_compiler::lexer::lex` for consistency) and `bytes = src.len()`. Budget JSON shape `{ fixtures: { id: { tokens, bytes } } }`. Ratchet-down on both; `--update` rebaselines; include the `golden_dir.is_dir()` guard from `syntax_k.rs:32`.
- [ ] **Step 2: Subcommand + dispatch** mirroring `KComplexityBudget` (`cmd_enums.rs:512`, `run_body.rs:356`) → `CiCmd::SourceTokenBudget { tolerance_percent, update }`.
- [ ] **Step 3: Baseline AFTER the codemod.** Run: `vox ci source-token-budget --update` — writes the shrunk counts.
- [ ] **Step 4: Wire into parity** next to the k-complexity call (`pipeline_parity.rs:44`).
- [ ] **Step 5: Run.** Run: `cargo test -p vox-cli && vox ci source-token-budget` — Expected: PASS.
- [ ] **Step 6: Commit.**

```bash
git add crates/vox-cli/src/commands/ci contracts/eval/source-token-budget.v1.json
git commit -m "feat(ci): source-token + byte budget gate (ladder-scoped)"
```

## Task 9: Shrink-witness + AI-first probe

**Files:** Create `crates/vox-cli/tests/source_token_shrink_test.rs`.

- [ ] **Step 1: Auto-baselined shrink test.** For ladder fixtures that actually contain Tier-1 decorators — **`crud_api`, `db_native_ir`, `mcp_tools`, `web_routing_fullstack`** (NOT `db_operations` (not in ladder) or `dashboard_ui` (no demoted constructs)) — measure PRE from git instead of hardcoding:

```rust
#[test] fn migrated_fixtures_shrank() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for id in ["crud_api","db_native_ir","mcp_tools","web_routing_fullstack"] {
        let now = vox_compiler::lexer::lex(
            &std::fs::read_to_string(root.join(format!("examples/golden/{id}.vox"))).unwrap()).len();
        // PRE = the same source with keyword→@decorator inverse substitution, re-lexed.
        // (self-maintaining; no git/magic constants)
        let pre = inflate_to_decorator_form_and_lex(id, &root);
        assert!(now < pre, "{id}: expected shrink, {now} !< {pre}");
    }
}
```

- [ ] **Step 2: AI-first reserved-word probe (min viable acceptance criterion).** Add a test asserting common-word identifiers still parse (this is the gate that catches a reserved-word regression):

```rust
#[test] fn common_word_identifiers_do_not_regress() {
    for src in ["fn f(query: str) to int { return 0 }",
                "type Search { query: str }",
                "@query fn g() to int { return len(db.query()) }"] {
        vox_compiler::parser::parse(vox_compiler::lexer::lex(src)).expect(src);
    }
}
```

- [ ] **Step 3: Run + commit.**

```bash
cargo test -p vox-cli source_token_shrink
git add crates/vox-cli/tests/source_token_shrink_test.rs
git commit -m "test: auto-baselined shrink witness + reserved-word regression probe"
```

## Task 10: Full verification (expanded gate set)

- [ ] `cargo test -p vox-compiler -p vox-cli -p vox-orchestrator-mcp -p vox-grammar-export -p vox-lsp -p vox-codegen` — Expected: PASS (audit-found asserting surfaces).
- [ ] `vox ci pipeline-parity` (k-complexity + source-token) and `vox ci grammar-ssot-parity` — Expected: PASS.
- [ ] `cargo clippy -p vox-compiler -- -D warnings` (repeat per touched crate). Do **not** `cargo fmt --all` (Windows os error 206) — use `cargo fmt -p <crate>` / `vox run scripts/fmt.vox`.
- [ ] Confirm one commit per task: `git log --oneline -12`.

---

## Out of scope (separate plans)

**Tier-2** (`@webhook @subagent @form @search` — real AST work, `@search` possibly mis-classified): own spec+plan. P1 reactive streams (`plans/2026-06-20-…-subproject-b.md`), P2 form primitives, P3 render control-flow lowering (existing plans). P4/P5/P6 roadmap.
