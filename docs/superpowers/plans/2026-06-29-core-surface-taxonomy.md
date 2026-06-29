# Core-Surface Taxonomy (P0 Foundation) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Demote 11 kind-defining decorators (`@table @index @query @mutation @tool @resource @server @webhook @form @search @subagent`) to keywords that subsume the `fn`/`type` they sat on, hard-error the old `@` spellings, codemod the corpus, and add a source-token budget CI gate — without changing HIR, typeck, or any codegen backend.

**Architecture:** Front-of-pipe rename only. Each new keyword parser produces the *same* AST node (`Decl::Endpoint{kind}`, table `Decl`, `Decl::McpTool`, `Decl::McpResource`) the decorator produced, so everything downstream is untouched and golden *outputs* stay byte-identical. The single enabling mechanic: `parse_fn_decl` consumes `Token::Fn` optionally, so `query user_count()` parses the same body `@query fn user_count()` did.

**Tech Stack:** Rust (`vox-compiler` lexer/parser, `vox-cli` CI), VoxScript (codemod), JSON budget files.

**Spec:** `docs/superpowers/specs/2026-06-29-core-surface-taxonomy-design.md`

---

## File Structure

| File | Responsibility | Change |
|---|---|---|
| `crates/vox-compiler/tests/keyword_decorator_equivalence.rs` | Proves new keyword ≡ old decorator AST | Create |
| `crates/vox-compiler/src/lexer/token.rs` | `Token` variants for 11 keywords | Modify |
| `crates/vox-compiler/src/lexer/cursor.rs` | str→keyword-token mapping | Modify |
| `crates/vox-compiler/src/language_surface.rs` | SSOT keyword/decorator/deprecated lists | Modify |
| `crates/vox-compiler/src/parser/descent/decl/head_fn.rs` | optional-`fn` consumption | Modify |
| `crates/vox-compiler/src/parser/descent/decl/head.rs` | endpoint/tool/resource keyword heads | Modify |
| `crates/vox-compiler/src/parser/descent/decl/mid.rs` | `table`/`index` keyword heads | Modify |
| `crates/vox-compiler/src/parser/descent/mod.rs` | dispatch new keyword tokens; tombstone old `@` | Modify |
| `crates/vox-compiler/src/hir/validate.rs:32-34` | diagnostic strings → new keyword spellings | Modify |
| `scripts/migrate-decorator-keywords.vox` | one-shot corpus rewrite | Create |
| `crates/vox-cli/src/commands/ci/cmd_enums.rs` | `CiCmd::SourceTokenBudget` | Modify |
| `crates/vox-cli/src/commands/ci/run_body.rs` | dispatch the new CI subcommand | Modify |
| `crates/vox-cli/src/commands/ci/run_body_helpers/source_tokens.rs` | the gate logic | Create |
| `crates/vox-cli/src/commands/ci/run_body_helpers/mod.rs` | re-export | Modify |
| `crates/vox-cli/src/commands/ci/pipeline_parity.rs` | wire gate into parity run | Modify |
| `contracts/eval/source-token-budget.v1.json` | per-fixture token budget | Create |

---

## Pre-flight

- [ ] **Baseline green.** Run: `cargo test -p vox-compiler` — Expected: PASS.
- [ ] **Gate harness works.** Run: `cargo test -p vox-cli k_complexity_budget` — Expected: PASS (this is the template we mirror in Task 7).
- [ ] Skip the stale-binary freshness warning; it does not block (`VOX_SKIP_FRESHNESS_CHECK=1` already set in this repo).

---

## Task 1: Parser-equivalence harness (the safety property)

**Files:**
- Create: `crates/vox-compiler/tests/keyword_decorator_equivalence.rs`

- [ ] **Step 1: Write the failing test**

