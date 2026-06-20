# Vox Placement Model Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give every Vox declaration a compiler-computed *placement* (`native` / `shared` / `gui`), inferred from the effect system and overridable with `@place(...)`, so the perf-vs-GUI boundary is a checked contract instead of convention.

**Architecture:** A new post-typeck pass `typeck::placement` reads `HirFn`/`HirReactiveComponent` effect data, seeds each declaration's placement, propagates along the call graph to a fixed point, then emits blocking diagnostics for conflicts (`E-PLACE-CONFLICT`), illegal direct gui→native calls (`E-PLACE-BOUNDARY`), and unsatisfiable `@place` overrides (`E-PLACE-UNSAT`). The dormant `@native` token is retired in favor of `@place(native)`. Enforcement is blocking from day one; broken golden fixtures are fixed in the same body of work (Task 11).

**Tech Stack:** Rust (`vox-compiler`, `vox-ast`, `vox-code-audit`), the existing `feature_matrix` parity machinery, the `vox-code-audit` detector ABI.

**Spec:** `docs/superpowers/specs/2026-06-20-vox-placement-model-design.md`

**Out of scope (separate sub-specs):** speed tiers / cargo broker (sub-spec 2); behavioral differential oracle + Vox Axis Parity panel (sub-spec 3).

---

## ⚠️ Verified ground truth (audited 2026-06-20 — do not re-guess these)

These were confirmed by reading the code. Trust them over your priors:

- `HirFn` (`crates/vox-compiler/src/hir/nodes/decl.rs:327`) derives **`Debug, Clone, serde::Serialize, serde::Deserialize`** — **NOT `Default`**. Do not hand-construct it in tests; build HIR from source (see the test helper below).
- Relevant `HirFn` fields (all present, exact names): `name: String`, `is_pure: bool`, `is_reactive: bool`, `is_remote: bool`, `is_llm: bool`, `is_mobile_native: bool`, `capabilities: Vec<HirCapability>`, `durability: Option<super::durability::DurabilityKind>`, `body: Vec<HirStmt>`, `span: Span`, `id: DefId`.
- `HirCapability` (`decl.rs:770`): `Net, Db, Fs, Env, Clock, Random, Spawn, GpuCompute, Mutate, Vcs, Mcp(String), Nothing`. Only `Mcp` carries data.
- `HirModule` (`decl.rs:29`) **derives `Default`**; fields used here: `functions: Vec<HirFn>`, `endpoint_fns: Vec<HirEndpointFn>`, `components: Vec<HirReactiveComponent>`. `HirEndpointFn.name: String` exists (`decl.rs:492`).
- `HirReactiveComponent` (`decl.rs:640`): `name`, `view: Option<HirExpr>`, `members`, `styles`, `layer`.
- `HirExpr` (`stmt_expr.rs:135`) variants are **tuple-style**: `Call(Box<HirExpr>, Vec<HirArg>, bool, Span)`, `Ident(String, Span)`, etc. `HirArg` (`stmt_expr.rs:217`) is a struct: `{ name: Option<String>, value: HirExpr }` — get the inner expr via `.value`.
- `HirStmt` (`stmt_expr.rs:298`) variants are **named-field**: `Expr { expr, span }`, `Let { value, .. }`, `Assign { target, value, span }`, `Return { value: Option<HirExpr>, span }`, `While { condition, body, span }`, `Loop { body, span }`, `Break { span }`, `Continue { span }`.
- `Diagnostic::error(message: String, span: Span, source: &str) -> Self`; `.with_code(impl Into<String>)`, `.with_suggestion(impl Into<String>)`; `code: Option<String>` (`diagnostics.rs:480,544,562`).
- Lint passes vec: `crates/vox-compiler/src/typeck/mod.rs:133`; closures call e.g. `effect_check::check_effect_compliance(hir, source)` where the fn is `pub fn check_effect_compliance(module: &HirModule, source: &str) -> Vec<Diagnostic>` (`effect_check.rs:16`). Submodule decls near `pub mod layer;` (`mod.rs:23`).
- **`@uses` is a bare clause, NOT a decorator.** Argument-bearing decorator template = `@require(...)` at `head_fn.rs:68-73` (`advance(); expect(LParen); …; expect(RParen)`).
- The current `@native` parser arm is `head_fn.rs:119-122`: `Token::AtFuzz | Token::AtNative => { self.advance(); is_mobile_native = true; }`. `@native` flowed into `HirFn.is_mobile_native`. After retirement, keep `Token::AtFuzz => { … is_mobile_native = true; }`.
- `is_mobile_native`/`is_pure` flow parser→lower verbatim: AST `FnDecl` (`crates/vox-ast/src/decl/fundecl.rs`) has the bool fields; `lower/decl.rs:53-63` copies `f.is_mobile_native`, `f.is_pure` straight into `HirFn`. **Use this exact flow as the template for `placement_override`.**
- Test pipeline imports (`tests/ai_fixture_typeck_diagnostics.rs:1-15`): `vox_compiler::lexer::cursor::lex`, `vox_compiler::parser::parse`, `vox_compiler::hir::lower_module`, `vox_compiler::typeck::typecheck_hir_module`.
- Integration-test compile entry (returns diagnostics): `vox_compiler::pipeline::run_frontend_str(source, file_path) -> Result<FrontendResult>` with `FrontendResult.diagnostics: Vec<Diagnostic>`. (Or reuse the lex/parse/lower/typecheck chain.)
- `feature_matrix.rs`: `DecoratorFeature` enum (`Native` at :135), `DecoratorFeature::ALL: [_; 56]` (`Native` at :303). Renaming the variant makes the compiler flag every match arm that needs updating — lean on that.
- `language_surface.rs`: only `LEXER_DECORATORS` (~:200) contains `"@native"`. `@native` is **not** in `LSP_DECORATOR_DOCS`. Sync test `lsp_decorator_spellings_exist_in_lexer_list` is one-way (LSP→LEXER), so removing `@native` from `LEXER_DECORATORS` is safe.
- `vox-code-audit` `retired_decorator.rs`: per-pattern `Regex` fields + `build_finding(...)` calls; test helper `fn source(code: &str) -> SourceFile { SourceFile::new(PathBuf::from("test.vox"), code.to_string()) }`. No central rule_count to bump.

