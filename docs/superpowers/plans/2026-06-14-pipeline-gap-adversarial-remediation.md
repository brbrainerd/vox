# Pipeline-Gap Adversarial-Review Remediation Plan

> **For agentic workers (Sonnet 4.6):** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:test-driven-development. Steps use checkbox (`- [ ]`) syntax. **Every finding below was hand-verified against the real code by an Opus adversarial review on 2026-06-14 — file:line references are accurate as of branch `claude/recursing-mendel-19246f` HEAD `92e170050f`.** Re-confirm each before fixing (the code may have moved), then fix + add a behavioral test that would have caught it.

**Goal:** Fix the bugs and silently-dead "completed" features that a 6-agent adversarial review found in the pipeline-gap branch, so the branch is genuinely correct before admin-merge to `main`.

**Architecture:** The review found a recurring failure mode: *a feature is implemented behind a build config or downstream layer that production never exercises, while a structural-only test passes.* Three "completed" Phase-8 tasks (8C async-view, 8G package.json deps, 8H library exports) are **dead in the production `vox build` path**, and two Phase-2/4 tasks emit uncompilable or dropped output. Fix order: Tier 0 (dead/uncompilable features) → Tier 1 (correctness bugs) → Tier 2 (test hardening that closes the holes which hid Tier 0/1) → Tier 3 (minor, optional).

**Tech Stack:** Rust (compiler/codegen), JavaScript (tree-sitter), Vox goldens, `cargo test`, `insta`.

---

## Conventions (READ FIRST)

- **Per-crate tests only:** `cargo test -p <crate>` / `cargo test -p <crate> --test <stem>`. Never `cargo test --workspace`.
- **Clippy before each commit on a touched crate:** `cargo clippy -p <crate> -- -D warnings`.
- **NEVER `cargo fmt --all`** (Windows `os error 206`). Use `cargo fmt -p <crate>`.
- **No stubs.** If a real fix is out of scope, emit a *coded diagnostic*, never a silent no-op.
- **The embedded-vs-standalone trap (critical context for all TS fixes):** `crates/vox-codegen/src/lib.rs:15` embeds `vox-codegen-ts/src/mod.rs` as module `codegen_ts` via `#[path]`, compiled with feature `standalone` **OFF**. The standalone `vox-codegen-ts` crate (and all its `tests/`) compiles with `standalone` **ON** (`default = ["standalone"]`). **Production `vox build` uses the embedded copy** (`crates/vox-cli/src/commands/build.rs:224` → `vox_codegen::codegen_ts::generate_with_options`). Therefore **any behavior gated behind `#[cfg(feature = "standalone")]` does nothing in production.**
  - Path resolution that works in BOTH modes: `crate::web_ir::…` resolves in both (re-exported via the `parent` alias at `mod.rs:7-16`; precedent: `scaffold.rs:24` uses `crate::web_ir::layer_emit` ungated). But `crate::external_libs` works **only** in standalone (in embedded it would be `crate::codegen_ts::external_libs`). Use **`super::external_libs`** from a `scaffold.rs`-level module to resolve in both modes (`super` = the `codegen_ts` module root = `mod.rs`, which has `pub mod external_libs`).
- **Verification net you just gained:** `crates/vox-codegen/tests/emit_compile_harness.rs` now compiles generated Rust (shared warm target dir, ~5s/test after cold). Use it to prove Rust-emit fixes actually compile.

---

## TIER 0 — Critical: dead or uncompilable "completed" features (BLOCK MERGE)

### R1: Async-view 4-branch emit is dead in production (Task 8C)

**Verified:** `crates/vox-codegen-ts/src/hir_emit/mod.rs:839-879`. The `HirExpr::AsyncView` arm gates the real `emit_async_view_tsx` call behind `#[cfg(feature = "standalone")]`; the `#[cfg(not(feature = "standalone"))]` branch returns bare `source_tsx`, dropping fetching/empty/error/ok. Production (embedded, standalone-off) hits the bare branch. The standalone-only `async_view_emit_test.rs` passes, masking it.

