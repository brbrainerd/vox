# `@deprecated` Decorator: Activate Warning + Thread Reason String — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `@deprecated` on functions actually do something (it is currently a silent no-op) and thread an optional `@deprecated("reason")` string into the usage-site deprecation warning, then correct the reference docs.

**Architecture:** Three problems sit on top of each other. (1) The parser at `head_fn.rs` only sets a `is_deprecated` boolean and treats `@deprecated("reason") fn …` as a **parse error** (`Expected fn, found (`). (2) Even the boolean is dead for functions: `register_hir_function` registers the env binding with `is_deprecated: false` *hardcoded* (only tables wire it through), so the usage-site warning in `typeck/checker/expr.rs` never fires for a deprecated function. (3) There is no field to carry a reason. We fix these bottom-up: first activate the boolean (Task 1 — independently shippable and the highest-value fix), then parse + carry the reason on the AST `FnDecl` and HIR `HirFn` (Tasks 2–3), then surface the reason in the warning via a small side-table on `TypeEnv` (Task 4), then make the docs honest (Task 5).

**Tech Stack:** Rust; `vox-compiler` (parser/HIR/typeck) and `vox-ast` (AST node defs) crates. Tests use the existing `parse(lex(src))` + `typecheck_ast_module(src, &m)` harness (mirror `crates/vox-compiler/tests/future_promise_deprecation.rs`).

**Why a reason side-table instead of a new `Binding` field:** The usage-site warning reads `env.lookup(name).is_deprecated` (a `bool` on `typeck::env::Binding`). Adding a sibling `Option<String>` to `Binding` would ripple across ~50 struct-literal construction sites (most in `typeck/builtins.rs`). A `HashMap<String,String>` on `TypeEnv`, populated only for the handful of deprecated names, achieves the same result touching 3 files.

**Scope note — confirmed but deliberately excluded:** `auth_provider` / `roles` / `cors` / `rate_limit` / `pii` / `webhook` parse onto `FnDecl` but are read **only** in the endpoint-lowering branch of `hir/lower/mod.rs` (`e.func.*`). `lower_fn` in `hir/lower/decl.rs` never reads them, so these decorators on a *plain* (non-endpoint) fn are silently dropped with no diagnostic. This is a separate concern (risk to goldens; arguably endpoint-only by design) and is left for a follow-up "warn on misplaced endpoint decorator" task — see the final section.

---

## File Structure

| File | Responsibility | Change |
|------|----------------|--------|
| `crates/vox-compiler/src/typeck/registration.rs` | Registers HIR fns into the type env | Wire `f.is_deprecated` (Task 1); populate reason side-table (Task 4) |
| `crates/vox-compiler/tests/deprecated_fn_warning.rs` | New integration test | Created in Task 1, extended in Tasks 2 & 4 |
| `crates/vox-compiler/src/parser/descent/decl/head_fn.rs` | Parses `fn` decls + decorators | Consume optional `(reason)` after `@deprecated` (Task 2) |
| `crates/vox-ast/src/decl/fundecl.rs` | AST `FnDecl` definition | Add `deprecated_reason` field (Task 2) |
| `crates/vox-compiler/src/hir/nodes/decl.rs` | HIR `HirFn` definition | Add `deprecated_reason` field (Task 3) |
| `crates/vox-compiler/src/hir/lower/decl.rs` | Lowers `FnDecl` → `HirFn` | Populate `deprecated_reason` from AST (Task 3) |
| `crates/vox-compiler/src/typeck/env.rs` | `TypeEnv` / `Binding` | Add `deprecation_reasons` map + accessors (Task 4) |
| `crates/vox-compiler/src/typeck/checker/expr.rs` | Usage-site type checking | Enrich deprecation message with reason (Task 4) |
| `docs/src/reference/ref-decorators.md` | Decorator reference | Correct the `@deprecated` entry (Task 5) |

**Compiler-driven field additions:** Adding a required field to `FnDecl` and `HirFn` (neither derives `Default`; all construction sites are full struct literals) will break every literal with `E0063 missing field`. The known sites are listed in Tasks 2 and 3, but the canonical step is: add the field, run `cargo build -p vox-compiler` (and `-p vox-ast`), and add `deprecated_reason: None` to each site the compiler reports. This is a reliable, DRY technique — let the compiler enumerate.

---