```rust
// Proves: a demoted keyword parses to the SAME AST as its old decorator form.
// `{:?}` of the Decl, with spans normalized to 0, is our structural equality.
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;

fn ast_debug_no_spans(src: &str) -> String {
    let module = parse(lex(src)).expect("source parses");
    // Span fields render as `span: Span { .. }`; strip every Span {...} payload
    // so only the structural shape remains.
    let raw = format!("{module:#?}");
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.char_indices().peekable();
    while let Some((i, _)) = chars.peek().copied() {
        if raw[i..].starts_with("Span {") {
            // skip to the matching close brace
            let mut depth = 0;
            for (j, c) in raw[i..].char_indices() {
                if c == '{' { depth += 1; }
                if c == '}' { depth -= 1; if depth == 0 { 
                    for _ in 0..=j { chars.next(); }
                    break; 
                }}
            }
            out.push_str("Span{}");
        } else {
            out.push(chars.next().unwrap().1);
        }
    }
    out
}

fn assert_equivalent(old_src: &str, new_src: &str) {
    assert_eq!(
        ast_debug_no_spans(old_src),
        ast_debug_no_spans(new_src),
        "keyword form must parse identically to decorator form"
    );
}

#[test]
fn query_equivalence() {
    assert_equivalent(
        "@query fn user_count() to int { return 0 }",
        "query user_count() to int { return 0 }",
    );
}
```

- [ ] **Step 2: Run the test, verify it fails**

Run: `cargo test -p vox-compiler --test keyword_decorator_equivalence query_equivalence`
Expected: FAIL — `new_src` does not parse (`query` lexes as a bare identifier today).

- [ ] **Step 3: Commit the failing harness**

```bash
git add crates/vox-compiler/tests/keyword_decorator_equivalence.rs
git commit -m "test: keyword/decorator AST equivalence harness (failing)"
```

---

## Task 2: `parse_fn_decl` consumes `fn` optionally

This is the single enabling mechanic. Today `parse_fn_decl` mandates `Token::Fn`. After this, the `fn` token is optional, so a keyword head can call it directly.

**Files:**
- Modify: `crates/vox-compiler/src/parser/descent/decl/head_fn.rs:10`

- [ ] **Step 1: Write the failing test** (append to the equivalence test file)

```rust
#[test]
fn headless_fn_parses() {
    // `parse_fn_decl` reached via a keyword head must accept a missing `fn`.
    // We assert via the public surface: a `query` with no `fn` parses.
    let m = parse(lex("query f() to int { return 1 }")).expect("headless query parses");
    assert_eq!(format!("{m:#?}").contains("Query"), true);
}
```

- [ ] **Step 2: Run, verify it fails**

Run: `cargo test -p vox-compiler --test keyword_decorator_equivalence headless_fn_parses`
Expected: FAIL — `query` not yet a keyword (covered by Tasks 3–4; this test goes green at Task 4).

- [ ] **Step 3: Make `fn` optional in `parse_fn_decl`**

Find where `parse_fn_decl` consumes the `fn` token (an `expect(&Token::Fn)` or equivalent near the top of `head_fn.rs`). Replace the mandatory consume with an optional eat:

```rust
// BEFORE (mandatory):
//   self.expect(&Token::Fn)?;
// AFTER (optional — keyword heads subsume `fn`; bare `fn name` still works):
self.eat(&Token::Fn); // `fn` is optional: a kind keyword (query/server/…) replaces it
```

- [ ] **Step 4: Run the standalone-fn regression**

