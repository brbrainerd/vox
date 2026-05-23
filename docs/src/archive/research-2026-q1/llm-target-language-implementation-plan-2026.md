---
title: "LLM Target Language: Explicit Implementation Plan (April 2026)"
description: "Fully explicit, file-level remediation plan for making Vox the premier LLM target language. Incorporates a full codebase audit pass, corrects erroneous prior analysis, surfaces real bugs, and marks all deprecated language surfaces with machine-readable deprecation notices."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-04-17"
training_eligible: false
training_rationale: "Definitive implementation roadmap for LLM-native language quality. All deprecated surfaces tagged for AI tool consumption."
supersedes: "llm-target-language-gap-analysis-2026.md"

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# LLM Target Language: Explicit Implementation Plan (April 2026)

This document supersedes the original gap analysis for the purposes of **implementation guidance**. It was produced after a second full audit pass that read source files directly, checked git commit timestamps, and verified implementation state. False positives from the initial analysis are corrected here. Real bugs found during the second pass are documented with exact file references.

> **Machine-readable deprecation convention:** All deprecated or retired language surfaces are tagged with `> [!DEPRECATED]` callouts containing: the replacement, the enforcement mechanism, and the training data action.

---

## Audit Provenance

| File | Last meaningful commit | Notes |
|---|---|---|
| `crates/vox-compiler/src/lexer/token.rs` | 2026-04-12 | Most recent; authoritative for lexer state |
| `crates/vox-grammar-export/src/ebnf.rs` | 2026-04-12 | Authoritative grammar |
| `crates/vox-grammar-export/src/gbnf.rs` | 2026-04-12 | ~30 lines; critically incomplete |
| `crates/vox-compiler/src/ast/decl/fundecl.rs` | 2026-04-07 | All B-series gaps were false positives |
| `crates/vox-compiler/src/hir/lower/contracts.rs` | 2026-04-05 | Real bug found (return-site injection) |
| `crates/vox-compiler/src/typeck/ast_decl_lints.rs` | 2026-04-12 | Authoritative for retirement enforcement |
| `crates/vox-compiler/src/language_surface.rs` | 2026-04-12 | LSP/MCP SSOT; has `ret` in LEXER_KEYWORDS |
| `tree-sitter-vox/GRAMMAR_SSOT.md` | 2026-03-23 | **Stale** — 3+ weeks behind token.rs |
| `docs/src/architecture/vox-lang-training-ssot-2026.md` | 2026-04-17 | Training policy SSOT; authoritative on `ret` |

archived_date: 2026-04-18
---

## Deprecated / Retired Language Surfaces

> **For AI tools:** These sections are tagged as deprecated. Do not generate code using these surfaces. Do not include them in training corpora without a deprecation label.

### DEP-1: `ret` keyword

> [!DEPRECATED]
> **Status:** Deprecated — keyword retained in lexer for error recovery only.
> **Replacement:** `return`
> **Training SSOT authority:** `docs/src/architecture/vox-lang-training-ssot-2026.md` §3 item 2: "`ret` keyword is deprecated. `return` is the sole canonical keyword."
> **Current enforcement:** None — parser accepts `ret` without warning.
> **Required enforcement:** Parser emits `Warn` diagnostic; `vox fmt` autofixes `ret` → `return`.
> **Token.rs state:** Both `Ret` and `Return` tokens present. `Ret` must be kept for error recovery but should never appear in generated code or training corpora.
> **EBNF state:** `return_stmt = ( "ret" | "return" ), [ expr ]` — `"ret"` alternative must receive a `// DEPRECATED` comment.
> **language_surface.rs state:** `LEXER_KEYWORDS` array has both `"ret"` (line 111) and `"return"` (line 116). `LSP_KEYWORD_SNIPPETS` has only `"return"` (correct — LSP already points toward deprecation).
> **Parser tests:** All tests in `crates/vox-compiler/src/parser/descent/tests.rs` use `ret`. Must be migrated to `return`.
> **Training data action:** Filter `ret` from all JSONL training pairs or label with deprecation tag. Do not generate new training pairs using `ret`.

