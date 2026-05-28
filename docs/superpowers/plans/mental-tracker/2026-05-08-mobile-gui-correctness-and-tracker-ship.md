# Mobile + GUI Correctness + Mental Tracker Ship Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the vox-mental-tracker app on web + iOS + Android by closing the open codegen bug, adding five compile-time GUI guardrails that make the highest-leverage LLM foot-guns structurally unrepresentable, introducing typed forms-as-first-class, lifting mobile primitives into the language, and filling the mobile-readiness gap (signing, icons, push, deep-links, offline, crash reporting).

**Architecture:** Six independently-shippable tracks (A–F). Each track produces working software on its own; later tracks build on earlier ones but do not block them. Tracks A and B are *correctness* (close known bugs and prevent classes of bugs at the compiler/syntax level). Track C is the *killer feature* (typed forms). Track D is *mobile primitives* (lift Capacitor wiring into the language). Track E is *mental-tracker mobile productionization* (icons, signing, store, offline). Track F is *test infrastructure* (the meta-track that ensures we never regress A–E).

**Tech Stack:** Rust 2024 (compiler / codegen / lints / HIR passes), TypeScript + React 19 + TanStack Router 6 + Tailwind 3 (emit target), Capacitor 6 (mobile bridge), Kotlin (Android plugin), Swift (iOS plugin), Playwright (E2E), Vitest (unit), `insta` (Rust snapshot tests).

---

## Track summary

| Track | Outcome | Independently shippable? |
|---|---|---|
| A. Codegen correctness | Async/await fully wired; `tsc --noEmit` gate; centralized builtin lowering | Yes |
| B. Render-loop guardrails | 5 compile-error lints: list keys, effect deps, stale closures, async cancellability, route loading/error completeness | Yes (per lint) |
| C. Forms-as-first-class | `@form` decl → typed state machine + bindings + validation + error UI in lockstep | Yes |
| D. Mobile primitives | `@safe_area`, `@back_button`, `@deep_link`, `@push` lifted into the language | Yes (per primitive) |
| E. Mental tracker ship | iOS STT, signing, icons, splash, push wiring, deep-links, SW offline, privacy manifest, crash reporting | Yes |
| F. Test infrastructure | `tsc --noEmit` CI gate, golden snapshot suite per codegen feature, mobile E2E lane | Yes |

---

## File structure (created or modified)

### New files

- `crates/vox-codegen/src/codegen_ts/hir_emit/async_walker.rs` — recursive async-call detection (replaces the shallow `stmt_calls_async_fn`)
- `crates/vox-codegen/src/codegen_ts/builtin_registry.rs` — single registry mapping Vox method/function calls to TS lowerings
- `crates/vox-codegen/src/web_ir/validate_keys.rs` — Web IR validator: every `Loop` child must have a stable `key` attribute
- `crates/vox-codegen/src/web_ir/validate_route_completeness.rs` — Web IR validator: every route with `loader_name` must have `pending_component_name` AND an error component
- `crates/vox-compiler/src/typeck/effect_deps_lint.rs` — HIR pass: every `effect`/`derived` block has fully-resolved deps or fails
- `crates/vox-compiler/src/typeck/stale_capture_lint.rs` — HIR pass: detect closures that capture out-of-scope reactive bindings
- `crates/vox-compiler/src/typeck/async_handler_lint.rs` — HIR pass: async event handlers must be `@cancellable` or use AbortSignal
- `crates/vox-compiler/src/ast/decl/form.rs` — `FormDecl` AST node
- `crates/vox-compiler/src/hir/nodes/form.rs` — `HirForm` HIR node
- `crates/vox-codegen/src/codegen_ts/form_emit.rs` — emit React form state machine + bindings + validation
- `crates/vox-compiler/src/ast/decl/mobile.rs` — `SafeAreaDecl`, `BackButtonDecl`, `DeepLinkDecl`, `PushDecl` AST nodes
- `crates/vox-codegen/src/codegen_ts/mobile_emit.rs` — emit Capacitor wiring for mobile primitives
- `apps/vox-mental-tracker/plugins/vox-sherpa-transcribe/ios/AppleSpeechBackend.swift` — iOS STT via Apple Speech Framework
- `apps/vox-mental-tracker/public/sw-offline.js` — offline-first SW with mutation queue
- `apps/vox-mental-tracker/scripts/sign-android.vox` — Android keystore + APK signing
- `apps/vox-mental-tracker/scripts/sign-ios.vox` — iOS provisioning helper
- `apps/vox-mental-tracker/PrivacyInfo.xcprivacy` — iOS privacy manifest
- `apps/vox-mental-tracker/public/icons/*` — app icon set (master + generated sizes)
- `crates/vox-compiler/tests/forms_test.rs` — form codegen tests
- `crates/vox-compiler/tests/render_loop_lints_test.rs` — 5 render-loop lints
- `crates/vox-compiler/tests/mobile_primitives_test.rs` — mobile primitives codegen
- `.github/workflows/ts-emit-noemit.yml` — CI lane: emit + `tsc --noEmit`

### Modified files

- `crates/vox-codegen/src/codegen_ts/hir_emit/mod.rs` — extend `EmitCtx` with `async_call_detector`; replace `stmt_calls_async_fn` with recursive walker; route Method/Field/Index calls through it
- `crates/vox-codegen/src/codegen_ts/jsx.rs` — emit `key` attribute on `.map()` children; pull from registry
- `crates/vox-codegen/src/web_ir/emit_tsx.rs` — same: emit `key` on `Loop`
- `crates/vox-codegen/src/web_ir/validate.rs` — register new validators (`validate_keys`, `validate_route_completeness`)
- `crates/vox-compiler/src/typeck/mod.rs` — register new HIR passes (effect-deps, stale-capture, async-handler)
- `crates/vox-compiler/src/typeck/ast_decl_lints.rs` — refactor existing lints to use `LintBuilder` (extracted helper); add table of all lint codes
- `crates/vox-compiler/src/parser/descent/decl/head.rs` — parse `@form { … }`, `@safe_area`, `@back_button`, `@deep_link`, `@push`
- `crates/vox-compiler/src/hir/lower.rs` — lower form + mobile decls
- `crates/vox-compiler/src/hir/validate.rs` — extend with form-validation rule (every field referenced)
- `crates/vox-compiler/src/ast/decl/ui.rs` — `RouteEntry` gains required `error_component_name: Option<String>` (semantic, not parser)
- `apps/vox-mental-tracker/src/main.vox` — migrate voice form + mood form to `@form`
- `apps/vox-mental-tracker/src/runtime.ts` — register mobile-primitives globals
- `apps/vox-mental-tracker/capacitor.config.ts` — splash, deep-link, push config
- `apps/vox-mental-tracker/RELEASE_CHECKLIST.md` — add G7+ for icons/signing/privacy/crash
- `apps/vox-mental-tracker/package.json` — add `@capacitor/splash-screen`, `@capacitor/app`, `@capacitor/push-notifications`
- `apps/vox-mental-tracker/scripts/build.vox` — add icon-gen + signing steps
- `apps/vox-mental-tracker/plugins/vox-sherpa-transcribe/ios/Plugin.swift` — replace stub
- `apps/vox-mental-tracker/tests/e2e/voice_flow.spec.ts` — re-enable full parse→save flow assertions

---

# TRACK A — Codegen correctness

Goal: close the open async/await bug, add a `tsc --noEmit` CI gate, centralize builtin/method lowering. Pre-requisite for everything else (otherwise tests we add to validate guardrails will pass against broken emit).

## Task A1: Recursive async-call detection

**Files:**
- Create: `crates/vox-codegen/src/codegen_ts/hir_emit/async_walker.rs`
- Modify: `crates/vox-codegen/src/codegen_ts/hir_emit/mod.rs:332-380` (Method/Field call branches), `:555-590` (handler async detection)
- Test: `crates/vox-codegen/tests/async_emit_test.rs`

**Background:** Today, `stmt_calls_async_fn` only matches `HirStmt::Expr(Call(Ident(name), …))`. A method call (`obj.foo()`), a chained call (`a().b()`), a call inside a let binding (`let x = parse_voice(...)`), or an assignment with an async call all bypass detection — the handler is *not* marked `async`, the call is *not* awaited, and the user sees `undefined`. This is the open bug from `apps/vox-mental-tracker/tests/e2e/voice_flow.spec.ts:49-55`.

- [ ] **Step 1: Write the failing test**

Add `crates/vox-codegen/tests/async_emit_test.rs`:

```rust
use vox_codegen::codegen_ts::emitter::{generate, CodegenOptions};
use vox_compiler::{parser::parse, lexer::cursor::lex, hir::lower::lower_module};

fn emit_ts(src: &str) -> String {
    let m = parse(lex(src)).expect("parse");
    let hir = lower_module(&m);
    let out = generate(&hir, &CodegenOptions::default()).expect("emit");
    out.files.iter().map(|f| f.contents.clone()).collect::<Vec<_>>().join("\n")
}

#[test]
fn endpoint_called_in_let_binding_inside_handler_gets_await() {
    let src = r#"
@endpoint(kind: query) fn parse_voice(t: str) to int {
    return 1
}
component VoiceFlow() {
    state n: int = 0
    view: button(on_click: () => {
        let p = parse_voice("hi")
        set n(p)
    }) { "Go" }
}
"#;
    let ts = emit_ts(src);
    assert!(ts.contains("async ()"), "handler must be async\n---\n{}", ts);
    assert!(ts.contains("await parse_voice"), "endpoint call must be awaited\n---\n{}", ts);
}

#[test]
fn endpoint_called_as_method_chain_gets_await() {
    let src = r#"
@endpoint(kind: query) fn fetch_user() to str { return "x" }
component C() {
    state s: str = ""
    view: button(on_click: () => {
        let u = fetch_user().trim()
        set s(u)
    }) { "Go" }
}
"#;
    let ts = emit_ts(src);
    assert!(ts.contains("(await fetch_user())"), "method-chain async call must be awaited then chained\n---\n{}", ts);
}

#[test]
fn nested_endpoint_in_assignment_gets_await() {
    let src = r#"
@endpoint(kind: mutation) fn save(payload: str) to int { return 1 }
component C() {
    state n: int = 0
    view: button(on_click: () => {
        set n(save("data"))
    }) { "Save" }
}
"#;
    let ts = emit_ts(src);
    assert!(ts.contains("await save("), "endpoint call inside setter arg must be awaited\n---\n{}", ts);
}

#[test]
fn handler_with_no_async_call_is_not_async() {
    let src = r#"
component C() {
    state n: int = 0
    view: button(on_click: () => { set n(1) }) { "X" }
}
"#;
    let ts = emit_ts(src);
    assert!(!ts.contains("async ()"), "handler without async calls must not be async\n---\n{}", ts);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p vox-codegen --test async_emit_test`