**Root cause:** The author gated it because the call used `vox_codegen::web_ir::async_state::emit_async_view_tsx` (the `vox_codegen::` prefix doesn't resolve inside the `vox-codegen` crate). But `crate::web_ir::async_state::…` resolves in **both** modes.

**Files:** Modify `crates/vox-codegen-ts/src/hir_emit/mod.rs:839-879`. Test: `crates/vox-codegen-ts/tests/async_view_emit_test.rs` (exists) + add an embedded-path assertion in `crates/vox-codegen/tests/` (a new `async_view_embedded_test.rs`) so the production path is covered.

- [ ] **Step 1 (failing test, embedded path):** Create `crates/vox-codegen/tests/async_view_embedded_test.rs` that calls `vox_codegen::codegen_ts::…` to emit TSX for an async-view HIR and asserts the output contains the fetching/empty/error/ok branch markers (not just `source_tsx`). This compiles with `standalone` OFF, so it reproduces the production bug → FAILS today.
- [ ] **Step 2 (fix):** Remove the `#[cfg(feature = "standalone")]` / `#[cfg(not(...))]` split. Keep ONE body that computes all four arm strings and calls `crate::web_ir::async_state::emit_async_view_tsx(...)`. Change the `vox_codegen::web_ir::async_state` reference to `crate::web_ir::async_state`. (Verify `async_state` is reachable via the re-exported `web_ir` — the standalone branch already calls it successfully, so the submodule is public.)
- [ ] **Step 3:** Add a `HirExpr::AsyncView` arm in `crates/vox-codegen/src/web_ir/lower.rs` only if the WebIR (not TS-expr) path also needs it — check whether async-views reach `lower.rs`; if they only flow through `hir_emit`, skip. Document the decision in a comment.
- [ ] **Step 4:** `cargo test -p vox-codegen --test async_view_embedded_test` → PASS; `cargo test -p vox-codegen-ts --test async_view_emit_test` → still PASS; `cargo clippy -p vox-codegen -- -D warnings`.
- [ ] **Step 5: Commit** `fix(codegen-ts): un-gate async-view 4-branch emit so it works in the embedded production path`

### R2: Imported-npm-deps in package.json is dead in production (Task 8G)

**Verified:** `crates/vox-codegen-ts/src/scaffold.rs:118-217`. `package_json_with_extra_deps`, `extra_deps_from_imports`, `package_json_for_test` are all `#[cfg(feature = "standalone")]`. The only caller of the first two is `package_json_for_test` (the test helper). The production scaffold entry points `web_config_files` (`scaffold.rs:18`) and `write_scaffold_if_missing` (`:220`) still emit `static_package_json()` unconditionally. So `vox build` never injects imported packages.

**Root cause:** Gated because the helper does `use crate::external_libs::{…}` which fails in embedded mode. Fix: use `super::external_libs`.

**Files:** Modify `crates/vox-codegen-ts/src/scaffold.rs`. Test: `crates/vox-codegen-ts/tests/package_manifest_test.rs` (exists) + new embedded-path test under `crates/vox-codegen/tests/`.

- [ ] **Step 1 (failing test):** Create `crates/vox-codegen/tests/package_manifest_embedded_test.rs` that drives the production scaffold path (`vox_codegen::codegen_ts::scaffold::web_config_files` or whatever `build.rs` calls) with a HIR that imports `@radix-ui/react-dialog`, and asserts the emitted `package.json` lists that dep. FAILS today.
- [ ] **Step 2 (fix imports):** In `scaffold.rs`, change `use crate::external_libs::{bare_package, LIBRARIES};` (line ~148) to `use super::external_libs::{bare_package, LIBRARIES};`. Remove the `#[cfg(feature = "standalone")]` from `package_json_with_extra_deps` and `extra_deps_from_imports` (keep `package_json_for_test` gated — it uses parser APIs that may be standalone-only; verify).
- [ ] **Step 3 (wire into production):** Change `web_config_files` (and `write_scaffold_if_missing`) to accept the `&HirModule` (or `&[HirImport]`) and call `extra_deps_from_imports` → `package_json_with_extra_deps` instead of the unconditional `static_package_json()`. Update the call site in `crates/vox-cli/src/commands/build.rs` and any `vox-codegen` re-export to pass imports through.
- [ ] **Step 4:** `cargo test -p vox-codegen --test package_manifest_embedded_test` → PASS; existing `package_manifest_test` PASS; clippy both crates.
- [ ] **Step 5 (harden, see R-I1):** Replace the brittle `replacen("    \"lucide-react\": \"^0.400.0\"\n  },", …)` string-surgery in `package_json_with_extra_deps` (`scaffold.rs:134-138`) with `serde_json`-based assembly (mirror `library_package_emit.rs`). A version-string drift currently silently drops all injected deps.
- [ ] **Step 6: Commit** `fix(codegen-ts): wire imported-npm-deps into the production package.json scaffold (Task 8G was test-only)`

### R3: Library mode emits `package.json` twice — Task 8H exports clobbered

**Verified:** `crates/vox-codegen-ts/src/emitter.rs:412` and `:581` both push `"package.json"` into the `files` Vec in `BuildMode::Library`, with no early return between (verified through :598). Last-write-wins, so the second (minimal `vox-generated-api` manifest) overwrites the first (the 8H `emit_library_package_json` output with `./components/<Name>` exports). All of 8H's reverse-export work never reaches disk.

**Files:** Modify `crates/vox-codegen-ts/src/emitter.rs`. Test: `crates/vox-codegen-ts/tests/library_exports_test.rs` (exists but tests `emit_library_package_json` directly, bypassing the clobber) — add a full-`generate`-path test.

- [ ] **Step 1 (failing test):** Add `library_exports_survive_generate` to `library_exports_test.rs`: run the full `generate_with_options(.., mode: Library)` on a module with a component, find the `package.json` entry in the OUTPUT file set, and assert it contains `./components/<Name>`. FAILS today (the minimal manifest wins).
- [ ] **Step 2 (fix):** Remove the second `package.json` block (`emitter.rs:567-583`). Fold any fields it added (e.g. `main`, `types`, `name: "vox-generated-api"`) that the first block lacks into `emit_library_package_json` / `LibraryPackageConfig`. Ensure exactly one `package.json` is pushed.
- [ ] **Step 3:** `cargo test -p vox-codegen-ts --test library_exports_test` → PASS; clippy.
- [ ] **Step 4: Commit** `fix(codegen-ts): stop emitting package.json twice in Library mode (was clobbering Task 8H component exports)`

### R4: Task 8H exports reference `.tsx` files Library mode never emits

**Verified:** `crates/vox-codegen-ts/src/emitter.rs:408` builds `component_names` from `hir.components`, and `library_package_emit.rs:36-41` emits `"./components/<Name>": "./components/<Name>.tsx"`. But component `.tsx` files are emitted only when `options.mode != BuildMode::Library` (`emitter.rs:234`). In Library mode the referenced files don't exist → dangling exports.

**Files:** Modify `crates/vox-codegen-ts/src/emitter.rs`. Depends on R3 (do R3 first).

- [ ] **Step 1 (decision):** Choose: **(A)** emit component `.tsx` files under `components/<Name>.tsx` in Library mode too, or **(B)** scope down — only add an export entry for components that ARE emitted. Recommended: **(A)** — a component library with no component files is useless.
- [ ] **Step 2 (failing test):** Extend `library_exports_survive_generate` (R3) to also assert the output file set CONTAINS `components/<Name>.tsx`. FAILS today.
- [ ] **Step 3 (fix):** In `emitter.rs`, emit component files in Library mode. Reuse `generate_reactive_component`; ensure the emitted filename EXACTLY matches the export target (`components/<Name>.tsx`). Verify name → path consistency.
- [ ] **Step 4:** test PASS; clippy.
- [ ] **Step 5: Commit** `fix(codegen-ts): emit component .tsx files in Library mode so 8H exports resolve`

### R5: `emit_const` produces uncompilable Rust for the common (unannotated) case (Task 2B)

**Verified:** `crates/vox-codegen/src/codegen_rust/emit/workflow.rs:123-132`. `None => "_".to_string()` → `const NAME: _ = value;` (rustc E0121 — `_` forbidden in const item type). For strings, the value goes through `emit_expr` (Owned) → `"...".to_string()` (`stmt_expr.rs:673`), non-const-evaluable (E0015), and even the `str`-annotated branch picks type `&'static str` but value `String` (type mismatch). The headline gap (`top-level let x = …`) is *exactly* the unannotated form. `const_emit_test.rs` is string-only (R-T1), so it passes on broken output.

**Files:** Modify `crates/vox-codegen/src/codegen_rust/emit/workflow.rs`. Test: rewrite `crates/vox-codegen/tests/const_emit_test.rs` (R-T1) + a compile-harness golden.

- [ ] **Step 1 (failing test):** In `const_emit_test.rs`, assert the emitted output for `let MAX = 3` contains `const MAX: i64 = 3` (concrete type) AND `assert!(!out.contains(": _"))`. For `let BASE_URL = "x"` assert `const BASE_URL: &'static str = "x"` and `assert!(!out.contains(".to_string()"))`. FAILS today.
- [ ] **Step 2 (fix):** In `emit_const`, when `type_ann` is `None`, infer the Rust type from the literal `c.value`: `HirExpr::IntLit→"i64"`, `FloatLit→"f64"`, `BoolLit→"bool"`, `StringLit→"&'static str"`. For `StringLit` (annotated `str` OR inferred), emit the value as a **borrowed** literal (`emit_expr_with(..., OwnershipMode::Borrowed, ...)` or a direct `format!("{:?}", s)`), NOT `.to_string()`. For any non-literal value (can't be a `const`), emit a coded `compile_error!("vox.codegen_rust.const_requires_literal: cannot emit const <name> from a non-literal initializer")` rather than `_`.
- [ ] **Step 3 (compile-net):** Add `examples/golden/top_level_const.vox` with `let MAX = 3` + `let NAME = "vox"` used in `main`, and a `#[test] fn golden_top_level_const_compiles()` in `emit_compile_harness.rs` (now runs by default). This proves the emitted const actually compiles.
- [ ] **Step 4:** `cargo test -p vox-codegen --test const_emit_test` + `--test emit_compile_harness golden_top_level_const_compiles` → PASS; clippy.
- [ ] **Step 5: Commit** `fix(codegen_rust): infer concrete const type + borrowed string literal (was emitting uncompilable const X: _ = ...)`

### R6: `@auth(provider:, roles:)` args parsed then dropped at AST→HIR (Task 4B)

**Verified:** Parser fills `FnDecl.auth_provider`/`roles` (`head_fn.rs:953-1017`) but `grep auth_provider|roles|auth:` across `crates/vox-compiler/src/hir/nodes/` returns **nothing** — `HirFn` (decl.rs:328) and `HirEndpointFn` (decl.rs:470) have no auth field. `lower_fn`/the `Decl::Endpoint` arm never copy them. So `@auth` parses then vanishes before codegen — the exact "gap relocated one layer down" pattern. The Rust emit (auth-guard middleware) the plan's Task 4B Step 4 required does not exist either.

**Files:** `crates/vox-compiler/src/hir/nodes/decl.rs`, `crates/vox-compiler/src/hir/lower/decl.rs`, `crates/vox-compiler/src/hir/lower/mod.rs` (Endpoint arm), `crates/vox-codegen/src/codegen_rust/emit/http.rs`. Test: `crates/vox-compiler/tests/auth_decorator_test.rs` (exists — currently only asserts the AST, extend to HIR) + a Rust-emit test.

- [ ] **Step 1 (failing test, HIR carry):** In `auth_decorator_test.rs`, add a test that lowers a `@auth(provider:"clerk", roles:["admin"]) @server fn` and asserts the resulting `HirEndpointFn.auth` is `Some` with `provider == "clerk"` and `roles == ["admin"]`. FAILS today (no field).
- [ ] **Step 2 (fix HIR):** Add `pub struct HirAuth { pub provider: String, pub roles: Vec<String> }` and `pub auth: Option<HirAuth>` to `HirEndpointFn` (and `HirFn` if bare-fn auth is ever meaningful). Populate it in the `Decl::Endpoint` lowering arm (`hir/lower/mod.rs`, near the `cors`/`rate_limit`/`pii` mappings) from `FnDecl.auth_provider`/`roles`.
- [ ] **Step 3 (failing test, Rust emit):** Add `examples/golden/auth_guard.vox` + a test asserting the generated Rust for an `@auth` endpoint contains an auth-guard middleware layer (mirror the cors layer at `http.rs:28-58`). FAILS today.
- [ ] **Step 4 (fix emit):** In `codegen_rust/emit/http.rs`, when `endpoint.auth.is_some()`, emit an auth-guard `from_fn` layer that checks the provider/roles (mirror the rate-limit/cors prelude+layer pattern). If a real auth runtime is out of scope, emit a prelude that reads a configurable header and rejects with 401 when absent — a real, compiling guard, not a comment.
- [ ] **Step 5:** `cargo test -p vox-compiler --test auth_decorator_test` + `cargo test -p vox-codegen` → PASS; clippy both.
- [ ] **Step 6: Commit** `feat(auth): carry @auth(provider/roles) to HIR and emit a real Rust auth-guard (Task 4B was parser-only)`

---

## TIER 1 — Important correctness bugs

### R7: `x |> str` (and 13 other builtins) silently evaluates to `Null` (Task 8A)

**Verified:** `crates/vox-compiler/src/eval/expr.rs:132-134` — the Pipe arm does `apply_closure(interp, &callee, vec![l])`. When rhs is a builtin `Ident` (`str`/`int`/`len`/…), `eval_expr` returns a **placeholder `Fn` with `body: Rc::new(vec![])`** (`expr.rs:55-62`); `apply_closure` runs the empty body → `Null`. Direct calls `str(5)` work via the `call_global_builtin` special-case (`expr.rs:389`), but the Pipe path bypasses it. So `5 |> str` = `Null` — silent-wrong-output, the class Phase 1A targeted.

**Files:** Modify `crates/vox-compiler/src/eval/expr.rs`. Test: `crates/vox-compiler/tests/interpreter_test.rs`.

- [ ] **Step 1 (failing test):** `assert run_source_for_test("fn main() to str { 5 |> str }")` (or the file's harness shape) returns `"5"`, not Null. Add `42 |> abs`, `[3,1,2] |> sorted` variants. FAILS today.
- [ ] **Step 2 (fix):** In the Pipe arm, before `apply_closure`, check if `right` is a `HirExpr::Ident(name)` that is a global builtin (not bound in scope). If so, call `call_global_builtin(interp, name, vec![l])` — mirror the Call-arm builtin-first dispatch at `expr.rs:389`. Only fall back to `apply_closure` for real closures.
- [ ] **Step 3:** `cargo test -p vox-compiler --test interpreter_test` → PASS; clippy.
- [ ] **Step 4: Commit** `fix(interp): route |> into builtins through call_global_builtin (was silently returning Null)`

### R8: `pub fn` visibility conflated with `@public` auth-exemption → false XOR error (Task 8J)

**Verified:** `crates/vox-compiler/src/typeck/ast_decl_lints.rs:648` keys the `public-auth-conflict` lint on `f.is_pub && f.auth_provider.is_some()`. But `is_pub` is set by THREE inputs: Rust-style `pub fn`, the `@public` decl prefix, and the `@public` fn-decorator. Only the latter two mean "skip auth." A legitimate `pub @auth(...) @server fn` (exported AND authenticated) falsely trips `vox/typeck/public-auth-conflict`.

**Files:** `crates/vox-ast/src/decl/fundecl.rs` (or wherever `FnDecl` lives), the parser `@public` paths, `crates/vox-compiler/src/typeck/ast_decl_lints.rs`. Test: `examples/forbidden/` + a new positive golden.

- [ ] **Step 1 (failing test):** Add `examples/golden/pub_authed_endpoint.vox` with `pub @auth(provider:"clerk") @server fn admin() -> Str { "ok" }` and assert (via a typeck test) that it produces NO `public-auth-conflict` diagnostic. FAILS today.
- [ ] **Step 2 (fix):** Add a distinct `is_auth_exempt: bool` (or `is_public_endpoint`) field to `FnDecl`, set `true` ONLY by the two `@public` paths (NOT by `pub fn`). Re-key the XOR lint at `ast_decl_lints.rs:648` to `f.is_auth_exempt && f.auth_provider.is_some()`.
- [ ] **Step 3:** Confirm `examples/forbidden/public_and_auth_conflict.vox` (uses `@public`) STILL errors. Run `cargo test -p vox-compiler --test forbidden_corpus_test` + the new golden test → PASS; clippy.
- [ ] **Step 4: Commit** `fix(typeck): distinguish pub-visibility from @public auth-exemption in the XOR lint`

### R9: `spawn` emits `tokio::spawn`, not the plan-mandated `spawn_process` (Task 8D)

**Verified:** `crates/vox-codegen/src/codegen_rust/emit/stmt_expr.rs:762-765` emits `tokio::spawn(async move { <target> })`. The plan (Task 8D) said emit `vox_actor_runtime::spawn_process(...)` (the symbol exists at `crates/vox-actor-runtime/src/process.rs:113`). Risk: in lib/script mode the generated crate may not depend on `tokio`, and the closure contract differs from Vox spawn semantics.

**Files:** Modify `crates/vox-codegen/src/codegen_rust/emit/stmt_expr.rs`. Test: `crates/vox-codegen/tests/spawn_workflow_emit_test.rs` + `emit_compile_harness.rs`.

- [ ] **Step 1 (decision + verify):** Determine whether every target that can contain `spawn` guarantees a `tokio` dep in the generated Cargo.toml (grep the manifest emitter). If YES and the semantics are acceptable, downgrade to: update the test/plan comment and add a compile-harness test proving `spawn` output compiles. If NO, switch to `spawn_process`.
- [ ] **Step 2 (failing test):** Add `examples/golden/spawn_smoke.vox` + `golden_spawn_compiles()` in `emit_compile_harness.rs`. If the current `tokio::spawn` output doesn't compile in the harness's script crate, this FAILS, proving the bug.
- [ ] **Step 3 (fix):** Per Step 1's decision, either emit `vox_actor_runtime::spawn_process(...)` with the correct closure shape, or ensure the tokio dep + Send/'static bounds hold. Update `spawn_workflow_emit_test.rs` accordingly.
- [ ] **Step 4:** tests PASS; clippy.
- [ ] **Step 5: Commit** `fix(codegen_rust): emit a compiling spawn (spawn_process or verified tokio path), not an unchecked tokio::spawn`

### R10: `WorkflowVersion` emits a `let _ = (...)` no-op stub (Task 8D)

**Verified:** `crates/vox-codegen/src/codegen_rust/emit/stmt_expr.rs:769-776` emits `{ let _ = (id, min, max); }` — discarded immediately. The comment claims the orchestrator checks the triple at replay, but nothing emitted writes it anywhere observable. This is a stub dressed as an implementation (violates no-stubs).

**Files:** Modify `crates/vox-codegen/src/codegen_rust/emit/stmt_expr.rs`. Reference the TS replay shape at `crates/vox-codegen-ts/src/hir_emit/mod.rs:~532`.

- [ ] **Step 1 (decision):** Either (A) emit a real call into the workflow-runtime version-gate API (mirror the TS replay shape), or (B) if no such Rust API exists yet, emit a coded `compile_error!("vox.codegen_rust.workflow_version_unimplemented: …")` so it's honest, and note it's tracked under the mesh/workflow epic. Do NOT keep the silent no-op.
- [ ] **Step 2 (test):** Update `spawn_workflow_emit_test.rs::workflow_version_*` to assert the chosen real behavior (a runtime call substring, or the coded error).
- [ ] **Step 3:** test PASS; clippy.
- [ ] **Step 4: Commit** `fix(codegen_rust): emit a real workflow-version gate or a coded error (was a no-op tuple stub)`

### R11: `@offline_capable` / `@collaborative` args silently discarded (Task 4B)

**Verified:** `crates/vox-compiler/src/parser/descent/decl/head_fn.rs:1018-1023` — `self.advance(); if self.eat(&LParen) { self.skip_paren_args_inner(); }` with no flag set. No `is_offline_capable`/`is_collaborative` field exists. The decorators are accepted and have zero effect.

**Files:** `crates/vox-ast/src/decl/…` (FnDecl), `head_fn.rs`. Test: a parse test.

- [ ] **Step 1 (failing test):** Add a parse test asserting a `@offline_capable fn` sets `FnDecl.is_offline_capable == true`. FAILS today.
- [ ] **Step 2 (fix):** Add `is_offline_capable`/`is_collaborative` bool fields to `FnDecl`; set them in the head_fn arm. If they need to reach HIR/codegen, carry them; if deferred, that's acceptable for a boolean flag (unlike dropped args) but add a `// not yet consumed by codegen` note. (If they take args that matter, emit a coded "not yet wired" diagnostic instead of `skip_paren_args_inner`.)
- [ ] **Step 3:** parse test PASS; clippy.
- [ ] **Step 4: Commit** `fix(parser): record @offline_capable/@collaborative as flags instead of discarding`

### R12: `lower_warnings` is write-only — Phase 1C warning never reaches the user

**Verified:** `crates/vox-compiler/src/hir/lower/mod.rs:630-639` pushes to `hir.lower_warnings` (field at `decl.rs:161`) then still pushes to `legacy_ast_nodes`. `grep lower_warnings` across `src/` shows only the producer + field decl + tests — no driver/diagnostic stage consumes it. The `LOWER_UNLOWERED_DECL` code is registered but never surfaced. Functionally identical to the silent drop it replaced, except a test can see it.

**Files:** `crates/vox-compiler/src/hir/lower/mod.rs`, wherever compiler diagnostics are collected/surfaced (find where typeck/parse diagnostics flow to the CLI). Also note R-related: `mod.rs:408-410` silently drops `Decl::V0Component/Page/Loading` bypassing even this path.

- [ ] **Step 1 (failing test):** Add a test that lowers a module with an unlowerable decl and asserts the warning appears in the compiler's REAL diagnostic stream (the one the CLI prints), not just `hir.lower_warnings`. FAILS today.
- [ ] **Step 2 (fix):** Route `lower_warnings` into the actual diagnostic collection (emit `Diagnostic::warning(LOWER_UNLOWERED_DECL, …)`), or have the driver drain `hir.lower_warnings` into diagnostics after lowering. Also route the `V0Component/Page/Loading` arm (`mod.rs:408-410`) through a `vox.lower.retired_decl` note instead of a bare silent drop.
- [ ] **Step 3:** test PASS; `cargo test -p vox-compiler`; clippy.
- [ ] **Step 4: Commit** `fix(lower): surface unlowered-decl warnings to the real diagnostic stream (was write-only)`

### R13: Mobile package.json / pnpm-lock drift (bundled scope-creep in `de49f16a2d`)

**Verified:** `apps/vox-mental-tracker/package.json` removed the `vox-tauri-stt-guest` dependency (`link:../../crates/vox-tauri-stt/guest-js`) but `pnpm-lock.yaml` still pins it → `pnpm install --frozen-lockfile` (CI default) fails "lockfile not up to date." The package physically exists and is the JS guest for the desktop STT Tauri plugin; the app's `src/main.vox` uses on-device STT. This unrelated change was bundled silently into a typeck commit.

**Files:** `apps/vox-mental-tracker/package.json` and/or `apps/vox-mental-tracker/pnpm-lock.yaml`.

- [ ] **Step 1 (decision):** Determine whether the desktop-Tauri STT path still needs `vox-tauri-stt-guest` (the web/mobile path uses the emitted `runtime-install.ts` shim, so web builds are fine). If desktop needs it → **restore** the line in `package.json`. If the removal was intentional → **regenerate** `pnpm-lock.yaml` (`pnpm install --lockfile-only` in that app dir) to drop it.
- [ ] **Step 2 (verify):** Ensure `package.json` and `pnpm-lock.yaml` agree (no frozen-install drift). If you have pnpm available, run `pnpm install --frozen-lockfile` in `apps/vox-mental-tracker/` and confirm it succeeds. NOTE: this repo's GUI/app tooling is **pnpm, not npm** (npm fails cryptically).
- [ ] **Step 3: Commit** `fix(vox-mental-tracker): reconcile package.json/pnpm-lock drift for vox-tauri-stt-guest`

### R14: `Match` in render position falls to `DomNode::Expr` and hides the gap from the metric (Task 8B)

**Verified:** `crates/vox-codegen/src/web_ir/lower.rs:218-225` — `HirExpr::Match` emits `DomNode::Expr` (stringified TS), not `DomNode::Conditional`, and unlike the generic fallback (`:227`) it does NOT increment `expr_fallback_count`. So the WebIR parity/fallback gate under-counts match-in-render as fully lowered.

**Files:** Modify `crates/vox-codegen/src/web_ir/lower.rs`. Test: `crates/vox-codegen/tests/web_ir_control_flow_test.rs`.

- [ ] **Step 1 (decision):** Either lower render-position `Match` to `DomNode::Conditional` (full fix), OR — if deferring — bump `expr_fallback_count` in the Match arm so the metric is honest.
- [ ] **Step 2 (test):** Add a test asserting either a `DomNode::Conditional` is produced for render-position match, or that `summary.dom_expr_fallbacks` counts it. FAILS today for whichever you choose.
- [ ] **Step 3 (fix):** Implement the chosen option.
- [ ] **Step 4:** test PASS; clippy.
- [ ] **Step 5: Commit** `fix(web_ir): make render-position match honest (lower to Conditional or count the fallback)`

### R15: If-branch lowering silently drops non-`Expr` statements (Task 8B)

**Verified:** `crates/vox-codegen/src/web_ir/lower.rs:179-203` uses `filter_map` keeping only `HirStmt::Expr`. A render `if c { let x = …; <div>{x}</div> }` silently drops the `let`, emitting only the trailing element (wrong output). The both-branches-empty fallback (`:205`) doesn't fire when one expr survives.

**Files:** Modify `crates/vox-codegen/src/web_ir/lower.rs`. Test: `web_ir_control_flow_test.rs`.

- [ ] **Step 1 (failing test):** Lower an `if` whose branch has a `let` + an element; assert either the `let` is represented or a fallback/diagnostic fires (not silent drop). FAILS today.
- [ ] **Step 2 (fix):** Either handle non-Expr stmts in render branches (lower the `let` into the conditional's scope) or, if unsupported, fall back to `DomNode::Expr` for the whole branch (and count it) rather than dropping statements.
- [ ] **Step 3:** test PASS; clippy.
- [ ] **Step 4: Commit** `fix(web_ir): stop silently dropping non-Expr statements in render-position if-branches`

---

## TIER 2 — Test hardening (close the holes that hid Tier 0/1)

These are the structural-only tests that let the above ship green. Fix them so regressions can't recur. Several overlap with Tier-0/1 fixes — if you added the behavioral test there, just delete/upgrade the weak one here.

### R-T1: `const_emit_test.rs` is structural-only
**Verified:** `crates/vox-codegen/tests/const_emit_test.rs:22-45` only does `out.contains("const MAX")`. Covered by R5 Step 1/3 — ensure the upgraded test asserts a concrete type and the compile-harness golden exists.
- [ ] Replace substring checks with concrete-type assertions + the `emit_compile_harness` golden from R5. Commit folded into R5.

### R-T2: `rate_limit_emit_test.rs` tests enum distinctness, not emission (Task 4D)
**Verified:** `crates/vox-codegen/tests/rate_limit_emit_test.rs:10-18` only asserts `RateLimitBy::Ip != UserId != ApiKey`. The emission claim is unexercised.
- [ ] **Step 1:** Add a test that generates Rust for a `@rate_limit(by: user_id)` endpoint (via the public lib emitter) and asserts the output contains the user-id-keyed extractor (`x-vox-user-id`), and likewise for `api_key` (`x-api-key`); assert the IP-only path is NOT used. FAILS if emission is wrong.
- [ ] **Step 2:** Commit `test(codegen_rust): exercise @rate_limit user_id/api_key emission (was only testing enum distinctness)`

### R-T3: tree-sitter decorator parity gate is fooled by the comment block + the rule is orphaned
**Verified:** `crates/vox-grammar-export/tests/export_test.rs:375-416` does `grammar_js.contains(d)`; grammar.js:488-495 is a comment listing every decorator, so the test passes on comments alone. Worse, `decorator:` (grammar.js:496) is **never referenced** as `$.decorator` by any production (grep confirms 1 occurrence = the definition). The grammar cannot actually parse `@server fn`.
- [ ] **Step 1:** Make the parity test strip JS comments before the `contains` check (so it tests the real grammar source, not the SSOT comment), AND add `assert!(grammar_js.contains("$.decorator"))` to prove the rule is wired into at least one production.
- [ ] **Step 2:** Wire `$.decorator` into the relevant declaration rules in `grammar.js` (function/component/endpoint declarations should accept leading decorators) so the new assertion passes. (The full `block` vs `block_repeat1` regen is out of scope — see Tier 3.)
- [ ] **Step 3:** `cargo test -p vox-grammar-export` → PASS. Commit `fix(tree-sitter): wire $.decorator into productions + make parity gate ignore comments`

### R-T4: `golden_interp_test.rs` allowlist has no execution floor
**Verified:** `crates/vox-compiler/tests/golden_interp_test.rs` — ~55-entry `KNOWN_INTERP_GAPS` allowlist + silent parse-skip (`:186-196`) can make the gate near-vacuous. No `assert!(ran >= N)` (contrast `forbidden_corpus_test.rs:78` which has `checked >= 6`).
- [ ] **Step 1:** Add `assert!(ran >= <floor>)` (pick a floor = current actually-executing count, e.g. count them and set floor to that minus small slack) so the gate fails if the executing set shrinks. Optionally convert the unbounded parse-skip into an explicit named skip list.
- [ ] **Step 2:** `cargo test -p vox-compiler --test golden_interp_test` → PASS. Commit `test(golden): add execution floor to golden-interp gate so it can't go vacuous`

### R-T5: `async_view_emit_test.rs` only checks 3 of 4 branches
**Verified:** `crates/vox-codegen-ts/tests/async_view_emit_test.rs:52-67` asserts `fetching`/`empty`/`error` but not the `ok` branch value. Covered partly by R1's embedded test.
- [ ] Add `assert!(out.contains("ok"))` (or the ok-arm value) + assert each arm's literal value appears so all four arms are proven wired. Fold into R1.

### R-T6: `interp_rejects_unsupported_expr` has vacuous escape hatches
**Verified:** `crates/vox-compiler/tests/interpreter_test.rs:441-480` — `return` early if JSX doesn't parse (`:452-455`) and accepts any non-Null `Ok` (`:466-477`). Only the Err + Ok(Null) paths test the claim.
- [ ] Replace the parse-dependent early-out with `.expect("must parse")` so a parse regression fails loudly; assert the specific CR-F4 diagnostic. Commit `test(interp): remove vacuous escape hatches from the unsupported-expr guard`

### R-T7: `emit_unsupported_expr_test.rs` tests the wrong subject
**Verified:** `crates/vox-codegen/tests/emit_unsupported_expr_test.rs:19-29` feeds a plain fn (no JSX/AsyncView/Spawn), so it never reaches the `compile_error!` arms it claims to guard.
- [ ] Feed a frontend expr (JSX/AsyncView) through the Rust emitter and assert it returns a `compile_error!(...)` string (Phase 1B behavior). Commit `test(codegen_rust): actually exercise the frontend-expr compile_error! arms`

---

## TIER 3 — Minor (optional; fix if cheap, else file follow-ups)

- [ ] **`field_ownership_map`/`to_semantic_hir` miss new `HirModule` fields** (`crates/vox-compiler/src/hir/nodes/decl.rs:195-256`): add `configs/themes/messages/skills/agent_defs` rows; add `consts` to `to_semantic_hir`; add a test that the map key-set equals the struct field-set.
- [ ] **Top-level `let mut` discards `mut`** (`crates/vox-compiler/src/parser/descent/mod.rs:632`): reject with a clear error or carry the flag.
- [ ] **Double diagnostic on bare-fn `@pii`** (`ast_decl_lints.rs` — both `decorator-requires-endpoint` and `pii-unimplemented` fire): suppress one so the remediation message isn't contradictory.
- [ ] **`slugify("")` → invalid empty Expo slug/npm name** (`crates/vox-rn-codegen/src/scaffold.rs:66-82`): fall back to `vox-app` when the slug is empty.
- [ ] **`relative_luminance` byte-slices `hex[0..2]`** (`ast_decl_lints.rs:14-20`, pre-existing): panics on a 6-byte multibyte `@theme` color; guard on char boundaries or `is_ascii()`.
- [ ] **tree-sitter grammar doesn't match real syntax + parser.c can't regen** (`block` vs `block_repeat1`, per `GRAMMAR_SSOT.md:49-53`): resolve the conflict with `prec`/`conflicts:`, regenerate `parser.c`, add a smoke test parsing one real golden `.vox` with zero ERROR nodes. Larger effort — file as a separate follow-up if not done here.

---

## Final verification before merge (after all Tier 0 + Tier 1 fixes)

- [ ] `cargo test -p vox-compiler` (lexer/parser/HIR/typeck/interp suites)
- [ ] `cargo test -p vox-codegen` (incl. `emit_compile_harness` — the new compile-net)
- [ ] `cargo test -p vox-codegen-ts`
- [ ] `cargo test -p vox-grammar-export`
- [ ] `cargo clippy -p vox-compiler -p vox-codegen -p vox-codegen-ts -p vox-rn-codegen -- -D warnings`
- [ ] `cargo run -p vox-arch-check`
- [ ] Pre-push hook (run the configured pre-push; it's the merge gate the user will rely on instead of full CI).

## Merge + cleanup (user-authorized: skip CI, admin-merge, clean worktree)

- [ ] Confirm working tree clean + all fixes committed on `claude/recursing-mendel-19246f`.
- [ ] Admin-merge into `main` (admin can bypass the required `Check, Build, and Test (Rust)` check; `enforce_admins=false`). Run touched-crate clippy first (admin-merge + fast pre-push skip clippy — see memory `feedback_admin_merge_clippy_gap`).
- [ ] After merge: remove this worktree and prune (`git worktree remove` / `git worktree prune`); see memory `feedback_worktree_target_bloat_cleanup` for the Windows "not empty" fallback.