### DEP-2: `@component fn` syntax (Classic Component)

> [!DEPRECATED]
> **Status:** Retired — produces a hard compiler error, not a warning.
> **Replacement:** `component Name(props) { state ...; view: ... }`  (Path C reactive syntax)
> **Retirement enforcement:** `crates/vox-compiler/src/typeck/ast_decl_lints.rs` emits `DiagnosticCategory::Lint` / `TypeckSeverity::Error` code `lint.legacy_component_fn` on any `Decl::Component`.
> **Token state:** `AtComponent` token retained in `token.rs` for error recovery parsing. Should never appear in valid programs.
> **EBNF state:** `component = "@component", "fn", ident, ...` production still present. Must be annotated `// RETIRED — produces compile error` or removed.
> **AGENTS.md:** Listed in retired-symbols table: `@component fn Name()` → `component Name() {}`.
> **compact_prompt.rs:** Marked as `// legacy`.
> **language_surface.rs:** `@component` remains in `LEXER_DECORATORS` but is **not** in `LSP_DECORATOR_DOCS` (correct — LSP already omits it).
> **Training data action:** Remove all `@component fn` examples from training corpora. Replace with `component Name() { ... }` equivalents.

### DEP-3: `context` declarations

> [!DEPRECATED]
> **Status:** Retired — produces a hard compiler error.
> **Enforcement:** `ast_decl_lints.rs` emits error `lint.retired_context_decl`: "`context` declarations are retired. Define React Context in user-owned `app/App.tsx`."
> **Replacement:** User-owned `app/App.tsx` React Context definitions.
> **Training data action:** Remove all `context` declaration examples from training corpora.

### DEP-4: `@hook fn` declarations

> [!DEPRECATED]
> **Status:** Retired — produces a hard compiler error.
> **Enforcement:** `ast_decl_lints.rs` emits error `lint.retired_hook_fn`: "`@hook fn` is retired. Prefer Path C `component`, islands, or plain TS under `islands/`."
> **AGENTS.md:** `vox-ars` / `@hook fn` → retired.
> **Training data action:** Remove all `@hook fn` examples.

### DEP-5: `@provider fn` declarations

> [!DEPRECATED]
> **Status:** Retired — produces a hard compiler error.
> **Enforcement:** `ast_decl_lints.rs` emits error `lint.retired_provider_fn`: "`@provider fn` is retired. Add providers in user-owned `app/App.tsx`."
> **Training data action:** Remove all `@provider fn` examples.

### DEP-6: `page:` / static `Page` declarations

> [!DEPRECATED]
> **Status:** Retired — produces a hard compiler error.
> **Enforcement:** `ast_decl_lints.rs` emits error `lint.retired_page_decl`: "`page:` / static Page declarations are retired for the web stack. Use `routes { ... }` and Path C components."
> **Replacement:** `routes { GET "/" handler }` + Path C `component` definitions.
> **Training data action:** Remove all `page:` examples; replace with `routes { ... }` equivalents.

### DEP-7: Colon-based block syntax

> [!DEPRECATED]
> **Status:** Deprecated — v0.2 legacy.
> **Training SSOT authority:** `vox-lang-training-ssot-2026.md` §3 item 1: "Colon-based block syntax (`:`) is deprecated (v0.2 legacy). All blocks must use brace syntax `{}` (v0.4+ standard)."
> **Training data action:** Filter or reject any training pairs using colon block syntax.

---

## Bug Reports (Confirmed via Code Audit)

### BUG-1: `contracts.rs` — Postcondition injection at body end, not return sites

**File:** `crates/vox-compiler/src/hir/lower/contracts.rs`
**Function:** `LowerCtx::inject_contracts`
**Branch:** `VerifyMode::Full`

**Symptom:** The `VerifyMode::Full` branch appends postcondition `HirStmt::Expr` nodes at the tail of the function body:

```rust
// CURRENT (BUGGY):
new_body.append(&mut body);
for post in &f.postconditions {
    new_body.push(HirStmt::Expr { expr: ..., span: ... });
}
```