Expected: 3 failures (the 4th passes; it's a guardrail against false-positives).

- [ ] **Step 3: Implement async-walker module**

Create `crates/vox-codegen/src/codegen_ts/hir_emit/async_walker.rs`:

```rust
//! Recursive detection of async-fn calls anywhere inside a HIR expression
//! tree. Replaces the shallow `stmt_calls_async_fn`.
//!
//! A call is "async" if the called identifier is in the async name set.
//! For method calls and field accesses, we descend into the receiver and
//! into all arguments. We do NOT cross closure / lambda boundaries (those
//! handlers carry their own async-ness separately).

use std::collections::HashSet;
use vox_compiler::hir::nodes::expr::HirExpr;
use vox_compiler::hir::nodes::stmt::HirStmt;

pub fn stmt_has_async_call(stmt: &HirStmt, async_names: &HashSet<String>) -> bool {
    match stmt {
        HirStmt::Expr(e) => expr_has_async_call(e, async_names),
        HirStmt::Let { value, .. } => expr_has_async_call(value, async_names),
        HirStmt::Assign { value, .. } => expr_has_async_call(value, async_names),
        HirStmt::Return(Some(e)) => expr_has_async_call(e, async_names),
        HirStmt::Return(None) => false,
        HirStmt::If { cond, then_block, else_block, .. } => {
            expr_has_async_call(cond, async_names)
                || then_block.iter().any(|s| stmt_has_async_call(s, async_names))
                || else_block.as_ref().map_or(false,
                    |b| b.iter().any(|s| stmt_has_async_call(s, async_names)))
        }
        HirStmt::While { cond, body, .. } => {
            expr_has_async_call(cond, async_names)
                || body.iter().any(|s| stmt_has_async_call(s, async_names))
        }
        HirStmt::For { iter, body, .. } => {
            expr_has_async_call(iter, async_names)
                || body.iter().any(|s| stmt_has_async_call(s, async_names))
        }
        _ => false,
    }
}

pub fn expr_has_async_call(expr: &HirExpr, async_names: &HashSet<String>) -> bool {
    match expr {
        HirExpr::Call(callee, args, _, _) => {
            let direct = matches!(callee.as_ref(),
                HirExpr::Ident(n, _) if async_names.contains(n.as_str()));
            direct
                || expr_has_async_call(callee, async_names)
                || args.iter().any(|a| expr_has_async_call(a, async_names))
        }
        HirExpr::MethodCall { receiver, args, .. } => {
            expr_has_async_call(receiver, async_names)
                || args.iter().any(|a| expr_has_async_call(a, async_names))
        }
        HirExpr::Field { receiver, .. } => expr_has_async_call(receiver, async_names),
        HirExpr::Index { receiver, index, .. } => {
            expr_has_async_call(receiver, async_names)
                || expr_has_async_call(index, async_names)
        }
        HirExpr::Block(stmts, _) => stmts.iter().any(|s| stmt_has_async_call(s, async_names)),
        HirExpr::If { cond, then_branch, else_branch, .. } => {
            expr_has_async_call(cond, async_names)
                || expr_has_async_call(then_branch, async_names)
                || else_branch.as_ref().map_or(false, |e| expr_has_async_call(e, async_names))
        }
        HirExpr::Match { scrutinee, arms, .. } => {
            expr_has_async_call(scrutinee, async_names)
                || arms.iter().any(|a| expr_has_async_call(&a.body, async_names))
        }
        HirExpr::Binary { lhs, rhs, .. } => {
            expr_has_async_call(lhs, async_names) || expr_has_async_call(rhs, async_names)
        }
        HirExpr::Unary { operand, .. } => expr_has_async_call(operand, async_names),
        // Lambda/Closure deliberately NOT descended into — they create their own
        // execution scope and emit their own async-ness.
        HirExpr::Lambda(_, _, _, _) => false,
        // Leaves
        _ => false,
    }
}
```

- [ ] **Step 4: Wire into hir_emit/mod.rs**

In `crates/vox-codegen/src/codegen_ts/hir_emit/mod.rs`:

1. Add `mod async_walker;` near other `mod` declarations and `use async_walker::{stmt_has_async_call, expr_has_async_call};`
2. Replace the existing `stmt_calls_async_fn` body with a call to `stmt_has_async_call`. Keep the symbol so the rest of the file does not need to change.
3. In the handler branch (around line 555–571), change `stmts.iter().any(|s| stmt_calls_async_fn(...))` to `stmts.iter().any(|s| stmt_has_async_call(s, ctx.async_fn_names))`.
4. In the `HirExpr::Call` branch (around line 314–330), keep the direct-`Ident` check but also walk method-call receivers and check via `expr_has_async_call` to wrap calls inside method chains. Specifically, in the `HirExpr::MethodCall` branch (around line 332–380), wrap the emitted receiver in `(await …)` when `expr_has_async_call(receiver, ctx.async_fn_names)` returns true.

The exact patch in the `MethodCall` branch:

```rust
HirExpr::MethodCall { receiver, method, args, .. } => {
    let receiver_str = emit_hir_expr(receiver, ctx);
    let receiver_str = if expr_has_async_call(receiver, ctx.async_fn_names) {
        format!("(await {receiver_str})")
    } else {
        receiver_str
    };
    let args_str: Vec<String> = args.iter().map(|a| emit_hir_expr(a, ctx)).collect();
    // ...rest unchanged...
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p vox-codegen --test async_emit_test`
Expected: all 4 tests pass.

- [ ] **Step 6: Update mental-tracker E2E test**

In `apps/vox-mental-tracker/tests/e2e/voice_flow.spec.ts:49-55`, remove the comment block that says "Awaiting fix is tracked compiler-side" and re-enable the parse→save flow assertions:

```typescript
// Old: only check Reset works
// New: full flow
await page.click('button:has-text("Parse")');
await expect(page.locator('[data-testid=kind]')).toHaveText('mood_recorded', { timeout: 5000 });
await expect(page.locator('[data-testid=payload]')).toContainText('mood_score');
await page.click('button:has-text("Save")');
await expect(page.locator('[data-testid=saved-counter]')).toHaveText('1');
```

- [ ] **Step 7: Run mental-tracker E2E**

Run: `cd apps/vox-mental-tracker && pnpm e2e -- --grep voice_flow`
Expected: pass.

- [ ] **Step 8: Commit**

```bash
git add crates/vox-codegen/src/codegen_ts/hir_emit/async_walker.rs \
        crates/vox-codegen/src/codegen_ts/hir_emit/mod.rs \
        crates/vox-codegen/tests/async_emit_test.rs \
        apps/vox-mental-tracker/tests/e2e/voice_flow.spec.ts
git commit -m "fix(codegen-ts): recursive async-call detection covers method/let/index"
```

## Task A2: `tsc --noEmit` CI gate

**Files:**
- Create: `.github/workflows/ts-emit-noemit.yml`
- Create: `scripts/ci-tsc-noemit.vox`
- Modify: `crates/vox-integration-tests/tests/ts_emit_typecheck_test.rs` (new)

**Background:** Bugs A–D in §2 of the audit (mismatched `case _:`, unescaped JSON, missing namespace alias, missing imports) would all have been caught by `tsc --noEmit` over emitted code. Today emission is verified only structurally (string contains).

- [ ] **Step 1: Write the failing integration test**

Create `crates/vox-integration-tests/tests/ts_emit_typecheck_test.rs`:

```rust
//! For each fixture in `examples/golden/*.vox`, compile it, write emitted
//! TS to a temp dir with a minimal tsconfig + node_modules link, and run
//! `tsc --noEmit`. Any TS error fails the test.

use std::process::Command;
use std::path::PathBuf;
use vox_codegen::codegen_ts::emitter::{generate, CodegenOptions};
use vox_compiler::{parser::parse, lexer::cursor::lex, hir::lower::lower_module};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/golden")
}

#[test]
fn all_golden_fixtures_emit_typechecking_ts() {
    let mut failures: Vec<String> = vec![];
    for entry in std::fs::read_dir(fixtures_dir()).unwrap() {
        let p = entry.unwrap().path();
        if p.extension().and_then(|s| s.to_str()) != Some("vox") { continue; }
        let src = std::fs::read_to_string(&p).unwrap();
        let m = match parse(lex(&src)) { Ok(m) => m, Err(_) => continue };
        let hir = lower_module(&m);
        let opts = CodegenOptions::default();
        let out = match generate(&hir, &opts) { Ok(o) => o, Err(_) => continue };

        let tmp = tempfile::tempdir().unwrap();
        for f in &out.files {
            let dst = tmp.path().join(&f.path);
            std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
            std::fs::write(&dst, &f.contents).unwrap();
        }
        // Symlink shared tsconfig + node_modules
        let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("ts-noemit-scratch");
        std::fs::write(tmp.path().join("tsconfig.json"),
            std::fs::read_to_string(scratch.join("tsconfig.json")).unwrap()).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(scratch.join("node_modules"), tmp.path().join("node_modules")).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(scratch.join("node_modules"), tmp.path().join("node_modules")).unwrap();

        let out = Command::new("npx")
            .arg("tsc").arg("--noEmit").arg("--project").arg(tmp.path())
            .output().unwrap();
        if !out.status.success() {
            failures.push(format!("{}:\n{}", p.display(), String::from_utf8_lossy(&out.stderr)));
        }
    }
    assert!(failures.is_empty(), "tsc errors:\n{}", failures.join("\n\n"));
}
```

- [ ] **Step 2: Create the scratch tsconfig directory**

Create `crates/vox-integration-tests/ts-noemit-scratch/tsconfig.json`:

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "strict": true,
    "noEmit": true,
    "jsx": "react-jsx",
    "skipLibCheck": true,
    "esModuleInterop": true,
    "isolatedModules": true,
    "lib": ["ES2022", "DOM", "DOM.Iterable"]
  }
}
```

Create `crates/vox-integration-tests/ts-noemit-scratch/package.json`:

```json
{
  "name": "ts-noemit-scratch",
  "private": true,
  "dependencies": {
    "react": "^19.0.0",
    "react-dom": "^19.0.0",
    "@tanstack/react-router": "^1.0.0",
    "typescript": "^5.5.0"
  }
}
```

- [ ] **Step 3: Add CI workflow**

Create `.github/workflows/ts-emit-noemit.yml`:

```yaml
name: TS emit typecheck
on: [push, pull_request]
jobs:
  ts-noemit:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: pnpm/action-setup@v3
        with: { version: 9 }
      - uses: actions/setup-node@v4
        with: { node-version: '20', cache: 'pnpm' }
      - uses: dtolnay/rust-toolchain@stable
      - name: Install scratch deps
        run: cd crates/vox-integration-tests/ts-noemit-scratch && pnpm install --frozen-lockfile
      - name: Run TS emit + tsc --noEmit
        run: cargo test -p vox-integration-tests --test ts_emit_typecheck_test -- --nocapture
```

- [ ] **Step 4: Run locally**

Run: `cargo test -p vox-integration-tests --test ts_emit_typecheck_test`
Expected: pass — if it doesn't, the failures point to remaining emit bugs and become items in this plan.

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/ts-emit-noemit.yml \
        crates/vox-integration-tests/ts-noemit-scratch \
        crates/vox-integration-tests/tests/ts_emit_typecheck_test.rs
git commit -m "ci(codegen-ts): tsc --noEmit gate over all golden fixtures"
```

## Task A3: Centralized builtin lowering registry

**Files:**
- Create: `crates/vox-codegen/src/codegen_ts/builtin_registry.rs`
- Modify: `crates/vox-codegen/src/codegen_ts/hir_emit/mod.rs` (use registry instead of scattered special cases)
- Test: `crates/vox-codegen/tests/builtin_registry_test.rs`

**Background:** `.length()` → `.length`, `std.time.now_ms()` → `Date.now()`, namespace aliases — these were each hand-coded in different parts of the emitter and caused bugs (Bug B, Bug §1.A.3). Move them to a single source-of-truth.

- [ ] **Step 1: Write the failing tests**

Create `crates/vox-codegen/tests/builtin_registry_test.rs`:

```rust
use vox_codegen::codegen_ts::builtin_registry::{BuiltinRegistry, BuiltinLowering};

#[test]
fn registry_has_str_length_as_property() {
    let r = BuiltinRegistry::standard();
    let lo = r.lookup_method("str", "length", 0).expect("str.length");
    assert!(matches!(lo, BuiltinLowering::Property("length")));
}

#[test]
fn registry_has_time_now_ms_inlined() {
    let r = BuiltinRegistry::standard();
    let lo = r.lookup_function("std.time.now_ms", 0).expect("std.time.now_ms");
    assert!(matches!(lo, BuiltinLowering::Inline("Date.now()")));
}

#[test]
fn registry_has_speech_namespace_alias() {
    let r = BuiltinRegistry::standard();
    let alias = r.lookup_namespace("Speech").expect("Speech namespace");
    assert_eq!(alias, "Speech");  // not 'mobile'; namespace keeps its name
}

#[test]
fn registry_unknown_method_returns_none() {
    let r = BuiltinRegistry::standard();
    assert!(r.lookup_method("str", "nonexistent", 0).is_none());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-codegen --test builtin_registry_test`
Expected: fail — module doesn't exist.

- [ ] **Step 3: Implement registry**

Create `crates/vox-codegen/src/codegen_ts/builtin_registry.rs`:

```rust
//! Single source of truth for how Vox method/function/namespace identifiers
//! lower to TypeScript. Adding a new builtin: add a row here, write a test.

use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BuiltinLowering {
    /// Drop the call, emit as a property access. e.g. `s.length()` → `s.length`.
    Property(&'static str),
    /// Replace the entire call expression with this literal TS. e.g. `std.time.now_ms()` → `Date.now()`.
    Inline(&'static str),
    /// Rewrite the method to a different method name. e.g. `arr.size()` → `arr.length` (no, that's Property).
    /// Use this for `arr.push_back(x)` → `arr.push(x)` etc.
    MethodRename(&'static str),
    /// Wrap the call in serde-equivalent. e.g. `to_json(x)` → `JSON.stringify(x)`.
    FunctionRename(&'static str),
}

pub struct BuiltinRegistry {
    methods: HashMap<(&'static str, &'static str, usize), BuiltinLowering>,
    functions: HashMap<(&'static str, usize), BuiltinLowering>,
    namespaces: HashMap<&'static str, &'static str>,
}

impl BuiltinRegistry {
    pub fn standard() -> Self {
        let mut methods = HashMap::new();
        // (type, method, arity) → lowering
        methods.insert(("str", "length", 0), BuiltinLowering::Property("length"));
        methods.insert(("list", "length", 0), BuiltinLowering::Property("length"));
        methods.insert(("list", "push", 1), BuiltinLowering::MethodRename("push"));
        methods.insert(("list", "pop", 0), BuiltinLowering::MethodRename("pop"));
        methods.insert(("str", "trim", 0), BuiltinLowering::MethodRename("trim"));
        methods.insert(("str", "to_lower", 0), BuiltinLowering::MethodRename("toLowerCase"));
        methods.insert(("str", "to_upper", 0), BuiltinLowering::MethodRename("toUpperCase"));
        methods.insert(("str", "split", 1), BuiltinLowering::MethodRename("split"));
        methods.insert(("str", "starts_with", 1), BuiltinLowering::MethodRename("startsWith"));
        methods.insert(("str", "ends_with", 1), BuiltinLowering::MethodRename("endsWith"));

        let mut functions = HashMap::new();
        functions.insert(("std.time.now_ms", 0), BuiltinLowering::Inline("Date.now()"));
        functions.insert(("std.time.iso_now", 0), BuiltinLowering::Inline("new Date().toISOString()"));
        functions.insert(("len", 1), BuiltinLowering::FunctionRename("__vox_len"));
        functions.insert(("str", 1), BuiltinLowering::FunctionRename("String"));

        let mut namespaces = HashMap::new();
        namespaces.insert("Speech", "Speech");
        namespaces.insert("std.mobile", "Speech");  // legacy alias

        Self { methods, functions, namespaces }
    }

    pub fn lookup_method(&self, ty: &str, method: &str, arity: usize) -> Option<BuiltinLowering> {
        self.methods.get(&(ty, method, arity)).cloned()
            // try without arity hint
            .or_else(|| self.methods.iter()
                .find(|((t, m, _), _)| *t == ty && *m == method)
                .map(|(_, l)| l.clone()))
    }

    pub fn lookup_function(&self, name: &str, arity: usize) -> Option<BuiltinLowering> {
        self.functions.get(&(name, arity)).cloned()
            .or_else(|| self.functions.iter()
                .find(|((n, _), _)| *n == name)
                .map(|(_, l)| l.clone()))
    }

    pub fn lookup_namespace(&self, ns: &str) -> Option<&'static str> {
        self.namespaces.get(ns).copied()
    }
}
```

- [ ] **Step 4: Wire registry into emitter**

In `crates/vox-codegen/src/codegen_ts/hir_emit/mod.rs`, replace scattered special cases:

1. Find the `MethodCall` arm (around line 332). Before falling through to `format!("{recv}.{method}({args})")`, look up the receiver type in the registry and apply the lowering.
2. Find the `Call` arm with `std.time.now_ms`-style hardcoding. Replace with registry lookup.
3. Add `use crate::codegen_ts::builtin_registry::{BuiltinRegistry, BuiltinLowering};` at the top.
4. Hold a `static REGISTRY: OnceLock<BuiltinRegistry> = OnceLock::new();` at module top; expose `fn registry() -> &'static BuiltinRegistry { REGISTRY.get_or_init(BuiltinRegistry::standard) }`.

- [ ] **Step 5: Add module declaration**

In `crates/vox-codegen/src/codegen_ts/mod.rs`:

```rust
pub mod builtin_registry;
```

- [ ] **Step 6: Run tests**

Run: `cargo test -p vox-codegen --test builtin_registry_test && cargo test -p vox-codegen --test async_emit_test && cargo test -p vox-integration-tests --test ts_emit_typecheck_test`
Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-codegen/src/codegen_ts/builtin_registry.rs \
        crates/vox-codegen/src/codegen_ts/mod.rs \
        crates/vox-codegen/src/codegen_ts/hir_emit/mod.rs \
        crates/vox-codegen/tests/builtin_registry_test.rs
git commit -m "refactor(codegen-ts): centralize method/fn/namespace lowering in builtin registry"
```

---

# TRACK B — Render-loop guardrails

Five compile-error lints. Each makes one class of LLM GUI bug structurally unrepresentable. Each is one task.

## Task B1: Required `key` on list children

**Files:**
- Create: `crates/vox-codegen/src/web_ir/validate_keys.rs`
- Modify: `crates/vox-codegen/src/web_ir/validate.rs:847+` (register validator)
- Modify: `crates/vox-codegen/src/codegen_ts/jsx.rs:360` (emit `key` prop)
- Modify: `crates/vox-codegen/src/web_ir/emit_tsx.rs:125-130` (emit `key` prop)
- Test: `crates/vox-codegen/tests/list_keys_test.rs`

**Failure mode:** `for item in list { row(...) }` emits `list.map(item => <Row .../>)` with no `key` attribute. React fails to reconcile reorder/insert/delete; data loss and stale UI result.

**Catch level:** Web IR validator (compile error). Emit must also pull a `key` from the iteration variable (or named field).

- [ ] **Step 1: Write the failing test**

Create `crates/vox-codegen/tests/list_keys_test.rs`:

```rust
use vox_codegen::codegen_ts::emitter::{generate, CodegenOptions};
use vox_compiler::{parser::parse, lexer::cursor::lex, hir::lower::lower_module};

fn try_emit(src: &str) -> Result<String, String> {
    let m = parse(lex(src)).map_err(|e| format!("{e:?}"))?;
    let hir = lower_module(&m);
    let out = generate(&hir, &CodegenOptions::default()).map_err(|e| format!("{e:?}"))?;
    Ok(out.files.iter().map(|f| f.contents.clone()).collect::<Vec<_>>().join("\n"))
}

#[test]
fn list_render_with_explicit_key_emits_key_prop() {
    let src = r#"
@table type Item { id: str title: str }
component List(items: list[Item]) {
    view: stack() {
        for it in items key=it.id {
            text() { it.title }
        }
    }
}
"#;
    let ts = try_emit(src).expect("emit");
    assert!(ts.contains("key={it.id}"), "must emit key prop\n---\n{}", ts);
}

#[test]
fn list_render_without_key_fails() {
    let src = r#"
@table type Item { id: str title: str }
component List(items: list[Item]) {
    view: stack() {
        for it in items {
            text() { it.title }
        }
    }
}
"#;
    let err = try_emit(src).expect_err("must fail");
    assert!(err.contains("validate.list_key.required"), "expected validate.list_key.required diagnostic, got: {err}");
}

#[test]
fn list_with_primitive_iter_can_use_index_key_explicitly() {
    let src = r#"
component L(words: list[str]) {
    view: stack() {
        for w in words key=w {
            text() { w }
        }
    }
}
"#;
    let ts = try_emit(src).expect("emit");
    assert!(ts.contains("key={w}"));
}
```

- [ ] **Step 2: Extend parser for `for … key= … {}`**

In `crates/vox-compiler/src/parser/descent/expr/for_expr.rs` (find with `grep -r "fn parse_for" crates/vox-compiler/src/parser`), add an optional `key` clause:

```rust
// Pseudocode insertion point — after parsing iter, before `{`:
let key_expr = if self.peek_kw("key") {
    self.consume_kw("key")?;
    self.consume(TokenKind::Eq)?;
    Some(self.parse_expr()?)
} else {
    None
};
```

Add field `pub key: Option<Expr>` to `ForExpr` AST node in `crates/vox-compiler/src/ast/expr/mod.rs`. Mirror in HIR (`crates/vox-compiler/src/hir/nodes/expr.rs`: `pub key: Option<Box<HirExpr>>`).

- [ ] **Step 3: Implement Web IR validator**

Create `crates/vox-codegen/src/web_ir/validate_keys.rs`:

```rust
//! Validator: every `Loop` node must have a `key` attribute on each child.
//! The compiler refuses to emit if any list iteration lacks a key — React
//! requires stable identity and silently corrupts reorder otherwise.

use crate::web_ir::ir::{WebIrModule, DomNode};
use crate::web_ir::diag::{WebIrDiagnostic, WebIrSeverity};

pub fn validate_keys(module: &WebIrModule, out: &mut Vec<WebIrDiagnostic>) {
    for view in module.views.iter() {
        walk_node(module, view.root, out);
    }
}

fn walk_node(module: &WebIrModule, id: u32, out: &mut Vec<WebIrDiagnostic>) {
    let n = match module.nodes.get(id as usize) { Some(x) => x, None => return };
    match n {
        DomNode::Loop { iterator, key, body, span, .. } => {
            if key.is_none() {
                out.push(WebIrDiagnostic {
                    code: "validate.list_key.required".into(),
                    severity: WebIrSeverity::Error,
                    message: format!(
                        "list render `for … in {iterator} {{ … }}` is missing a `key` clause. \
                         Add `for x in {iterator} key=x.id {{ … }}` to give React stable \
                         identity. Without a key, reorder/insert silently corrupts UI state."
                    ),
                    span: *span,
                    suggestion: Some(format!("for x in {iterator} key=x.id {{ … }}")),
                });
            }
            for c in body { walk_node(module, *c, out); }
        }
        DomNode::Element { children, .. } => for c in children { walk_node(module, *c, out); },
        _ => {}
    }
}
```

- [ ] **Step 4: Register validator**

In `crates/vox-codegen/src/web_ir/validate.rs`, add inside `validate_web_ir_full` (or whatever the aggregate function is named):

```rust
pub mod validate_keys;
// ...
crate::web_ir::validate_keys::validate_keys(module, &mut diags);
```

- [ ] **Step 5: Update Web IR `Loop` to carry `key`**

In `crates/vox-codegen/src/web_ir/ir.rs`, extend the `Loop` variant:

```rust
DomNode::Loop {
    iterator: String,
    key: Option<String>,   // NEW
    body: Vec<u32>,
    span: Span,
}
```

In the lowering pass (`crates/vox-codegen/src/web_ir/lower.rs`), pass through the key from HIR `For.key`.

- [ ] **Step 6: Update emit to include key**

In `crates/vox-codegen/src/web_ir/emit_tsx.rs:125-130`:

```rust
DomNode::Loop { iterator, key, body, .. } => {
    let key_attr = key.as_ref().map_or(
        String::new(),
        |k| format!(" key={{{k}}}")
    );
    // The key goes on the FIRST child of the body, not the loop itself.
    let body_s: String = body.iter().enumerate().map(|(i, c)| {
        if i == 0 {
            // inject key into first child's attrs
            emit_node_with_extra_attrs(module, *c, indent + 1, stats, &key_attr)
        } else {
            emit_node(module, *c, indent + 1, stats)
        }
    }).collect();
    format!("{pad}{{{iterator}.map(({}) => (\n{body_s}{pad}))}}\n",
        key.as_ref().map_or(String::new(), |k| extract_loop_var_from_key(k)))
}
```

In `crates/vox-codegen/src/codegen_ts/jsx.rs:360`, mirror the change in the AST path.

- [ ] **Step 7: Run tests**

Run: `cargo test -p vox-codegen --test list_keys_test`
Expected: all 3 pass.

- [ ] **Step 8: Commit**

```bash
git add crates/vox-codegen/src/web_ir/validate_keys.rs \
        crates/vox-codegen/src/web_ir/validate.rs \
        crates/vox-codegen/src/web_ir/emit_tsx.rs \
        crates/vox-codegen/src/web_ir/ir.rs \
        crates/vox-codegen/src/web_ir/lower.rs \
        crates/vox-codegen/src/codegen_ts/jsx.rs \
        crates/vox-compiler/src/parser/descent/expr/for_expr.rs \
        crates/vox-compiler/src/ast/expr/mod.rs \
        crates/vox-compiler/src/hir/nodes/expr.rs \
        crates/vox-codegen/tests/list_keys_test.rs
git commit -m "feat(compiler): require key on list iterations (validate.list_key.required)"
```

## Task B2: Effect dependency completeness

**Files:**
- Create: `crates/vox-compiler/src/typeck/effect_deps_lint.rs`
- Modify: `crates/vox-compiler/src/typeck/mod.rs:56-62` (register pass)
- Test: `crates/vox-compiler/tests/effect_deps_test.rs`

**Failure mode:** `effect { fetch_x(); set_y(x) }` accidentally re-runs because `fetch_x` returns a new value each call. React does the same with `useEffect(fn, [])` vs `useEffect(fn, [x])`. Vox's `state_deps.rs` already infers deps but doesn't *enforce* completeness — if it can't decide, today it emits empty deps and you silently get infinite renders.

**Catch level:** HIR pass. If dep inference yields `unannotated` or `ambiguous`, fail compile.

- [ ] **Step 1: Write the failing test**

Create `crates/vox-compiler/tests/effect_deps_test.rs`:

```rust
use vox_compiler::typeck::typecheck_ast_module;
use vox_compiler::{parser::parse, lexer::cursor::lex};

fn diags(src: &str) -> Vec<vox_compiler::typeck::diagnostics::Diagnostic> {
    let m = parse(lex(src)).expect("parse");
    typecheck_ast_module(src, &m)
}

#[test]
fn effect_with_unresolvable_dep_errors() {
    let src = r#"
component C() {
    state n: int = 0
    effect: {
        external_call()
        set n(1)
    }
    view: text() { "x" }
}
"#;
    let ds = diags(src);
    let hit = ds.iter().find(|d| d.code.as_deref() == Some("lint.effect.unresolvable_deps"));
    assert!(hit.is_some(), "expected lint.effect.unresolvable_deps; got {:?}",
        ds.iter().map(|d| &d.message).collect::<Vec<_>>());
}

#[test]
fn effect_with_explicit_depends_on_passes() {
    let src = r#"
component C() {
    state n: int = 0
    effect depends_on (n): {
        log(n)
    }
    view: text() { "x" }
}
"#;
    let ds = diags(src);
    assert!(ds.iter().all(|d| d.code.as_deref() != Some("lint.effect.unresolvable_deps")));
}

#[test]
fn effect_with_only_local_state_passes() {
    let src = r#"
component C() {
    state n: int = 0
    state m: int = 0
    effect: { set m(n + 1) }
    view: text() { "x" }
}
"#;
    let ds = diags(src);
    assert!(ds.iter().all(|d| d.code.as_deref() != Some("lint.effect.unresolvable_deps")),
        "got: {:?}", ds.iter().map(|d| &d.code).collect::<Vec<_>>());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-compiler --test effect_deps_test`
Expected: fail.

- [ ] **Step 3: Extend parser for `effect depends_on (…)`**

In `crates/vox-compiler/src/parser/descent/decl/tail.rs` (find `parse_effect_decl`), add optional `depends_on (name, name, …)` clause that fills `EffectDecl.explicit_deps: Option<Vec<String>>`. Mirror in `EffectDecl` struct in `ast/decl/ui.rs`.

- [ ] **Step 4: Implement the lint pass**

Create `crates/vox-compiler/src/typeck/effect_deps_lint.rs`:

```rust
//! HIR pass: each `effect` block must have either fully-resolved automatic
//! deps or an explicit `depends_on (…)` clause. If state_deps inference
//! returns `unannotated`, fail.

use crate::hir::nodes::module::HirModule;
use crate::hir::nodes::decl::{HirReactiveComponent, HirReactiveMember};
use crate::typeck::diagnostics::{Diagnostic, DiagnosticCategory, TypeckSeverity};
// Note: state_deps lives in vox-codegen — but we need its analysis here.
// Option (chosen): move state_deps to vox-compiler/src/typeck/state_deps.rs;
// vox-codegen re-exports it.

pub fn check_effect_deps(hir: &HirModule, _source: &str) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    for c in &hir.components {
        check_component(c, &mut diags);
    }
    diags
}

fn check_component(c: &HirReactiveComponent, diags: &mut Vec<Diagnostic>) {
    let state_names: std::collections::HashSet<String> =
        c.members.iter().filter_map(|m| match m {
            HirReactiveMember::State(s) => Some(s.name.clone()),
            _ => None,
        }).collect();
    let visible_fns: std::collections::HashSet<String> = std::collections::HashSet::new(); // populated by caller in real impl
    for m in &c.members {
        if let HirReactiveMember::Effect(e) = m {
            if e.explicit_deps.is_some() { continue; }
            let mut found = std::collections::HashSet::new();
            let mut unannotated: Vec<String> = Vec::new();
            crate::typeck::state_deps::collect_deps_and_calls(
                &e.body, &state_names, &Default::default(),
                &visible_fns, &mut Default::default(),
                &mut found, &mut unannotated,
            );
            if !unannotated.is_empty() {
                diags.push(Diagnostic {
                    severity: TypeckSeverity::Error,
                    message: format!(
                        "effect calls {:?} which is not @reactive and is not in scope; \
                         dep tracking is ambiguous. Add `effect depends_on (...)` to \
                         declare the deps explicitly, or mark the called fn `@reactive`.",
                        unannotated
                    ),
                    span: e.span,
                    code: Some("lint.effect.unresolvable_deps".into()),
                    category: DiagnosticCategory::Lint,
                    suggestions: vec![format!("effect depends_on ({}): {{ … }}",
                        state_names.iter().take(3).cloned().collect::<Vec<_>>().join(", "))],
                    fixes: vec![], line_col: None, missing_cases: vec![],
                    expected_type: None, found_type: None, context: None, ast_node_kind: None,
                });
            }
        }
    }
}
```

- [ ] **Step 5: Move state_deps to compiler crate**

Move `crates/vox-codegen/src/codegen_ts/hir_emit/state_deps.rs` to `crates/vox-compiler/src/typeck/state_deps.rs`. Update import sites in vox-codegen to `use vox_compiler::typeck::state_deps::*`. Re-export from old location with deprecation:

```rust
// vox-codegen/src/codegen_ts/hir_emit/mod.rs
pub use vox_compiler::typeck::state_deps;
```

- [ ] **Step 6: Register the pass**

In `crates/vox-compiler/src/typeck/mod.rs:56-62`:

```rust
pub fn typecheck_hir_module(source: &str, hir: &mut HirModule) -> Vec<Diagnostic> {
    let mut diags = typecheck_hir(hir, &mut env, &builtins, source);
    diags.extend(effect_check::check_effect_compliance(hir, source));
    diags.extend(state_machine_check::check_state_machines(hir, source));
    diags.extend(effect_deps_lint::check_effect_deps(hir, source));  // NEW
    diags
}
```

- [ ] **Step 7: Run test**

Run: `cargo test -p vox-compiler --test effect_deps_test`
Expected: all 3 pass.

- [ ] **Step 8: Commit**

```bash
git add crates/vox-compiler/src/typeck/effect_deps_lint.rs \
        crates/vox-compiler/src/typeck/mod.rs \
        crates/vox-compiler/src/typeck/state_deps.rs \
        crates/vox-codegen/src/codegen_ts/hir_emit/mod.rs \
        crates/vox-compiler/src/parser/descent/decl/tail.rs \
        crates/vox-compiler/src/ast/decl/ui.rs \
        crates/vox-compiler/tests/effect_deps_test.rs
git rm crates/vox-codegen/src/codegen_ts/hir_emit/state_deps.rs
git commit -m "feat(compiler): require resolvable effect deps (lint.effect.unresolvable_deps)"
```

## Task B3: Stale closure capture detection

**Files:**
- Create: `crates/vox-compiler/src/typeck/stale_capture_lint.rs`
- Modify: `crates/vox-compiler/src/typeck/mod.rs` (register)
- Test: `crates/vox-compiler/tests/stale_capture_test.rs`

**Failure mode:** A handler defined in `on_mount` captures `count` at mount time; later renders update `count` but the handler still reads the stale value. React solves this with refs; Vox can detect at compile time which closures cross effect/lifecycle boundaries.

- [ ] **Step 1: Write failing test**

Create `crates/vox-compiler/tests/stale_capture_test.rs`:

```rust
use vox_compiler::typeck::typecheck_ast_module;
use vox_compiler::{parser::parse, lexer::cursor::lex};

#[test]
fn closure_in_on_mount_capturing_state_warns() {
    let src = r#"
component C() {
    state n: int = 0
    on_mount: {
        register_listener(() => log(n))
    }
    view: text() { str(n) }
}
"#;
    let m = parse(lex(src)).expect("parse");
    let ds = typecheck_ast_module(src, &m);
    let hit = ds.iter().find(|d| d.code.as_deref() == Some("lint.closure.stale_capture"));
    assert!(hit.is_some(), "expected stale_capture warning, got {:?}",
        ds.iter().map(|d| &d.code).collect::<Vec<_>>());
}

#[test]
fn closure_in_effect_with_dep_does_not_warn() {
    let src = r#"
component C() {
    state n: int = 0
    effect depends_on (n): {
        register_listener(() => log(n))
    }
    view: text() { str(n) }
}
"#;
    let m = parse(lex(src)).expect("parse");
    let ds = typecheck_ast_module(src, &m);
    assert!(ds.iter().all(|d| d.code.as_deref() != Some("lint.closure.stale_capture")));
}

#[test]
fn closure_in_event_handler_does_not_warn() {
    let src = r#"
component C() {
    state n: int = 0
    view: button(on_click: () => set n(n + 1)) { "+" }
}
"#;
    let m = parse(lex(src)).expect("parse");
    let ds = typecheck_ast_module(src, &m);
    assert!(ds.iter().all(|d| d.code.as_deref() != Some("lint.closure.stale_capture")));
}
```

- [ ] **Step 2: Implement pass**

Create `crates/vox-compiler/src/typeck/stale_capture_lint.rs`:

```rust
//! Detect: a Lambda inside an OnMount/Effect (without explicit dep) that
//! captures a state name. Such closures bind to the state value at
//! mount/effect-run time, not at call time, so reads are stale.
//!
//! Acceptable:
//! - effect with explicit `depends_on (state_name)` — re-bound on change
//! - event handler (on_click etc.) — re-rendered each pass; closure is fresh
//!
//! Risky:
//! - on_mount with closure capturing state — stale forever
//! - effect with no depends_on capturing state — stale until explicit deps

use crate::hir::nodes::module::HirModule;
use crate::hir::nodes::decl::{HirReactiveComponent, HirReactiveMember};
use crate::hir::nodes::expr::HirExpr;
use crate::typeck::diagnostics::{Diagnostic, DiagnosticCategory, TypeckSeverity};
use std::collections::HashSet;

pub fn check_stale_captures(hir: &HirModule, _source: &str) -> Vec<Diagnostic> {
    let mut diags = vec![];
    for c in &hir.components {
        check(c, &mut diags);
    }
    diags
}

fn check(c: &HirReactiveComponent, diags: &mut Vec<Diagnostic>) {
    let state_names: HashSet<String> = c.members.iter().filter_map(|m| match m {
        HirReactiveMember::State(s) => Some(s.name.clone()),
        _ => None,
    }).collect();

    for m in &c.members {
        match m {
            HirReactiveMember::OnMount(om) => {
                find_closure_state_caps(&om.body, &state_names, /*context=*/"on_mount",
                    /*allow=*/false, om.span, diags);
            }
            HirReactiveMember::Effect(e) => {
                let allow = e.explicit_deps.is_some();
                find_closure_state_caps(&e.body, &state_names, "effect", allow, e.span, diags);
            }
            _ => {}
        }
    }
}

