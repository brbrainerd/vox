# Vox Pipeline Gap Remediation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the ~55 verified wiring gaps from `graphify-out/PIPELINE_GAP_AUDIT.md` so every Vox language construct flows `script → AST → HIR → Web IR → Rust/TS emission` without silently vanishing, panicking, or going untested.

**Architecture:** Eight self-contained phases. Phase 1 (catch-alls) is the keystone — it converts silent/ panicking gaps into compile-time or clean-diagnostic gaps, which makes every later phase's tests trivial to assert. Phases are ordered by leverage; each ends in a green build + commit and can ship alone.

**Tech Stack:** Rust (compiler/codegen), JavaScript (tree-sitter grammar), Vox (`.vox` goldens), `cargo test`, `insta` snapshots.

---

## Conventions for the executing engineer (READ FIRST)

You know Rust but nothing about this repo. Internalize these or you will break the build:

- **Per-crate tests only:** `cargo test -p <crate>` or `cargo test -p <crate> --test <file_stem>` (e.g. `cargo test -p vox-compiler --test forbidden_corpus_test`). Never `cargo test --workspace` (too slow, unrelated failures).
- **Formatting (Windows-safe):** `cargo fmt -p <crate>` for a single crate. **NEVER `cargo fmt --all`** — it overflows the Windows command line (`os error 206`). Whole-workspace format is `vox run scripts/fmt.vox`.
- **Clippy before any commit on a touched crate:** `cargo clippy -p <crate> -- -D warnings`.
- **Architecture gate:** after adding/removing files or crate deps, run `cargo run -p vox-arch-check`.
- **Exhaustive matches are your friend:** when you delete a `_ =>` catch-all on a Rust `enum`, the compiler will list every unhandled variant. That compile error IS your test for "did I cover everything."
- **The repo's "must-fail" harness** is `crates/vox-compiler/tests/forbidden_corpus_test.rs`: drop a `.vox` file under `examples/forbidden/` whose first line is `// expect-error: <CODE>` and the test runs it through the full pipeline asserting that diagnostic code. Use this for every "should now be a clean error" task.
- **Diagnostic codes:** compiler diagnostics carry stable string codes (see `crates/vox-compiler/src/<...>/diagnostics.rs` and the `compiler_diagnostic_codes_are_unique` test). When this plan says "emit code `vox.lower.unsupported_decl`", add it like the neighbouring codes in the same file.
- **Commit cadence:** one commit per task (the final step of each task). Branch is already a worktree branch; do not open PRs unless asked.
- **The reference pattern for "this construct can't run on this target, say so clearly" (CR-F4)** lives at `crates/vox-compiler/src/eval/expr.rs:513-524` (the `Scrape`/`Browser` interp guard). Mirror it whenever this plan says "emit a CR-F4-style diagnostic."

When a step says **"mirror <file>:<line>"** it means: open that exact location, read the existing handled sibling, and apply the same shape to the new variant. The transformation is always shown in the step; the reference tells you the local idioms (helper names, error constructors).

---

## Phase 0 — Persistent OpenRouter backend (ALREADY DONE — verify only)

Done in the session that produced this plan. Verify it still holds; do not redo.

- [ ] **Step 1: Verify the persistent backend resolves to OpenRouter**

Run:
```bash
PY=$(cat graphify-out/.graphify_python)
unset GEMINI_API_KEY
"$PY" -c "import graphify.llm as l; print(l.detect_backend()); print('openrouter' in l.BACKENDS)"
```
Expected: prints `openrouter` then `True`. Config lives at `~/.graphify/providers.json`; the placeholder `GEMINI_API_KEY` was removed from Windows User env (backup at `~/.graphify/removed-GEMINI_API_KEY.bak`); the skill note is in `~/.claude/skills/graphify/SKILL.md`. If it prints `None`, confirm `OPENROUTER_API_KEY` is set in the User environment.

No commit (config is outside the repo).

---

## Phase 1 — Close the three catch-alls (KEYSTONE)

**Why first:** the audit's Pattern A. Three catch-alls each hide gaps differently: AST→HIR silently drops to `legacy_ast_nodes` (`hir/lower/mod.rs:602`), HIR→Rust panics via `unreachable!()` (`codegen_rust/emit/stmt_expr.rs:749`), interp returns silent `Ok(Null)` (`eval/expr.rs:745`). We replace each with an explicit, total handler that either lowers/emits the construct or raises a clean, coded diagnostic. After this phase, "is X wired?" becomes a compile error or an asserted diagnostic, not a runtime mystery.

> Do Phase 2 (Const etc.) and Phase 6 (`when{}`) BEFORE flipping the AST→HIR catch-all to a hard error, because they remove real variants from the catch-all. Order within this phase: 1A (interp, safe now) → 1B (Rust emit, safe now) → 1C (AST→HIR diagnostic, after Phase 2/6). The steps below note this.

### Task 1A: Interpreter — replace silent `Ok(Null)` with a coded "unsupported in interp" diagnostic

**Files:**
- Modify: `crates/vox-compiler/src/eval/expr.rs:745` (the `_ => Ok(VoxValue::Null)` catch-all in `eval_expr`)
- Test: `crates/vox-compiler/tests/interpreter_test.rs`

- [ ] **Step 1: Write the failing test**

Append to `crates/vox-compiler/tests/interpreter_test.rs`:
```rust
#[test]
fn interp_rejects_unsupported_expr_with_clean_diagnostic() {
    // `spawn` is a web/actor construct with no interpreter semantics.
    // It must produce an actionable error, never a silent Null.
    let src = "fn main() { let a = spawn Worker() }";
    let err = vox_compiler::eval::run_source_for_test(src)
        .expect_err("spawn must not silently evaluate to Null in --mode interp");
    let msg = format!("{err}");
    assert!(
        msg.contains("not supported in --mode interp"),
        "expected a CR-F4-style interp diagnostic, got: {msg}"
    );
}
```
If `eval::run_source_for_test` does not exist, mirror how the existing tests in this file invoke the interpreter (open the top of `interpreter_test.rs` and copy the harness call they use; substitute the `src` above).

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-compiler --test interpreter_test interp_rejects_unsupported_expr_with_clean_diagnostic`
Expected: FAIL — currently returns `Ok(Null)`, so no error is produced.

- [ ] **Step 3: Replace the catch-all**

In `crates/vox-compiler/src/eval/expr.rs`, change the final arm of the `eval_expr` match (currently around line 745):
```rust
            // BEFORE:
            _ => Ok(VoxValue::Null),