## Task 1: Activate the function deprecation warning (the no-op bug)

This is the root fix and is shippable on its own. No new syntax — just makes the existing `@deprecated fn` boolean reach the warning.

**Files:**
- Create: `crates/vox-compiler/tests/deprecated_fn_warning.rs`
- Modify: `crates/vox-compiler/src/typeck/registration.rs:402`

- [ ] **Step 1: Write the failing test**

Create `crates/vox-compiler/tests/deprecated_fn_warning.rs`:

```rust
//! `@deprecated fn` must emit a usage-site deprecation warning.
//! Regression guard: the function binding was previously registered with
//! `is_deprecated: false` hardcoded, so the warning never fired for fns.

use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;
use vox_compiler::typeck::typecheck_ast_module;

fn diag_codes(src: &str) -> Vec<String> {
    let m = parse(lex(src)).expect("parse");
    typecheck_ast_module(src, &m)
        .into_iter()
        .filter_map(|d| d.code)
        .collect()
}

#[test]
fn deprecated_fn_use_warns() {
    let src = "@deprecated fn old() to int { return 0 }\n\
               fn main() to int { return old() }";
    let codes = diag_codes(src);
    assert!(
        codes.iter().any(|c| c == "typecheck.deprecated_ident"),
        "expected a deprecation warning on use of `old`; got {codes:?}"
    );
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p vox-compiler --test deprecated_fn_warning deprecated_fn_use_warns`
Expected: FAIL — the assertion fires (`got [...]` without `typecheck.deprecated_ident`), because the function binding is registered with `is_deprecated: false`.

- [ ] **Step 3: Wire the boolean through**

In `crates/vox-compiler/src/typeck/registration.rs`, in `register_hir_function`, change the binding at line ~402:

```rust
    env.define(
        f.name.clone(),
        Binding {
            ty: Ty::Fn(param_tys, Box::new(ret_ty)),
            mutable: false,
            kind: BindingKind::Function,
            is_deprecated: f.is_deprecated,
        },
    );
```

(Only the `is_deprecated` line changes: `false` → `f.is_deprecated`.)

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p vox-compiler --test deprecated_fn_warning deprecated_fn_use_warns`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vox-compiler/tests/deprecated_fn_warning.rs crates/vox-compiler/src/typeck/registration.rs
git commit -m "fix(typeck): @deprecated fn now emits a usage warning (was a no-op)"
```

---

## Task 2: Parse `@deprecated("reason")` and carry it on the AST `FnDecl`

After this task the documented `@deprecated("…")` syntax parses (today it is `Expected fn, found (`) and the reason is preserved on the AST node. The reason is not yet shown anywhere — that is Tasks 3–4.

**Files:**
- Modify: `crates/vox-ast/src/decl/fundecl.rs:42` (add field)
- Modify: `crates/vox-compiler/src/parser/descent/decl/head_fn.rs` (declare local ~line 23; consume arg at the `Token::AtDeprecated` arm ~line 115; set field in the `FnDecl { … }` literal ~line 1086)
- Modify (compiler-driven `E0063`): `crates/vox-compiler/src/parser/descent/mod.rs:341`, `crates/vox-compiler/src/parser/descent/decl/head.rs:315`, `crates/vox-compiler/tests/decl_lowering_test.rs:22`
- Test: `crates/vox-compiler/tests/deprecated_fn_warning.rs` (add parse assertions)

- [ ] **Step 1: Write the failing test**

Append to `crates/vox-compiler/tests/deprecated_fn_warning.rs`:

```rust
use vox_compiler::ast::decl::{Decl, FnDecl};

fn first_fn(src: &str) -> FnDecl {
    let m = parse(lex(src)).expect("parse");
    m.declarations
        .into_iter()
        .find_map(|d| match d {
            Decl::Function(f) => Some(f),
            _ => None,
        })
        .expect("a function decl")
}

#[test]
fn deprecated_reason_parses_and_is_captured() {
    let f = first_fn("@deprecated(\"Use new_function instead\") fn old() to int { return 0 }");
    assert!(f.is_deprecated);
    assert_eq!(f.deprecated_reason.as_deref(), Some("Use new_function instead"));
}

#[test]
fn bare_deprecated_still_parses_with_no_reason() {
    let f = first_fn("@deprecated fn old() to int { return 0 }");
    assert!(f.is_deprecated);
    assert_eq!(f.deprecated_reason, None);
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p vox-compiler --test deprecated_fn_warning`
Expected: FAIL to **compile** — `no field deprecated_reason on type FnDecl`. (The `deprecated_reason_parses…` test would also fail at runtime with a parse error once the field exists but the parser is unchanged.)