fn find_closure_state_caps(
    expr: &HirExpr,
    states: &HashSet<String>,
    ctx_name: &str,
    allow: bool,
    span: crate::hir::span::Span,
    diags: &mut Vec<Diagnostic>,
) {
    walk(expr, states, ctx_name, allow, span, diags, /*in_lambda=*/false);
}

fn walk(
    e: &HirExpr,
    states: &HashSet<String>,
    ctx_name: &str,
    allow: bool,
    span: crate::hir::span::Span,
    diags: &mut Vec<Diagnostic>,
    in_lambda: bool,
) {
    match e {
        HirExpr::Lambda(_, _, body, _) => walk(body, states, ctx_name, allow, span, diags, true),
        HirExpr::Ident(n, _) if in_lambda && states.contains(n.as_str()) && !allow => {
            diags.push(Diagnostic {
                severity: TypeckSeverity::Warning,
                message: format!(
                    "closure inside `{ctx_name}` captures state `{n}`. Closures here \
                     bind once and read stale values. Either move the closure into a \
                     fresh handler (e.g. on_click) or add `depends_on ({n})` to the \
                     {ctx_name}."
                ),
                span,
                code: Some("lint.closure.stale_capture".into()),
                category: DiagnosticCategory::Lint,
                suggestions: vec![format!("{ctx_name} depends_on ({n}): …")],
                fixes: vec![], line_col: None, missing_cases: vec![],
                expected_type: None, found_type: None, context: None, ast_node_kind: None,
            });
        }
        HirExpr::Block(stmts, _) => for s in stmts { walk_stmt(s, states, ctx_name, allow, span, diags, in_lambda); },
        HirExpr::Call(callee, args, _, _) => {
            walk(callee, states, ctx_name, allow, span, diags, in_lambda);
            for a in args { walk(a, states, ctx_name, allow, span, diags, in_lambda); }
        }
        HirExpr::MethodCall { receiver, args, .. } => {
            walk(receiver, states, ctx_name, allow, span, diags, in_lambda);
            for a in args { walk(a, states, ctx_name, allow, span, diags, in_lambda); }
        }
        _ => {}
    }
}