Run: `cargo test -p vox-compiler`
Expected: PASS — existing `fn name()` declarations still parse (the eat consumes `fn` when present).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-compiler/src/parser/descent/decl/head_fn.rs crates/vox-compiler/tests/keyword_decorator_equivalence.rs
git commit -m "feat(parser): parse_fn_decl consumes `fn` optionally for kind keywords"
```

---

## Task 3: Lex the 11 keyword tokens

**Files:**
- Modify: `crates/vox-compiler/src/lexer/token.rs`
- Modify: `crates/vox-compiler/src/lexer/cursor.rs`
- Modify: `crates/vox-compiler/src/language_surface.rs`

- [ ] **Step 1: Write the failing lexer test** (in `cursor.rs` test module, near `test_keywords` at line 114)

```rust
#[test]
fn kind_keywords_lex() {
    use crate::lexer::token::Token;
    let toks = lex_tokens("table query mutation server webhook form search subagent tool resource index");
    assert_eq!(toks[0], Token::Table);
    assert_eq!(toks[1], Token::Query);
    assert_eq!(toks[9], Token::Resource);
    assert_eq!(toks[10], Token::Index);
}
```

- [ ] **Step 2: Run, verify it fails**

Run: `cargo test -p vox-compiler kind_keywords_lex`
Expected: FAIL — `Token::Table` etc. do not exist / strings lex as `Ident`.

- [ ] **Step 3: Add the `Token` variants**

In `token.rs`, add to the `Token` enum (place near the other declaration keywords like `Component`):

```rust
    Table,
    Index,
    Query,
    Mutation,
    Server,
    Webhook,
    Form,
    Search,
    Subagent,
    Tool,
    Resource,
```

Add matching `Display` arms in the `impl fmt::Display for Token` block (near line 577):

```rust
            Token::Table => write!(f, "table"),
            Token::Index => write!(f, "index"),
            Token::Query => write!(f, "query"),
            Token::Mutation => write!(f, "mutation"),
            Token::Server => write!(f, "server"),
            Token::Webhook => write!(f, "webhook"),
            Token::Form => write!(f, "form"),
            Token::Search => write!(f, "search"),
            Token::Subagent => write!(f, "subagent"),
            Token::Tool => write!(f, "tool"),
            Token::Resource => write!(f, "resource"),
```

- [ ] **Step 4: Map the strings to tokens**

In `cursor.rs`, find the identifier→keyword resolution (the place that maps `"component"` → `Token::Component`). Add the 11 spellings to that mapping, following the exact mechanism already used (match arm or lookup table):

```rust
            "table" => Token::Table,
            "index" => Token::Index,
            "query" => Token::Query,
            "mutation" => Token::Mutation,
            "server" => Token::Server,
            "webhook" => Token::Webhook,
            "form" => Token::Form,
            "search" => Token::Search,
            "subagent" => Token::Subagent,
            "tool" => Token::Tool,
            "resource" => Token::Resource,
```

- [ ] **Step 5: Register in the SSOT**

In `language_surface.rs`, add the 11 spellings to `LEXER_KEYWORDS` (the array starting line 135) and add snippets to `LSP_KEYWORD_SNIPPETS` (line 10), e.g.:

```rust
    ("table", "table $1 { \n\t$0 \n}"),
    ("query", "query $1($2) to $3 { \n\t$0 \n}"),
    // …one per keyword
```

- [ ] **Step 6: Run, verify it passes**

Run: `cargo test -p vox-compiler kind_keywords_lex`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-compiler/src/lexer/token.rs crates/vox-compiler/src/lexer/cursor.rs crates/vox-compiler/src/language_surface.rs
git commit -m "feat(lexer): add 11 kind keyword tokens"
```

---

## Task 4: Parser heads + dispatch (endpoint kinds)

`parse_query`/`parse_mutation`/`parse_server_endpoint` already exist (`head.rs:249-279`) and eat `@query` etc. We make them eat the *keyword* token instead and add dispatch. The body (`parse_fn_decl` + `EndpointKind`) is unchanged.

**Files:**
- Modify: `crates/vox-compiler/src/parser/descent/decl/head.rs:249-279` (and add `parse_webhook`/`parse_form`/`parse_search`/`parse_subagent`)
- Modify: `crates/vox-compiler/src/parser/descent/mod.rs:632`

- [ ] **Step 1: Point existing heads at the keyword token**

The `self.advance()` in each head eats whatever token triggered dispatch. Update the comments and ensure dispatch passes the keyword token. In `head.rs`, the bodies stay structurally identical — only the leading comment changes:

```rust
    pub(crate) fn parse_query(&mut self) -> Result<Decl, ()> {
        self.advance(); // eat `query` keyword (was `@query`)
        self.skip_newlines();
        let f = self.parse_fn_decl(false)?;
        Ok(Decl::Endpoint(EndpointDecl { kind: EndpointKind::Query, func: f }))
    }
```

Add the four missing endpoint heads in the same shape (they map to existing `EndpointKind` variants — confirm the variant names in `head_types.rs`; `Webhook`/`Form`/`Search`/`Subagent` may need adding to `EndpointKind` if absent, in which case mirror `Query` through HIR `HirEndpointKind` and `ContractEndpointKind` per `contract_ir/mod.rs:181`):

```rust
    pub(crate) fn parse_webhook(&mut self) -> Result<Decl, ()> {
        self.advance(); // eat `webhook`
        self.skip_newlines();
        let f = self.parse_fn_decl(false)?;
        Ok(Decl::Endpoint(EndpointDecl { kind: EndpointKind::Webhook, func: f }))
    }
    // parse_form / parse_search / parse_subagent identical, with their kind.
```

> NOTE: `@webhook @form @search @subagent` currently parse through a different
> path than `EndpointDecl` (verify each in `descent/mod.rs`). If a construct does
> NOT lower to `EndpointDecl` today, its keyword head MUST reproduce that
> construct's existing `Decl` node, not force it into `EndpointDecl`. The
> equivalence test in Step 4 is what proves you matched it.

- [ ] **Step 2: Add dispatch for the keyword tokens**

In `descent/mod.rs`, alongside `Token::AtQuery => self.parse_query()` (line 632), add:

```rust
            Token::Query => self.parse_query(),
            Token::Mutation => self.parse_mutation(),
            Token::Server => self.parse_server_endpoint(),
            Token::Webhook => self.parse_webhook(),
            Token::Form => self.parse_form(),
            Token::Search => self.parse_search(),
            Token::Subagent => self.parse_subagent(),
```

- [ ] **Step 3: Run the equivalence + headless tests**

Run: `cargo test -p vox-compiler --test keyword_decorator_equivalence query_equivalence headless_fn_parses`
Expected: PASS.

- [ ] **Step 4: Add equivalence tests for the other endpoint kinds**

```rust
#[test] fn mutation_equivalence() {
    assert_equivalent(
        "@mutation fn add(b: str) to int { return 0 }",
        "mutation add(b: str) to int { return 0 }",
    );
}
#[test] fn server_equivalence() {
    assert_equivalent(
        "@server fn handler() to int { return 0 }",
        "server handler() to int { return 0 }",
    );
}
// webhook / form / search / subagent — one each, mirroring their old @form.
```

Run: `cargo test -p vox-compiler --test keyword_decorator_equivalence`
Expected: PASS (all).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-compiler/src/parser/descent/decl/head.rs crates/vox-compiler/src/parser/descent/mod.rs crates/vox-compiler/tests/keyword_decorator_equivalence.rs
git commit -m "feat(parser): endpoint kind keywords (query/mutation/server/webhook/form/search/subagent)"
```

---

## Task 5: `table`/`index` + `tool`/`resource` heads

`table` has a richer head (`parse_table` at `mid.rs:392`, with `(pk: …)` args). `tool`/`resource` carry an optional/required string.

**Files:**
- Modify: `crates/vox-compiler/src/parser/descent/decl/mid.rs:392` (`parse_table`; clone for `index`)
- Modify: `crates/vox-compiler/src/parser/descent/decl/head.rs` (`parse_mcp_tool`, `parse_mcp_resource`)
- Modify: `crates/vox-compiler/src/parser/descent/mod.rs:855`

- [ ] **Step 1: Failing equivalence tests**

```rust
#[test] fn table_equivalence() {
    assert_equivalent(
        "@table type User { name: str }",
        "table User { name: str }",
    );
}
#[test] fn tool_name_defaults_to_ident() {
    // `tool search(...)` ≡ `@tool("search") fn search(...)` when name == ident.
    assert_equivalent(
        "@tool(\"search\") fn search(q: str) to str { return q }",
        "tool search(q: str) to str { return q }",
    );
}
#[test] fn resource_keeps_uri() {
    assert_equivalent(
        "@resource(\"vox://x\") fn load() to str { return \"\" }",
        "resource \"vox://x\" load() to str { return \"\" }",
    );
}
```

Run: `cargo test -p vox-compiler --test keyword_decorator_equivalence table_equivalence`
Expected: FAIL.

- [ ] **Step 2: `table`/`index` keyword heads**

`parse_table` (`mid.rs:392`) eats `@table` then parses a `type` decl. For the keyword form `table User { … }`, the keyword replaces `type`. After `self.advance()` (eats `table`), ensure the head does NOT also require a `type` token — make the `type` consumption optional exactly as Task 2 did for `fn`:

```rust
    pub(crate) fn parse_table(&mut self) -> Result<Decl, ()> {
        let start = self.span();
        self.advance(); // eat `table` keyword (was `@table`)
        // …existing (pk: …) arg parsing unchanged…
        self.eat(&Token::Type); // `type` is optional: `table` subsumes it
        // …existing type-body parsing unchanged…
    }