### Shared test helper (used by every unit test below)

Put this at the top of the `#[cfg(test)] mod tests` in `placement.rs`:

```rust
fn hir_of(src: &str) -> crate::hir::nodes::decl::HirModule {
    use crate::hir::lower_module;
    use crate::lexer::cursor::lex;
    use crate::parser::parse;
    lower_module(&parse(lex(src)).expect("fixture must parse"))
}
```

`lower_module` populates `capabilities` (from the `uses` clause), `is_reactive`, `is_pure`, `durability`, etc., so placement inputs are real — no hand-built `HirFn`.

---

## File Structure

| File | Responsibility | New/Modify |
|------|----------------|-----------|
| `crates/vox-compiler/src/typeck/placement.rs` | The pass: `Placement` enum, `PlacementMap`, seed/propagate/conflict/boundary/override, `infer()` | **Create** |
| `crates/vox-compiler/src/typeck/diagnostics.rs` | Add 3 placement codes + register | Modify |
| `crates/vox-compiler/src/typeck/mod.rs` | `pub mod placement;`; wire pass into `passes` vec | Modify |
| `crates/vox-ast/src/decl/fundecl.rs` | Add `placement_override: Option<PlacementHint>` to `FnDecl`; define `PlacementHint` enum | Modify |
| `crates/vox-compiler/src/hir/nodes/decl.rs` | Add `placement_override: Option<PlacementHint>` to `HirFn` | Modify |
| `crates/vox-compiler/src/hir/lower/decl.rs` | Copy `placement_override` parser→HIR (line 53–63 block) | Modify |
| `crates/vox-compiler/src/lexer/token.rs` | Add `@place` token; remove `@native` token + Display arm | Modify |
| `crates/vox-compiler/src/parser/descent/decl/head_fn.rs` | Parse `@place(...)`; split out `AtNative` from the `AtFuzz` arm (line 119–122) | Modify |
| `crates/vox-compiler/src/feature_matrix.rs` | Rename `Native`→`Place` (enum :135, ALL :303, spelling, any match arms) | Modify |
| `crates/vox-compiler/src/language_surface.rs` | Swap `"@native"`→`"@place"` in `LEXER_DECORATORS` | Modify |
| `crates/vox-compiler/src/lexer/cursor.rs` | Update the lexer test string referencing `@native` (:253) | Modify |
| `crates/vox-code-audit/src/detectors/retired_decorator.rs` | Add `@native`→`@place(native)` regex + finding (Error) | Modify |
| `AGENTS.md` | Add `@native` row to Retired Surfaces table (~:407) | Modify |

---

## Task 1: Placement diagnostic codes