```
to an explicit per-variant arm that names the construct (mirror the CR-F4 message at `eval/expr.rs:513-524`):
```rust
            HirExpr::With(..) => Err(EvalError::AssertionFailed(
                "with(...) is not supported in --mode interp; run the compiled backend".into())),
            HirExpr::Spawn(..) => Err(EvalError::AssertionFailed(
                "spawn is not supported in --mode interp; run the compiled backend".into())),
            HirExpr::WorkflowVersion(..) => Err(EvalError::AssertionFailed(
                "workflow.version(...) is a compiled-workflow marker; not supported in --mode interp".into())),
            HirExpr::AsyncView(..) => Err(EvalError::AssertionFailed(
                "Async[T] when-views are not supported in --mode interp; use the web/compiled backend".into())),
            HirExpr::Jsx(..) | HirExpr::JsxSelfClosing(..) | HirExpr::JsxFragment(..) => Err(EvalError::AssertionFailed(
                "JSX is not supported in --mode interp; use the web backend".into())),
```
Do **not** keep a trailing `_ =>`. Removing it makes the match exhaustive; if the compiler complains about a missing variant, add it with the same `AssertionFailed("<name> is not supported in --mode interp")` shape — that compiler error is the audit's "what else does interp silently drop" list.

(`HirBinOp::Pipe` is handled separately in Task 7A — it lives in the `Binary` arm, not this catch-all.)

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-compiler --test interpreter_test interp_rejects_unsupported_expr_with_clean_diagnostic`
Expected: PASS. Then `cargo test -p vox-compiler --test interpreter_test` (whole file green) and `cargo test -p vox-compiler --test eval_typeck_parity_test`.

- [ ] **Step 5: Commit**

```bash
cargo clippy -p vox-compiler -- -D warnings
cargo fmt -p vox-compiler
git add crates/vox-compiler/src/eval/expr.rs crates/vox-compiler/tests/interpreter_test.rs
git commit -m "fix(interp): replace silent Ok(Null) catch-all with coded unsupported-construct diagnostics"
```

### Task 1B: Rust codegen — replace panicking `unreachable!()` with a coded codegen diagnostic

**Files:**
- Modify: `crates/vox-codegen/src/codegen_rust/emit/stmt_expr.rs:749`
- Test: `crates/vox-codegen/tests/emit_unsupported_expr_test.rs` (Create)

- [ ] **Step 1: Write the failing test**

Create `crates/vox-codegen/tests/emit_unsupported_expr_test.rs`:
```rust
// A JSX expression reaching the Rust (server/script) emitter must produce a
// recoverable codegen error, never panic the compiler.
#[test]
fn rust_emitter_does_not_panic_on_frontend_expr() {
    let src = "@server fn handler() -> Int { let x = <div></div>; 1 }";
    // mirror how the other tests in crates/vox-codegen/tests/ build + emit Rust:
    let result = std::panic::catch_unwind(|| {
        vox_codegen::emit_rust_for_test(src) // see note below
    });
    assert!(result.is_ok(), "Rust emitter panicked on a JSX expr instead of erroring cleanly");
}
```
Find the real emit entry point by opening any existing test in `crates/vox-codegen/tests/` (e.g. `binary_op_emit.rs`) and copying its parse→lower→emit call; substitute `src`. The assertion is: it returns (an `Err` or a diagnostic), it does not `panic`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-codegen --test emit_unsupported_expr_test`
Expected: FAIL — `unreachable!()` panics, so `catch_unwind` returns `Err`.

- [ ] **Step 3: Replace the `unreachable!`**

At `crates/vox-codegen/src/codegen_rust/emit/stmt_expr.rs:749`, change:
```rust
            _ => unreachable!("HIR expr variants not handled ... (delegate order bug)"),
```
to an explicit arm per dropped variant that returns the emitter's error type (look 30 lines up for how this function signals errors — it either returns `Result<String, _>` or pushes a diagnostic; use that same channel):
```rust
            HirExpr::Jsx(..) | HirExpr::JsxSelfClosing(..) | HirExpr::JsxFragment(..) | HirExpr::AsyncView(..) =>
                return Err(emit_error("vox.codegen_rust.frontend_expr_in_server",
                    "JSX / async-view expressions cannot be emitted to the Rust (server/script) target")),
            HirExpr::Spawn(..) =>
                return Err(emit_error("vox.codegen_rust.spawn_unimplemented",
                    "spawn-expression Rust emission is not yet implemented (see Task 8 of the gap plan)")),
            HirExpr::WorkflowVersion(..) =>
                return Err(emit_error("vox.codegen_rust.workflow_version_unimplemented",
                    "workflow.version() Rust emission is not yet implemented (see Task 8 of the gap plan)")),
```
Replace `emit_error(code, msg)` with whatever this module already uses to construct an error (grep this file for how other arms build errors). Remove the `_ =>`; if the compiler flags another missing variant, add it the same way.

> NOTE: `Spawn` and `WorkflowVersion` get *real* Rust emission in Task 8B/8C. Until then a clean error beats a panic.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-codegen --test emit_unsupported_expr_test`
Expected: PASS. Then `cargo test -p vox-codegen` (crate green).

- [ ] **Step 5: Commit**

```bash
cargo clippy -p vox-codegen -- -D warnings
cargo fmt -p vox-codegen
git add crates/vox-codegen/src/codegen_rust/emit/stmt_expr.rs crates/vox-codegen/tests/emit_unsupported_expr_test.rs
git commit -m "fix(codegen_rust): replace unreachable!() expr catch-all with recoverable codegen diagnostics"
```

### Task 1C: AST→HIR — turn the silent `legacy_ast_nodes` drop into a coded diagnostic

> **DEPENDENCY:** do this AFTER Phase 2 (wires Const/Config/Theme/Skill/AgentDef/Message) so the only things left hitting the catch-all are genuinely-unknown future decls.

**Files:**
- Modify: `crates/vox-compiler/src/hir/lower/mod.rs:602-604`
- Test: `crates/vox-compiler/tests/forbidden_corpus_test.rs` via a new `examples/forbidden/unknown_decl_is_diagnosed.vox`

- [ ] **Step 1: Write the failing fixture-test**

The `legacy_ast_nodes` push is fine for *truly unknown* decls, but it must also raise a diagnostic so nothing is silently dropped. After Phase 2 there is no current decl that hits it, so this task hardens the path. Add the diagnostic emission at the push site:

In `crates/vox-compiler/src/hir/lower/mod.rs`, change:
```rust
                _ => {
                    hir.legacy_ast_nodes.push(decl.clone());
                }
```
to:
```rust
                _ => {
                    hir.diagnostics.push(Diagnostic::warning(
                        "vox.lower.unlowered_decl",
                        format!("declaration kind `{}` has no HIR lowering and was dropped", decl.kind_name()),
                        decl.span(),
                    ));
                    hir.legacy_ast_nodes.push(decl.clone());
                }
```
If `hir.diagnostics` / `Diagnostic::warning` / `decl.kind_name()` differ here, mirror how the neighbouring `note_lowering_gaps` helper (referenced at `web_ir/lower.rs:677-691`) builds its diagnostic and how `Decl::span()` is called (it exists — `vox-ast/src/decl/types.rs:206`). Add `kind_name()` to `Decl` if absent (a `match self { Decl::Const(_) => "const", ... }` returning `&'static str`).