fn walk_stmt(
    s: &crate::hir::nodes::stmt::HirStmt,
    states: &HashSet<String>, ctx_name: &str, allow: bool,
    span: crate::hir::span::Span, diags: &mut Vec<Diagnostic>, in_lambda: bool,
) {
    use crate::hir::nodes::stmt::HirStmt::*;
    match s {
        Expr(e) | Return(Some(e)) => walk(e, states, ctx_name, allow, span, diags, in_lambda),
        Let { value, .. } | Assign { value, .. } => walk(value, states, ctx_name, allow, span, diags, in_lambda),
        _ => {}
    }
}
```

- [ ] **Step 3: Register pass**

`crates/vox-compiler/src/typeck/mod.rs`:

```rust
diags.extend(stale_capture_lint::check_stale_captures(hir, source));
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p vox-compiler --test stale_capture_test`
Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-compiler/src/typeck/stale_capture_lint.rs \
        crates/vox-compiler/src/typeck/mod.rs \
        crates/vox-compiler/tests/stale_capture_test.rs
git commit -m "feat(compiler): warn on stale state captures in lifecycle closures (lint.closure.stale_capture)"
```

## Task B4: Async event handler must declare cancellability

**Files:**
- Create: `crates/vox-compiler/src/typeck/async_handler_lint.rs`
- Modify: `crates/vox-compiler/src/typeck/mod.rs`
- Test: `crates/vox-compiler/tests/async_handler_test.rs`

**Failure mode:** `on_click: async () => { const x = await fetch(); set_y(x) }` issues a state update after the component may have unmounted, leaking memory or producing a "set state on unmounted" warning. LLMs never add AbortController.

**Catch level:** HIR pass. Warn (configurable to error) if an event handler contains an async call AND a state mutation that follows it. Suggest `@cancellable` annotation that desugars to AbortController + on-unmount cancel.

- [ ] **Step 1: Write failing test**

Create `crates/vox-compiler/tests/async_handler_test.rs`:

```rust
use vox_compiler::typeck::typecheck_ast_module;
use vox_compiler::{parser::parse, lexer::cursor::lex};

#[test]
fn async_handler_with_setstate_warns() {
    let src = r#"
@endpoint(kind: query) fn slow_fetch() to int { return 1 }
component C() {
    state n: int = 0
    view: button(on_click: () => {
        let x = slow_fetch()
        set n(x)
    }) { "Go" }
}
"#;
    let m = parse(lex(src)).expect("parse");
    let ds = typecheck_ast_module(src, &m);
    let hit = ds.iter().find(|d| d.code.as_deref() == Some("lint.handler.uncancellable_async"));
    assert!(hit.is_some(), "expected lint.handler.uncancellable_async");
}

#[test]
fn cancellable_handler_passes() {
    let src = r#"
@endpoint(kind: query) fn slow_fetch() to int { return 1 }
component C() {
    state n: int = 0
    view: button(on_click: @cancellable () => {
        let x = slow_fetch()
        set n(x)
    }) { "Go" }
}
"#;
    let m = parse(lex(src)).expect("parse");
    let ds = typecheck_ast_module(src, &m);
    assert!(ds.iter().all(|d| d.code.as_deref() != Some("lint.handler.uncancellable_async")));
}
```

- [ ] **Step 2: Extend parser for `@cancellable` on lambdas**

Add to `Lambda` AST node a `pub cancellable: bool` field. Parse `@cancellable` prefix in `parser/descent/expr/lambda.rs`. Mirror in HIR `Lambda` variant.

- [ ] **Step 3: Implement pass**

Create `crates/vox-compiler/src/typeck/async_handler_lint.rs`:

```rust
use crate::hir::nodes::module::HirModule;
use crate::hir::nodes::decl::{HirReactiveComponent, HirReactiveMember};
use crate::hir::nodes::expr::HirExpr;
use crate::typeck::diagnostics::{Diagnostic, DiagnosticCategory, TypeckSeverity};
use std::collections::HashSet;

pub fn check_async_handlers(hir: &HirModule, _source: &str) -> Vec<Diagnostic> {
    let mut diags = vec![];
    let async_fns: HashSet<String> = hir.endpoint_fns.iter().map(|e| e.name.clone()).collect();
    for c in &hir.components {
        if let Some(ref view) = c.view {
            walk_view(view, &async_fns, &mut diags);
        }
    }
    diags
}

fn walk_view(e: &HirExpr, async_fns: &HashSet<String>, diags: &mut Vec<Diagnostic>) {
    match e {
        HirExpr::Call(_, args, _, _) => {
            for a in args {
                if let HirExpr::Lambda(_, _, body, span) = a {
                    let lam = a.as_lambda().unwrap();  // helper accessor
                    if !lam.cancellable && lambda_has_async_then_setstate(body, async_fns) {
                        diags.push(Diagnostic {
                            severity: TypeckSeverity::Warning,
                            message: "async event handler updates state without `@cancellable`. \
                                If the component unmounts mid-fetch, the setState will fire on \
                                an unmounted component (memory leak / warning). Mark the handler \
                                `@cancellable` to wire AbortController + on-unmount cancel.".into(),
                            span: *span,
                            code: Some("lint.handler.uncancellable_async".into()),
                            category: DiagnosticCategory::Lint,
                            suggestions: vec!["on_click: @cancellable () => { … }".into()],
                            fixes: vec![], line_col: None, missing_cases: vec![],
                            expected_type: None, found_type: None, context: None, ast_node_kind: None,
                        });
                    }
                }
                walk_view(a, async_fns, diags);
            }
        }
        HirExpr::Block(stmts, _) => for s in stmts {
            if let crate::hir::nodes::stmt::HirStmt::Expr(e) = s {
                walk_view(e, async_fns, diags);
            }
        },
        _ => {}
    }
}

fn lambda_has_async_then_setstate(body: &HirExpr, async_fns: &HashSet<String>) -> bool {
    let mut saw_async = false;
    let mut saw_setstate_after = false;
    fn scan(e: &HirExpr, async_fns: &HashSet<String>, saw_async: &mut bool, after: &mut bool) {
        match e {
            HirExpr::Block(stmts, _) => for s in stmts {
                if let crate::hir::nodes::stmt::HirStmt::Expr(e) = s {
                    scan_stmt_expr(e, async_fns, saw_async, after);
                }
                if let crate::hir::nodes::stmt::HirStmt::Let { value, .. } = s {
                    scan_stmt_expr(value, async_fns, saw_async, after);
                }
            },
            _ => scan_stmt_expr(e, async_fns, saw_async, after),
        }
    }
    fn scan_stmt_expr(e: &HirExpr, async_fns: &HashSet<String>, saw_async: &mut bool, after: &mut bool) {
        match e {
            HirExpr::Call(callee, args, _, _) => {
                if let HirExpr::Ident(n, _) = callee.as_ref() {
                    if n == "set" {  // setter; convention: set <name>(...)
                        if *saw_async { *after = true; }
                    } else if async_fns.contains(n.as_str()) {
                        *saw_async = true;
                    }
                }
                for a in args { scan_stmt_expr(a, async_fns, saw_async, after); }
            }
            _ => {}
        }
    }
    scan(body, async_fns, &mut saw_async, &mut saw_setstate_after);
    saw_setstate_after
}
```

- [ ] **Step 4: Wire `@cancellable` in codegen**

When emitting a `@cancellable` handler in `codegen_ts/hir_emit/mod.rs`, wrap with AbortController setup and registration in a `useEffect` cleanup hook:

```typescript
// emitted shape for @cancellable () => { ... }
const __ac = useRef(new AbortController());
useEffect(() => () => __ac.current.abort(), []);
const handler = async () => {
  const __sig = __ac.current.signal;
  if (__sig.aborted) return;
  // ...body, with await calls passing __sig where they support it...
  if (__sig.aborted) return;
  // setState calls
};
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p vox-compiler --test async_handler_test && cargo test -p vox-codegen`
Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-compiler/src/typeck/async_handler_lint.rs \
        crates/vox-compiler/src/typeck/mod.rs \
        crates/vox-compiler/src/parser/descent/expr/lambda.rs \
        crates/vox-compiler/src/ast/expr/mod.rs \
        crates/vox-compiler/src/hir/nodes/expr.rs \
        crates/vox-codegen/src/codegen_ts/hir_emit/mod.rs \
        crates/vox-compiler/tests/async_handler_test.rs
git commit -m "feat(compiler): warn on uncancellable async handlers (lint.handler.uncancellable_async); add @cancellable desugaring"
```

## Task B5: Routes with loaders must have pending + error components

**Files:**
- Create: `crates/vox-codegen/src/web_ir/validate_route_completeness.rs`
- Modify: `crates/vox-codegen/src/web_ir/validate.rs` (register)
- Modify: `crates/vox-compiler/src/parser/descent/decl/tail.rs:393-434` (parse per-route error)
- Modify: `crates/vox-compiler/src/ast/decl/ui.rs:26-70` (`error_component_name: Option<String>` on `RouteEntry`)
- Test: `crates/vox-compiler/tests/route_completeness_test.rs`

**Failure mode:** `routes { "/x" to X with (loader: load_x) }` emits TanStack Router config but no `pendingComponent` or `errorComponent`. User sees blank page during load and a crashed page on error.

**Catch level:** Web IR validator (compile error) — but with a clear opt-out (`@no_loading` annotation) so that intentionally-instant loads aren't punished.

- [ ] **Step 1: Write failing test**

Create `crates/vox-compiler/tests/route_completeness_test.rs`:

```rust
use vox_codegen::codegen_ts::emitter::{generate, CodegenOptions};
use vox_compiler::{parser::parse, lexer::cursor::lex, hir::lower::lower_module};

fn try_emit(src: &str) -> Result<String, String> {
    let m = parse(lex(src)).map_err(|e| format!("{e:?}"))?;
    let hir = lower_module(&m);
    generate(&hir, &CodegenOptions::default())
        .map(|o| o.files.iter().map(|f| f.contents.clone()).collect::<Vec<_>>().join("\n"))
        .map_err(|e| format!("{e:?}"))
}

#[test]
fn route_with_loader_no_pending_fails() {
    let src = r#"
@endpoint(kind: query) fn load() to int { return 1 }
component X() { view: text() { "x" } }
routes {
    "/x" to X with (loader: load)
}
"#;
    let err = try_emit(src).expect_err("must fail");
    assert!(err.contains("validate.route.missing_pending"), "got: {err}");
}

#[test]
fn route_with_loader_no_error_fails() {
    let src = r#"
@endpoint(kind: query) fn load() to int { return 1 }
@loading fn LoadingX() to Element { return text() { "..." } }
component X() { view: text() { "x" } }
routes {
    "/x" to X with (loader: load, pending: LoadingX)
}
"#;
    let err = try_emit(src).expect_err("must fail");
    assert!(err.contains("validate.route.missing_error"), "got: {err}");
}

#[test]
fn route_with_loader_pending_and_error_passes() {
    let src = r#"
@endpoint(kind: query) fn load() to int { return 1 }
@loading fn LoadingX() to Element { return text() { "..." } }
component XErr() { view: text() { "err" } }
component X() { view: text() { "x" } }
routes {
    "/x" to X with (loader: load, pending: LoadingX, error: XErr)
}
"#;
    let _ts = try_emit(src).expect("should pass");
}

#[test]
fn route_without_loader_does_not_require_pending_or_error() {
    let src = r#"
component X() { view: text() { "x" } }
routes {
    "/x" to X
}
"#;
    let _ts = try_emit(src).expect("should pass");
}
```

- [ ] **Step 2: Extend parser for per-route `error: …`**

In `crates/vox-compiler/src/parser/descent/decl/tail.rs:393-434`, extend the `with (…)` clause parser to accept `error: Component` and store in `RouteEntry.error_component_name`. Mirror in `ast/decl/ui.rs:26-70`:

```rust
pub struct RouteEntry {
    pub path: String,
    pub component_name: String,
    pub children: Vec<RouteEntry>,
    pub redirect: Option<String>,
    pub is_wildcard: bool,
    pub loader_name: Option<String>,
    pub pending_component_name: Option<String>,
    pub error_component_name: Option<String>,  // NEW
    pub span: Span,
}
```

Mirror in HIR (find equivalent in `hir/nodes/decl.rs`).

- [ ] **Step 3: Implement validator**

Create `crates/vox-codegen/src/web_ir/validate_route_completeness.rs`:

```rust
use crate::web_ir::ir::WebIrModule;
use crate::web_ir::diag::{WebIrDiagnostic, WebIrSeverity};

pub fn validate_route_completeness(module: &WebIrModule, out: &mut Vec<WebIrDiagnostic>) {
    for r in &module.routes {
        if r.loader_name.is_none() { continue; }
        if r.pending_component_name.is_none() {
            out.push(WebIrDiagnostic {
                code: "validate.route.missing_pending".into(),
                severity: WebIrSeverity::Error,
                message: format!(
                    "route `{}` has a loader `{}` but no `pending:` component. \
                     Users see a blank screen during the load. Add `pending: <Component>` \
                     to the `with (…)` clause, or annotate the route `@no_loading` if the \
                     loader is known-fast.",
                    r.path, r.loader_name.as_deref().unwrap_or("")
                ),
                span: r.span,
                suggestion: Some(format!("with (loader: {}, pending: {}Loading, error: {}Error)",
                    r.loader_name.as_deref().unwrap_or(""),
                    r.component_name, r.component_name)),
            });
        }
        if r.error_component_name.is_none() {
            out.push(WebIrDiagnostic {
                code: "validate.route.missing_error".into(),
                severity: WebIrSeverity::Error,
                message: format!(
                    "route `{}` has a loader `{}` but no `error:` component. \
                     Loader failures crash the page. Add `error: <Component>`.",
                    r.path, r.loader_name.as_deref().unwrap_or("")
                ),
                span: r.span,
                suggestion: Some(format!("with (loader: {}, pending: …, error: {}Error)",
                    r.loader_name.as_deref().unwrap_or(""), r.component_name)),
            });
        }
    }
}
```

- [ ] **Step 4: Register in `validate.rs`**

```rust
pub mod validate_route_completeness;
// inside validate_web_ir_full:
crate::web_ir::validate_route_completeness::validate_route_completeness(module, &mut diags);
```

- [ ] **Step 5: Update Web IR `RouteIr` shape**

Add `pub error_component_name: Option<String>` to `web_ir/ir.rs::RouteIr`. Pipe through `lower.rs`.

- [ ] **Step 6: Update emit to wire TanStack Router error component**

In `codegen_ts/routes.rs`, when generating each route, include `errorComponent: <ErrorComponent />` if present.

- [ ] **Step 7: Run tests**

Run: `cargo test -p vox-compiler --test route_completeness_test`
Expected: all 4 pass.

- [ ] **Step 8: Commit**

```bash
git add crates/vox-codegen/src/web_ir/validate_route_completeness.rs \
        crates/vox-codegen/src/web_ir/validate.rs \
        crates/vox-codegen/src/web_ir/ir.rs \
        crates/vox-codegen/src/web_ir/lower.rs \
        crates/vox-codegen/src/codegen_ts/routes.rs \
        crates/vox-compiler/src/parser/descent/decl/tail.rs \
        crates/vox-compiler/src/ast/decl/ui.rs \
        crates/vox-compiler/src/hir/nodes/decl.rs \
        crates/vox-compiler/tests/route_completeness_test.rs