**Files:**
- Modify: `crates/vox-compiler/src/typeck/diagnostics.rs`
- Test: same file's `#[cfg(test)] mod tests`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn placement_codes_are_registered() {
    for code in [
        codes::PLACEMENT_CONFLICT,
        codes::PLACEMENT_BOUNDARY,
        codes::PLACEMENT_UNSAT,
    ] {
        assert!(
            codes::ALL_COMPILER_DIAGNOSTIC_CODES.contains(&code),
            "{code} must be registered in ALL_COMPILER_DIAGNOSTIC_CODES"
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-compiler placement_codes_are_registered`
Expected: FAIL — `no associated item named PLACEMENT_CONFLICT`.

- [ ] **Step 3: Add the constants and register them**

In the `codes` module (alongside `PARITY_FRONTEND_ONLY`, ~:757):

```rust
/// A declaration is pulled toward both native-only and gui-only tiers.
pub const PLACEMENT_CONFLICT: &str = "vox/placement/conflict";
/// A `gui` declaration calls a `native` declaration directly (no endpoint).
pub const PLACEMENT_BOUNDARY: &str = "vox/placement/boundary";
/// An explicit `@place(...)` override cannot be satisfied by the declaration's effects.
pub const PLACEMENT_UNSAT: &str = "vox/placement/unsat";
```

Append to the `ALL_COMPILER_DIAGNOSTIC_CODES` array (`&[&str]`, ~:795+):

```rust
    PLACEMENT_CONFLICT,
    PLACEMENT_BOUNDARY,
    PLACEMENT_UNSAT,
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-compiler placement_codes_are_registered`
Expected: PASS.

- [ ] **Step 5: Verify no audit-rule collision**

Run: `cargo test -p vox-code-audit no_audit_rule_collides_with_compiler_diagnostic_code`
Expected: PASS (the `vox/placement/*` codes are disjoint from audit rule ids).

- [ ] **Step 6: Commit**

```bash
git add crates/vox-compiler/src/typeck/diagnostics.rs
git commit -m "feat(placement): register placement diagnostic codes"
```

---

## Task 2: `Placement` type + seed inference, tested from source

**Files:**
- Create: `crates/vox-compiler/src/typeck/placement.rs`
- Modify: `crates/vox-compiler/src/typeck/mod.rs` (add `pub mod placement;` near :23)
- Test: in `placement.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/vox-compiler/src/typeck/placement.rs`:

```rust
//! Placement inference: assigns each declaration a tier (native/shared/gui).
//! See docs/superpowers/specs/2026-06-20-vox-placement-model-design.md.

use crate::hir::nodes::decl::{HirCapability, HirFn};

/// Where a declaration may be emitted. `Shared` is the top of the lattice
/// (emits to native + gui + interp); `Native` and `Gui` are incompatible
/// specializations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    Shared,
    Native,
    Gui,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hir_of(src: &str) -> crate::hir::nodes::decl::HirModule {
        use crate::hir::lower_module;
        use crate::lexer::cursor::lex;
        use crate::parser::parse;
        lower_module(&parse(lex(src)).expect("fixture must parse"))
    }

    #[test]
    fn pure_fn_seeds_shared() {
        let m = hir_of("@pure fn checksum() { 0 }");
        assert_eq!(seed_fn(&m.functions[0]), Placement::Shared);
    }

    #[test]
    fn db_fn_seeds_native() {
        let m = hir_of("fn load() uses db { 0 }");
        assert_eq!(seed_fn(&m.functions[0]), Placement::Native);
    }

    #[test]
    fn reactive_fn_seeds_gui() {
        let m = hir_of("@reactive fn Counter() { 0 }");
        assert_eq!(seed_fn(&m.functions[0]), Placement::Gui);
    }

    #[test]
    fn unannotated_fn_seeds_shared() {
        let m = hir_of("fn add(a: Int, b: Int) -> Int { a + b }");
        assert_eq!(seed_fn(&m.functions[0]), Placement::Shared);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Add `pub mod placement;` to `crates/vox-compiler/src/typeck/mod.rs` near :23.
Run: `cargo test -p vox-compiler placement::tests`
Expected: FAIL — `cannot find function seed_fn`.

> If any fixture fails to *parse/lower* (e.g. `uses db` syntax differs), fix the fixture string to match real grammar — check an existing `.vox` golden that uses a `uses` clause (`grep -rl "uses " examples/golden`).

- [ ] **Step 3: Implement `seed_fn`**

Above the test module:

```rust
fn is_native_capability(cap: &HirCapability) -> bool {
    matches!(
        cap,
        HirCapability::Net
            | HirCapability::Db
            | HirCapability::Fs
            | HirCapability::Env
            | HirCapability::Clock
            | HirCapability::Random
            | HirCapability::Spawn
            | HirCapability::GpuCompute
            | HirCapability::Vcs
            | HirCapability::Mcp(_)
    )
}

/// Seed a single function's placement from its own decorators/effects.
#[must_use]
pub fn seed_fn(f: &HirFn) -> Placement {
    if f.is_reactive {
        return Placement::Gui;
    }
    if f.capabilities.iter().any(is_native_capability)
        || f.is_remote
        || f.is_llm
        || f.durability.is_some()
    {
        return Placement::Native;
    }
    Placement::Shared // @pure, `uses nothing`, or unannotated leaf
}
```

> `HirCapability::Mutate` and `Nothing` are deliberately not native — local mutation / explicit purity do not require the native tier.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vox-compiler placement::tests`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-compiler/src/typeck/placement.rs crates/vox-compiler/src/typeck/mod.rs
git commit -m "feat(placement): Placement type + per-declaration seed inference"
```

---

## Task 3: Components seed `gui`; module-level `PlacementMap::seed`

**Files:**
- Modify: `crates/vox-compiler/src/typeck/placement.rs`
- Test: in `placement.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn map_seeds_functions_endpoints_components() {
    let m = hir_of(
        "fn fmt() { 0 }\n\
         fn load() uses db { 0 }\n\
         @query fn list() { 0 }\n\
         component Panel() { <div></div> }",
    );
    let map = PlacementMap::seed(&m);
    assert_eq!(map.get("fmt"), Some(Placement::Shared));
    assert_eq!(map.get("load"), Some(Placement::Native));
    assert_eq!(map.get("list"), Some(Placement::Native)); // endpoints are native
    assert_eq!(map.get("Panel"), Some(Placement::Gui));   // components are gui
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-compiler map_seeds_functions_endpoints_components`
Expected: FAIL — `cannot find type PlacementMap`.

> If `@query fn list()` lowers into `hir.endpoint_fns` (it should) confirm with a quick `dbg!(m.endpoint_fns.len())` if the assertion misbehaves; if `component Panel()` fails to parse, copy a real component from a golden fixture.

- [ ] **Step 3: Implement `PlacementMap` + `seed`**

```rust
use crate::hir::nodes::decl::{HirModule, HirReactiveComponent};
use std::collections::HashMap;

/// Components are always GUI-tier.
#[must_use]
pub fn seed_component(_c: &HirReactiveComponent) -> Placement {
    Placement::Gui
}

/// Placement per declaration, keyed by name (unique within a module at this stage).
#[derive(Debug, Default)]
pub struct PlacementMap(HashMap<String, Placement>);

impl PlacementMap {
    #[must_use]
    pub fn seed(m: &HirModule) -> PlacementMap {
        let mut map = HashMap::new();
        for f in &m.functions {
            map.insert(f.name.clone(), seed_fn(f));
        }
        for ep in &m.endpoint_fns {
            map.insert(ep.name.clone(), Placement::Native);
        }
        for c in &m.components {
            map.insert(c.name.clone(), seed_component(c));
        }
        PlacementMap(map)
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<Placement> {
        self.0.get(name).copied()
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-compiler map_seeds_functions_endpoints_components`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-compiler/src/typeck/placement.rs
git commit -m "feat(placement): components seed gui; PlacementMap module seed"
```

---

## Task 4: Exhaustive call-graph walker (`callee_names`)

**Files:**
- Modify: `crates/vox-compiler/src/typeck/placement.rs`
- Test: in `placement.rs`

This walker is a **correctness invariant** — a missed sub-expression is a false negative no test catches unless added. Below is the complete walker against the verified `HirExpr`/`HirStmt` shapes.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn walker_finds_calls_in_nested_positions() {
    let m = hir_of("fn f() { let x = g(h()); if x { i() } }");
    let names = callee_names(&m.functions[0].body);
    for want in ["g", "h", "i"] {
        assert!(names.iter().any(|n| n == want), "missing call to {want} in {names:?}");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-compiler walker_finds_calls_in_nested_positions`
Expected: FAIL — `cannot find function callee_names`.

- [ ] **Step 3: Implement the exhaustive walker**

```rust
use crate::hir::nodes::stmt_expr::{HirArg, HirExpr, HirStmt};

fn walk_expr(e: &HirExpr, out: &mut Vec<String>) {
    use HirExpr::*;
    match e {
        IntLit(..) | FloatLit(..) | StringLit(..) | BoolLit(..) | DecimalLit(..)
        | Ident(..) | WorkflowVersion(..) => {}
        ObjectLit(fields, _) => {
            for (_, v) in fields { walk_expr(v, out); }
        }
        ListLit(xs, _) | TupleLit(xs, _) | JsxFragment(xs, _) => {
            for x in xs { walk_expr(x, out); }
        }
        Binary(_, a, b, _) | With(a, b, _) | Index(a, b, _) => {
            walk_expr(a, out);
            walk_expr(b, out);
        }
        Unary(_, a, _) | FieldAccess(a, _, _) | Spawn(a, _) => walk_expr(a, out),
        Call(callee, args, _, _) => {
            if let Ident(name, _) = &**callee {
                out.push(name.clone());
            } else {
                walk_expr(callee, out);
            }
            for a in args { walk_arg(a, out); }
        }
        MethodCall(recv, _, args, _, _) => {
            walk_expr(recv, out);
            for a in args { walk_arg(a, out); }
        }
        If(c, then_b, else_b, _) => {
            walk_expr(c, out);
            for s in then_b { walk_stmt(s, out); }
            if let Some(eb) = else_b {
                for s in eb { walk_stmt(s, out); }
            }
        }
        For(_, _, iter, body, key, _) => {
            walk_expr(iter, out);
            walk_expr(body, out);
            if let Some(k) = key { walk_expr(k, out); }
        }
        Lambda(_, _, body, _, _) => walk_expr(body, out),
        Block(stmts, _) => {
            for s in stmts { walk_stmt(s, out); }
        }
        Match(scrut, arms, _) => {
            walk_expr(scrut, out);
            // HirMatchArm holds a body — read its definition in stmt_expr.rs and
            // recurse into the arm body (expr or Vec<HirStmt>). Add a unit test
            // proving a call inside a match arm is found.
            let _ = arms;
        }
        // JSX / Async / Try carry nested exprs in their wrapper structs
        // (HirJsxElement.children, HirAsyncView arms, HirTry.target). These are
        // gui/async-tier; recurse by reading the three struct defs. The corpus
        // sweep (Task 11) will surface any missed call site as a fixture finding.
        Jsx(_) | JsxSelfClosing(_) | AsyncView(_) | Try(_) => {}
    }
}

fn walk_arg(a: &HirArg, out: &mut Vec<String>) {
    walk_expr(&a.value, out);
}

fn walk_stmt(s: &HirStmt, out: &mut Vec<String>) {
    use HirStmt::*;
    match s {
        Expr { expr, .. } => walk_expr(expr, out),
        Let { value, .. } => walk_expr(value, out),
        Assign { target, value, .. } => {
            walk_expr(target, out);
            walk_expr(value, out);
        }
        Return { value, .. } => {
            if let Some(v) = value { walk_expr(v, out); }
        }
        While { condition, body, .. } => {
            walk_expr(condition, out);
            for s in body { walk_stmt(s, out); }
        }
        Loop { body, .. } => {
            for s in body { walk_stmt(s, out); }
        }
        Break { .. } | Continue { .. } => {}
    }
}

/// Names of functions called anywhere in a body.
#[must_use]
pub fn callee_names(body: &[HirStmt]) -> Vec<String> {
    let mut out = Vec::new();
    for s in body { walk_stmt(s, &mut out); }
    out
}
```

> The `Match` arm and `Jsx/Async/Try` are marked for follow-up with explicit anchors — close them by reading `HirMatchArm`, `HirJsxElement`, `crate::hir::nodes::async_view::HirAsyncView`, and `HirTry`, then add a test per case. Do this before declaring the walker complete.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-compiler walker_finds_calls_in_nested_positions`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-compiler/src/typeck/placement.rs
git commit -m "feat(placement): exhaustive call-graph walker"
```

---

## Task 5: Propagation to a fixed point (`PlacementMap::infer`)

**Files:**
- Modify: `crates/vox-compiler/src/typeck/placement.rs`
- Test: in `placement.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn caller_of_native_becomes_native() {
    // `wrapper` is unannotated (seeds Shared) but calls `read_db` (native).
    let m = hir_of("fn read_db() uses db { 0 }\nfn wrapper() { read_db() }");
    let map = PlacementMap::infer(&m);
    assert_eq!(map.get("wrapper"), Some(Placement::Native));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-compiler caller_of_native_becomes_native`
Expected: FAIL — `no function infer`.

- [ ] **Step 3: Implement `infer` + `set_if_stronger`**

```rust
impl PlacementMap {
    /// Seed, then propagate callee constraints to a fixed point. A Shared
    /// declaration that calls a specialized declaration adopts that
    /// specialization. (Native+Gui contradictions are left to the conflict
    /// pass in Task 6 so it can report both pulls.)
    #[must_use]
    pub fn infer(m: &HirModule) -> PlacementMap {
        let mut map = PlacementMap::seed(m);
        let edges: Vec<(String, Vec<String>)> = m
            .functions
            .iter()
            .map(|f| (f.name.clone(), callee_names(&f.body)))
            .collect();

        let mut changed = true;
        while changed {
            changed = false;
            for (caller, callees) in &edges {
                for callee in callees {
                    if let Some(cp) = map.get(callee) {
                        if cp != Placement::Shared && map.set_if_stronger(caller, cp) {
                            changed = true;
                        }
                    }
                }
            }
        }
        map
    }

    /// Promote `name` from Shared to a specialization. Returns true if it moved.
    fn set_if_stronger(&mut self, name: &str, p: Placement) -> bool {
        let cur = self.0.get(name).copied().unwrap_or(Placement::Shared);
        if cur == Placement::Shared && p != Placement::Shared {
            self.0.insert(name.to_string(), p);
            true
        } else {
            false
        }
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-compiler caller_of_native_becomes_native`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-compiler/src/typeck/placement.rs
git commit -m "feat(placement): call-graph propagation to fixed point"
```

---

## Task 6: Pass entry `infer()` + conflict detection (`E-PLACE-CONFLICT`)

**Files:**
- Modify: `crates/vox-compiler/src/typeck/placement.rs`
- Test: in `placement.rs`

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn native_fn_calling_gui_is_conflict() {
    // hybrid seeds native (uses db) but calls Widget (gui).
    let m = hir_of("fn hybrid() uses db { Widget() }\n@reactive fn Widget() { 0 }");
    let diags = infer(&m, "");
    assert!(diags.iter().any(|d| d.code.as_deref() == Some(codes::PLACEMENT_CONFLICT)));
}

#[test]
fn shared_fn_calling_native_is_not_conflict() {
    let m = hir_of("fn read_db() uses db { 0 }\nfn wrapper() { read_db() }");
    let diags = infer(&m, "");
    assert!(diags.iter().all(|d| d.code.as_deref() != Some(codes::PLACEMENT_CONFLICT)));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p vox-compiler native_fn_calling_gui_is_conflict`
Expected: FAIL — free function `infer` not defined.

- [ ] **Step 3: Implement the pass entry `infer`**

```rust
use crate::typeck::diagnostics::{codes, Diagnostic};

/// Placement pass entry point wired into typeck. Emits conflict diagnostics
/// (boundary + override added in Tasks 7 & 9).
#[must_use]
pub fn infer(m: &HirModule, source: &str) -> Vec<Diagnostic> {
    let map = PlacementMap::infer(m);
    let mut diags = Vec::new();

    for f in &m.functions {
        let own = seed_fn(f);
        for callee in callee_names(&f.body) {
            if let Some(cp) = map.get(&callee) {
                let incompatible = matches!(
                    (own, cp),
                    (Placement::Native, Placement::Gui) | (Placement::Gui, Placement::Native)
                );
                if incompatible {
                    diags.push(
                        Diagnostic::error(
                            format!(
                                "`{}` is {:?}-placed but calls `{}` which is {:?}-placed — split the function or cross via an endpoint",
                                f.name, own, callee, cp
                            ),
                            f.span,
                            source,
                        )
                        .with_code(codes::PLACEMENT_CONFLICT)
                        .with_suggestion(format!(
                            "extract the {cp:?}-tier work, or wrap `{callee}` in `@query fn` and call across the boundary"
                        )),
                    );
                }
            }
        }
    }
    diags
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vox-compiler native_fn_calling_gui_is_conflict shared_fn_calling_native_is_not_conflict`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-compiler/src/typeck/placement.rs
git commit -m "feat(placement): E-PLACE-CONFLICT for native<->gui pulls"
```

---

## Task 7: Boundary check — `E-PLACE-BOUNDARY` (direct gui→native call)

**Files:**
- Modify: `crates/vox-compiler/src/typeck/placement.rs`
- Test: in `placement.rs`

Endpoints (`hir.endpoint_fns`) are the legal crossing and are exempt as callees.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn gui_calling_native_nonendpoint_is_boundary_error() {
    let m = hir_of("@reactive fn View() { read_db() }\nfn read_db() uses db { 0 }");
    let diags = infer(&m, "");
    assert!(diags.iter().any(|d| d.code.as_deref() == Some(codes::PLACEMENT_BOUNDARY)));
}

#[test]
fn gui_calling_endpoint_is_allowed() {
    let m = hir_of("@reactive fn View() { list_tasks() }\n@query fn list_tasks() { 0 }");
    let diags = infer(&m, "");
    assert!(diags.iter().all(|d| d.code.as_deref() != Some(codes::PLACEMENT_BOUNDARY)));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p vox-compiler gui_calling_native_nonendpoint_is_boundary_error`
Expected: FAIL — no boundary diagnostic emitted.

- [ ] **Step 3: Extend `infer` with the boundary check**

Append inside `infer`, before `diags` is returned:

```rust
    let endpoint_names: std::collections::HashSet<&str> =
        m.endpoint_fns.iter().map(|e| e.name.as_str()).collect();

    for f in &m.functions {
        if map.get(&f.name) != Some(Placement::Gui) {
            continue;
        }
        for callee in callee_names(&f.body) {
            let callee_native = map.get(&callee) == Some(Placement::Native);
            if callee_native && !endpoint_names.contains(callee.as_str()) {
                diags.push(
                    Diagnostic::error(
                        format!(
                            "GUI function `{}` calls native function `{}` directly — cross the boundary via an endpoint",
                            f.name, callee
                        ),
                        f.span,
                        source,
                    )
                    .with_code(codes::PLACEMENT_BOUNDARY)
                    .with_suggestion(format!(
                        "wrap `{callee}` in `@query fn` (or `@server`/`@mutation`) and call the generated client"
                    )),
                );
            }
        }
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vox-compiler gui_calling_native_nonendpoint_is_boundary_error gui_calling_endpoint_is_allowed`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-compiler/src/typeck/placement.rs
git commit -m "feat(placement): E-PLACE-BOUNDARY for direct gui->native calls"
```

---

## Task 8: Wire the pass into typeck (end-to-end)

**Files:**
- Modify: `crates/vox-compiler/src/typeck/mod.rs` (passes vec, :133)
- Test: `crates/vox-compiler/tests/placement_pass_wired.rs` (new)

- [ ] **Step 1: Write the failing integration test**

```rust
//! Proves the placement pass runs inside typeck end-to-end.
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;
use vox_compiler::typeck::typecheck_hir_module;

fn codes_for(src: &str) -> Vec<String> {
    let mut hir = lower_module(&parse(lex(src)).expect("parse"));
    typecheck_hir_module(src, &mut hir)
        .into_iter()
        .filter_map(|d| d.code)
        .collect()
}

#[test]
fn gui_calling_native_directly_is_rejected_by_typeck() {
    let codes = codes_for("@reactive fn View() { read_db() }\nfn read_db() uses db { 0 }");
    assert!(
        codes.iter().any(|c| c == "vox/placement/boundary"),
        "expected placement boundary diagnostic; got: {codes:?}"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-compiler --test placement_pass_wired`
Expected: FAIL — no boundary code (pass not wired).

- [ ] **Step 3: Wire the pass**

In `crates/vox-compiler/src/typeck/mod.rs`, add to the `passes` vec (after the `effect_check` closure, ~:134), matching the sibling closure form exactly:

```rust
        Box::new(|| placement::infer(hir, source)),
```

(`hir` is captured as `&HirModule` in this block — same as `effect_check::check_effect_compliance(hir, source)`.)

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-compiler --test placement_pass_wired`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-compiler/src/typeck/mod.rs crates/vox-compiler/tests/placement_pass_wired.rs
git commit -m "feat(placement): wire placement pass into typeck"
```

---

## Task 9: `@place(...)` override + satisfiability (`E-PLACE-UNSAT`)

**Files:**
- Modify: `crates/vox-ast/src/decl/fundecl.rs` (define `PlacementHint`; add field to `FnDecl`)
- Modify: `crates/vox-compiler/src/hir/nodes/decl.rs` (add field to `HirFn`)
- Modify: `crates/vox-compiler/src/hir/lower/decl.rs` (copy field, :53–63 block)
- Modify: `crates/vox-compiler/src/lexer/token.rs` (`@place` token + Display)
- Modify: `crates/vox-compiler/src/parser/descent/decl/head_fn.rs` (parse `@place(...)`)
- Modify: `crates/vox-compiler/src/typeck/placement.rs` (honor + verify override)
- Test: parser test + `placement.rs` unit tests

`PlacementHint` lives in `vox-ast` so both `FnDecl` and `HirFn` (lowered verbatim) share one type. `placement.rs` re-exports it.

- [ ] **Step 1: Define `PlacementHint` and thread the field (no behavior yet)**

In `crates/vox-ast/src/decl/fundecl.rs`, define near the top:

```rust
/// Explicit `@place(...)` tier override.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PlacementHint { Shared, Native, Gui }
```

Add to `FnDecl` (mirror `is_mobile_native`'s position): `pub placement_override: Option<PlacementHint>,` and set it to `None` everywhere `FnDecl` is constructed (the compiler will list those sites).

Add to `HirFn` (`crates/vox-compiler/src/hir/nodes/decl.rs`, near `is_mobile_native`):

```rust
    /// Explicit `@place(...)` override; `None` = inferred.
    #[serde(default)]
    pub placement_override: Option<vox_ast::decl::fundecl::PlacementHint>,
```

(Confirm the exact `vox_ast` path with `grep -rn "pub enum FnDecl\|pub struct FnDecl" crates/vox-ast/`.) In `crates/vox-compiler/src/hir/lower/decl.rs` line 53–63 block add: `placement_override: f.placement_override,`.

In `placement.rs` add: `pub use vox_ast::decl::fundecl::PlacementHint;` and a mapping:

```rust
fn hint_to_placement(h: PlacementHint) -> Placement {
    match h {
        PlacementHint::Shared => Placement::Shared,
        PlacementHint::Native => Placement::Native,
        PlacementHint::Gui => Placement::Gui,
    }
}
```

Run: `cargo build -p vox-compiler` — must compile (field added, defaulted everywhere).

- [ ] **Step 2: Write the failing satisfiability tests**

In `placement.rs` tests:

```rust
#[test]
fn place_gui_on_db_fn_is_unsat() {
    let m = hir_of("@place(gui) fn bad() uses db { 0 }");
    let diags = check_override(&m.functions[0], "");
    assert!(diags.iter().any(|d| d.code.as_deref() == Some(codes::PLACEMENT_UNSAT)));
}

#[test]
fn place_native_is_always_sat() {
    let m = hir_of("@place(native) fn ok() uses db { 0 }");
    assert!(check_override(&m.functions[0], "").is_empty());
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p vox-compiler place_gui_on_db_fn_is_unsat`
Expected: FAIL — `check_override` missing AND `@place` not parsed yet (so `placement_override` is `None`).

- [ ] **Step 4: Add the `@place` token + parser**

In `token.rs`, near the other `At*` tokens:

```rust
    #[token("@place")]
    AtPlace,
```

and a `Display` arm: `Token::AtPlace => write!(f, "@place"),`.

In `head_fn.rs`: first split the existing combined arm at :119–122 so `@native` is on its own (Task 11 removes it):

```rust
Token::AtFuzz => {
    self.advance();
    is_mobile_native = true;
}
```

Then add a `@place` arm, modeled on `@require` (`head_fn.rs:68-73`):

```rust
Token::AtPlace => {
    self.advance();
    self.expect(&Token::LParen)?;
    if let Token::Ident(p) = self.peek().clone() {
        self.advance();
        placement_override = Some(match p.as_str() {
            "native" => PlacementHint::Native,
            "gui" => PlacementHint::Gui,
            _ => PlacementHint::Shared,
        });
    }
    self.expect(&Token::RParen)?;
}
```

Declare `let mut placement_override: Option<PlacementHint> = None;` alongside the other decorator-flag locals (near `is_mobile_native`'s declaration in this function), `use vox_ast::decl::fundecl::PlacementHint;`, and add `placement_override,` to the `FnDecl { … }` construction in this file.

- [ ] **Step 5: Add a parser test**

Create/extend a parser test (follow a sibling test in `crates/vox-compiler/tests/` that lowers a string and inspects `hir.functions`):

```rust
#[test]
fn parses_place_native_override() {
    use vox_ast::decl::fundecl::PlacementHint;
    let m = lower_module(&parse(lex("@place(native) fn f() { 0 }")).unwrap());
    assert_eq!(m.functions[0].placement_override, Some(PlacementHint::Native));
}
```

Run: `cargo test -p vox-compiler parses_place_native_override`
Expected: PASS.

- [ ] **Step 6: Implement `check_override`, honor override in `seed_fn`, call from `infer`**

In `placement.rs`:

```rust
/// Verify an explicit `@place(...)` override is satisfiable given effects.
#[must_use]
pub fn check_override(f: &HirFn, source: &str) -> Vec<Diagnostic> {
    let Some(hint) = f.placement_override else { return Vec::new() };
    let needs_native = f.capabilities.iter().any(is_native_capability)
        || f.is_remote || f.is_llm || f.durability.is_some();
    let unsat = match hint {
        PlacementHint::Native => false, // native is always satisfiable
        PlacementHint::Gui | PlacementHint::Shared => needs_native,
    };
    if unsat {
        vec![Diagnostic::error(
            format!("`@place({hint:?})` on `{}` is unsatisfiable — it uses native-only effects", f.name),
            f.span,
            source,
        )
        .with_code(codes::PLACEMENT_UNSAT)
        .with_suggestion("remove the override, use @place(native), or route the effect through an endpoint")]
    } else {
        Vec::new()
    }
}
```

At the top of `seed_fn`, honor the override first:

```rust
    if let Some(h) = f.placement_override {
        return hint_to_placement(h);
    }
```

In `infer`, after the existing loops, append override diagnostics:

```rust
    for f in &m.functions {
        diags.extend(check_override(f, source));
    }
```

- [ ] **Step 7: Run tests to verify they pass**

Run: `cargo test -p vox-compiler place_gui_on_db_fn_is_unsat place_native_is_always_sat parses_place_native_override`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/vox-ast/src/decl/fundecl.rs crates/vox-compiler/src/hir/nodes/decl.rs crates/vox-compiler/src/hir/lower/decl.rs crates/vox-compiler/src/lexer/token.rs crates/vox-compiler/src/parser/descent/decl/head_fn.rs crates/vox-compiler/src/typeck/placement.rs
git commit -m "feat(placement): @place(...) override + E-PLACE-UNSAT satisfiability"
```

---

## Task 10: Feature-matrix integration — rename `Native`→`Place`, parity test

**Files:**
- Modify: `crates/vox-compiler/src/feature_matrix.rs`
- Test: in `feature_matrix.rs` test module

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn place_decorator_registered_and_native_gone() {
    let _ = support(Feature::Decorator(DecoratorFeature::Place), Target::TypeScript);
    assert_eq!(DecoratorFeature::Place.lexer_spelling(), "@place");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-compiler place_decorator_registered_and_native_gone`
Expected: FAIL — `no variant Place`.

- [ ] **Step 3: Rename the variant**

Rename `Native`→`Place` in: the enum (`:135`), the `ALL` array (`:303`), the `lexer_spelling` match (`Native => "@native"` → `Place => "@place"`), and any other match arm the compiler flags. **Run `cargo build -p vox-compiler` and fix each "no variant named Native" / non-exhaustive error** — the rename is compiler-guided. Count stays **56** (swap, not add); `feature_all_has_expected_count` must still pass.

`Place` is not in `is_ladder_proven_decorator`, so it routes to `unverified()` until sub-spec 3 adds a ladder fixture (decision 5B). That is intentional and consistent with other new decorators.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vox-compiler -- feature_matrix`
Expected: PASS (`feature_all_has_expected_count`, `matrix_is_total`, the new test).

- [ ] **Step 5: Add the placement↔matrix parity test**

```rust
#[test]
fn placement_matches_feature_matrix_frontend_backend() {
    // JSX is gui-only (unsupported on Rust); Spawn is native-only (unsupported on TS).
    assert!(matches!(
        support(Feature::Expr(ExprFeature::Jsx), Target::RustAxum),
        Support::Unsupported(_)
    ));
    assert!(matches!(
        support(Feature::Expr(ExprFeature::Spawn), Target::TypeScript),
        Support::Unsupported(_)
    ));
}
```

Run: `cargo test -p vox-compiler placement_matches_feature_matrix_frontend_backend`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-compiler/src/feature_matrix.rs
git commit -m "feat(placement): rename @native->@place in feature matrix; parity test"
```

---

## Task 11: Retire `@native` everywhere + corpus sweep (decisions A + B)

**Files:**
- Modify: `crates/vox-compiler/src/lexer/token.rs`, `parser/descent/decl/head_fn.rs`, `language_surface.rs`, `lexer/cursor.rs`
- Modify: `crates/vox-code-audit/src/detectors/retired_decorator.rs`
- Modify: `AGENTS.md`

- [ ] **Step 1: Write the failing detector test**

In `retired_decorator.rs` test module (using the existing `source(code)` helper):

```rust
#[test]
fn flags_retired_native_decorator() {
    let d = RetiredDecoratorDetector::new();
    let f = source("@native fn perf() { 0 }");
    let findings = d.detect(&f, None);
    assert!(findings.iter().any(|x| x.message.contains("@native")
        && x.suggestion.as_deref().unwrap_or("").contains("@place(native)")));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-code-audit flags_retired_native_decorator`
Expected: FAIL.

- [ ] **Step 3: Add `@native` to the retired-decorator detector**

Add a `native_decorator: Regex` field; init `Regex::new(r"@native\b").expect("valid regex")` in `new()`; in `detect()`, add a `build_finding(...)` call with **`Severity::Error`** (spec decision B; existing entries use `Warning` — `@native` is intentionally stricter), message `"Retired form `@native` — use `@place(native)` instead."`, suggestion containing `@place(native)`, and an `AGENTS.md §Retired Surfaces` rationale.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-code-audit flags_retired_native_decorator`
Expected: PASS.

- [ ] **Step 5: Remove the `@native` token + surfaces**

- `token.rs`: delete `#[token("@native")] AtNative,` and `Token::AtNative => write!(f, "@native"),`.
- `head_fn.rs`: the `AtFuzz`/`AtNative` arm was already split in Task 9 Step 4; now delete the standalone `Token::AtNative => { … is_mobile_native = true; }` arm entirely.
- `language_surface.rs`: in `LEXER_DECORATORS` replace `"@native"` with `"@place"`.
- `cursor.rs:253`: change the lex-test string from `… @native` to `… @place` and update the expected token assertion to `Token::AtPlace`.

Run: `cargo build -p vox-compiler`
Expected: compiles — no `AtNative`/`Native` references remain.

- [ ] **Step 6: Update AGENTS.md Retired Surfaces table**

After the `@py.import` row (~:407) add:

```markdown
| `@native` (decorator) | `@place(native)` |
```

- [ ] **Step 7: Corpus sweep + triage (parallel sub-agents — decision B)**

Dispatch parallel sub-agents (one per cluster: `examples/golden/**`, `crates/vox-codegen/tests/`, ladder fixtures in `contracts/pipeline/canonical-ladder.v1.yaml`). Each runs the corpus through the new pass and triages every new placement diagnostic:
- **True positive** (real cross-tier bug) → fix the `.vox` fixture.
- **False positive** (inference wrong) → fix the rule in `placement.rs`, add a regression unit test; do NOT edit the fixture to silence it.
- **False-negative watch** → for each fixture with a known legitimate cross-tier call, confirm the pass fired where expected.

Driver command per cluster: `cargo test -p vox-compiler` plus golden checks (`cargo run -q -p vox-cli -- check <fixture>.vox`). Milestone = **green corpus under blocking placement diagnostics**.

- [ ] **Step 8: Commit**

```bash
git add crates/vox-compiler/src/lexer/token.rs crates/vox-compiler/src/parser/descent/decl/head_fn.rs crates/vox-compiler/src/language_surface.rs crates/vox-compiler/src/lexer/cursor.rs crates/vox-code-audit/src/detectors/retired_decorator.rs AGENTS.md examples/ crates/vox-codegen/tests/
git commit -m "feat(placement): retire @native token; corpus green under placement gate"
```

---

## Task 12: Re-export the public placement API (seam for sub-specs 2/3)

**Files:**
- Modify: `crates/vox-compiler/src/lib.rs`
- Test: `crates/vox-compiler/tests/placement_public_api.rs` (new)

- [ ] **Step 1: Write the failing test**

```rust
use vox_compiler::{Placement, PlacementMap};

#[test]
fn placement_types_are_public() {
    let _ = Placement::Shared;
    fn _takes(_m: &PlacementMap) {}
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-compiler --test placement_public_api`
Expected: FAIL — not re-exported at crate root.

- [ ] **Step 3: Re-export from lib.rs**

```rust
pub use typeck::placement::{Placement, PlacementMap};
pub use vox_ast::decl::fundecl::PlacementHint;
```

Ensure `Placement` and `PlacementMap` are `pub` in `placement.rs` (they are).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-compiler --test placement_public_api`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-compiler/src/lib.rs crates/vox-compiler/tests/placement_public_api.rs
git commit -m "feat(placement): re-export Placement/PlacementMap for downstream sub-specs"
```

---

## Final verification

- [ ] `cargo test -p vox-compiler` — all placement unit + integration tests green.
- [ ] `cargo test -p vox-code-audit retired_decorator` — `@native` flagged at Error.
- [ ] `cargo build -p vox-compiler` — no `AtNative`/`DecoratorFeature::Native` references remain.
- [ ] `cargo run -q -p vox-arch-check` — no new layer violations (note: `vox-compiler` now references `vox-ast::…::PlacementHint`, which is already an allowed dependency direction).
- [ ] `cargo clippy -p vox-compiler -p vox-ast -p vox-code-audit -- -D warnings` (pre-push clippy-gap, per AGENTS.md).
- [ ] `vox run scripts/fmt.vox` (never `cargo fmt --all` on Windows).
- [ ] Confirm the Task 4 walker's `Match` / `Jsx` / `Try` / `AsyncView` follow-ups are closed with tests.
- [ ] Confirm the Task 11 corpus sweep left zero unresolved placement diagnostics.

## Notes for the implementer

- **Effect data is the source of truth.** Placement is only as accurate as `HirFn.capabilities` / `is_pure` / `is_reactive` / `durability`. A missing capability on a function that clearly does I/O is an upstream effect-inference gap — note it, don't paper over it in placement.
- **Build HIR from source in tests** (the `hir_of` helper) — never hand-construct `HirFn` (no `Default`, 30+ fields). If a fixture won't parse, copy the shape from a real `examples/golden/*.vox`.
- **The walker is a correctness invariant.** Close the `Match`/`Jsx`/`Try`/`AsyncView` arms (Task 4) before claiming completeness; each needs a test proving a call inside it is found.
- **Names vs DefIds.** `PlacementMap` is name-keyed for simplicity. If the corpus sweep surfaces shadowing/overload collisions, switch the key to `HirFn.id: DefId` — a localized change.