- [ ] **Step 2: Add the assertion test**

Append to `crates/vox-compiler/tests/forbidden_corpus_test.rs` (or the nearest lowering test) a positive test asserting a known decl no longer warns:
```rust
#[test]
fn const_decl_no_longer_falls_into_legacy_nodes() {
    let hir = vox_compiler::lower_source_for_test("const MAX = 10");
    assert!(hir.legacy_ast_nodes.is_empty(),
        "const must lower to HIR after Phase 2, not legacy_ast_nodes: {:?}", hir.legacy_ast_nodes);
}
```
(Uses Phase 2's wiring; this test will pass only once Phase 2 lands — that's intended ordering.)

- [ ] **Step 3: Run**

Run: `cargo test -p vox-compiler --test forbidden_corpus_test`
Expected: PASS (after Phase 2). The warning path is exercised by any genuinely-unknown decl.

- [ ] **Step 4: Commit**

```bash
cargo clippy -p vox-compiler -- -D warnings
cargo fmt -p vox-compiler
git add crates/vox-compiler/src/hir/lower/mod.rs crates/vox-ast/src/decl/types.rs crates/vox-compiler/tests/forbidden_corpus_test.rs
git commit -m "fix(lower): emit vox.lower.unlowered_decl diagnostic instead of silently dropping to legacy_ast_nodes"
```

---

## Phase 2 — Wire the six swallowed declarations

**Why:** Pattern A's AST→HIR victims. `Decl::Const, Config, Theme, Skill, AgentDef, Message` hit the catch-all and vanish. The highest-impact is `Const`: the parser turns **every top-level `let x = …`** into `Decl::Const` (`parser/descent/mod.rs:627`), so module-level bindings disappear from codegen entirely. We add HIR nodes + lowering for each, mirroring the handled `McpTool` (`hir/lower/mod.rs:225`) and `Tokens` (`mod.rs:547`) siblings.

### Task 2A: HIR `HirConst` node + lowering for `Decl::Const`

**Files:**
- Modify: `crates/vox-compiler/src/hir/nodes/decl.rs` (add `HirConst` struct + `consts: Vec<HirConst>` field on `HirModule`; field list at `decl.rs:163-194`)
- Modify: `crates/vox-compiler/src/hir/lower/mod.rs` (add a `Decl::Const(c) => …` arm before the catch-all)
- Modify: `crates/vox-compiler/src/hir/lower/decl.rs` (add `lower_const` helper)
- Test: `crates/vox-compiler/tests/const_lowering_test.rs` (Create)

- [ ] **Step 1: Write the failing test**

Create `crates/vox-compiler/tests/const_lowering_test.rs`:
```rust
#[test]
fn top_level_const_lowers_to_hir_consts() {
    let hir = vox_compiler::lower_source_for_test("const MAX_RETRIES = 3");
    assert_eq!(hir.consts.len(), 1, "const must produce a HirConst");
    assert_eq!(hir.consts[0].name, "MAX_RETRIES");
    assert!(hir.legacy_ast_nodes.is_empty(), "const must not fall into legacy_ast_nodes");
}

#[test]
fn top_level_let_lowers_to_hir_consts() {
    // parser emits Decl::Const for a top-level `let` (descent/mod.rs:627)
    let hir = vox_compiler::lower_source_for_test("let base_url = \"https://api.example.com\"");
    assert_eq!(hir.consts.len(), 1);
    assert_eq!(hir.consts[0].name, "base_url");
}
```
If `lower_source_for_test` doesn't exist, add a thin test helper to `crates/vox-compiler/src/lib.rs` behind `#[cfg(any(test, feature = "test-helpers"))]` that runs `lex → parse → lower_module` and returns the `HirModule`; mirror the existing parse-then-lower used in `web_ir_lower_emit_test.rs`.

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-compiler --test const_lowering_test`
Expected: FAIL — `hir.consts` field does not exist (compile error) → that's your first signal.

- [ ] **Step 3: Add the HIR node + field**

In `crates/vox-compiler/src/hir/nodes/decl.rs`, near the other small HIR decl structs:
```rust
#[derive(Debug, Clone, PartialEq)]
pub struct HirConst {
    pub name: String,
    pub value: HirExpr,
    pub type_ann: Option<HirType>,
    pub is_pub: bool,
    pub span: Span,
}
```
Add to `HirModule` (in the struct + its constructor/`Default`):
```rust
    pub consts: Vec<HirConst>,
```
Update the `field_ownership_map` near `decl.rs:163-194` to include `consts` (mirror how `tokens`/`forms` are listed).

- [ ] **Step 4: Add the lowering arm + helper**

In `crates/vox-compiler/src/hir/lower/mod.rs`, before the `_ =>` catch-all:
```rust
                Decl::Const(c) => {
                    let lowered = self.lower_const(c);
                    hir.consts.push(lowered);
                }
```
In `crates/vox-compiler/src/hir/lower/decl.rs` add (mirror `lower_type` at `decl.rs:268` for the type, and `lower_expr` usage elsewhere):
```rust
    pub(crate) fn lower_const(&mut self, c: &crate::ast::decl::ConstDecl) -> HirConst {
        HirConst {
            name: c.name.clone(),
            value: self.lower_expr(&c.value),
            type_ann: c.type_ann.as_ref().map(|t| self.lower_type(t)),
            is_pub: c.is_pub,
            span: c.span,
        }
    }