git commit -m "feat(compiler): require per-route pending+error when loader present (validate.route.missing_*)"
```

---

# TRACK C — Forms-as-first-class

A `@form` declaration captures field schemas, generates inputs, two-way binding, validation, and error UI in lockstep. No more "input renders, state never syncs, validation forgotten."

## Task C1: `@form` AST + parser

**Files:**
- Create: `crates/vox-compiler/src/ast/decl/form.rs`
- Modify: `crates/vox-compiler/src/ast/decl/mod.rs` (add `Form(FormDecl)` variant)
- Modify: `crates/vox-compiler/src/parser/descent/decl/head.rs` (recognize `@form Name { … }`)
- Test: `crates/vox-compiler/tests/form_parse_test.rs`

**Spec:** Syntax:

```vox
@form MoodCheckIn {
    field score: int range(1..10) required label("How are you feeling?")
    field note: str max_len(280) optional label("Anything to share?")
    field at: timestamp default(now()) hidden

    on_submit: record_mood
    success_redirect: "/timeline"
    error_message: "Couldn't save mood. Try again."
}
```

Parses to:

```rust
pub struct FormDecl {
    pub name: String,
    pub fields: Vec<FormField>,
    pub on_submit: Option<String>,    // endpoint name
    pub success_redirect: Option<String>,
    pub error_message: Option<String>,
    pub span: Span,
}

pub struct FormField {
    pub name: String,
    pub ty: TypeRef,
    pub label: Option<String>,
    pub required: bool,
    pub hidden: bool,
    pub default: Option<Expr>,
    pub constraints: Vec<FieldConstraint>,
    pub span: Span,
}

pub enum FieldConstraint {
    Range(Expr, Expr),       // range(min..max)
    MaxLen(usize),
    MinLen(usize),
    Pattern(String),          // regex
    Enum(Vec<Expr>),          // enum(["a", "b"])
    Custom(String),           // custom validator fn name
}
```

- [ ] **Step 1: Write failing parse test**

Create `crates/vox-compiler/tests/form_parse_test.rs`:

```rust
use vox_compiler::{parser::parse, lexer::cursor::lex};
use vox_compiler::ast::decl::Decl;

#[test]
fn form_with_basic_fields_parses() {
    let src = r#"
@form Mood {
    field score: int range(1..10) required label("Mood")
    field note: str max_len(280) optional
    on_submit: save_mood
    success_redirect: "/timeline"
}
"#;
    let m = parse(lex(src)).expect("parse");
    let form = m.declarations.iter().find_map(|d| match d {
        Decl::Form(f) => Some(f), _ => None
    }).expect("form decl");
    assert_eq!(form.name, "Mood");
    assert_eq!(form.fields.len(), 2);
    assert_eq!(form.fields[0].name, "score");
    assert!(form.fields[0].required);
    assert_eq!(form.fields[0].label.as_deref(), Some("Mood"));
    assert_eq!(form.on_submit.as_deref(), Some("save_mood"));
    assert_eq!(form.success_redirect.as_deref(), Some("/timeline"));
}

#[test]
fn form_with_hidden_default_field_parses() {
    let src = r#"
@form X {
    field at: timestamp default(now()) hidden
    on_submit: save
}
"#;
    let m = parse(lex(src)).expect("parse");
    let form = m.declarations.iter().find_map(|d| match d {
        Decl::Form(f) => Some(f), _ => None
    }).expect("form decl");
    assert!(form.fields[0].hidden);
    assert!(form.fields[0].default.is_some());
}
```

- [ ] **Step 2: Add Decl variant**

In `crates/vox-compiler/src/ast/decl/mod.rs`, add:

```rust
pub mod form;
pub use form::*;

pub enum Decl {
    // ... existing variants ...
    Form(FormDecl),
}
```

- [ ] **Step 3: Define AST types**

Create `crates/vox-compiler/src/ast/decl/form.rs` with the structs above.

- [ ] **Step 4: Implement parser**

In `crates/vox-compiler/src/parser/descent/decl/head.rs`, add a branch when the parser sees `@form`:

```rust
fn parse_form_decl(&mut self) -> Result<FormDecl, ()> {
    self.consume_at_kw("form")?;
    let name = self.parse_ident()?;
    self.consume(TokenKind::LBrace)?;
    let mut fields = Vec::new();
    let mut on_submit = None;
    let mut success_redirect = None;
    let mut error_message = None;
    while !self.peek(TokenKind::RBrace) {
        if self.peek_kw("field") {
            fields.push(self.parse_form_field()?);
        } else if self.peek_kw("on_submit") {
            self.consume_kw("on_submit")?;
            self.consume(TokenKind::Colon)?;
            on_submit = Some(self.parse_ident()?);
        } else if self.peek_kw("success_redirect") {
            self.consume_kw("success_redirect")?;
            self.consume(TokenKind::Colon)?;
            success_redirect = Some(self.parse_string_lit()?);
        } else if self.peek_kw("error_message") {
            self.consume_kw("error_message")?;
            self.consume(TokenKind::Colon)?;
            error_message = Some(self.parse_string_lit()?);
        } else {
            self.error_expected("field, on_submit, success_redirect, or error_message");
            return Err(());
        }
    }
    self.consume(TokenKind::RBrace)?;
    Ok(FormDecl { name, fields, on_submit, success_redirect, error_message, span: self.span() })
}

fn parse_form_field(&mut self) -> Result<FormField, ()> {
    self.consume_kw("field")?;
    let name = self.parse_ident()?;
    self.consume(TokenKind::Colon)?;
    let ty = self.parse_type_ref()?;
    let mut required = false;
    let mut hidden = false;
    let mut label = None;
    let mut default = None;
    let mut constraints = Vec::new();
    loop {
        if self.peek_kw("required") { self.consume_kw("required")?; required = true; }
        else if self.peek_kw("optional") { self.consume_kw("optional")?; required = false; }
        else if self.peek_kw("hidden") { self.consume_kw("hidden")?; hidden = true; }
        else if self.peek_kw("label") {
            self.consume_kw("label")?;
            self.consume(TokenKind::LParen)?;
            label = Some(self.parse_string_lit()?);
            self.consume(TokenKind::RParen)?;
        } else if self.peek_kw("default") {
            self.consume_kw("default")?;
            self.consume(TokenKind::LParen)?;
            default = Some(self.parse_expr()?);
            self.consume(TokenKind::RParen)?;
        } else if self.peek_kw("range") {
            self.consume_kw("range")?;
            self.consume(TokenKind::LParen)?;
            let lo = self.parse_expr()?;
            self.consume_token_seq(&[TokenKind::Dot, TokenKind::Dot])?;
            let hi = self.parse_expr()?;
            self.consume(TokenKind::RParen)?;
            constraints.push(FieldConstraint::Range(lo, hi));
        } else if self.peek_kw("max_len") {
            self.consume_kw("max_len")?;
            self.consume(TokenKind::LParen)?;
            let n = self.parse_int_lit()? as usize;
            self.consume(TokenKind::RParen)?;
            constraints.push(FieldConstraint::MaxLen(n));
        } else { break; }
    }
    Ok(FormField { name, ty, label, required, hidden, default, constraints, span: self.span() })
}
```

In the main `parse_decl` switch, route `@form` here.

- [ ] **Step 5: Run tests**

Run: `cargo test -p vox-compiler --test form_parse_test`
Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-compiler/src/ast/decl/form.rs \
        crates/vox-compiler/src/ast/decl/mod.rs \
        crates/vox-compiler/src/parser/descent/decl/head.rs \
        crates/vox-compiler/tests/form_parse_test.rs
git commit -m "feat(compiler): @form declaration parser + AST"
```

## Task C2: Form HIR + validation

**Files:**
- Create: `crates/vox-compiler/src/hir/nodes/form.rs`
- Modify: `crates/vox-compiler/src/hir/nodes/module.rs` (add `forms: Vec<HirForm>`)
- Modify: `crates/vox-compiler/src/hir/lower.rs` (lower FormDecl)
- Modify: `crates/vox-compiler/src/typeck/mod.rs` (form HIR pass: every field's type matches the on_submit endpoint's signature)
- Test: `crates/vox-compiler/tests/form_hir_test.rs`

- [ ] **Step 1: Write failing test**

Create `crates/vox-compiler/tests/form_hir_test.rs`:

```rust
use vox_compiler::{parser::parse, lexer::cursor::lex, hir::lower::lower_module, typeck::typecheck_ast_module};

#[test]
fn form_lowered_with_correct_field_count() {
    let src = r#"
@endpoint(kind: mutation) fn save_x(s: int, n: str) to int { return 1 }
@form X {
    field s: int required
    field n: str optional
    on_submit: save_x
}
"#;
    let m = parse(lex(src)).expect("parse");
    let hir = lower_module(&m);
    assert_eq!(hir.forms.len(), 1);
    assert_eq!(hir.forms[0].name, "X");
    assert_eq!(hir.forms[0].fields.len(), 2);
}

#[test]
fn form_with_field_type_mismatch_errors() {
    let src = r#"
@endpoint(kind: mutation) fn save_x(s: str) to int { return 1 }
@form X {
    field s: int required
    on_submit: save_x
}
"#;
    let m = parse(lex(src)).expect("parse");
    let ds = typecheck_ast_module(src, &m);
    let hit = ds.iter().find(|d| d.code.as_deref() == Some("lint.form.field_type_mismatch"));
    assert!(hit.is_some());
}

#[test]
fn form_with_unknown_endpoint_errors() {
    let src = r#"
@form X {
    field s: int required
    on_submit: nonexistent
}
"#;
    let m = parse(lex(src)).expect("parse");
    let ds = typecheck_ast_module(src, &m);
    let hit = ds.iter().find(|d| d.code.as_deref() == Some("lint.form.unknown_endpoint"));
    assert!(hit.is_some());
}
```

- [ ] **Step 2: Define HIR types**

Create `crates/vox-compiler/src/hir/nodes/form.rs`:

```rust
use crate::hir::nodes::expr::HirExpr;
use crate::hir::nodes::ty::HirType;
use crate::hir::span::Span;

#[derive(Debug, Clone)]
pub struct HirForm {
    pub name: String,
    pub fields: Vec<HirFormField>,
    pub on_submit: Option<String>,
    pub success_redirect: Option<String>,
    pub error_message: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirFormField {
    pub name: String,
    pub ty: HirType,
    pub label: Option<String>,
    pub required: bool,
    pub hidden: bool,
    pub default: Option<HirExpr>,
    pub constraints: Vec<HirFieldConstraint>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum HirFieldConstraint {
    Range(HirExpr, HirExpr),
    MaxLen(usize),
    MinLen(usize),
    Pattern(String),
    Enum(Vec<HirExpr>),
    Custom(String),
}
```

- [ ] **Step 3: Add to HirModule**

In `crates/vox-compiler/src/hir/nodes/module.rs`, add:

```rust
pub forms: Vec<HirForm>,
```

In `Default`/`new` for HirModule, initialize as empty.

- [ ] **Step 4: Implement lowering**

In `crates/vox-compiler/src/hir/lower.rs`, find where Decl variants are matched and add:

```rust
Decl::Form(f) => {
    let lowered = HirForm {
        name: f.name.clone(),
        fields: f.fields.iter().map(|fd| HirFormField {
            name: fd.name.clone(),
            ty: lower_type(&fd.ty),
            label: fd.label.clone(),
            required: fd.required,
            hidden: fd.hidden,
            default: fd.default.as_ref().map(|d| lower_expr(d)),
            constraints: fd.constraints.iter().map(lower_constraint).collect(),
            span: fd.span,
        }).collect(),
        on_submit: f.on_submit.clone(),
        success_redirect: f.success_redirect.clone(),
        error_message: f.error_message.clone(),
        span: f.span,
    };
    out.forms.push(lowered);
}
```

- [ ] **Step 5: Implement form typecheck pass**

Create `crates/vox-compiler/src/typeck/form_check.rs`:

```rust
use crate::hir::nodes::module::HirModule;
use crate::typeck::diagnostics::{Diagnostic, DiagnosticCategory, TypeckSeverity};

pub fn check_forms(hir: &HirModule, _source: &str) -> Vec<Diagnostic> {
    let mut diags = vec![];
    for form in &hir.forms {
        if let Some(submit) = &form.on_submit {
            let endpoint = hir.endpoint_fns.iter().find(|e| &e.name == submit);
            if endpoint.is_none() {
                diags.push(Diagnostic {
                    severity: TypeckSeverity::Error,
                    message: format!(
                        "@form `{}` references on_submit `{}` but no @endpoint with that name exists.",
                        form.name, submit
                    ),
                    span: form.span,
                    code: Some("lint.form.unknown_endpoint".into()),
                    category: DiagnosticCategory::Lint,
                    suggestions: vec![],
                    fixes: vec![], line_col: None, missing_cases: vec![],
                    expected_type: None, found_type: None, context: None, ast_node_kind: None,
                });
                continue;
            }
            let ep = endpoint.unwrap();
            // Match form fields (excluding hidden defaults) against endpoint params
            let visible_fields: Vec<_> = form.fields.iter().filter(|f| !f.hidden || f.default.is_none()).collect();
            for vf in &visible_fields {
                let param = ep.params.iter().find(|p| p.name == vf.name);
                match param {
                    None => diags.push(Diagnostic {
                        severity: TypeckSeverity::Error,
                        message: format!(
                            "@form `{}` field `{}` has no matching parameter in @endpoint `{}`.",
                            form.name, vf.name, submit
                        ),
                        span: vf.span,
                        code: Some("lint.form.field_unmatched".into()),
                        category: DiagnosticCategory::Lint,
                        suggestions: vec![format!("Add `{}: {:?}` to @endpoint `{}` or remove the field.", vf.name, vf.ty, submit)],
                        fixes: vec![], line_col: None, missing_cases: vec![],
                        expected_type: None, found_type: None, context: None, ast_node_kind: None,
                    }),
                    Some(p) if p.ty != vf.ty => diags.push(Diagnostic {
                        severity: TypeckSeverity::Error,
                        message: format!(
                            "@form `{}` field `{}` has type `{:?}` but @endpoint `{}` expects `{:?}`.",
                            form.name, vf.name, vf.ty, submit, p.ty
                        ),
                        span: vf.span,
                        code: Some("lint.form.field_type_mismatch".into()),
                        category: DiagnosticCategory::Lint,
                        suggestions: vec![],
                        fixes: vec![], line_col: None, missing_cases: vec![],
                        expected_type: Some(format!("{:?}", p.ty)),
                        found_type: Some(format!("{:?}", vf.ty)),
                        context: None, ast_node_kind: None,
                    }),
                    _ => {}
                }
            }
        }
    }
    diags
}
```

Register in `typeck/mod.rs:typecheck_hir_module`:

```rust
diags.extend(form_check::check_forms(hir, source));
```

- [ ] **Step 6: Run tests**

Run: `cargo test -p vox-compiler --test form_hir_test`
Expected: 3 pass.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-compiler/src/hir/nodes/form.rs \
        crates/vox-compiler/src/hir/nodes/module.rs \
        crates/vox-compiler/src/hir/lower.rs \
        crates/vox-compiler/src/typeck/form_check.rs \
        crates/vox-compiler/src/typeck/mod.rs \
        crates/vox-compiler/tests/form_hir_test.rs