```

Clone as `parse_index` for `Token::Index` (mirror whatever `@index` does today).

- [ ] **Step 3: `tool` head defaults name to ident; `resource` keeps URI**

In `parse_mcp_tool` (`head.rs:40`), the current code reads an optional leading `StringLit` as the description/name, then `parse_fn_decl`. The keyword form already supports "optional string then fn" — so eating `tool` instead of `@tool` and relying on `parse_fn_decl`'s now-optional `fn` is sufficient. When no string is present, default the name to the fn identifier (read `f.name` after parsing):

```rust
        self.advance(); // eat `tool` (was `@tool`)
        let explicit = if let Token::StringLit(s) = self.peek().clone() {
            self.advance(); Some(s)
        } else { None };
        self.skip_newlines();
        let f = self.parse_fn_decl(false)?;
        let name = explicit.unwrap_or_else(|| f.name.clone()); // default to ident
        Ok(Decl::McpTool(McpToolDecl { description: name, func: f }))
```

For `parse_mcp_resource`, eat `resource`, require the URI string (existing logic), then `parse_fn_decl`.

- [ ] **Step 4: Dispatch**

In `descent/mod.rs` near line 855 (`Token::AtTable => self.parse_table()`):

```rust
            Token::Table => self.parse_table(),
            Token::Index => self.parse_index(),
            Token::Tool => self.parse_mcp_tool(false),
            Token::Resource => self.parse_mcp_resource(),
```

- [ ] **Step 5: Run, verify pass**

Run: `cargo test -p vox-compiler --test keyword_decorator_equivalence`
Expected: PASS (all 11 constructs).

- [ ] **Step 6: Commit**

```bash
git add crates/vox-compiler/src/parser/descent/decl/mid.rs crates/vox-compiler/src/parser/descent/decl/head.rs crates/vox-compiler/src/parser/descent/mod.rs crates/vox-compiler/tests/keyword_decorator_equivalence.rs
git commit -m "feat(parser): table/index + tool/resource kind keywords"
```

---

## Task 6: Tombstone the old `@` spellings

**Files:**
- Modify: `crates/vox-compiler/src/parser/descent/mod.rs` (the `Token::AtTable`/`AtQuery`/… arms)
- Modify: `crates/vox-compiler/src/language_surface.rs` (`LEXER_DEPRECATED_DECORATORS`)
- Modify: `crates/vox-compiler/src/hir/validate.rs:32-34` (diagnostic spellings)

- [ ] **Step 1: Failing test**

```rust
#[test]
fn old_table_decorator_is_tombstoned() {
    let res = parse(lex("@table type User { name: str }"));
    let err = res.expect_err("@table must now error");
    assert!(format!("{err:?}").contains("table"), "diagnostic names the replacement");
}
```

Run: `cargo test -p vox-compiler --test keyword_decorator_equivalence old_table_decorator_is_tombstoned`
Expected: FAIL — `@table` still parses successfully.

- [ ] **Step 2: Replace the `@`-dispatch arms with tombstone errors**

In `descent/mod.rs`, change each demoted decorator arm from `self.parse_*()` to a tombstone push. Follow the existing `Tombstoned` pattern (used by `@mcp.tool` at `head.rs:44`):

```rust
            Token::AtTable => {
                let span = self.span();
                self.errors.push(ParseError::classified(
                    span,
                    "`@table` is retired; use the `table` keyword",
                    vec!["table".into()],
                    Some("@table".into()),
                    ParseErrorClass::Tombstoned,
                ));
                Err(())
            }
            // repeat for AtQuery→`query`, AtMutation→`mutation`, AtServer→`server`,
            // AtWebhook→`webhook`, AtForm→`form`, AtSearch→`search`,
            // AtSubagent→`subagent`, AtTool→`tool`, AtResource→`resource`, AtIndex→`index`.