```

- [ ] **Step 5: Run to verify it passes**

Run: `cargo test -p vox-compiler --test const_lowering_test`
Expected: PASS. Then `cargo test -p vox-compiler` (catch any field-init sites you missed).

- [ ] **Step 6: Commit**

```bash
cargo clippy -p vox-compiler -- -D warnings
cargo fmt -p vox-compiler
git add crates/vox-compiler/src/hir/nodes/decl.rs crates/vox-compiler/src/hir/lower/mod.rs crates/vox-compiler/src/hir/lower/decl.rs crates/vox-compiler/tests/const_lowering_test.rs crates/vox-compiler/src/lib.rs
git commit -m "feat(hir): lower Decl::Const (incl. top-level let) into HirModule.consts"
```

### Task 2B: Rust emission for `HirConst`

**Files:**
- Modify: `crates/vox-codegen/src/codegen_rust/emit/mod.rs` (call site that emits module items) + a new `emit_const` (mirror `emit_fn` at `emit/workflow.rs`)
- Test: `crates/vox-codegen/tests/const_emit_test.rs` (Create)

- [ ] **Step 1: Write the failing test**

Create `crates/vox-codegen/tests/const_emit_test.rs`:
```rust
#[test]
fn const_emits_rust_const() {
    let rust = vox_codegen::emit_rust_for_test("const MAX = 3\nfn main() -> Int { MAX }");
    assert!(rust.contains("const MAX") || rust.contains("static MAX"),
        "expected an emitted Rust const for MAX, got:\n{rust}");
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-codegen --test const_emit_test`
Expected: FAIL — no const is emitted.

- [ ] **Step 3: Emit consts**

Find where module-level items are emitted in `crates/vox-codegen/src/codegen_rust/emit/mod.rs` (grep for `for f in &hir.fns` or the function that walks `hir`), and add a loop over `hir.consts` that emits `const <NAME>: <ty> = <value>;` (map the type via the existing type emitter `emit/types.rs`; emit the value via `emit_expr_with`). Keep it minimal: `const` for primitive types, `static` only if the value is non-const.

- [ ] **Step 4: Run / Commit**

Run: `cargo test -p vox-codegen --test const_emit_test` → PASS, then `cargo test -p vox-codegen`.
```bash
cargo clippy -p vox-codegen -- -D warnings && cargo fmt -p vox-codegen
git add crates/vox-codegen/src/codegen_rust/emit/mod.rs crates/vox-codegen/tests/const_emit_test.rs
git commit -m "feat(codegen_rust): emit module-level consts from HirModule.consts"
```

### Task 2C: TS emission for `HirConst`

**Files:**
- Modify: `crates/vox-codegen-ts/src/hir_emit/mod.rs` (module-item walk)
- Test: `crates/vox-codegen-ts/tests/const_ts_emit_test.rs` (Create — first test file for this crate; add `[[test]]` discovery works by default)

- [ ] **Step 1: Failing test**
```rust
#[test]
fn const_emits_ts_const() {
    let ts = vox_codegen_ts::emit_ts_for_test("const MAX = 3");
    assert!(ts.contains("const MAX = 3"), "got:\n{ts}");
}
```
(Mirror the emit entry used in `crates/vox-codegen/tests/golden_ts_test.rs` for `emit_ts_for_test`.)

- [ ] **Step 2-4: Run-fail → emit `export const <name> = <value>;` in the module walk of `hir_emit/mod.rs` → Run-pass.**

- [ ] **Step 5: Commit** `feat(codegen-ts): emit module-level consts`

### Task 2D: Config / Theme / Message / Skill / AgentDef lowering

For each of these five, the **minimum real fix** is: add a HIR node + lowering arm so it leaves `legacy_ast_nodes`, then decide emission per construct. Skill and AgentDef wrap a `FnDecl` whose body must reach `lower_fn` (mirror `McpTool` at `hir/lower/mod.rs:225`).

- [ ] **Step 1 (Config): failing test** in `crates/vox-compiler/tests/decl_lowering_test.rs` (Create):
```rust
#[test]
fn config_decl_lowers_out_of_legacy_nodes() {
    let hir = vox_compiler::lower_source_for_test("@config Settings { api_key: String }");
    assert!(hir.legacy_ast_nodes.is_empty());
    assert_eq!(hir.configs.len(), 1);
}
```
- [ ] **Step 2:** add `HirConfig { name, fields: Vec<(String, HirType)>, span }` to `hir/nodes/decl.rs`, `configs: Vec<HirConfig>` to `HirModule`, and a `Decl::Config(c) => hir.configs.push(self.lower_config(c))` arm. Config feeds `~/.vox/config.toml` resolution (see memory `project_gui_configurable_values_secrets_plan_2026`); for now lowering + a typed HIR node is the deliverable, emission is out of scope (document with a `// codegen: config is resolved at runtime from Vox.toml, not emitted` comment).
- [ ] **Step 3 (Theme):** test `theme_decl_lowers`; add `HirTheme { name, tokens: Vec<(String,String)>, span }` + `themes` field + arm. Theme should emit alongside `Tokens` — wire `emit_tokens_css`/`_ts` (`codegen-ts/src/tokens_emit.rs`) to also consume `hir.themes` (add a test asserting the theme's CSS variables appear in `emit_tokens_css` output).
- [ ] **Step 4 (Message):** test `message_decl_lowers`; add `HirMessage { name, fields, span }` + `messages` field + arm. Message shapes feed actor/agent typing; lowering + node is the deliverable.
- [ ] **Step 5 (Skill):** test asserting the skill's inner fn is lowered:
```rust
#[test]
fn skill_inner_fn_reaches_hir() {
    let hir = vox_compiler::lower_source_for_test("@skill fn summarize(x: String) -> String { x }");
    assert!(hir.fns.iter().any(|f| f.name == "summarize"), "skill body must be lowered via lower_fn");
}
```
  Add a `Decl::Skill(s) => { let f = self.lower_fn(&s.func); hir.fns.push(f); /* + HirSkill metadata */ }` arm (mirror `McpTool` at `mod.rs:225`). Optionally add `HirSkill { name, fn_name, span }` to record the skill wrapper.
- [ ] **Step 6 (AgentDef):** same shape as Skill — `Decl::AgentDef(a) => { let f = self.lower_fn(&a.func); hir.fns.push(f); }`. Note the existing handled `Decl::Agent` (`mod.rs:513`) is a *different* variant; do not confuse them.
- [ ] **Step 7:** `cargo test -p vox-compiler --test decl_lowering_test` → PASS; `cargo test -p vox-compiler`.
- [ ] **Step 8: Commit** `feat(hir): lower Config/Theme/Message/Skill/AgentDef decls out of legacy_ast_nodes`

---

## Phase 3 — Behavioral golden harness (the missing gate)

**Why:** Pattern F. The 71 `examples/golden/**.vox` get parse + lower + web_ir-validate only; there is **no interpreter/runtime-output harness** and the TS-snapshot test points at the wrong dir. This gate is what prevents every other fix from regressing.

### Task 3A: Run the whole golden corpus through the interpreter

**Files:**
- Test: `crates/vox-compiler/tests/golden_interp_test.rs` (Create)

- [ ] **Step 1: Write the harness test**
```rust
use std::path::PathBuf;

/// Every golden that declares `fn main()` must execute under --mode interp without
/// error (or with an explicitly-allowed, coded diagnostic). This catches silent
/// runtime drops like the Pipe/With/Spawn interp gaps.
#[test]
fn all_runnable_goldens_execute_under_interp() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/golden");
    let mut failures = vec![];
    for entry in walk_vox_files(&root) {
        let src = std::fs::read_to_string(&entry).unwrap();
        if !src.contains("fn main") { continue; }
        if let Err(e) = vox_compiler::eval::run_source_for_test(&src) {
            // allow the documented interp-N/A diagnostics (web/actor constructs)
            let m = format!("{e}");
            if m.contains("not supported in --mode interp") { continue; }
            failures.push(format!("{}: {m}", entry.display()));
        }
    }
    assert!(failures.is_empty(), "goldens failed under interp:\n{}", failures.join("\n"));
}
```
Add a small `walk_vox_files` helper (or reuse the one in `golden_vox_examples_test.rs` — grep for how it enumerates the corpus and copy it).

- [ ] **Step 2: Run — expect failures (this is discovery)**

Run: `cargo test -p vox-compiler --test golden_interp_test -- --nocapture`
Expected: FAIL initially, listing goldens that error (e.g. any using `|>` before Task 7A, or `std.mobile`/`std.crypto` before Task 6). **Triage:** for each failure, either it's a real gap fixed in a later phase (add the golden to a documented `KNOWN_INTERP_GAPS` allowlist in this test with a `// fixed by Task X` note) or it's a new bug to fix now.

- [ ] **Step 3: Add the allowlist + make green**

Add `const KNOWN_INTERP_GAPS: &[&str] = &[/* "examples/golden/foo.vox", // Task 7A */];` and skip those, so the test is green now and each later phase deletes its entry. This makes the gate live immediately and self-documenting.

- [ ] **Step 4: Commit** `test(golden): execute the golden corpus under --mode interp with a tracked gap allowlist`

### Task 3B: Point the TS-emit snapshot harness at the real corpus

**Files:**
- Modify: `crates/vox-codegen/tests/golden_ts_test.rs:17`

- [ ] **Step 1:** Change the dir from `examples/golden-ts` to also cover `examples/golden` (or add a second `#[test]` `golden_corpus_ts_snapshots` iterating `examples/golden/**.vox` that have a component/view, running parse→typeck→generate-TS and `insta::assert_snapshot!`). Mirror the existing test body exactly; only the directory and snapshot-name prefix change.
- [ ] **Step 2:** Run `cargo test -p vox-codegen --test golden_ts_test`; on first run `INSTA_UPDATE=always cargo test -p vox-codegen --test golden_ts_test` to accept the new snapshots, then **review each `.snap`** for obviously-wrong output (empty TSX, `/* async view */` stubs — those are real gaps for Phase 8/later, note them).
- [ ] **Step 3: Commit** `test(golden): TS-emit snapshots over the real examples/golden corpus`

### Task 3C: Behavioral compile-net — un-ignore the cargo-build goldens

**Files:**
- Modify: `crates/vox-codegen/tests/emit_compile_harness.rs` (remove `#[ignore]` from the ~12 script goldens that now compile; keep `#[ignore]` only where a later phase is required, with a `// ignore: needs Task X` note)

- [ ] **Step 1-3:** Remove `#[ignore]` one file at a time; run `cargo test -p vox-codegen --test emit_compile_harness -- --include-ignored` to see which actually `cargo build` clean; keep ignores only for genuinely-unimplemented constructs (annotate each with the blocking task). Commit `test(golden): un-ignore compiling script goldens in emit_compile_harness`.

---

## Phase 4 — The decorator cliff (decide: emit or hard-error, never silent)

**Why:** Pattern B + C. `@inference/@training_step/@distributed_train/@remote/@webhook/@pii/@embed/@auth` parse+lower+typecheck but no codegen reads them (write-only placeholders), and `@webhook/@cors/@rate_limit/@pii/@layer` on a bare fn are silently dropped. The **decision** (your call as implementer, default below): MENS/mesh decorators (`@inference/@training_step/@distributed_train/@remote`) are a large separate epic — make them an explicit, coded codegen error so they never silently no-op. Security/HTTP decorators on the wrong target get a clean parse/typeck error.

### Task 4A: Bare-fn HTTP/security decorators → clean typeck error (Pattern C)

**Files:**
- Modify: `crates/vox-compiler/src/typeck/ast_decl_lints.rs` (add a lint)
- Test: `examples/forbidden/webhook_on_bare_fn.vox` (Create) + it runs via `forbidden_corpus_test.rs`

- [ ] **Step 1:** Create `examples/forbidden/webhook_on_bare_fn.vox`:
```
// expect-error: vox.typeck.decorator_requires_endpoint
@webhook(provider: "stripe")
fn helper() -> Int { 1 }
```
- [ ] **Step 2:** Run `cargo test -p vox-compiler --test forbidden_corpus_test` → FAIL (no such diagnostic yet).
- [ ] **Step 3:** In `typeck/ast_decl_lints.rs`, add a check: if a `FnDecl` carries `webhook`/`cors_spec`/`rate_limit`/`pii` but is not an endpoint (`@query/@mutation/@server`), or carries `layer` but is not a component, emit `Diagnostic::error("vox.typeck.decorator_requires_endpoint", "...@webhook requires @query/@mutation/@server...", fn.span)`. Mirror the existing `check_*` lints in this file (e.g. the `@layer` system-overlay guard already here).
- [ ] **Step 4:** Run → PASS. Commit `fix(typeck): error on HTTP/security decorators applied to a bare fn instead of silently dropping`.

### Task 4B: `@auth`/`@offline_capable`/`@collaborative` — populate AST fields (stop swallowing args)

**Files:**
- Modify: `crates/vox-compiler/src/parser/descent/decl/head_fn.rs:946-951` (currently `skip_paren_args_inner()` discards args)
- Modify: `crates/vox-ast/src/decl/fundecl.rs` (`auth_provider`, `roles` already exist at :148,150)
- Modify: `crates/vox-compiler/src/hir/lower/decl.rs` (copy into a new `HirFn.auth`)
- Test: `crates/vox-compiler/tests/auth_decorator_test.rs` (Create)

- [ ] **Step 1: Failing test**
```rust
#[test]
fn auth_decorator_populates_fields() {
    let ast = vox_compiler::parse_source_for_test("@auth(provider: \"clerk\", roles: [\"admin\"]) @server fn admin() -> Int { 1 }");
    let f = ast.first_fn();
    assert_eq!(f.auth_provider.as_deref(), Some("clerk"));
    assert_eq!(f.roles, vec!["admin".to_string()]);
}
```
- [ ] **Step 2:** Run → FAIL (fields stay `None`/empty).
- [ ] **Step 3:** At `head_fn.rs:946-951`, replace the `skip_paren_args_inner()` discard for `Token::AtAuth` with a real arg parse that sets `f.auth_provider` / `f.roles` (mirror how `@cors` parses `origins:` args at `head_fn.rs:691-765`). Leave `@offline_capable`/`@collaborative` as boolean flags (`f.is_offline_capable = true`).
- [ ] **Step 4:** Add `HirFn.auth: Option<HirAuth>` to `hir/nodes/decl.rs` and copy it in `lower_fn` (`hir/lower/decl.rs:53-231`). Then add Rust emit in `codegen_rust/emit/http.rs` (mirror the cors layer at `http.rs:28-58`) that emits an auth-guard middleware when `endpoint.auth` is set. Add a golden `examples/golden/auth_guard.vox` and a behavioral assertion (extend `openapi_crud_api_test.rs` style).
- [ ] **Step 5:** Run `cargo test -p vox-compiler --test auth_decorator_test` + `cargo test -p vox-codegen`. Commit `feat(auth): parse @auth args, carry to HIR, emit Rust auth-guard middleware`.

### Task 4C: MENS/mesh decorators → explicit coded codegen error (no silent no-op)

**Files:**
- Modify: `crates/vox-codegen/src/codegen_rust/emit/mod.rs` (where fns are emitted)
- Test: `examples/forbidden/inference_decorator_unimplemented.vox` (Create)

- [ ] **Step 1:** Create the fixture:
```
// expect-error: vox.codegen.mens_decorator_unimplemented
@inference(model: "qwen3.5-2b")
fn classify(text: String) -> String { text }
```
- [ ] **Step 2:** Run forbidden test → FAIL.
- [ ] **Step 3:** In the Rust emit fn-walk, if `hir_fn.inference_model.is_some() || hir_fn.training_step || hir_fn.distributed_train.is_some() || hir_fn.is_remote`, return `Err(emit_error("vox.codegen.mens_decorator_unimplemented", "@inference/@training_step/@distributed_train/@remote codegen is not yet implemented; run via the MENS runtime (vox-populi) — see project_mesh_mens_distributed_plan_2026"))`. This converts the write-only placeholders into an honest, traceable error. (Wiring real MENS codegen is tracked separately as `project_mesh_mens_distributed_plan_2026`; do **not** attempt it here.)
- [ ] **Step 4:** Run → PASS. Commit `fix(codegen): error on unimplemented MENS/mesh decorators instead of emitting placeholder no-ops`.

### Task 4D: `@rate_limit by:user_id|api_key` Rust emission

**Files:**
- Modify: `crates/vox-codegen/src/codegen_rust/emit/http.rs:111-122` (only `Ip` emits today)
- Test: `crates/vox-codegen/tests/rate_limit_emit_test.rs` (Create)

- [ ] Failing test asserting `by: user_id` emits a per-user keyed limiter (extractor on `user_id`) → implement the `UserId`/`ApiKey` arms mirroring the `Ip` prelude (`http.rs:61-62`) → pass → commit `feat(codegen_rust): emit user_id/api_key rate-limit guards, not just IP`.

### Task 4E: `@embed` / `@pii` emission (or coded N/A)

- [ ] `@pii`: add Rust emit that wraps the endpoint response in a redaction marker, OR (if redaction runtime is out of scope) emit `vox.codegen.pii_unimplemented` error like 4C. `@embed`: same decision. Pick emit if the runtime helper exists in `vox-actor-runtime` (grep for `embed`/`redact`); else coded error. One commit each. Add a forbidden or golden fixture per the choice.

---

## Phase 5 — Delete the split-brain + add the parity gate

**Why:** Pattern G. `crates/vox-codegen/src/codegen_ts/reactive/` is a 6-file git-tracked dead duplicate rustc never compiles (`lib.rs:15-16` `#[path]`-redirects `mod codegen_ts` to the `vox-codegen-ts` crate). And WebIR re-derives endpoint contracts/views in three places with no CI gate.

### Task 5A: Remove the orphan duplicate directory

**Files:**
- Delete: `crates/vox-codegen/src/codegen_ts/reactive/{bindings,effects,hooks,imports,mod,view}.rs`

- [ ] **Step 1:** Confirm it's truly unreferenced: `grep -rn "codegen_ts::reactive\|codegen_ts/reactive" crates/vox-codegen/src` — expect only the `#[path]` redirect in `lib.rs` pointing at the *vox-codegen-ts crate*, not this dir. Also confirm `crates/vox-codegen/src/codegen_ts/mod.rs` does **not** `mod reactive;` the local dir.
- [ ] **Step 2:** `git rm -r crates/vox-codegen/src/codegen_ts/reactive/`
- [ ] **Step 3:** Build: `cargo build -p vox-codegen && cargo test -p vox-codegen`. Expected: green (the files were never compiled). If anything breaks, the dir was NOT dead — revert and investigate.
- [ ] **Step 4:** `cargo run -p vox-arch-check` (orphan detector should be happier). Commit `chore(codegen): delete orphaned never-compiled codegen_ts/reactive duplicate`.

### Task 5B: WebIR↔contract parity CI gate

**Files:**
- Test: `crates/vox-codegen/tests/webir_contract_parity_test.rs` (Create)

- [ ] **Step 1:** Write a test that lowers a fixture with endpoints + a route tree and asserts the endpoint contracts derived in `web_ir/lower.rs:635` match the canonical ones from `app_contract.rs:104` (same count, same names/paths). This locks T1-1/T1-3/T1-4 so the two derivations cannot diverge. Mirror `wire_format_golden.rs` for the comparison style.
- [ ] **Step 2-4:** Run (should pass today if they're consistent; if not, that's a found bug — reconcile to the `app_contract` SSOT). Commit `test(codegen): WebIR endpoint-contract parity gate against app_contract SSOT`.

---

## Phase 6 — Finish the `Async[T] when {}` parser production

**Why:** Pattern E. Lexer tokens (`when`/`fetching`/`empty`), `HirAsyncView`, 10+ typeck passes, and an exhaustiveness checker all exist — only the parser production was never written, so `HirAsyncView` is built only under `#[cfg(test)]`.

**Files:**
- Modify: `crates/vox-compiler/src/parser/descent/expr/pratt_match.rs` (add a `Token::When` prefix production)
- Modify: `crates/vox-compiler/src/hir/lower/expr.rs` (lower `Expr::AsyncView` → `HirExpr::AsyncView`)
- Modify: `crates/vox-ast/src/expr.rs` (add `Expr::AsyncView` if absent)
- Test: `crates/vox-compiler/tests/async_view_parse_test.rs` (Create)

- [ ] **Step 1: Failing parse test**
```rust
#[test]
fn when_view_parses_all_four_arms() {
    let ast = vox_compiler::parse_source_for_test(
        "fn v(d: Async[Int]) -> Int { when d { fetching => 0 empty => 0 error e => -1 ok x => x } }");
    assert!(ast.contains_async_view(), "when{{}} must parse into Expr::AsyncView");
}
```
- [ ] **Step 2:** Run → FAIL ("Unexpected token in expression" at `when`).
- [ ] **Step 3:** Add `Expr::AsyncView { source: Box<Expr>, fetching, empty, error_binding, error, ok_binding, ok, span }` to `vox-ast/src/expr.rs` (+ its `span()` arm). In `pratt_match.rs`, register `Token::When` as an expr-start (mirror `Token::SideEffect` at `pratt_match.rs:228`) and write `parse_when_view` that consumes `when <expr> { fetching => <expr> empty => <expr> error <ident> => <expr> ok <ident> => <expr> }` (arms optional per the HIR node's `Option` arms). Consume `Token::Fetching/Empty` keywords.
- [ ] **Step 4:** In `hir/lower/expr.rs`, add the `Expr::AsyncView{..} =>` arm building `HirExpr::AsyncView(HirAsyncView{ fetching_arm, empty_arm, error_arm, ok_arm, .. })` (the node already exists at `hir/nodes/async_view.rs:13`).
- [ ] **Step 5:** Run the parse test + the existing exhaustiveness tests (`typeck/async_exhaustiveness.rs`) now that real source can reach them. Add a golden `examples/golden/async_view.vox` exercising all four arms; delete its allowlist entry from Task 3A if present.
- [ ] **Step 6:** Remove `when/fetching/empty` from the tree-sitter dead-token list in Phase 7. Commit `feat(parser): implement Async[T] when{} view production (fetching/empty/error/ok)`.

> The TS web emission for AsyncView is wired in Task 8D; until then it routes through the compat stub (already non-panicking).

---

## Phase 7 — Regenerate tree-sitter grammar from the lexer/parser SSOT

**Why:** Pattern G editor drift. `grammar.js` covers 2 of 56 decorators, uses `ret` where the compiler uses `return`, omits `while/loop/break/continue`, reactive keywords, async, operators `% == != += -= *= /= ? =>`, dec/raw/template strings, and `#` comments. `GRAMMAR_SSOT.md` is 0 bytes.

**Files:**
- Modify: `tree-sitter-vox/grammar.js`
- Create/fill: `tree-sitter-vox/GRAMMAR_SSOT.md`
- Modify: `crates/vox-grammar-export/` (if it owns generation — check first)

- [ ] **Step 1:** First check whether `crates/vox-grammar-export` already generates grammar artifacts from the lexer SSOT (`grep -rn "grammar.js\|GRAMMAR_SSOT" crates/vox-grammar-export`). If a generator exists, fix the generator + regenerate (per repo rule "never hand-edit generated files"). If not, `grammar.js` is hand-maintained and you edit it directly.
- [ ] **Step 2: Add a grammar parity test** in `crates/vox-grammar-export/tests/grammar_parity_test.rs` (Create): read the decorator/keyword set from the lexer SSOT (the `language_surface.rs` lists or `token.rs`) and assert every keyword/decorator the compiler recognizes appears in `grammar.js`. Run → FAIL listing the ~52 missing decorators + `return`/`ret`.
- [ ] **Step 3:** Fix `grammar.js`: rename `ret`→`return`; add `while/loop/break/continue` statements; add reactive `component/view/state/derived/effect/mount/cleanup/fragment` rules; add `async` modifier; add the missing operators and compound-assigns; add `dec` suffix, raw `r"..."`, and template `"{...}"` strings; add `#` line comments; tokenize all 56 `@`-decorators (a single `decorator` rule matching `@` + identifier(.identifier)? covers most). Update `queries/highlights.scm` to match. Remove the `@external` ghost (no such compiler token) and add real `extern`.
- [ ] **Step 4:** Fill `tree-sitter-vox/GRAMMAR_SSOT.md` documenting that the keyword/decorator surface is owned by `crates/vox-compiler/src/lexer/token.rs` + `language_surface.rs`, and that `grammar.js` must stay in parity (enforced by the Step-2 test).
- [ ] **Step 5:** Regenerate the parser (`cd tree-sitter-vox && npx tree-sitter generate` if the toolchain is present; otherwise note it in the commit). Run the parity test → PASS. Commit `fix(tree-sitter): regenerate grammar to cover all decorators/keywords/operators; fix return vs ret; fill GRAMMAR_SSOT`.

---

## Phase 8 — Targeted emission + React interop completion

**Why:** the remaining high/medium gaps that aren't catch-alls: VUV control-flow, AsyncView web emit, Spawn/WorkflowVersion Rust emit, interp Pipe, builtin interp parity, and React C4.

### Task 8A: Interpreter `|>` (Pipe) operator (HIGH — general-purpose operator)

**Files:** Modify `crates/vox-compiler/src/eval/expr.rs` (the `Binary` arm ~131-306). Test `crates/vox-compiler/tests/interpreter_test.rs`.

- [ ] Failing test: `let r = vox_compiler::eval::run_source_for_test("fn inc(x:Int)->Int{x+1} fn main()->Int{ 2 |> inc }")` asserts result `3`. → Add a `HirBinOp::Pipe` arm in the interp Binary match that evaluates `lhs` then applies `rhs` as a call with `lhs` as the argument (mirror how codegen lowers it at `stmt_expr_tail.rs:139`). → Pass → delete the Pipe entry from Task 3A allowlist → commit `fix(interp): implement the |> pipe operator (interp/codegen parity)`.

### Task 8B: VUV control-flow — lower `For`/`If`/`Match` in render position to `DomNode::Conditional`/`Loop` (HIGH)

**Files:** Modify `crates/vox-codegen/src/web_ir/lower.rs` (the `lower_expr` JSX path ~139-162). Test `crates/vox-codegen/tests/web_ir_control_flow_test.rs` (Create).

- [ ] Failing test: lower a component whose view contains `for x in items { <li>{x}</li> }` and assert a `DomNode::Loop` is produced (not a raw `DomNode::Expr`). → Add `HirExpr::For/If/Match` arms in `lower_expr` constructing `DomNode::Loop`/`DomNode::Conditional` (the emit + validate targets already exist: `emit_tsx.rs:149-183`, `validate.rs:77-86`). → Pass → commit `feat(web_ir): lower render-position for/if/match to DomNode::Loop/Conditional`.

### Task 8C: AsyncView real web/TS emission (wire the dead `emit_async_view_tsx`) (HIGH)

**Files:** Modify `crates/vox-codegen/src/web_ir/lower.rs` (add `HirExpr::AsyncView` arm) + `crates/vox-codegen-ts/src/hir_emit/mod.rs:839-845` (replace the `{src} /* async view */` stub). Test: extend `golden_ts_test.rs` snapshot for `async_view.vox`.

- [ ] Failing test: assert emitted TS for an async view contains all four branches (fetching/empty/error/ok), not `/* async view */`. → Call the existing `emit_async_view_tsx` (`web_ir/async_state.rs:37`) from the lowering/emit path; remove the compat stub. → Update snapshot, review it. → commit `fix(codegen): wire the real async-view emitter; drop the compat stub`.

### Task 8D: Spawn + WorkflowVersion Rust emission (replaces the Task 1B errors)

**Files:** Modify `crates/vox-codegen/src/codegen_rust/emit/stmt_expr_tail.rs` (+ `workflow.rs`). Test `crates/vox-codegen/tests/spawn_emit_test.rs` (Create).

- [ ] **Spawn:** failing test asserting `spawn Worker(x)` emits a `vox_actor_runtime::spawn_process(...)` call (the runtime symbol exists — `durability_lower.rs:187`). Implement the `HirExpr::Spawn` arm. Pass → commit.
- [ ] **WorkflowVersion:** failing test asserting `workflow.version("v2", 1, 2)` inside a workflow emits a version-gate (consume it in `workflow.rs`; the TS replay path at `hir_emit/mod.rs:532-535` shows the expected shape). Implement → pass → remove the Task 1B `*_unimplemented` errors for these two → commit `feat(codegen_rust): emit spawn and workflow.version (remove the unimplemented stubs)`.

### Task 8E: Interpreter builtin parity — OpenClaw + std.mobile clean diagnostics

**Files:** Modify `crates/vox-compiler/src/eval/expr.rs:513-524` (the native-only guard). Test `crates/vox-compiler/tests/interpreter_test.rs`.

- [ ] Extend the `Scrape`/`Browser` guard to also match `OpenClaw` and the `std.mobile`/`std.crypto` namespaces, emitting the same CR-F4 "only available in compiled builds" diagnostic (so `OpenClaw.list_skills()` and `std.mobile.take_photo()` no longer die as opaque `UndefinedVariable`/`Field not found`). Failing test asserts the clean message. → implement → pass → commit `fix(interp): clean native-only diagnostics for OpenClaw and std.mobile (not opaque errors)`.

### Task 8F: std.mobile native Rust codegen — emit a clean error, not invalid `::std::mobile::...`

**Files:** Modify `crates/vox-codegen/src/codegen_rust/builtin_registry.rs` (`std_namespace_runtime_call`, catch-all ~:1103). Test `crates/vox-codegen/tests/mobile_codegen_test.rs` (Create).

- [ ] Failing test: emitting `std.mobile.take_photo()` to the Rust target must error with `vox.codegen.mobile_target_only`, not emit `::std::mobile::take_photo(...)`. → add a `"mobile" =>` arm returning that coded error (mobile is a TS/RN-only target). → pass → commit.

### Task 8G: React interop C4 — emit imported npm deps into package.json (HIGH)

**Files:** Modify `crates/vox-codegen-ts/src/scaffold.rs:110-134` (static package.json) + consume `external_libs.rs peers`. Test `crates/vox-codegen-ts/tests/package_manifest_test.rs` (Create).

- [ ] **Step 1: Failing test**
```rust
#[test]
fn imported_lib_and_peers_land_in_package_json() {
    let pkg = vox_codegen_ts::scaffold::package_json_for_test(
        "import react {Dialog} from \"@radix-ui/react-dialog\"\n@component App(){ <Dialog/> }");
    assert!(pkg.contains("@radix-ui/react-dialog"), "imported pkg missing from deps:\n{pkg}");
}
```
- [ ] **Step 2:** Run → FAIL (static manifest ignores imports).
- [ ] **Step 3:** Change `web_config_files` / `package_json` generation to accept the `HirModule` (or its `imports`), collect each `HirImport.es_module_specifier`'s package name + its `external_libs.rs` `peers`, and add them to `dependencies`. Mirror how `css_imports` are already collected in `reactive/imports.rs:75-88`.
- [ ] **Step 4:** Run → PASS. Commit `feat(codegen-ts): emit imported npm packages + peers into generated package.json (React interop C4)`.

### Task 8H: React interop C5 + reverse exports (MEDIUM)

- [ ] **C5:** when a library has `provider_mandatory` (`external_libs.rs:51-53`), emit the actual `<Provider>` wrap in the app entry AND a `vox check` diagnostic if missing (today it's a comment only, `reactive/imports.rs:78-83`). Test asserts the wrapper appears. Commit.
- [ ] **Reverse exports:** add a `has_components` flag + `.tsx` entries to the library `exports` map in `library_package_emit.rs:18-34` so emitted Vox components are importable from the published package. Test asserts the export entry. Commit.

### Task 8I: HIR→TS secondary gaps

- [ ] `HirReactiveMember::Stmt` in reactive-MODULE emit (`reactive_module_emit.rs:185-189`, silently skipped): emit the statement like the component path does. Test + commit.
- [ ] Shared `map_hir_type_to_ts` (`lowering_shared/jsx.rs:37` `_ => "any"`): add real `Function`/`Tuple`/`Unit` arms (mirror `schema/type_maps.rs:83-99`). Test asserting a callback prop type is not `any`. Commit.
- [ ] `state_deps.rs:464` `_ => {}`: walk `Try`/`AsyncView`/`WorkflowVersion` for reactive deps (stale-closure fix). Test + commit.

### Task 8J: `@public` parsing (HIGH) + golden coverage backfill

- [ ] **@public:** add `Token::AtPublic` to the fn-decorator loop (`head_fn.rs`) and the decl dispatch (mirror `@auth` at `head_fn.rs:946`), enforce the `@public XOR @auth` rule (`token.rs:259-261`) as a typeck error. Forbidden fixture `examples/forbidden/public_and_auth_conflict.vox` + a golden using `@public`. Commit.
- [ ] **Golden backfill:** for each decorator still lacking a golden after the above (`@tool/@resource/@index/@ensure/@invariant/@forall/@fuzz/@native/@reactive/@tracked/@cancellable/@cors/@rate_limit/@pii/@embed/@webhook/@offline_capable/@collaborative`), add a minimal `examples/golden/<decorator>_showcase.vox` exercising it, covered by the Phase 3 harness. Also fix the MCP goldens to use canonical `@tool`/`@resource` (not retired `@mcp.tool`). One commit per ~5 decorators.

---

## Self-Review (completed by plan author)

**Spec coverage** — every audit dimension maps to a task:
1 AST→HIR → Phase 2 (+1C). 2 HIR→Rust → 1B, 8D, 2B. 3 HIR→TS parity → 8C, 8I, 2C. 4 HIR→WebIR/VUV → 8B, 8C, 5B (+dead emitters: Slot/semantic-ui/paginated/sitemap/Mark tracked in 8B/8C scope or accepted as low-pri dead code — see note). 5 lexer→parser dead tokens → 6 (when/fetching/empty), 8J (@public), Phase 7 (migrate noted). 6 builtin parity → 8E, 8F. 7 interp vs compiled → 1A, 8A. 8 golden coverage → Phase 3, 8J. 9 tree-sitter → Phase 7. 10 React interop → 8G, 8H. 11 split-brain → Phase 5. 12 decorator end-to-end → Phase 4, 4B, 8J.

**Known deferrals (explicitly out of scope, documented not silent):** full MENS/mesh decorator *codegen* (Task 4C makes it a coded error; real impl = `project_mesh_mens_distributed_plan_2026`); `repo.*`/`@versioned` compiled-mode VCS emission (`project_jj_first_class_vcs_2026`); the low-severity dead emitters `Listbox`/`Combobox`/`DomNode::Slot`/`emit_sitemap_xml`/`HirMark` (wire on demand — add a one-line `// dead until a consumer exists` note at each site so the audit doesn't re-flag them).

**Placeholder scan:** no "TODO/handle edge cases" steps; every code step shows code or an exact reference site to mirror with the transformation given.

**Type consistency:** `lower_source_for_test`/`parse_source_for_test`/`emit_rust_for_test`/`emit_ts_for_test` are the test helpers used throughout — Task 2A Step 1 establishes the first; add the others the same way on first use. `HirConst{name,value,type_ann,is_pub,span}` is used identically in 2A and 2B. Diagnostic codes are namespaced `vox.<stage>.<reason>` consistently.

---

## Execution Handoff

This plan is large (8 phases). Recommended order matches the phases (1→8); Phase 1 is the keystone, Phase 3 is the safety net — do both early. Each task ends green and committable, so it is safe to stop between any two tasks.