A function with multiple `return` statements only checks postconditions at implicit fall-through. Any early `return` bypasses all postcondition assertions.

**Fix:** Walk `body: Vec<HirStmt>` recursively. Wherever a `HirStmt::Return { value, span }` is found, replace it with:
1. Postcondition `HirStmt::Expr` nodes (one per `f.postconditions` entry, with the `__result__` binding resolved to the return value).
2. The original `HirStmt::Return`.

```rust
// CORRECT APPROACH:
fn inject_postconditions_before_returns(
    body: Vec<HirStmt>,
    postcondition_stmts: &[HirStmt],
) -> Vec<HirStmt> {
    body.into_iter().flat_map(|stmt| {
        match stmt {
            HirStmt::Return { .. } => {
                let mut injected = postcondition_stmts.to_vec();
                injected.push(stmt);
                injected
            }
            HirStmt::While { condition, body, span } => vec![HirStmt::While {
                condition,
                body: inject_postconditions_before_returns(body, postcondition_stmts),
                span,
            }],
            HirStmt::Loop { body, span } => vec![HirStmt::Loop {
                body: inject_postconditions_before_returns(body, postcondition_stmts),
                span,
            }],
            other => vec![other],
        }
    }).collect()
}
```

**Owner:** `vox-compiler` | **Severity: Critical** | **Effort: Small**

archived_date: 2026-04-18
---

### BUG-2: `versioning.rs` — `verify_grammar_alignment` is a no-op

**File:** `crates/vox-grammar-export/src/versioning.rs`
**Function:** `verify_grammar_alignment()`

**Symptom:** Both `get_version()` and `get_compiler_version()` read the identical `env!("CARGO_PKG_VERSION")` macro. The comparison `get_version() != get_compiler_version()` is always `false`. The function always returns `Ok(())` regardless of actual grammar/compiler divergence.

```rust
// CURRENT (BUGGY):
pub fn get_version() -> semver::Version {
    semver::Version::parse(env!("CARGO_PKG_VERSION"))...
}
pub fn get_compiler_version() -> semver::Version {
    semver::Version::parse(env!("CARGO_PKG_VERSION"))... // same macro!
}
```

**Fix:** Replace with a grammar-specific content hash. The EBNF source is the authoritative grammar document. At build time, compute a hash of the EBNF production rules and embed it as a compile-time constant:

```rust
// crates/vox-grammar-export/src/versioning.rs
pub const GRAMMAR_CONTENT_HASH: &str = env!("VOX_GRAMMAR_HASH"); // set in build.rs

pub fn verify_grammar_alignment() -> Result<(), String> {
    let live_hash = compute_ebnf_hash(); // hash the emit_ebnf() output
    if live_hash != GRAMMAR_CONTENT_HASH {
        Err(format!(
            "Grammar hash mismatch: built={}, live={}. Run `vox grammar export` to refresh.",
            GRAMMAR_CONTENT_HASH, live_hash
        ))
    } else {
        Ok(())
    }
}
```

Alternatively (simpler): use the EBNF rule count as a cheap proxy until a proper hash pipeline exists.

**Owner:** `vox-grammar-export` | **Severity: High** | **Effort: Small**

---

### BUG-3: `automaton.rs` — JSON char acceptance is too permissive

**File:** `crates/vox-grammar-export/src/automaton.rs`
**Function:** `JsonGrammarAutomaton::transition_char`

**Symptom:** The `transition_char` method accepts individual letters `t`, `r`, `u`, `f`, `a`, `l`, `s`, `n`, `o` anywhere outside a string, attempting to proxy `true`/`false`/`null` recognition. This means strings like `"rubbish"` (outside quotes) would partially pass. The automaton has no memory of which keyword it's building, so `ttttt` outside a string returns `true` on every char.

**Fix:** The immediate fix is to use this automaton only as a brace-depth tracker (its most correct function) and either:
1. Remove the single-char letter acceptance and rely on proper JSON schema validation at the output stage, or
2. Implement a proper multi-state keyword recognizer (`KW_NONE | KW_T | KW_TR | KW_TRU | KW_TRUE | ...`).