```

- [ ] **Step 3: Move spellings into the deprecated SSOT list**

In `language_surface.rs`, add all 11 `@`-spellings to `LEXER_DEPRECATED_DECORATORS` (line 303) and remove them from `LSP_DECORATOR_DOCS`/`LSP_DECORATOR_SNIPPETS`. **Do not** remove their `DecoratorFeature`/`Token::At*` variants or their `LEXER_AT_DECORATORS` entries — they stay as zombies so `decorator_feature_lexer_parity_mismatch()` stays `None` and `ALL.len() == 56` holds.

- [ ] **Step 4: Update HIR diagnostic strings**

In `hir/validate.rs:32-34`, change the user-facing spellings:

```rust
            crate::hir::HirEndpointKind::Server => "server fn",   // already keyword-ish
            crate::hir::HirEndpointKind::Query => "query",        // was "@query fn"
            crate::hir::HirEndpointKind::Mutation => "mutation",  // was "@mutation fn"
```

- [ ] **Step 5: Confirm the 5 "kill" decorators are dead**

`@component @mcp.tool @mcp.resource @v0` are already in `LEXER_DEPRECATED_DECORATORS`. `@place` is **not** — add it. Verify each errors:

```rust
#[test]
fn killed_decorators_error() {
    for src in ["@place fn f() {}", "@v0 fn C() to Element {}"] {
        assert!(parse(lex(src)).is_err(), "{src} must error");
    }
}
```

Ensure `@place` is appended to `LEXER_DEPRECATED_DECORATORS` in `language_surface.rs` and dispatched to a `Tombstoned` error in `descent/mod.rs` (mirror Step 2). The other four already have this path — just confirm with the test.

- [ ] **Step 6: Run parity + tombstone tests**

Run: `cargo test -p vox-compiler && cargo test -p vox-compiler decorator_feature_lexer_parity`
Expected: PASS — tombstone fires; `ALL.len()==56`; parity `None`.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-compiler/src/parser/descent/mod.rs crates/vox-compiler/src/language_surface.rs crates/vox-compiler/src/hir/validate.rs crates/vox-compiler/tests/keyword_decorator_equivalence.rs
git commit -m "feat(parser): hard-error retired decorators (@table/@query/… → keywords)"
```

---

## Task 7: Codemod the corpus

**Files:**
- Create: `scripts/migrate-decorator-keywords.vox`

- [ ] **Step 1: Write the codemod (VoxScript — no .ps1/.sh/.py)**

```vox
// scripts/migrate-decorator-keywords.vox
// Rewrites retired decorator spellings to keyword forms across .vox sources.
// Idempotent. Pass --apply to write; default is dry-run (prints diffs).
//   @table type X   -> table X
//   @index type X   -> index X
//   @query fn f      -> query f      (and mutation/server/webhook/form/search/subagent)
//   @tool("n") fn n  -> tool n       (drops the string when it equals the fn name)
//   @tool("n") fn f  -> tool "n" f   (keeps the string when it differs)
//   @resource("u") fn f -> resource "u" f
```