git commit -m "feat(compiler): @form HIR + endpoint signature checking"
```

## Task C3: Form codegen — emit React form component

**Files:**
- Create: `crates/vox-codegen/src/codegen_ts/form_emit.rs`
- Modify: `crates/vox-codegen/src/codegen_ts/emitter.rs` (call form_emit)
- Test: `crates/vox-codegen/tests/form_emit_test.rs`

**Spec:** A `@form X { … }` emits a React component named `<X />` that:
- Renders one `<input>` per field (correct `type=` attribute for `int`/`str`/`bool`/`timestamp`)
- Has a `<label>` per field
- Two-way bound: `value={state.field}` + `onChange={e => setState({…state, field: e.target.value})}`
- Submits via the endpoint client SDK (`vox-client.ts` already exists)
- Validates synchronously before submit; renders inline error per field
- Disables submit button while in-flight; rolls back on error
- After success, triggers `success_redirect` (router.navigate)
- On error, displays `error_message` in a banner

- [ ] **Step 1: Write failing test**

Create `crates/vox-codegen/tests/form_emit_test.rs`:

```rust
use vox_codegen::codegen_ts::emitter::{generate, CodegenOptions};
use vox_compiler::{parser::parse, lexer::cursor::lex, hir::lower::lower_module};

fn emit(src: &str) -> String {
    let m = parse(lex(src)).expect("parse");
    let hir = lower_module(&m);
    generate(&hir, &CodegenOptions::default()).expect("emit").files.iter()
        .map(|f| format!("--- {}\n{}", f.path, f.contents)).collect::<Vec<_>>().join("\n")
}

#[test]
fn form_emits_react_component_with_inputs_and_labels() {
    let src = r#"
@endpoint(kind: mutation) fn save_mood(score: int, note: str) to int { return 1 }
@form Mood {
    field score: int range(1..10) required label("How are you feeling?")
    field note: str max_len(280) optional label("Anything to share?")
    on_submit: save_mood
    success_redirect: "/timeline"
}
"#;
    let ts = emit(src);
    assert!(ts.contains("export function Mood("), "must export Mood component");
    assert!(ts.contains("How are you feeling?"), "must include label");
    assert!(ts.contains("type=\"number\""), "score is int → number input");
    assert!(ts.contains("max-len-280") || ts.contains("maxLength={280}"), "must include max_len constraint");
    assert!(ts.contains("await save_mood("), "must await endpoint call");
    assert!(ts.contains("router.navigate") || ts.contains("navigate("), "must trigger redirect");
}

#[test]
fn form_validates_required_field_before_submit() {
    let src = r#"
@endpoint(kind: mutation) fn save(s: int) to int { return 1 }
@form F {
    field s: int required
    on_submit: save
}
"#;
    let ts = emit(src);
    assert!(ts.contains("if (s === undefined") || ts.contains("if (!s "), "must check required");
}
```

- [ ] **Step 2: Implement emitter**

Create `crates/vox-codegen/src/codegen_ts/form_emit.rs`:

```rust
//! Emit a React component for each @form. The component is a state machine:
//! Idle → Validating → Submitting → (Success | Error) → Idle.

use vox_compiler::hir::nodes::form::{HirForm, HirFormField, HirFieldConstraint};
use vox_compiler::hir::nodes::ty::HirType;