- [ ] **Step 3: Add the field to the AST `FnDecl`**

In `crates/vox-ast/src/decl/fundecl.rs`, immediately after the `is_deprecated` field (line ~42):

```rust
    /// Whether the function is marked as deprecated.
    pub is_deprecated: bool,
    /// Optional human-readable reason from `@deprecated("reason")`. `None` for the bare form.
    pub deprecated_reason: Option<String>,
```

- [ ] **Step 4: Parse the optional reason argument**

In `crates/vox-compiler/src/parser/descent/decl/head_fn.rs`:

(a) Declare the accumulator next to `is_deprecated` (line ~23):

```rust
        let mut is_deprecated = false;
        let mut deprecated_reason: Option<String> = None;
```

(b) Replace the `Token::AtDeprecated` arm (lines ~115–118) with:

```rust
                Token::AtDeprecated => {
                    self.advance();
                    is_deprecated = true;
                    // Optional `("reason")` argument. Bare `@deprecated` is still valid.
                    if self.eat(&Token::LParen) {
                        if let Token::StringLit(reason) = self.peek().clone() {
                            self.advance();
                            deprecated_reason = Some(reason);
                        }
                        let _ = self.expect(&Token::RParen);
                    }
                }
```

(c) Add the field to the `Ok(FnDecl { … })` literal (next to `is_deprecated,` at line ~1086):

```rust
            is_deprecated,
            deprecated_reason,
```

- [ ] **Step 5: Fix the remaining `FnDecl` literal sites (compiler-driven)**

Run: `cargo build -p vox-ast && cargo build -p vox-compiler`
For each `E0063 missing field deprecated_reason` the compiler reports, add `deprecated_reason: None,` next to the existing `is_deprecated: …` line. Known sites:
- `crates/vox-compiler/src/parser/descent/mod.rs:341` (synthetic `main` fn) → `deprecated_reason: None,`
- `crates/vox-compiler/src/parser/descent/decl/head.rs:315` → `deprecated_reason: None,`
- `crates/vox-compiler/tests/decl_lowering_test.rs:22` (the `fn_decl` helper) → `deprecated_reason: None,`