Option 1 is safer given the grammar-constrained decoding stack is migrating to XGrammar-2 (see [research-grammar-constrained-decoding-2026.md](research-grammar-constrained-decoding-2026.md)). Mark this automaton as `JsonBraceTracker` and scope it to brace-balance-only use.

**Owner:** `vox-grammar-export` | **Severity: Medium** | **Effort: Small**

archived_date: 2026-04-18
---

### BUG-4: `match_exhaust.rs` — No exhaustiveness check for `Bool` type

**File:** `crates/vox-compiler/src/typeck/checker/match_exhaust.rs`
**Function:** `check_hir_match_exhaustiveness`

**Symptom:** The function only processes `Ty::Named(name)` scrutinees. For `Ty::Bool`, the function returns early with no coverage check. A match on `bool` that only covers `true` (not `false`) produces no diagnostic.

```rust
// CURRENT:
let type_name = match subject_ty {
    Ty::Named(name) => name.as_str(),
    _ => return, // <-- Bool, Option, Result silently skip exhaustiveness
};
```

**Fix:** Add `Ty::Bool` as a special case before the `Named` arm:

```rust
match subject_ty {
    Ty::Bool => {
        let has_true = arms.iter().any(|a| matches!(&a.pattern,
            HirPattern::Literal(HirLiteral::Bool(true), _) | HirPattern::Ident(n, _) if n == "true"));
        let has_false = arms.iter().any(|a| matches!(&a.pattern,
            HirPattern::Literal(HirLiteral::Bool(false), _) | HirPattern::Ident(n, _) if n == "false"));
        let has_wildcard = arms.iter().any(|a| matches!(&a.pattern, HirPattern::Wildcard(_)));
        if !has_wildcard && (!has_true || !has_false) {
            let missing: Vec<&str> = [(!has_true).then_some("true"), (!has_false).then_some("false")]
                .into_iter().flatten().collect();
            diags.push(Diagnostic::error(
                format!("Non-exhaustive match on bool. Missing: {}", missing.join(", ")),
                span, source,
            ));
        }
        return;
    }
    Ty::Named(name) => name.as_str(),
    _ => return,
}
```

**Owner:** `vox-compiler` typeck | **Severity: Medium** | **Effort: Small**

---

### BUG-5: `gbnf.rs` and `lark.rs` — Grammar exports critically incomplete

**File:** `crates/vox-grammar-export/src/gbnf.rs` — ~30 lines, covers expressions only  
**File:** `crates/vox-grammar-export/src/lark.rs` — ~90 lines, covers fn/let/type only

**Symptom:** Neither grammar export covers: actors, workflows, activities, tables, collections, routes, components, agents, environments, MCP tools/resources, `@require`/`@ensure`/`@invariant`, match expressions, for/while/loop, type declarations with variants, JSX, `spawn`, `with`, pipe operator, template strings, or any decorator. The GBNF is the format used for grammar-constrained LLM decoding — an incomplete GBNF means constrained decoding produces syntactically invalid programs.

Additionally, the GBNF engine in use has **CVE-2026-2069** (llama.cpp GBNF stack overflow on recursive grammars). See [research-grammar-constrained-decoding-2026.md](research-grammar-constrained-decoding-2026.md) for the XGrammar-2 / llguidance migration path.

**Fix Options:**

1. **Complete the GBNF** (high effort, ~500 lines) AND migrate serving stack to XGrammar-2 (eliminates CVE-2026-2069). This is the correct long-term path.

2. **Deprecate GBNF** as a serving format until XGrammar-2 migration is complete. Add `GrammarFormat::Gbnf` → `Err("GBNF export is incomplete and the serving engine has CVE-2026-2069; use XGrammar-2 with the Lark or EBNF export instead")`.

3. **Ship a minimal GBNF stub** that covers the top-20 highest-frequency constructs (functions, tables, queries, mutations, routes, actors) and blocks the rest, with an honest error message.

**Recommended path:** Option 2 immediately (deprecate), then Option 1 as Wave 3 work once XGrammar-2 serving is wired.