Implement with the standard library file walk + regex replace used by other
`scripts/*.vox` (mirror `scripts/sync-cursor-skills.vox` structure). Targets:
`examples/golden/**.vox`, `contracts/eval/**.vox`, and doc anchors under `docs/`.

- [ ] **Step 2: Dry-run and review**

Run: `vox run scripts/migrate-decorator-keywords.vox`
Expected: prints the set of rewrites; no files changed.

- [ ] **Step 3: Apply**

Run: `vox run scripts/migrate-decorator-keywords.vox --apply`
Expected: every `@table/@query/…` occurrence rewritten.

- [ ] **Step 4: Prove outputs unchanged (the invariant in action)**

Run: `cargo test -p vox-codegen golden && cargo test -p vox-cli golden`
Expected: PASS — emitted TS/rust/interp artifacts are byte-identical; only `.vox` *sources* changed.

- [ ] **Step 5: Commit**

```bash
git add scripts/migrate-decorator-keywords.vox examples/golden contracts/eval docs
git commit -m "refactor: codemod retired decorators to keyword forms across corpus"
```

---

## Task 8: Source-token budget gate

**Files:**
- Create: `crates/vox-cli/src/commands/ci/run_body_helpers/source_tokens.rs`
- Modify: `crates/vox-cli/src/commands/ci/run_body_helpers/mod.rs`
- Modify: `crates/vox-cli/src/commands/ci/cmd_enums.rs:512`
- Modify: `crates/vox-cli/src/commands/ci/run_body.rs:356`
- Modify: `crates/vox-cli/src/commands/ci/pipeline_parity.rs:43`
- Create: `contracts/eval/source-token-budget.v1.json`

- [ ] **Step 1: Write the gate (mirror `syntax_k.rs`)**

```rust
// source_tokens.rs — measure = lex(source).len() per golden ladder fixture.
use anyhow::{Result, anyhow};
use std::{collections::HashMap, fs, path::Path};
use vox_compiler::lexer::cursor::lex;

#[derive(serde::Deserialize, serde::Serialize, Default)]
struct TokenBudget { #[serde(default)] fixtures: HashMap<String, usize> }

pub(crate) fn run_source_token_budget(root: &Path, tolerance: f64, update: bool) -> Result<()> {
    let budget_path = root.join("contracts/eval/source-token-budget.v1.json");
    let mut budget: TokenBudget = if budget_path.exists() {
        serde_json::from_str(&fs::read_to_string(&budget_path)?)?
    } else { TokenBudget::default() };

    let ladder = vox_codegen::canonical_ladder::CanonicalLadder::load_from_repo_root(root)
        .map_err(|e| anyhow!("load ladder: {e}"))?;
    let ladder_ids = ladder.fixture_ids();

    let mut failures = Vec::new();
    let mut measured = HashMap::new();
    for entry in fs::read_dir(root.join("examples/golden"))? {
        let path = entry?.path();
        if path.extension().and_then(|s| s.to_str()) != Some("vox") { continue; }
        let id = path.file_stem().unwrap().to_str().unwrap().to_string();
        if !ladder_ids.contains(&id) { continue; }
        let n = lex(&fs::read_to_string(&path)?).len();
        measured.insert(id.clone(), n);
        if let Some(&allowed) = budget.fixtures.get(&id) {
            let limit = (allowed as f64 * (1.0 + tolerance / 100.0)).ceil() as usize;
            if n > limit { failures.push(format!("{id}: {n} > {limit} (budget {allowed})")); }
        } else if !update {
            failures.push(format!("{id}: no source-token budget defined"));
        }
    }
    if update {
        budget.fixtures = measured;
        fs::write(&budget_path, serde_json::to_string_pretty(&budget)?)?;
        println!("Updated source-token budget: {}", budget_path.display());
    }
    if !failures.is_empty() { anyhow::bail!("source-token budget failed: {}", failures.join("; ")); }
    println!("source-token budget OK");
    Ok(())
}
```