Re-run the build until clean.

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p vox-compiler --test deprecated_fn_warning`
Expected: PASS (all four tests). The earlier `@deprecated("…")` source no longer errors with `Expected fn, found (`.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-ast/src/decl/fundecl.rs crates/vox-compiler/src/parser/descent/decl/head_fn.rs crates/vox-compiler/src/parser/descent/mod.rs crates/vox-compiler/src/parser/descent/decl/head.rs crates/vox-compiler/tests/decl_lowering_test.rs crates/vox-compiler/tests/deprecated_fn_warning.rs
git commit -m "feat(parser): parse @deprecated(\"reason\") and carry it on FnDecl"
```

---

## Task 3: Carry the reason through lowering onto `HirFn`

**Files:**
- Modify: `crates/vox-compiler/src/hir/nodes/decl.rs:386` (add field)
- Modify: `crates/vox-compiler/src/hir/lower/decl.rs:53` (populate from `f.deprecated_reason`)
- Modify (compiler-driven `E0063`): other `HirFn { … }` literals (list below)
- Test: a lowering assertion in `crates/vox-compiler/tests/deprecated_fn_warning.rs`

- [ ] **Step 1: Write the failing test**

Append to `crates/vox-compiler/tests/deprecated_fn_warning.rs`:

```rust
use vox_compiler::typeck::lower_module; // re-exported lowering entry; see note in Step 2 if the path differs

#[test]
fn deprecated_reason_survives_lowering() {
    let m = parse(lex(
        "@deprecated(\"gone soon\") fn old() to int { return 0 }",
    ))
    .expect("parse");
    let hir = lower_module(&m);
    let f = hir
        .functions
        .iter()
        .find(|f| f.name == "old")
        .expect("lowered fn `old`");
    assert!(f.is_deprecated);
    assert_eq!(f.deprecated_reason.as_deref(), Some("gone soon"));
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p vox-compiler --test deprecated_fn_warning deprecated_reason_survives_lowering`
Expected: FAIL to compile — `no field deprecated_reason on type HirFn`.

> Note: if `lower_module` is not re-exported under `vox_compiler::typeck`, use the actual public path. Confirm with `cargo doc` or `rg "pub fn lower_module" crates/vox-compiler/src`. It is called internally at `typeck/mod.rs:671`; if it is not `pub` at a stable path, mark the lowered fn instead by typechecking and asserting the warning message in Task 4 and delete this lowering test. Prefer keeping it if a public lowering entry exists.

- [ ] **Step 3: Add the field to `HirFn`**

In `crates/vox-compiler/src/hir/nodes/decl.rs`, immediately after the `is_deprecated` field (line ~386):

```rust
    /// `@deprecated` on the source `fn`.
    #[serde(default)]
    pub is_deprecated: bool,
    /// Optional reason string from `@deprecated("reason")`. `None` for the bare form.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deprecated_reason: Option<String>,
```

- [ ] **Step 4: Populate it in `lower_fn`**

In `crates/vox-compiler/src/hir/lower/decl.rs`, in the `HirFn { … }` literal returned by `lower_fn` (the `is_deprecated: f.is_deprecated,` line at ~212), add directly below it:

```rust
            is_deprecated: f.is_deprecated,
            deprecated_reason: f.deprecated_reason.clone(),
```

- [ ] **Step 5: Fix the remaining `HirFn` literal sites (compiler-driven)**

Run: `cargo build -p vox-compiler`
For each `E0063 missing field deprecated_reason`, add `deprecated_reason: None,` next to the existing `is_deprecated: …` line. These constructs are not functions written by a user (synthetic/derived fns or test fixtures), so `None` is correct. Known sites:
- `crates/vox-compiler/src/hir/lower/decl.rs:552` (workflow), `:593` (activity), `:642` (actor), `:694` (actor handler)
- `crates/vox-compiler/src/hir/lower/expr.rs:237` (synthesised fn)
- `crates/vox-compiler/src/hir/lower/json_as.rs:103`, `:429` (derived to/from-JSON fns)
- `crates/vox-compiler/src/hir/lower/mod.rs:1047`
- `crates/vox-compiler/src/hir/validate.rs:346` (`named_fn` test helper)
- `crates/vox-compiler/src/hir/dead_code.rs:250`, `:295`, `:340` (test fixtures)
- `crates/vox-compiler/src/typeck/cuda_gate.rs:67` (test fixture)

Re-run the build until clean. (The compiler is the source of truth — add to whatever else it reports.)

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p vox-compiler --test deprecated_fn_warning`
Expected: PASS (all five tests).

- [ ] **Step 7: Commit**

```bash
git add crates/vox-compiler/src/hir/ crates/vox-compiler/src/typeck/cuda_gate.rs crates/vox-compiler/tests/deprecated_fn_warning.rs
git commit -m "feat(hir): thread deprecated_reason from FnDecl onto HirFn"
```

---

## Task 4: Surface the reason in the deprecation warning

Adds a name→reason side-table to `TypeEnv`, populates it during function registration, and enriches the warning message at the usage site.

**Files:**
- Modify: `crates/vox-compiler/src/typeck/env.rs` (struct field ~line 95; init in `new` ~line 126; two accessor methods on `impl TypeEnv` ~line 122)
- Modify: `crates/vox-compiler/src/typeck/registration.rs` (in `register_hir_function`, after `env.define(...)`)
- Modify: `crates/vox-compiler/src/typeck/checker/expr.rs:177-180` (message)
- Test: extend `crates/vox-compiler/tests/deprecated_fn_warning.rs`

- [ ] **Step 1: Write the failing test**

Append to `crates/vox-compiler/tests/deprecated_fn_warning.rs`:

```rust
fn diag_messages(src: &str) -> Vec<String> {
    let m = parse(lex(src)).expect("parse");
    typecheck_ast_module(src, &m)
        .into_iter()
        .map(|d| d.message)
        .collect()
}

#[test]
fn deprecation_warning_includes_reason() {
    let src = "@deprecated(\"Use new_function instead\") fn old() to int { return 0 }\n\
               fn main() to int { return old() }";
    let msgs = diag_messages(src);
    assert!(
        msgs.iter().any(|m| m.contains("deprecated: Use new_function instead")),
        "expected the reason in the warning message; got {msgs:?}"
    );
}

#[test]
fn bare_deprecation_warning_has_no_reason_suffix() {
    let src = "@deprecated fn old() to int { return 0 }\n\
               fn main() to int { return old() }";
    let msgs = diag_messages(src);
    assert!(
        msgs.iter().any(|m| m == "'old' is deprecated"),
        "bare @deprecated should produce the plain message; got {msgs:?}"
    );
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p vox-compiler --test deprecated_fn_warning deprecation_warning_includes_reason`
Expected: FAIL — the message is `'old' is deprecated` (no reason suffix yet).

- [ ] **Step 3: Add the side-table to `TypeEnv`**

In `crates/vox-compiler/src/typeck/env.rs`:

(a) Add the field to `struct TypeEnv` (after `scopes: Vec<Scope>,` ~line 95):

```rust
    scopes: Vec<Scope>,
    /// Reasons for `@deprecated("reason")` names, keyed by name. Populated only
    /// for deprecated declarations; read at the usage site to enrich the warning.
    deprecation_reasons: HashMap<String, String>,
```

(b) Initialise it in `TypeEnv::new` (the `Self { scopes: vec![Scope::new()], … }` near line 126):

```rust
            scopes: vec![Scope::new()],
            deprecation_reasons: HashMap::new(),
```

(c) Add accessors inside `impl TypeEnv` (anywhere in the block starting ~line 122):

```rust
    /// Record the `@deprecated("reason")` text for a name.
    pub fn set_deprecation_reason(&mut self, name: impl Into<String>, reason: impl Into<String>) {
        self.deprecation_reasons.insert(name.into(), reason.into());
    }

    /// Reason recorded for a deprecated name, if any.
    #[must_use]
    pub fn deprecation_reason(&self, name: &str) -> Option<&str> {
        self.deprecation_reasons.get(name).map(String::as_str)
    }
```

(`HashMap` is already imported at the top of `env.rs`.)

- [ ] **Step 4: Populate the side-table during registration**

In `crates/vox-compiler/src/typeck/registration.rs`, in `register_hir_function`, immediately after the `env.define(f.name.clone(), Binding { … });` block (ends ~line 404):

```rust
    if let Some(reason) = &f.deprecated_reason {
        env.set_deprecation_reason(f.name.clone(), reason.clone());
    }
```

- [ ] **Step 5: Enrich the warning message**

In `crates/vox-compiler/src/typeck/checker/expr.rs`, in the `HirExpr::Ident` arm (the `if binding.is_deprecated {` block at ~line 177), replace the hardcoded `message: format!("'{name}' is deprecated"),` with a precomputed message:

```rust
                    if binding.is_deprecated {
                        let message = match self.env.deprecation_reason(name) {
                            Some(reason) => format!("'{name}' is deprecated: {reason}"),
                            None => format!("'{name}' is deprecated"),
                        };
                        self.diags.push(Diagnostic {
                            severity: TypeckSeverity::Warning,
                            message,
                            span: *span,
```

(Leave the rest of the `Diagnostic { … }` literal — `code: Some("typecheck.deprecated_ident".into())`, etc. — unchanged. `binding` and `self.env.deprecation_reason(name)` are both immutable borrows of `self.env`, which is allowed.)

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p vox-compiler --test deprecated_fn_warning`
Expected: PASS (all seven tests).

- [ ] **Step 7: Commit**

```bash
git add crates/vox-compiler/src/typeck/env.rs crates/vox-compiler/src/typeck/registration.rs crates/vox-compiler/src/typeck/checker/expr.rs crates/vox-compiler/tests/deprecated_fn_warning.rs
git commit -m "feat(typeck): include @deprecated reason in the deprecation warning"
```

---

## Task 5: Correct the reference docs

The current `@deprecated` entry is doubly wrong: it carries a `Planned — not yet parseable` note (the bare form **does** parse), and it shows a reason-string form that — before this plan — did not parse.

**Files:**
- Modify: `docs/src/reference/ref-decorators.md:80-85`

- [ ] **Step 1: Rewrite the `@deprecated` entry**

Replace lines 80–85 (the `### `@deprecated`` block, including the `> [!NOTE] Planned — not yet parseable.` callout) with:

```markdown
### `@deprecated`
- **Goal**: Marks a function as pending removal.
- **Effect**: Emits a `typecheck.deprecated_ident` warning at every call site. With a reason argument, the reason is appended to the warning message.
- **Usage**:
  - Bare: `@deprecated fn old() to int { return 0 }` → warning: `'old' is deprecated`
  - With reason: `@deprecated("Use new_function instead") fn old() to int { return 0 }` → warning: `'old' is deprecated: Use new_function instead`
```

(Do not add `// vox:skip`. These are single-line, in-language examples and should compile; if a fenced ```vox block is used, ensure it type-checks — `fn old() to int { return 0 }` is valid Vox.)

- [ ] **Step 2: Verify the doc pipeline accepts the change**

Run: `VOX_FMT_CHECK=1 vox run scripts/fmt.vox` is **not** needed (no Rust changed). Instead validate the docs build / fenced-block compilation the way the repo does — check `AGENTS.md` for the doc check command (e.g. `vox ci docs` or the mdbook/test target). Run it and confirm no frontmatter or fenced-`vox` compile errors for `ref-decorators.md`.
Expected: PASS (no new doc errors).

- [ ] **Step 3: Commit**

```bash
git add docs/src/reference/ref-decorators.md
git commit -m "docs(decorators): correct @deprecated entry to match parser reality"
```

---

## Final Verification

- [ ] **Whole-crate test + lint on touched crates**

```bash
cargo test -p vox-compiler --test deprecated_fn_warning
cargo test -p vox-compiler decl_lowering
cargo build -p vox-ast -p vox-compiler
cargo clippy -p vox-compiler -- -D warnings
```
Expected: all green. (Per project guidance, run `cargo clippy -p <crate> -- -D warnings` on touched crates before any admin-merge; never `cargo fmt --all` on Windows — use `cargo fmt -p vox-compiler` / `cargo fmt -p vox-ast`.)

- [ ] **Format the two touched crates**

```bash
cargo fmt -p vox-ast
cargo fmt -p vox-compiler
```

---

## Out of Scope — Confirmed Follow-up Finding

**Silent drop of endpoint decorators on plain functions.** Verified during investigation: `auth_provider`, `roles`, `cors`, `rate_limit`, `pii`, and `webhook` are parsed onto `FnDecl` (e.g. `head_fn.rs:57-69`) but read **only** in the endpoint-lowering branch of `crates/vox-compiler/src/hir/lower/mod.rs:275-367` (via `e.func.webhook`, `e.func.rate_limit`, `e.func.auth_provider`, …). `lower_fn` in `crates/vox-compiler/src/hir/lower/decl.rs` has **zero** references to them, so `@rate_limit fn foo()` on a non-endpoint function parses successfully and is discarded with no diagnostic.

This is intentionally **not** fixed here: it is a distinct subsystem, the decorators are arguably endpoint-only by design, and emitting a new warning risks breaking structural golden tests. Recommended follow-up (separate plan/PR): emit a `typecheck`/lint warning ("`@rate_limit` has no effect on a non-endpoint function; did you mean to mark it `@endpoint`/`@get`/…?") when any of these decorators appear on a function that is not lowered as an endpoint. Decide first whether any in-repo `.vox` corpus or goldens rely on the current silent behaviour.

---

## Self-Review

- **Spec coverage:** Task description offered (a) wire the reason through — covered by Tasks 2–4 (`deprecated_reason` on `FnDecl` and `HirFn`, populated in lowering at `decl.rs`), plus Task 1 which fixes the deeper no-op bug not mentioned in the spec. The spec's option (b) (doc-only) is folded in as Task 5 so the docs are correct regardless. The "also investigate" auth/roles/cors/rate_limit/pii/webhook drop is confirmed and documented in the Out-of-Scope section.
- **Placeholder scan:** No `TBD`/`handle edge cases`; every code step shows exact code. The one conditional is the `lower_module` public-path check in Task 3 Step 2, with an explicit fallback.
- **Type consistency:** Field name `deprecated_reason: Option<String>` is identical across `FnDecl`, `HirFn`, parser local, and lowering. Accessors `set_deprecation_reason` / `deprecation_reason` names match between `env.rs` (def) and `registration.rs` / `expr.rs` (use). Warning code `typecheck.deprecated_ident` matches `expr.rs:187`.