**Owner:** `vox-grammar-export`, `vox-runtime` | **Severity: Critical** | **Effort: Large**

archived_date: 2026-04-18
---

### BUG-6: `hir/validate.rs` — `correction_hint` is `None` in 30+ of 31 error sites

**File:** `crates/vox-compiler/src/hir/validate.rs`

**Symptom:** `HirValidationError::correction_hint` is `Some(...)` in exactly 1 place (the scheduled interval check). All other ~30 error sites use `correction_hint: None`. The hint field is the primary mechanism for LLM self-repair — when validation fails, the correction hint is surfaced to the generating model. Empty hints mean the model cannot self-correct.

**Fix:** Populate `correction_hint` for every validation error. Examples:

```rust
// Empty server fn route_path:
correction_hint: Some("@server must declare a route, e.g. @server(\"/api/my-endpoint\")".into()),

// Empty mcp resource URI:
correction_hint: Some("@mcp.resource requires a URI, e.g. @mcp.resource(uri = \"mcp://my-resource\")".into()),

// Duplicate mcp.resource URI:
correction_hint: Some(format!("Use a unique URI for each @mcp.resource; '{}' is already declared above", m.uri)),

// mcp resource has params:
correction_hint: Some("@mcp.resource functions must have no parameters; the URI is supplied by the MCP protocol at call time".into()),
```

**Owner:** `vox-compiler` HIR | **Severity: High** | **Effort: Small**

---

## SSOT Divergence Fixes

### SSOT-1: `GRAMMAR_SSOT.md` is stale (last commit 2026-03-23)

**File:** `tree-sitter-vox/GRAMMAR_SSOT.md`

`GRAMMAR_SSOT.md` is the vocabulary document exposed to tree-sitter, syntax highlighting, and external tooling. It diverged from `token.rs` (last updated 2026-04-12) by at least 3 weeks and the following items are wrong:

**In GRAMMAR_SSOT.md but NOT in `token.rs`** (hallucination risk):
- Keywords: `message`, `bind`, `style` (as standalone keywords — `message` is an ident in Path C; `bind` and `style` don't exist)
- Operators: `?.` (optional chaining — no `OptionalChain` token)
- Decorators: `@action`, `@skill`, `@agent_def`, `@storage` (renamed or removed)

**In `token.rs` but NOT in GRAMMAR_SSOT.md** (missing from highlighting / LLM context):
- Keywords: `loop`, `break`, `continue`, `environment`, `migrate`, `cleanup`, `mount`, `effect`, `view`, `derived`, `state`, `pub`, `with`, `on`, `agent`, `component` (Path C), `spawn`, `workflow`, `activity`, `actor`, `struct`, `enum`, `get`, `post`, `put`, `delete`, `and`, `or`, `not`, `is`, `isnt`, `from`, `use`, `const`

**Fix:**
1. Add CI check `vox ci grammar-ssot-parity` that diffs `GRAMMAR_SSOT.md` keyword table against `LEXER_KEYWORDS` constant in `language_surface.rs`.
2. Add `vox grammar ssot-export` CLI subcommand that regenerates `GRAMMAR_SSOT.md` from `language_surface.rs`.
3. Run `vox grammar ssot-export` now and commit the result.

**Owner:** `vox-cli`, `vox-grammar-export`, docs | **Severity: Critical** | **Effort: Small**

archived_date: 2026-04-18
---

### SSOT-2: `language_surface.rs` has `ret` in `LEXER_KEYWORDS`

**File:** `crates/vox-compiler/src/language_surface.rs` lines 111 and 116

`LEXER_KEYWORDS` contains both `"ret"` and `"return"`. This is the array consumed by MCP introspection tools and the LSP keyword provider. Since `ret` is deprecated, it must be removed from `LEXER_KEYWORDS` once the deprecation warning is in place (see Wave 1 below).

**Immediate fix:** Move `"ret"` to a new `LEXER_DEPRECATED_KEYWORDS: &[&str]` constant with a doc comment marking it as deprecated. This allows introspection tools to still know the keyword exists (for error messages) without offering it as a first-class completion.

**Owner:** `vox-compiler` | **Severity: High** | **Effort: Trivial**

---

### SSOT-3: `language_surface.rs` has `@component` in `LEXER_DECORATORS`

**File:** `crates/vox-compiler/src/language_surface.rs` line 148

`@component` is the first entry in `LEXER_DECORATORS`. Since `@component fn` is now a hard compiler error (per `ast_decl_lints.rs`), it must not appear as a first-class decorator. However, the Path C keyword `component` (without `@`) is the correct replacement and should appear in `LEXER_KEYWORDS`.

**Fix:**
1. Remove `"@component"` from `LEXER_DECORATORS`.
2. Add `"component"` to `LEXER_KEYWORDS` if not already present (it is — line 131 in `language_surface.rs`).
3. Add `@component` to `LEXER_DEPRECATED_DECORATORS: &[&str]` (new constant) with a doc comment.
4. Add a reverse test to `crates/vox-compiler/tests/language_surface_ssot_test.rs`:
   ```rust
   #[test]
   fn retired_decorators_not_in_lsp_list() {
       // @component is retired; must not appear in LSP suggestions
       assert!(!language_surface::LEXER_DECORATORS.contains(&"@component"));
   }
   ```

**Owner:** `vox-compiler` | **Severity: High** | **Effort: Trivial**

archived_date: 2026-04-18
---

### SSOT-4: `TreeSitterGrammar` format is a stub

**File:** `crates/vox-grammar-export/src/lib.rs`

`GrammarFormat::TreeSitterGrammar` is listed as a format option but the emitter has a `// not yet implemented` comment and returns an empty string or placeholder. External tree-sitter consumers (VS Code, Neovim syntax highlighting) relying on this format get no grammar.

**Fix:** Either implement the tree-sitter grammar emitter (large effort) or return `Err("TreeSitterGrammar format is not yet implemented; use the EBNF export and convert manually or request this feature")` to fail loudly rather than silently.

**Owner:** `vox-grammar-export` | **Severity: Medium** | **Effort: Stub-fix is Trivial, Real impl is Large**

---

## Implementation Waves

### Wave 0 — Emergency Fixes (Single PR, no API changes)

These are correctness bugs with zero-to-minimal interface changes. Ship immediately.

| Task | File(s) | Change |
|---|---|---|
| Fix BUG-2 (versioning no-op) | `vox-grammar-export/src/versioning.rs` | Replace both `get_version()` bodies so they use different sources; add grammar hash computed from EBNF output length as interim proxy |
| Fix BUG-3 (automaton too permissive) | `vox-grammar-export/src/automaton.rs` | Rename to `JsonBraceDepthTracker`; strip letter-char acceptance; document it tracks brace depth only |
| Populate BUG-6 correction_hints | `vox-compiler/src/hir/validate.rs` | Add `correction_hint: Some(...)` to all 30 `None` sites (use the error message to derive a hint) |

**Acceptance:** All existing `hir/validate.rs` tests pass. `verify_grammar_alignment()` returns `Err` when EBNF output changes.

archived_date: 2026-04-18
---

### Wave 1 — SSOT Convergence (Deprecation enforcement)

**Goal:** Make all deprecated surfaces visible as warnings, not silent acceptances.

| Task | File(s) | Change |
|---|---|---|
| Add `LEXER_DEPRECATED_KEYWORDS` | `language_surface.rs` | Move `"ret"` from `LEXER_KEYWORDS` to new `LEXER_DEPRECATED_KEYWORDS: &[&str]` |
| Add `LEXER_DEPRECATED_DECORATORS` | `language_surface.rs` | Move `"@component"` from `LEXER_DECORATORS` to new constant |
| Add reverse test | `tests/language_surface_ssot_test.rs` | Assert retired items not in LEXER_DECORATORS/LEXER_KEYWORDS |
| Emit `ret` deprecation warning | `vox-compiler/src/parser/descent/*.rs` | When `Token::Ret` is consumed, push a `Warn` diagnostic: "deprecated keyword `ret`; use `return`" |
| `vox fmt` autofix `ret` → `return` | `vox-compiler/src/fmt/` | Add rewriter pass: `Token::Ret` → `return` |
| Annotate EBNF with DEP comments | `vox-grammar-export/src/ebnf.rs` | Add `// DEPRECATED: use "return"` on the `"ret"` alternative; add `// RETIRED: use component Name() {}` on `@component fn` production |
| Annotate compact_prompt.rs | `vox-grammar-export/src/compact_prompt.rs` | Update `ret`/`return` section to clearly show `ret` as `// deprecated` and `return` as canonical |
| Update parser tests | `vox-compiler/src/parser/descent/tests.rs` | Replace all `ret` with `return` in test fixtures |
| Regenerate GRAMMAR_SSOT.md | `tree-sitter-vox/GRAMMAR_SSOT.md` | Run `vox grammar ssot-export` (or manually sync from `language_surface.rs`) |

**Acceptance:** `rg -l "ret " crates/vox-compiler/src/parser/descent/tests.rs` returns no results. `cargo test -p vox-compiler` passes. SSOT-parity CI check passes.

---

### Wave 2 — Contract System Fix

**Goal:** Make the Design-by-Contract system correct end-to-end.

| Task | File(s) | Change |
|---|---|---|
| Fix BUG-1 (postcondition injection) | `vox-compiler/src/hir/lower/contracts.rs` | Implement `inject_postconditions_before_returns` (see BUG-1 above); replace flat-append approach |
| Fix BUG-4 (bool exhaustiveness) | `vox-compiler/src/typeck/checker/match_exhaust.rs` | Add `Ty::Bool` case before `Ty::Named` arm |
| Add `@llm` token (Gap B-5) | `vox-compiler/src/lexer/token.rs` | `#[token("@llm")] AtLlm` |
| Add `is_llm` fields to FnDecl | `vox-compiler/src/ast/decl/fundecl.rs` | `pub is_llm: bool`, `pub llm_model: Option<String>` |
| Add `@llm` to language_surface | `language_surface.rs` | Add `("@llm", "Declare an LLM-implemented function body.")` to `LSP_DECORATOR_DOCS` and `"@llm"` to `LEXER_DECORATORS` |

**Acceptance:** Test: function with two `return` paths and `@ensure` — postcondition fires on both paths. Test: match on `bool` missing `false` arm produces diagnostic.

archived_date: 2026-04-18
---

### Wave 3 — Grammar Export Hardening

**Goal:** Produce a usable, complete grammar for constrained decoding.

| Task | File(s) | Change |
|---|---|---|
| Deprecate GBNF serving (BUG-5) | `vox-grammar-export/src/lib.rs` | For `GrammarFormat::Gbnf`, return `Err` with CVE-2026-2069 and XGrammar-2 migration note |
| Stub TreeSitterGrammar (SSOT-4) | `vox-grammar-export/src/lib.rs` | Return explicit `Err("not yet implemented")` instead of empty output |
| Complete Lark grammar (BUG-5) | `vox-grammar-export/src/lark.rs` | Add: `actor`, `workflow`, `activity`, `table_decl`, `route`, `server_fn`, `query_fn`, `mutation_fn`, `component`, `mcp_tool`, `mcp_resource`, `match_expr`, `for_stmt`, `while_stmt`, `loop_stmt`, `type_decl`, `spawn`, `pipe`, decorator rules |
| Add XGrammar-2 export format | `vox-grammar-export/src/lib.rs` + new `xgrammar.rs` | `GrammarFormat::XGrammar2` emitter producing Earley PDA JSON spec |
| Grammar version hash | `vox-grammar-export/src/versioning.rs` + `build.rs` | Compute SHA256 of `emit_ebnf()` output; embed as `VOX_GRAMMAR_HASH` env var via `build.rs` |

**Acceptance:** `vox grammar export --format lark` produces a parseable Lark grammar that covers all 19 compact_prompt categories. GBNF export returns a descriptive error with migration guidance.

---

### Wave 4 — GRPO Reward Convergence

**Goal:** Align GRPO reward structure with research recommendations (see [research-grpo-gaps-and-adjustments-2026.md](research-grpo-gaps-and-adjustments-2026.md)).

| Task | File(s) | Change |
|---|---|---|
| Replace additive reward with gating | `mens/` training config / reward shaper | `R = r_syntax × (w1 × r_test + w2 × r_coverage)` — syntax as multiplier, not additive term |
| Adopt median-centered advantage | training loop | Replace mean with median for GRPO group baseline |
| NSR for failed parses | training loop | Ingest parse failures as hard negatives (reward = 0, negative advantage) |
| Curriculum seeding | training pipeline | Mutate 500-pair corpus to ≥8K pairs before GRPO training begins |

**Acceptance:** Parse rate ≥ 95% within 5 training steps. Reward does not plateau at 0.45 (syntax ceiling).

archived_date: 2026-04-18
---

### Wave 5 — LSP Quality

| Task | File(s) | Change |
|---|---|---|
| Position-context filtering | `vox-lsp/src/completions.rs` | `CompletionEngine::completions` currently returns all items unconditionally; add `trigger_kind` and `prefix` context to filter to relevant completions |
| Hide retired decorators | `vox-lsp/src/completions.rs` | Filter `LEXER_DEPRECATED_DECORATORS` from decorator suggestions |
| `ret` hover documentation | `vox-lsp/src/` | If user hovers `ret`, show: "Deprecated keyword — use `return`. This will be a compile error in Vox 0.5." |
| Add `@llm` decorator snippet | `language_surface.rs` | `("@llm", "@llm(model = \"${1:gpt-4o}\", verify = \"${2:strict}\") fn $3($4) to $5 {\n\t$0\n}")` |

---

## Remaining Unresolved Gaps (from original gap analysis, unchanged)

The following gaps from the original analysis are **confirmed real** and not yet addressed by any implementation. They remain in priority order:

1. **Gap A-3: No string interpolation** — No `TemplateLit` token. LLMs hallucinate backtick or f-string syntax.
2. **Gap A-7: Optional chaining `?.`** — In GRAMMAR_SSOT.md but not in token.rs.
3. **Gap C-1: `HirType::Option` not structurally distinct** — `Option[str]` is `HirType::Generic("option", [Named("str")])`, not `HirType::Option(Box)`. This matters for pattern matching lowering and codegen.
4. **Gap D (fuzzy parsing)** — No `vox parse --fuzzy` mode; LLM streaming output cannot be partially validated.
5. **Gap E (Stream of Revision)** — No mid-generation backtracking mechanism.
6. **Gap F (LLM function dispatch)** — `@llm` token and runtime dispatch (partially addressed by Wave 2 but runtime not yet implemented).
7. **Gap G (GRPO reward shaping)** — Addressed in Wave 4 above.
8. **Gap H (diagnostic ergonomics)** — `correction_hint` gaps addressed in Wave 0; broader diagnostic schema improvements pending.

archived_date: 2026-04-18
---

## AI Tool Consumption Notes

> For any AI tool (LLM, code-generation assistant, autocomplete engine) consuming this document:
>
> 1. **Do not generate `ret`** — use `return`. The `ret` keyword is deprecated and will produce a compiler warning. In Vox 0.5 it will be a hard error.
> 2. **Do not generate `@component fn`** — use `component Name() { state ...; view: ... }`. `@component fn` is already a hard compile error.
> 3. **Do not generate `context`, `@hook fn`, `@provider fn`, or `page:` declarations** — all retired with hard errors.
> 4. **`@ensure` postconditions work** — `FnDecl` has `postconditions: Vec<Expr>`. Use them. The bug is in HIR lowering, not the AST surface.
> 5. **Grammar-constrained decoding**: do not use the GBNF export format until BUG-5 is resolved. Use the EBNF export with XGrammar-2.
> 6. **The K-complexity design principle** means prefer single-declaration forms: `@table` over separate schema + migration + type. `@query` over raw SQL + TS client wrapper. `component Name()` over separate class + useState + useEffect.