- [ ] **Step 2: Re-export + subcommand + dispatch**

In `run_body_helpers/mod.rs`: `pub(crate) use source_tokens::run_source_token_budget;` and `mod source_tokens;`.
In `cmd_enums.rs` (mirror `KComplexityBudget` at line 512):

```rust
    #[command(name = "source-token-budget")]
    SourceTokenBudget { #[arg(long, default_value_t = 0.0)] tolerance_percent: f64, #[arg(long)] update: bool },
```

In `run_body.rs` (mirror line 356):

```rust
        CiCmd::SourceTokenBudget { tolerance_percent, update } =>
            run_source_token_budget(&root, tolerance_percent, update),
```

- [ ] **Step 3: Baseline AFTER the codemod**

Run: `vox ci source-token-budget --update`
Expected: writes `source-token-budget.v1.json` with the *shrunk* counts.

- [ ] **Step 4: Wire into parity (mirror `pipeline_parity.rs:44`)**

```rust
    println!("pipeline-parity: source-token budget (ladder-scoped)…");
    super::run_body::run_body_helpers::run_source_token_budget(root, 0.0, false)?;
```

- [ ] **Step 5: Run the gate**

Run: `cargo test -p vox-cli && vox ci source-token-budget`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-cli/src/commands/ci contracts/eval/source-token-budget.v1.json
git commit -m "feat(ci): source-token budget gate (lex-token count per ladder fixture)"
```

---

## Task 9: Shrink-proof test

**Files:**
- Create: `crates/vox-cli/tests/source_token_shrink_test.rs`

- [ ] **Step 1: Capture pre-migration counts (one-time measurement)**

Before Task 7 runs, you recorded each migrated fixture's `lex().len()`. Encode the deltas as a guard that the post-migration count is strictly lower for at least the fixtures that use the 11 constructs:

```rust
#[test]
fn migrated_fixtures_shrank() {
    // (fixture, pre_migration_token_count) — measured once on the commit before Task 7.
    const PRE: &[(&str, usize)] = &[
        ("crud_api", 0 /* replace with measured value */),
        ("db_operations", 0 /* … */),
    ];
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (id, pre) in PRE {
        let src = std::fs::read_to_string(root.join(format!("examples/golden/{id}.vox"))).unwrap();
        let now = vox_compiler::lexer::cursor::lex(&src).len();
        assert!(now < *pre, "{id}: expected shrink, {now} !< {pre}");
    }
}
```

- [ ] **Step 2: Fill in real `PRE` values**

Measure on the pre-Task-7 commit: `git stash; <count>; git stash pop`, or read from the k-complexity baseline diff. Replace the `0` placeholders with the actual counts.

- [ ] **Step 3: Run, verify pass**

Run: `cargo test -p vox-cli migrated_fixtures_shrank`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-cli/tests/source_token_shrink_test.rs
git commit -m "test: prove migrated golden fixtures shrank in source tokens"
```

---

## Task 10: Full verification

- [ ] `cargo test -p vox-compiler -p vox-lsp -p vox-cli -p vox-codegen` — Expected: PASS.
- [ ] `vox ci pipeline-parity` — Expected: PASS (k-complexity **and** source-token gates).
- [ ] `cargo clippy -p vox-compiler -- -D warnings` (repeat per touched crate). Do **not** `cargo fmt --all` (Windows arg-limit, os error 206) — use `cargo fmt -p vox-compiler` / `vox run scripts/fmt.vox`.
- [ ] `git log --oneline -12` — confirm one commit per task.

---

## Out of scope (separate plans)

P1 reactive streams (`plans/2026-06-20-vox-native-frontend-ssot-subproject-b.md`),
P2 form primitives (`plans/2026-06-29-vox-form-primitives.md`),
P3 render control-flow lowering (`plans/2026-06-29-render-control-flow-lowering.md`),
P4 interop hardening / P5 mobile-PWA / P6 fallback gate (roadmap; need own brainstorm).