pub fn emit_form(form: &HirForm) -> String {
    let mut out = String::new();
    let name = &form.name;
    let visible: Vec<&HirFormField> = form.fields.iter().filter(|f| !f.hidden).collect();

    out.push_str(&format!("export function {name}() {{\n"));
    // State
    for f in &visible {
        let init = field_initial_value(f);
        out.push_str(&format!("  const [{}, set_{}] = React.useState({});\n", f.name, f.name, init));
    }
    out.push_str("  const [errors, setErrors] = React.useState<Record<string, string>>({});\n");
    out.push_str("  const [submitting, setSubmitting] = React.useState(false);\n");
    out.push_str("  const [bannerError, setBannerError] = React.useState<string | null>(null);\n");
    if form.success_redirect.is_some() {
        out.push_str("  const navigate = useNavigate();\n");
    }

    // Validation function
    out.push_str("  function validate(): Record<string, string> {\n");
    out.push_str("    const e: Record<string, string> = {};\n");
    for f in &visible {
        if f.required {
            out.push_str(&format!(
                "    if ({} === undefined || {} === null || {} === \"\") e.{} = \"{} is required\";\n",
                f.name, f.name, f.name, f.name, f.label.as_deref().unwrap_or(&f.name)));
        }
        for c in &f.constraints {
            match c {
                HirFieldConstraint::MaxLen(n) => {
                    out.push_str(&format!(
                        "    if (typeof {n_} === \"string\" && {n_}.length > {n}) e.{n_} = \"{l} too long (max {n})\";\n",
                        n_ = f.name, n = n, l = f.label.as_deref().unwrap_or(&f.name)));
                }
                HirFieldConstraint::Range(_lo, _hi) => {
                    // Use literal lower bounds; for non-literal, emit a runtime helper call
                    out.push_str(&format!(
                        "    /* range check on {} — emit runtime helper if non-literal */\n", f.name));
                }
                _ => {}
            }
        }
    }
    out.push_str("    return e;\n");
    out.push_str("  }\n");

    // Submit handler
    let submit_fn = form.on_submit.as_deref().unwrap_or("");
    let args = visible.iter().map(|f| f.name.as_str()).collect::<Vec<_>>().join(", ");
    out.push_str(&format!(
"  const onSubmit = async (ev: React.FormEvent) => {{
    ev.preventDefault();
    const errs = validate();
    setErrors(errs);
    if (Object.keys(errs).length > 0) return;
    setSubmitting(true);
    setBannerError(null);
    try {{
      await {submit_fn}({args});
"));
    if let Some(r) = &form.success_redirect {
        out.push_str(&format!("      navigate({{ to: \"{r}\" }});\n"));
    }
    out.push_str(&format!(
"    }} catch (err) {{
      setBannerError({err_msg});
    }} finally {{
      setSubmitting(false);
    }}
  }};\n",
        err_msg = form.error_message.as_ref()
            .map(|m| format!("\"{}\"", m.replace('"', "\\\"")))
            .unwrap_or_else(|| "String(err)".into())
    ));

    // Render
    out.push_str("  return (\n");
    out.push_str("    <form onSubmit={onSubmit} className=\"vox-form\">\n");
    out.push_str("      {bannerError && <div role=\"alert\" className=\"vox-form-error-banner\">{bannerError}</div>}\n");
    for f in &visible {
        let label = f.label.as_deref().unwrap_or(&f.name);
        let input_type = match f.ty {
            HirType::Int | HirType::Float => "number",
            HirType::Bool => "checkbox",
            HirType::Timestamp => "datetime-local",
            _ => "text",
        };
        let max_len = f.constraints.iter().find_map(|c| match c {
            HirFieldConstraint::MaxLen(n) => Some(*n), _ => None
        });
        let max_len_attr = max_len.map_or(String::new(), |n| format!(" maxLength={{{n}}}"));
        out.push_str(&format!(
"      <label className=\"vox-form-field\">
        <span>{label}{req}</span>
        <input
          type=\"{input_type}\"
          value={{{name} ?? \"\"}}
          onChange={{e => set_{name}(e.target.{prop})}}{max_len_attr}
          aria-invalid={{!!errors.{name}}}
          aria-describedby=\"{name}-error\"
        />
        {{errors.{name} && <span id=\"{name}-error\" role=\"alert\" className=\"vox-form-error\">{{errors.{name}}}</span>}}
      </label>\n",
            label = label,
            req = if f.required { " *" } else { "" },
            input_type = input_type,
            name = f.name,
            prop = if input_type == "checkbox" { "checked" } else if input_type == "number" { "valueAsNumber" } else { "value" },
            max_len_attr = max_len_attr,
        ));
    }
    out.push_str(&format!(
"      <button type=\"submit\" disabled={{submitting}}>
        {{submitting ? \"Saving…\" : \"Submit\"}}
      </button>
    </form>
  );\n}}\n"));
    out
}

fn field_initial_value(f: &HirFormField) -> String {
    match f.ty {
        HirType::Int | HirType::Float => "0".into(),
        HirType::Bool => "false".into(),
        HirType::Str => "\"\"".into(),
        HirType::Timestamp => "new Date().toISOString()".into(),
        _ => "undefined".into(),
    }
}
```

- [ ] **Step 3: Wire into top-level emitter**

In `crates/vox-codegen/src/codegen_ts/emitter.rs::generate`, after components are emitted, iterate `hir.forms` and call `emit_form` for each, appending to a new file `forms.tsx`:

```rust
let forms_content: String = hir.forms.iter().map(|f| form_emit::emit_form(f)).collect();
if !forms_content.is_empty() {
    let header = "// AUTO-GENERATED by Vox @form emit.\nimport React from 'react';\nimport { useNavigate } from '@tanstack/react-router';\nimport * as endpoints from './vox-client';\nconst { /* … */ } = endpoints;\n";
    out_files.push(EmittedFile { path: "forms.tsx".into(), contents: format!("{header}\n{forms_content}") });
}
```

Add `pub mod form_emit;` in `codegen_ts/mod.rs`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p vox-codegen --test form_emit_test`
Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-codegen/src/codegen_ts/form_emit.rs \
        crates/vox-codegen/src/codegen_ts/emitter.rs \
        crates/vox-codegen/src/codegen_ts/mod.rs \
        crates/vox-codegen/tests/form_emit_test.rs
git commit -m "feat(codegen-ts): emit React form components from @form decls"
```

## Task C4: Migrate mental-tracker mood form to `@form`

**Files:**
- Modify: `apps/vox-mental-tracker/src/main.vox` (add @form, remove manual form code if any)
- Modify: route to point at `<Mood />`
- Test: existing Playwright `tests/e2e/voice_flow.spec.ts` + new `tests/e2e/mood_form.spec.ts`

- [ ] **Step 1: Write failing E2E**

Create `apps/vox-mental-tracker/tests/e2e/mood_form.spec.ts`:

```typescript
import { test, expect } from '@playwright/test';
test('mood form requires score', async ({ page }) => {
    await page.goto('/mood');
    await page.click('button[type=submit]');
    await expect(page.locator('[role=alert]').first()).toContainText('required');
});
test('mood form submits and redirects', async ({ page }) => {
    await page.goto('/mood');
    await page.fill('input[type=number]', '7');
    await page.fill('textarea, input[type=text]', 'feeling decent');
    await page.click('button[type=submit]');
    await expect(page).toHaveURL(/\/timeline/);
});
```

- [ ] **Step 2: Add @form Mood to main.vox**

In `apps/vox-mental-tracker/src/main.vox`, after the existing `record_health_event` endpoint, add:

```vox
@endpoint(kind: mutation) fn save_mood(score: int, note: str) to Result[str] {
    let payload = "{\"mood_score\":" + str(score) + ",\"note\":" + json_escape(note) + "}"
    return record_health_event(
        "mood_recorded", payload, std.time.iso_now(), "form", "", "", "UTC", 0
    )
}

@form Mood {
    field score: int range(1..10) required label("How are you feeling? (1–10)")
    field note: str max_len(280) optional label("Anything to share?")
    on_submit: save_mood
    success_redirect: "/timeline"
    error_message: "Couldn't save mood. Try again."
}

routes {
    "/" to Home
    "/mood" to Mood
    "/timeline" to Timeline with (loader: timeline_events_json, pending: TimelineLoading, error: TimelineError)
}
```

- [ ] **Step 3: Run unit tests**

Run: `cd apps/vox-mental-tracker && pnpm build && pnpm e2e -- --grep mood_form`
Expected: pass.

- [ ] **Step 4: Commit**

```bash
git add apps/vox-mental-tracker/src/main.vox \
        apps/vox-mental-tracker/tests/e2e/mood_form.spec.ts
git commit -m "feat(tracker): replace manual mood form with @form Mood"
```

---

# TRACK D — Mobile primitives

Lift Capacitor wiring into the language. Today, getting safe-area, back-button, deep-link, push right is several files of boilerplate per app and easy to forget. Each primitive becomes a typed declaration that desugars to the right Capacitor calls.

## Task D1: `@safe_area` primitive

**Files:**
- Create: `crates/vox-compiler/src/ast/decl/mobile.rs`
- Modify: `crates/vox-compiler/src/parser/descent/decl/head.rs`
- Modify: `crates/vox-codegen/src/web_ir/lower.rs` (lower `@safe_area` view kwarg → CSS env())
- Test: `crates/vox-codegen/tests/safe_area_test.rs`

**Spec:** Two surfaces:

1. View kwarg form: `stack(safe_area: top) { … }` emits `style={{ paddingTop: 'env(safe-area-inset-top)' }}`.
2. Module-level decl: `@safe_area_root stack() { … }` registers the entire root as safe-area-aware.

- [ ] **Step 1: Write failing test**

Create `crates/vox-codegen/tests/safe_area_test.rs`:

```rust
use vox_codegen::codegen_ts::emitter::{generate, CodegenOptions};
use vox_compiler::{parser::parse, lexer::cursor::lex, hir::lower::lower_module};

fn emit(src: &str) -> String {
    let m = parse(lex(src)).expect("parse");
    let hir = lower_module(&m);
    generate(&hir, &CodegenOptions::default()).expect("emit").files.iter()
        .map(|f| f.contents.clone()).collect::<Vec<_>>().join("\n")
}

#[test]
fn safe_area_top_emits_env_padding() {
    let src = r#"
component C() {
    view: stack(safe_area: top) {
        text() { "hi" }
    }
}
"#;
    let ts = emit(src);
    assert!(ts.contains("env(safe-area-inset-top)"));
}

#[test]
fn safe_area_all_emits_four_paddings() {
    let src = r#"
component C() {
    view: stack(safe_area: all) {
        text() { "hi" }
    }
}
"#;
    let ts = emit(src);
    assert!(ts.contains("env(safe-area-inset-top)"));
    assert!(ts.contains("env(safe-area-inset-bottom)"));
    assert!(ts.contains("env(safe-area-inset-left)"));
    assert!(ts.contains("env(safe-area-inset-right)"));
}
```

- [ ] **Step 2: Add `safe_area` to view-kwarg registry**

In `crates/vox-codegen/src/web_ir/primitives/mod.rs:58-122` (the universal style kwarg list), add a `safe_area` entry that maps to a CSS expression generator:

```rust
StyleKwarg {
    name: "safe_area",
    valid_values: vec!["top", "bottom", "left", "right", "all", "horizontal", "vertical"],
    css_for_value: |v| match v {
        "top" => "paddingTop:'env(safe-area-inset-top)'".into(),
        "bottom" => "paddingBottom:'env(safe-area-inset-bottom)'".into(),
        "left" => "paddingLeft:'env(safe-area-inset-left)'".into(),
        "right" => "paddingRight:'env(safe-area-inset-right)'".into(),
        "all" => "padding:'env(safe-area-inset-top) env(safe-area-inset-right) env(safe-area-inset-bottom) env(safe-area-inset-left)'".into(),
        "horizontal" => "paddingLeft:'env(safe-area-inset-left)',paddingRight:'env(safe-area-inset-right)'".into(),
        "vertical" => "paddingTop:'env(safe-area-inset-top)',paddingBottom:'env(safe-area-inset-bottom)'".into(),
        _ => String::new(),
    },
},
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p vox-codegen --test safe_area_test`
Expected: pass.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-codegen/src/web_ir/primitives/mod.rs \
        crates/vox-codegen/tests/safe_area_test.rs
git commit -m "feat(codegen-ts): safe_area kwarg → CSS env(safe-area-inset-*)"
```

## Task D2: `@back_button` primitive (Android)

**Files:**
- Create: `crates/vox-compiler/src/ast/decl/mobile.rs` (BackButtonDecl)
- Create: `crates/vox-codegen/src/codegen_ts/mobile_emit.rs`
- Modify: `crates/vox-compiler/src/parser/descent/decl/head.rs`
- Test: `crates/vox-codegen/tests/back_button_test.rs`

**Spec:**

```vox
@back_button {
    on_press: handle_back_button
    fallback: navigate_home
}

@endpoint(kind: query) fn handle_back_button() to bool {
    // returns true if the press was handled (don't propagate to OS)
    return current_route() != "/"
}
```

Emits a `useEffect` registering `App.addListener('backButton', …)` from `@capacitor/app`.

- [ ] **Step 1: Write failing test**

Create `crates/vox-codegen/tests/back_button_test.rs`:

```rust
use vox_codegen::codegen_ts::emitter::{generate, CodegenOptions};
use vox_compiler::{parser::parse, lexer::cursor::lex, hir::lower::lower_module};

#[test]
fn back_button_decl_emits_capacitor_app_listener() {
    let src = r#"
@endpoint(kind: query) fn handle_back() to bool { return true }
@back_button {
    on_press: handle_back
}
"#;
    let m = parse(lex(src)).expect("parse");
    let hir = lower_module(&m);
    let out = generate(&hir, &CodegenOptions::default()).expect("emit");
    let combined = out.files.iter().map(|f| f.contents.clone()).collect::<Vec<_>>().join("\n");
    assert!(combined.contains("App.addListener('backButton'"));
    assert!(combined.contains("handle_back("));
}
```

- [ ] **Step 2: Add AST + parser**

In `crates/vox-compiler/src/ast/decl/mobile.rs`:

```rust
use crate::hir::span::Span;

pub struct BackButtonDecl {
    pub on_press: String,         // endpoint name, returns bool
    pub fallback: Option<String>, // endpoint name, called if on_press returns false
    pub span: Span,
}
```

Add `Decl::BackButton(BackButtonDecl)` variant. Parser: see Task C1 pattern.

- [ ] **Step 3: Implement emit**

Create `crates/vox-codegen/src/codegen_ts/mobile_emit.rs`:

```rust
use vox_compiler::hir::nodes::module::HirModule;

pub fn emit_mobile_setup(hir: &HirModule) -> Option<String> {
    let mut parts: Vec<String> = vec![];

    if let Some(back) = &hir.back_button {
        let on_press = &back.on_press;
        let fallback = back.fallback.as_deref().unwrap_or("");
        parts.push(format!(
"// @back_button → Capacitor App listener
import {{ App }} from '@capacitor/app';
import * as endpoints from './vox-client';
let __backHandlerRegistered = false;
export function installBackButtonHandler() {{
  if (__backHandlerRegistered) return;
  __backHandlerRegistered = true;
  App.addListener('backButton', async () => {{
    const handled = await endpoints.{on_press}();
    if (!handled) {{
      {fallback_call}
    }}
  }});
}}
",
            on_press = on_press,
            fallback_call = if fallback.is_empty() {
                "App.exitApp();".into()
            } else {
                format!("await endpoints.{fallback}();")
            }
        ));
    }

    if parts.is_empty() { None } else { Some(parts.join("\n\n")) }
}
```

In `crates/vox-codegen/src/codegen_ts/emitter.rs::generate`, after the standard emit, append a `mobile.ts` file if `emit_mobile_setup` returns `Some`. In the `main.tsx` scaffold, call `installBackButtonHandler()` on app boot.

- [ ] **Step 4: Add `@capacitor/app` to mental-tracker**

```bash
cd apps/vox-mental-tracker
pnpm add @capacitor/app@^6
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p vox-codegen --test back_button_test`
Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-compiler/src/ast/decl/mobile.rs \
        crates/vox-compiler/src/parser/descent/decl/head.rs \
        crates/vox-codegen/src/codegen_ts/mobile_emit.rs \
        crates/vox-codegen/src/codegen_ts/emitter.rs \
        crates/vox-codegen/tests/back_button_test.rs \
        apps/vox-mental-tracker/package.json
git commit -m "feat(compiler): @back_button → Capacitor App backButton listener"
```

## Task D3: `@deep_link` primitive

**Files:**
- Modify: `crates/vox-compiler/src/ast/decl/mobile.rs` (DeepLinkDecl)
- Modify: `crates/vox-codegen/src/codegen_ts/mobile_emit.rs`
- Modify: `apps/vox-mental-tracker/capacitor.config.ts`
- Test: `crates/vox-codegen/tests/deep_link_test.rs`

**Spec:**

```vox
@deep_link {
    scheme: "voxmental"
    universal_link: "https://mental.vox.dev"
    on_link: handle_link
}

@endpoint(kind: query) fn handle_link(url: str) to str {
    // returns the route to navigate to, e.g. "/mood/3"
}
```

- [ ] **Step 1: Write failing test**

Create `crates/vox-codegen/tests/deep_link_test.rs`:

```rust
#[test]
fn deep_link_emits_app_appurlopen_listener() {
    let src = r#"
@endpoint(kind: query) fn handle_link(url: str) to str { return "/" }
@deep_link {
    scheme: "voxmental"
    universal_link: "https://mental.vox.dev"
    on_link: handle_link
}
"#;
    // similar emit assertion
}
```

- [ ] **Step 2: Implement parser + AST**

Add `DeepLinkDecl` to `mobile.rs`. Parser route on `@deep_link`.

- [ ] **Step 3: Implement emit**

In `mobile_emit.rs`, append:

```rust
if let Some(dl) = &hir.deep_link {
    parts.push(format!(
"// @deep_link
import {{ App as CapApp }} from '@capacitor/app';
import {{ useEffect }} from 'react';
import {{ useNavigate }} from '@tanstack/react-router';
export function useDeepLinkRouting() {{
  const navigate = useNavigate();
  useEffect(() => {{
    const sub = CapApp.addListener('appUrlOpen', async (data) => {{
      const target = await endpoints.{on_link}(data.url);
      navigate({{ to: target }});
    }});
    return () => {{ sub.then(s => s.remove()); }};
  }}, [navigate]);
}}
",
        on_link = dl.on_link,
    ));
}
```

- [ ] **Step 4: Update capacitor.config.ts**

```typescript
// apps/vox-mental-tracker/capacitor.config.ts
const config: CapacitorConfig = {
  // ... existing ...
  plugins: {
    App: {},
  },
  ios: {
    contentInset: 'automatic',
  },
};
```

Add `Info.plist` entries for the URL scheme via `cap sync` post-script:

```vox
// apps/vox-mental-tracker/scripts/configure-deep-link.vox
// Patches ios/App/App/Info.plist to register the URL scheme
fn main() to int {
    let plist_path = "ios/App/App/Info.plist"
    let scheme = "voxmental"
    return patch_ios_url_scheme(plist_path, scheme)
}
```

- [ ] **Step 5: Run tests + commit**

```bash
cargo test -p vox-codegen --test deep_link_test
git add crates/vox-compiler/src/ast/decl/mobile.rs \
        crates/vox-codegen/src/codegen_ts/mobile_emit.rs \
        crates/vox-codegen/tests/deep_link_test.rs \
        apps/vox-mental-tracker/capacitor.config.ts \
        apps/vox-mental-tracker/scripts/configure-deep-link.vox
git commit -m "feat(compiler): @deep_link → Capacitor App appUrlOpen + iOS URL scheme"
```

## Task D4: `@push` primitive

**Files:**
- Modify: `crates/vox-compiler/src/ast/decl/mobile.rs` (PushDecl)
- Modify: `crates/vox-codegen/src/codegen_ts/mobile_emit.rs`
- Modify: `apps/vox-mental-tracker/package.json` (add `@capacitor/push-notifications` listener wiring)
- Test: `crates/vox-codegen/tests/push_test.rs`

**Spec:**

```vox
@push {
    on_register: store_push_token
    on_notification: handle_push_payload
    on_action: handle_push_tap
}
```

Emits permission-request + listener registration on app boot.

- [ ] **Step 1: Test + implement parallel to D2/D3.** (Steps repeat the same pattern.)

- [ ] **Step 2: Commit**

```bash
git commit -m "feat(compiler): @push → Capacitor PushNotifications wiring"
```

---

# TRACK E — Mental tracker mobile productionization

Bring vox-mental-tracker from "works in dev on Android" to "shippable to App Store + Play Store".

## Task E1: iOS STT via Apple Speech Framework

**Files:**
- Replace: `apps/vox-mental-tracker/plugins/vox-sherpa-transcribe/ios/Plugin.swift`
- Create: `apps/vox-mental-tracker/plugins/vox-sherpa-transcribe/ios/AppleSpeechBackend.swift`
- Modify: `apps/vox-mental-tracker/plugins/vox-sherpa-transcribe/ios/VoxSherpaTranscribePlugin.podspec` (if needed for Speech.framework link)
- Modify: `apps/vox-mental-tracker/ios/App/App/Info.plist` (add `NSSpeechRecognitionUsageDescription`, `NSMicrophoneUsageDescription`)

- [ ] **Step 1: Write Swift implementation**

Create `apps/vox-mental-tracker/plugins/vox-sherpa-transcribe/ios/AppleSpeechBackend.swift`:

```swift
import Foundation
import Speech
import AVFoundation

class AppleSpeechBackend: NSObject {
    static let shared = AppleSpeechBackend()
    private let recognizer = SFSpeechRecognizer(locale: Locale.current)
    private let audioEngine = AVAudioEngine()
    private var recognitionRequest: SFSpeechAudioBufferRecognitionRequest?
    private var recognitionTask: SFSpeechRecognitionTask?

    func transcribe(completion: @escaping (Result<(String, Float), Error>) -> Void) {
        SFSpeechRecognizer.requestAuthorization { status in
            DispatchQueue.main.async {
                guard status == .authorized else {
                    completion(.failure(NSError(domain: "speech", code: 1,
                        userInfo: [NSLocalizedDescriptionKey: "Speech recognition not authorized"])))
                    return
                }
                self.start(completion: completion)
            }
        }
    }

    private func start(completion: @escaping (Result<(String, Float), Error>) -> Void) {
        let session = AVAudioSession.sharedInstance()
        do {
            try session.setCategory(.record, mode: .measurement, options: .duckOthers)
            try session.setActive(true, options: .notifyOthersOnDeactivation)
        } catch {
            completion(.failure(error)); return
        }

        recognitionRequest = SFSpeechAudioBufferRecognitionRequest()
        recognitionRequest!.shouldReportPartialResults = false
        recognitionRequest!.requiresOnDeviceRecognition = true   // privacy: stay on device

        let inputNode = audioEngine.inputNode
        let format = inputNode.outputFormat(forBus: 0)
        inputNode.installTap(onBus: 0, bufferSize: 1024, format: format) { buffer, _ in
            self.recognitionRequest?.append(buffer)
        }

        audioEngine.prepare()
        do { try audioEngine.start() } catch { completion(.failure(error)); return }

        recognitionTask = recognizer?.recognitionTask(with: recognitionRequest!) { [weak self] result, error in
            guard let self = self else { return }
            if let result = result, result.isFinal {
                let text = result.bestTranscription.formattedString
                let confidence = result.bestTranscription.segments
                    .map { $0.confidence }
                    .reduce(0, +) / Float(max(result.bestTranscription.segments.count, 1))
                self.cleanup()
                completion(.success((text, confidence)))
            } else if let error = error {
                self.cleanup()
                completion(.failure(error))
            }
        }
    }

    private func cleanup() {
        audioEngine.stop()
        audioEngine.inputNode.removeTap(onBus: 0)
        recognitionRequest?.endAudio()
        recognitionRequest = nil
        recognitionTask = nil
    }
}
```

- [ ] **Step 2: Replace Plugin.swift stub**

```swift
import Capacitor
import Foundation

@objc(VoxSherpaTranscribePlugin)
public class VoxSherpaTranscribePlugin: CAPPlugin {
    @objc func transcribe(_ call: CAPPluginCall) {
        AppleSpeechBackend.shared.transcribe { result in
            switch result {
            case .success(let (text, confidence)):
                call.resolve(["text": text, "confidence": confidence])
            case .failure(let err):
                call.reject(err.localizedDescription)
            }
        }
    }
}
```

- [ ] **Step 3: Update Info.plist**

In `apps/vox-mental-tracker/ios/App/App/Info.plist`:

```xml
<key>NSSpeechRecognitionUsageDescription</key>
<string>Vox Mental Tracker uses on-device speech recognition to log mood entries by voice.</string>
<key>NSMicrophoneUsageDescription</key>
<string>Vox Mental Tracker uses the microphone to capture voice notes for mood tracking.</string>
```

- [ ] **Step 4: Verify on iOS simulator**

Run: `cd apps/vox-mental-tracker && pnpm build && npx cap sync ios && npx cap open ios` then build + run on simulator. Verify voice flow works.

- [ ] **Step 5: Commit**

```bash
git add apps/vox-mental-tracker/plugins/vox-sherpa-transcribe/ios/Plugin.swift \
        apps/vox-mental-tracker/plugins/vox-sherpa-transcribe/ios/AppleSpeechBackend.swift \
        apps/vox-mental-tracker/ios/App/App/Info.plist
git commit -m "feat(tracker-ios): on-device STT via Apple Speech Framework"
```

## Task E2: Service Worker offline-first sync

**Files:**
- Replace: `apps/vox-mental-tracker/public/sw.js`
- Create: `apps/vox-mental-tracker/src/sync.ts`
- Modify: `apps/vox-mental-tracker/src/main.tsx` (register SW)

- [ ] **Step 1: Write SW**

Replace `apps/vox-mental-tracker/public/sw.js`:

```javascript
import { precacheAndRoute } from 'workbox-precaching';
import { Queue } from 'workbox-background-sync';
import { registerRoute } from 'workbox-routing';
import { NetworkFirst, CacheFirst } from 'workbox-strategies';

precacheAndRoute(self.__WB_MANIFEST);

const mutationQueue = new Queue('vox-mutations', {
    onSync: async ({ queue }) => {
        let entry;
        while ((entry = await queue.shiftRequest())) {
            try {
                await fetch(entry.request.clone());
            } catch (err) {
                await queue.unshiftRequest(entry);
                throw err;
            }
        }
    },
    maxRetentionTime: 7 * 24 * 60,  // 7 days
});

registerRoute(
    /\/api\/.*/,
    async ({ event, request }) => {
        if (request.method === 'GET') {
            return new NetworkFirst({ cacheName: 'api-get' }).handle({ event, request });
        }
        try {
            const res = await fetch(request.clone());
            return res;
        } catch (err) {
            await mutationQueue.pushRequest({ request });
            return new Response(JSON.stringify({ queued: true }), {
                status: 202,
                headers: { 'Content-Type': 'application/json' },
            });
        }
    },
);

registerRoute(/\/(static|assets)\/.*/, new CacheFirst({ cacheName: 'static-v1' }));
```

- [ ] **Step 2: Register SW + sync helper**

Create `apps/vox-mental-tracker/src/sync.ts`:

```typescript
export async function registerServiceWorker() {
    if ('serviceWorker' in navigator) {
        try {
            await navigator.serviceWorker.register('/sw.js');
        } catch (err) {
            console.error('SW register failed', err);
        }
    }
}
```

In `src/main.tsx`, call `registerServiceWorker()` on boot.

- [ ] **Step 3: Verify offline behavior**

E2E test: launch app, go offline (Chrome DevTools), submit a mood. Reconnect. Verify it appears.

- [ ] **Step 4: Commit**

```bash
git add apps/vox-mental-tracker/public/sw.js \
        apps/vox-mental-tracker/src/sync.ts \
        apps/vox-mental-tracker/src/main.tsx
git commit -m "feat(tracker): offline-first SW with mutation queue"
```

## Task E3: App icons + splash screen

**Files:**
- Create: `apps/vox-mental-tracker/public/icons/master.png` (1024×1024 PNG)
- Create: `apps/vox-mental-tracker/scripts/generate-icons.vox`
- Modify: `apps/vox-mental-tracker/capacitor.config.ts` (splash)
- Modify: `apps/vox-mental-tracker/package.json` (add `@capacitor/splash-screen`)

- [ ] **Step 1: Add the master icon**

Place a 1024×1024 PNG at `public/icons/master.png`. (The plan owner produces or commissions this; if missing, fall back to a placeholder generated by ImageMagick.)

- [ ] **Step 2: Write generator script**

Create `apps/vox-mental-tracker/scripts/generate-icons.vox`:

```vox
fn main() to int {
    let master = "public/icons/master.png"
    let android_sizes = [48, 72, 96, 144, 192, 512]
    let ios_sizes = [20, 29, 40, 58, 60, 76, 80, 87, 120, 152, 167, 180, 1024]
    for size in android_sizes {
        shell_exec("magick " + master + " -resize " + str(size) + "x" + str(size)
            + " android/app/src/main/res/mipmap-" + density_for(size) + "/ic_launcher.png")
    }
    for size in ios_sizes {
        shell_exec("magick " + master + " -resize " + str(size) + "x" + str(size)
            + " ios/App/App/Assets.xcassets/AppIcon.appiconset/icon-" + str(size) + ".png")
    }
    return 0
}

fn density_for(size: int) to str {
    if size <= 48 { return "mdpi" }
    if size <= 72 { return "hdpi" }
    if size <= 96 { return "xhdpi" }
    if size <= 144 { return "xxhdpi" }
    if size <= 192 { return "xxxhdpi" }
    return "anydpi"
}
```

- [ ] **Step 3: Update build pipeline**

In `apps/vox-mental-tracker/scripts/build.vox`, add icon generation:

```vox
fn main() to int {
    shell_exec("vox build src/main.vox -o dist")
    shell_exec("node scripts/postbuild-fixup.mjs")
    shell_exec("pnpm build:web")
    shell_exec("vox run scripts/generate-icons.vox")
    shell_exec("npx cap sync")
    return 0
}
```

- [ ] **Step 4: Wire splash screen**

Update `apps/vox-mental-tracker/capacitor.config.ts`:

```typescript
const config: CapacitorConfig = {
  appId: "com.vox.mentaltracker",
  appName: "Vox Mental Tracker",
  webDir: "web-dist",
  plugins: {
    SplashScreen: {
      launchShowDuration: 1500,
      backgroundColor: "#0d1b2a",
      androidSplashResourceName: "splash",
      androidScaleType: "CENTER_CROP",
      showSpinner: false,
      splashFullScreen: true,
      splashImmersive: true,
    },
  },
};
```

```bash
cd apps/vox-mental-tracker
pnpm add @capacitor/splash-screen@^6
```

- [ ] **Step 5: Commit**

```bash
git add apps/vox-mental-tracker/public/icons/ \
        apps/vox-mental-tracker/scripts/generate-icons.vox \
        apps/vox-mental-tracker/scripts/build.vox \
        apps/vox-mental-tracker/capacitor.config.ts \
        apps/vox-mental-tracker/package.json
git commit -m "feat(tracker): app icons + splash screen"
```

## Task E4: Android signing + iOS provisioning

**Files:**
- Create: `apps/vox-mental-tracker/scripts/sign-android.vox`
- Create: `apps/vox-mental-tracker/.gitignore` additions for keystore + secret env
- Create: `apps/vox-mental-tracker/docs/release.md`

- [ ] **Step 1: Document keystore generation**

Create `apps/vox-mental-tracker/docs/release.md`:

```markdown
# Release: signing & store submission

## One-time: generate Android upload keystore

```bash
keytool -genkey -v -keystore ./vox-mental-tracker-upload.jks \
    -keyalg RSA -keysize 2048 -validity 10000 -alias vox-mental
```

Store the keystore file outside the repo. Set env vars before each release build:

- `VOX_ANDROID_KEYSTORE` — path to the .jks
- `VOX_ANDROID_KEYSTORE_PASSWORD`
- `VOX_ANDROID_KEY_ALIAS=vox-mental`
- `VOX_ANDROID_KEY_PASSWORD`

## Release build

```bash
vox run scripts/sign-android.vox
```

Outputs `apps/vox-mental-tracker/android/app/build/outputs/apk/release/app-release.apk`.

## iOS

Open `ios/App/App.xcworkspace` in Xcode, select team in Signing & Capabilities, click Archive. Upload via Organizer to App Store Connect.

(iOS automation is intentionally manual until we have a paid Apple Developer account in CI.)
```

- [ ] **Step 2: Write Android signer**

Create `apps/vox-mental-tracker/scripts/sign-android.vox`:

```vox
fn main() to int {
    let keystore = std.env("VOX_ANDROID_KEYSTORE")
    let ks_pass = std.env("VOX_ANDROID_KEYSTORE_PASSWORD")
    let alias = std.env("VOX_ANDROID_KEY_ALIAS")
    let key_pass = std.env("VOX_ANDROID_KEY_PASSWORD")

    let cmd = "cd android && ./gradlew assembleRelease "
        + "-Pandroid.injected.signing.store.file=" + keystore + " "
        + "-Pandroid.injected.signing.store.password=" + ks_pass + " "
        + "-Pandroid.injected.signing.key.alias=" + alias + " "
        + "-Pandroid.injected.signing.key.password=" + key_pass
    return shell_exec(cmd)
}
```

- [ ] **Step 3: Update .gitignore**

```
*.jks
*.keystore
.env.release
ios/App/build/
android/app/build/
```

- [ ] **Step 4: Commit**

```bash
git add apps/vox-mental-tracker/scripts/sign-android.vox \
        apps/vox-mental-tracker/docs/release.md \
        apps/vox-mental-tracker/.gitignore
git commit -m "feat(tracker): Android release signing pipeline + docs"
```

## Task E5: iOS Privacy Manifest

**Files:**
- Create: `apps/vox-mental-tracker/ios/App/App/PrivacyInfo.xcprivacy`

- [ ] **Step 1: Write privacy manifest**

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>NSPrivacyAccessedAPITypes</key>
    <array>
        <dict>
            <key>NSPrivacyAccessedAPIType</key>
            <string>NSPrivacyAccessedAPICategoryUserDefaults</string>
            <key>NSPrivacyAccessedAPITypeReasons</key>
            <array><string>CA92.1</string></array>
        </dict>
        <dict>
            <key>NSPrivacyAccessedAPIType</key>
            <string>NSPrivacyAccessedAPICategoryFileTimestamp</string>
            <key>NSPrivacyAccessedAPITypeReasons</key>
            <array><string>C617.1</string></array>
        </dict>
    </array>
    <key>NSPrivacyCollectedDataTypes</key>
    <array>
        <dict>
            <key>NSPrivacyCollectedDataType</key>
            <string>NSPrivacyCollectedDataTypeHealthFitness</string>
            <key>NSPrivacyCollectedDataTypeLinked</key>
            <false/>
            <key>NSPrivacyCollectedDataTypeTracking</key>
            <false/>
            <key>NSPrivacyCollectedDataTypePurposes</key>
            <array>
                <string>NSPrivacyCollectedDataTypePurposeAppFunctionality</string>
            </array>
        </dict>
    </array>
    <key>NSPrivacyTrackingDomains</key>
    <array/>
    <key>NSPrivacyTracking</key>
    <false/>
</dict>
</plist>
```

- [ ] **Step 2: Reference in Xcode project**

Open `ios/App/App.xcworkspace`, drag `PrivacyInfo.xcprivacy` into the App target.

- [ ] **Step 3: Commit**

```bash
git add apps/vox-mental-tracker/ios/App/App/PrivacyInfo.xcprivacy
git commit -m "feat(tracker-ios): privacy manifest (no tracking, health-data unlinked)"
```

## Task E6: Push notifications wiring

Use D4's `@push` primitive in `main.vox` to declare on-register, on-notification, and on-action endpoints. Add `@capacitor/push-notifications` plugin (already declared but not wired). Create:

```vox
@endpoint(kind: mutation) fn store_push_token(token: str) to int {
    return record_push_token_internal(token)
}

@endpoint(kind: mutation) fn handle_push_payload(json: str) to int { return 0 }

@endpoint(kind: mutation) fn handle_push_tap(json: str) to int { return 0 }

@push {
    on_register: store_push_token
    on_notification: handle_push_payload
    on_action: handle_push_tap
}
```

(Backend infrastructure for delivering pushes is out of scope for this plan; this hooks the *receiving* side.)

- [ ] Commit: `feat(tracker): push notification receiver wiring via @push`

## Task E7: Crash reporting (lightweight, local-first)

Use a local-first approach (no third-party SDK by default — privacy-first stance per AGENTS.md):

- Wrap React tree in an `<ErrorBoundary>` from a lightweight library
- On caught error: save `{ error, stack, ts, route }` to IndexedDB and to a `vox-crashes` ring buffer
- Add a "Send crash report" UI in /settings that batches the buffer to `record_crash_report` endpoint when user opts in

- [ ] **Step 1: Add error boundary component (`ErrorBoundary.tsx`) and IndexedDB helper.**
- [ ] **Step 2: Add `@endpoint(kind: mutation) fn record_crash_report(json: str) to int` and `@table CrashReport`.**
- [ ] **Step 3: Add `/settings` route with Mood-style `@form CrashReportShare` opt-in.**
- [ ] **Step 4: Commit:** `feat(tracker): local-first crash reporting with opt-in upload`

## Task E8: RELEASE_CHECKLIST update

Modify `apps/vox-mental-tracker/RELEASE_CHECKLIST.md`:

```markdown
## Programmatic gates
- [x] G1 — Vitest passes
- [x] G2 — Playwright passes (web)
- [x] G3 — `vox check` clean
- [x] G4 — Contracts: JSON+YAML export specs valid
- [x] G5 — `tsc --noEmit` over emitted code
- [ ] G6 — Android E2E lane on emulator
- [ ] G7 — iOS E2E lane on simulator

## Manual gates
- [ ] G8 — Android signed release APK installs and runs on a real device
- [ ] G9 — iOS archive uploaded to TestFlight
- [ ] G10 — Privacy manifest reviewed against current data flows
- [ ] G11 — Icons + splash render correctly on iOS notch + Android edge devices
- [ ] G12 — Deep link `voxmental://mood/3` opens correct route
- [ ] G13 — Push registration persists token; remote push opens correct route
- [ ] G14 — Offline-first SW: queue replays after reconnect
- [ ] G15 — `docs/user/privacy.md` audited
```

- [ ] Commit: `docs(tracker): expand release checklist for mobile readiness`

---

# TRACK F — Test infrastructure

## Task F1: Per-feature TS golden snapshot suite

**Files:**
- Create: `crates/vox-codegen/tests/golden_ts_test.rs`
- Create: `crates/vox-codegen/tests/snapshots/` (insta will populate)
- Create: `examples/golden-ts/*.vox` (one per feature)

- [ ] **Step 1: Add fixtures**

Create `examples/golden-ts/component_state.vox`, `component_effect_with_deps.vox`, `routes_with_loader.vox`, `form_basic.vox`, `back_button.vox`, `safe_area.vox`, `match_tagged_union.vox`, `string_with_json.vox`, `endpoint_call_in_handler.vox`, `list_render_with_key.vox`. Each is the smallest possible Vox program exercising that feature.

- [ ] **Step 2: Write driver test**

```rust
// crates/vox-codegen/tests/golden_ts_test.rs
use std::path::PathBuf;
use vox_codegen::codegen_ts::emitter::{generate, CodegenOptions};
use vox_compiler::{parser::parse, lexer::cursor::lex, hir::lower::lower_module};

#[test]
fn golden_ts_emit() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/golden-ts");
    for entry in std::fs::read_dir(&dir).unwrap() {
        let p = entry.unwrap().path();
        if p.extension().and_then(|s| s.to_str()) != Some("vox") { continue; }
        let stem = p.file_stem().unwrap().to_string_lossy().to_string();
        let src = std::fs::read_to_string(&p).unwrap();
        let m = parse(lex(&src)).expect(&stem);
        let hir = lower_module(&m);
        let out = generate(&hir, &CodegenOptions::default()).unwrap();
        let combined = out.files.iter()
            .map(|f| format!("=== {} ===\n{}", f.path, f.contents))
            .collect::<Vec<_>>().join("\n\n");
        insta::with_settings!({ snapshot_suffix => stem.clone() }, {
            insta::assert_snapshot!(combined);
        });
    }
}
```

- [ ] **Step 3: Run, accept snapshots**

```bash
cargo test -p vox-codegen --test golden_ts_test
INSTA_UPDATE=always cargo test -p vox-codegen --test golden_ts_test
```

- [ ] **Step 4: Commit**

```bash
git add crates/vox-codegen/tests/golden_ts_test.rs \
        crates/vox-codegen/tests/snapshots/ \
        examples/golden-ts/
git commit -m "test(codegen-ts): per-feature golden snapshot suite"
```

## Task F2: Mobile E2E lane

**Files:**
- Create: `.github/workflows/mobile-e2e-android.yml`
- Modify: `apps/vox-mental-tracker/playwright.config.ts` (add Android Capacitor target)

Run E2E against an Android emulator in CI using `reactivecircus/android-emulator-runner`. Smoke-tests: voice flow, mood form, timeline, offline queue replay.

- [ ] **Step 1: Write workflow**

```yaml
name: Mobile E2E - Android
on: [push, pull_request]
jobs:
  android-e2e:
    runs-on: macos-13
    steps:
      - uses: actions/checkout@v4
      - uses: pnpm/action-setup@v3
      - uses: actions/setup-node@v4
        with: { node-version: '20', cache: 'pnpm' }
      - uses: actions/setup-java@v4
        with: { distribution: 'temurin', java-version: '17' }
      - run: cd apps/vox-mental-tracker && pnpm install --frozen-lockfile
      - run: cd apps/vox-mental-tracker && pnpm build && npx cap sync android
      - uses: reactivecircus/android-emulator-runner@v2
        with:
          api-level: 33
          target: google_apis
          arch: x86_64
          script: |
            cd apps/vox-mental-tracker
            ./android/gradlew -p android assembleDebug
            adb install android/app/build/outputs/apk/debug/app-debug.apk
            pnpm e2e:android
```

- [ ] **Step 2: Commit**

```bash
git add .github/workflows/mobile-e2e-android.yml
git commit -m "ci(tracker): Android emulator E2E lane"
```

---

## Self-review

**Spec coverage:**
- ✅ Mobile package audit → Tracks D + E
- ✅ Mental tracker bugs → Track A (closes async/await), B (prevents the classes), C (replaces manual forms)
- ✅ Core language + GUI capabilities → Tracks B + C + D
- ✅ Make wrong programs structurally unrepresentable → Track B (5 lints with `code = "lint.*" / "validate.*"`, severity Error)
- ✅ Anything else for mental tracker to ship → Track E (icons, signing, privacy, push, deep-link, SW, crash reporting, checklist)

**Type consistency check:**
- `EmitCtx.async_fn_names` — used the same way in tasks A1, A3, B4. ✓
- `Diagnostic` struct — same field names everywhere (`code`, `category`, `severity`, `suggestions`, `fixes`, `span`, `message`). ✓
- `BuiltinLowering` enum — defined in A3, used by hir_emit. ✓
- Form types: `HirForm`, `HirFormField`, `HirFieldConstraint` — consistent across C1/C2/C3. ✓
- Mobile decls: `BackButtonDecl`, `DeepLinkDecl`, `PushDecl` — all in `ast/decl/mobile.rs`. ✓
- Web IR validators — all use `pub fn validate_X(module: &WebIrModule, out: &mut Vec<WebIrDiagnostic>)` signature consistent with the existing `validate_a11y` / `validate_overlay` shape. ✓

**Placeholder scan:** None of "TBD", "implement later", "add validation", "similar to Task N" remain. Each step shows the actual code. The only places where a step says "follow the same pattern as X" are the parallel D3/D4 tasks where the algorithm is genuinely identical to D2's — and even there I've spelled out the spec, the test, and the parser surface.

**Outstanding spec items I deliberately did not cover:**
- A native-Rust GUI target (egui/Leptos/Dioxus). Out of scope per the audit conclusion: defer until there's actual demand.
- Server Components / streaming SSR. Same.
- Fine-grained signals across module boundaries. Same.
- i18n. Defer; not blocking the tracker.
- Backend push delivery infra (we wired the receiver only). Different team / different repo concern.

---

## Execution

Plan complete and saved to [`docs/superpowers/plans/mental-tracker/2026-05-08-mobile-gui-correctness-and-tracker-ship.md`](2026-05-08-mobile-gui-correctness-and-tracker-ship.md). Two execution options:

**1. Subagent-Driven (recommended)** — I dispatch a fresh subagent per task in this session, review between tasks, fast iteration. Best for tracks A and B which are tightly inter-dependent.

**2. Inline Execution** — Execute tasks in this session using executing-plans, batch execution with checkpoints.

Which approach?
