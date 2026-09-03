# Language AI-Authorship Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Agents can write idiomatic Vox (`|x|` sugar on existing lambdas, honest `@ai`, a test loop that names failing tests in `.vox` lines) and the compiler refuses silent no-ops.

**Architecture:** Closures already parse as `fn(x) …` → `HirExpr::Lambda` with `list`/`Option`/`Result` `.map` and Rust `move |` emit. L01 is **prefix `|` in expr position** onto that pipeline — **do not add `HirExpr::Closure`**. `parse_script` takes owned tokens; `lower_module` returns `HirModule` not `Result`. Copy helpers from `emission_ladder_test.rs`. `@ai` stops emitting name-only schemas, uses doc comments as the prompt, gains `EffectAnnotation::Llm` **and** `HirCapability::Llm`, and binds budget fields or errors. `vox test --json` **replaces or nests** `BuildLaneEnvelope` — never two schemas. Remaining honesty items are small isolated tasks.

## Audit corrections (spec §9)

- `lower_module(&module)` is infallible — do not `.unwrap_or_else`.
- Match `HirExpr::Lambda`, not `Closure`. Body is `Box<HirExpr>` (use `HirExpr::Block` for `{ }` form).
- Task 3: do **not** register methods (already in `typeck/builtins.rs`). Golden: `xs.map(|x| x * 2)` plus regression `xs.map(fn(x: int) to int { … })` and `type A = | Foo | Bar` / `|>`.
- L18: **prove** native artifact name (`compile_native_artifact_name`); help/doctor must not advertise `--target server|client|fullstack` as shipped.
- L19: verify `emit_main` HTTP vs `main_boot` before adding `durable-http-boot-unimplemented`.
- `EffectAnnotation::Llm` forces `HirCapability::Llm` exhaustiveness.

**Tech Stack:** `vox-compiler`, `vox-ast`, `vox-codegen`, `vox-cli`, `vox-actor-runtime` (cassette), JSON contracts.

**Spec:** [`docs/superpowers/specs/2026-08-31-platform-parity-design.md`](../specs/2026-08-31-platform-parity-design.md) §3.4–3.6. RFC: [`docs/src/architecture/closures-rfc-2026-05-23.md`](../../src/architecture/closures-rfc-2026-05-23.md). Do not duplicate remaining work already specified in [`docs/src/architecture/ai-first-plan-1-language-toolchain-2026-07-02.md`](../../src/architecture/ai-first-plan-1-language-toolchain-2026-07-02.md) if those tasks are still open — finish them first, then this plan’s Task 4+ (schema fallback is the delta).

**Closes:** L01–L20.

## Global Constraints

Inherit spec §6. Additional:

- No `vox-compiler` → `vox-orchestrator` crate edge. Model pins validate against `contracts/models/known-slugs.v1.json`.
- Closures v1: expression or block body; no `FnMut` recursive closures; TS/client emit is a hard diagnostic, not a wrong lowering.
- First step of Task 1: `rg "Token::Bar" crates/vox-compiler` and note existing uses so `|x|` does not break `|>` (`Token::PipeOp`).

---

## File map

| File | Role |
|---|---|
| Modify: `crates/vox-compiler/src/parser/descent/` (expr parse) | `| params | body` |
| Modify: `crates/vox-compiler/src/hir/nodes/stmt_expr.rs` | **Do not add Closure** — lower bar-lambda to existing `HirExpr::Lambda` |
| Modify: `crates/vox-compiler/src/typeck/` | closure inference |
| Modify: `crates/vox-codegen/src/codegen_rust/` | `move \|...\|` |
| Create: `crates/vox-compiler/tests/closures_parse.rs` | parse/typeck goldens |
| Modify: `crates/vox-ast/src/decl/effect.rs` | `Llm` |
| Modify: `crates/vox-codegen/src/codegen_rust/emit/ai_fixture/llm.rs` | prompt + schema error |
| Modify: `crates/vox-compiler/src/hir/nodes/decl.rs` | `doc_comment`, budget fields actually read |
| Create: `contracts/reports/vox-test-run.v1.json` | schema |
| Create: `contracts/models/known-slugs.v1.json` | L11 |
| Modify: `crates/vox-cli/src/commands/test.rs` | JSONL/remap/exit codes |
| Modify: `crates/vox-cli/src/commands/repair.rs` (rg `openrouter` there) | facade |

---

### Task 1: Parse `|x| x` into existing `HirExpr::Lambda`

**Files:**
- Modify: parser expr descent (rg `fn parse_expr` under `crates/vox-compiler/src/parser`) — **expr position only**; ADT `eat(&Token::Bar)` in `decl/mid.rs` stays
- Test: `crates/vox-compiler/tests/closures_parse.rs`

**Interfaces:**
- Consumes: `Token::Bar`
- Produces: `HirExpr::Lambda` as live today for `fn(x) x`

- [ ] **Step 1: Write the failing test**

Use the same pipeline as `crates/vox-compiler/tests/emission_ladder_test.rs`: `lex` → `parse_script(tokens)` (owned `Vec`, not `&tokens`) → `lower_module` → walk HIR. Do **not** invent `test_util::hir_from_source`. Do **not** treat `lower_module` as `Result`.

```rust
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::lex;
use vox_compiler::parser::parse_script;
use vox_compiler::hir::nodes::stmt_expr::HirExpr;

#[test]
fn parse_identity_closure() {
    let src = "fn main() { let f = |x| x }";
    let tokens = lex(src);
    let ast = parse_script(tokens).unwrap_or_else(|e| panic!("{e:?}"));
    let hir = lower_module(&ast);
    let init = find_let_init(&hir, "f").expect("let f");
    match init {
        HirExpr::Lambda(params, _, body, _, _) => {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "x");
            let _ = body;
        }
        other => panic!("expected Lambda, got {other:?}"),
    }
}

#[test]
fn adt_bar_and_pipe_op_still_parse() {
    let src = "type A = | Foo | Bar\nfn main() { xs |> map }";
    let tokens = lex(src);
    let ast = parse_script(tokens);
    assert!(ast.is_ok() || /* map unbound is typeck, parse must succeed */ ast.is_ok());
}
```

Adjust the pipe golden if `xs` is unbound at parse — parse-only: `1 |> id` or whatever `pratt_ops` accepts. `find_let_init` is a 10-line walk in the test file.

- [ ] **Step 2:** `cargo test -p vox-compiler parse_identity_closure -- --nocapture` FAIL (parse error on `|` in expr position).

- [ ] **Step 3:** When current token is `Bar` in expr position, parse param list until `Bar`, then expr or `{` block. Lower to **`HirExpr::Lambda`** (same as `fn(x)`). Empty `||` special-case two `Bar`s. Do not change decl variant parsing.

- [ ] **Step 4:** PASS `parse_identity_closure` + ADT/`|>` regression.

- [ ] **Step 5:** commit `feat: parse |x| bar-lambda sugar into HirExpr::Lambda`

- [ ] **Step 5: Commit**

```bash
git add crates/vox-compiler
git commit -m "feat: parse closure literals into HIR"
```

---

### Task 2: Typeck + emit for `|x|` (Lambda already works for `fn(x)`)

If `fn(x: int) x` already typecks and emits `move |` (`list_hof_emit_test.rs`), this task is **bar-syntax only**: `apply(|n| n, 1)` typecks; `emit_rust` of `|x: int| x` contains `move |`. Extend `list_hof_emit_test.rs` rather than duplicating the HOF pipeline. Client/TS: diagnostic `closures-not-emitted-for-client` if not already.

Skip a greenfield typeck visitor rewrite.

### Task 3: Bar-syntax golden for existing `.map` (not method registration)

**Files:** `examples/golden/closures_map.vox` **or** extend `list_hof_emit_test.rs`.

LIVE: `List.map` / `Option.map` / `Result.map` already registered. Task is:

```vox
fn doubled(xs: list[int]) to list[int] {
    xs.map(|x| x)
}
```

plus keep `xs.map(fn(x: int) to int { x * 2 })` compiling.

- [ ] **Step 1:** Golden or compiler test that `|x|` form typecks. If it fails, Task 1/2 incomplete — not missing methods.

- [ ] **Step 2:** If `vox check` fails with “no map method”, you are on a stale branch — `rg map crates/vox-compiler/src/typeck/builtins.rs` before adding a second table.

- [ ] **Step 3:** No method-table rewrite unless `rg` proves absence.

- [ ] **Step 4:** `vox check` + `cargo test -p vox-codegen list_map` green.

- [ ] **Step 5:** commit `test: bar-lambda sugar on list.map`

---

### Task 4: `@ai` unknown type is a compile error; prompt from docs

**Files:**
- Modify: `crates/vox-codegen/src/codegen_rust/emit/ai_fixture/llm.rs` (delete the name-only `None` branch)
- Modify: typeck `@ai` structured output
- Modify: `HirFn` + parser trivia → `doc_comment: Option<String>`
- Test: `crates/vox-codegen/tests/ai_structured_output_emit.rs` (already exists — add cases)

**Interfaces:**
- Consumes: existing `schema_for`
- Produces: no name-only payload; prompt uses `doc_comment`

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn unknown_structured_output_type_is_err() {
    let src = r#"
        @ai(structured_output = NotAType)
        fn f() -> NotAType { }
    "#;
    assert!(typeck_err_code(src, "ai-unknown-schema-type"));
}

#[test]
fn prompt_uses_doc_comment() {
    let rust = emit_rust(
        r#"
        /// Return the user's display name
        @ai
        fn display_name(id: str) -> str { }
    "#,
    );
    assert!(rust.contains("Return the user's display name"));
    assert!(!rust.contains("Implement the function: display_name"));
}
```

- [ ] **Step 2: Run tests — FAIL** (name-only still emitted; hardcoded prompt).

- [ ] **Step 3: Implementation** — `schema_for` None → diagnostic, skip emit. Parser fills `doc_comment`. Replace `Implement the function: {name}` with doc comment if present, else `name` as a **fallback suffix** after a diagnostic warning is **not** enough — spec says never the sole user message. If no doc and no `@prompt`, typeck error `ai-missing-prompt`.

- [ ] **Step 4: Tests PASS.**

- [ ] **Step 5: Commit** `fix: @ai requires a schema body and a real prompt channel`

---

### Task 5: `EffectAnnotation::Llm` and budget bind

**Files:**
- Modify: `crates/vox-ast/src/decl/effect.rs` (`Llm`, `from_keyword("llm")`)
- Modify: typeck `@ai` implies Llm; `@pure` + `@ai` error
- Modify: codegen reads `cost_ceiling_usd_per_call` / `ai_max_iterations` into `LlmConfig` (rg those names in HIR)
- Test: typeck tests + emit test that generated Rust contains the ceiling / loop bound

**Interfaces:**
- Consumes: spec §3.5
- Produces: `uses llm`; budgets not dropped

- [ ] **Step 1: Failing tests**

```rust
#[test]
fn pure_ai_is_error() {
    assert!(typeck_err("@pure @ai fn f() -> str {}", "pure-ai-conflict"));
}

#[test]
fn budget_appears_in_emit() {
    let rust = emit_rust("@ai(cost_ceiling_usd_per_call = 0.05, max_iterations = 3) fn f() -> str {}");
    assert!(rust.contains("0.05") || rust.contains("max_iterations"));
}
```

Use the real decorator/attribute syntax from `examples/golden/ai_fixtures/` — do not guess. `rg "cost_ceiling" examples crates/vox-compiler`.

- [ ] **Step 3:** add `EffectAnnotation::Llm` **and** `HirCapability::Llm` (exhaustiveness in `decl.rs` lower_fn, effect_check, semcov, Display, `from_keyword`, unit tests in `effect.rs`). `@pure` + `@ai` error. If HIR `cost_ceiling_usd_per_call` exists and codegen ignores it, **read them** or typeck `budget-annotation-dropped`. Use golden syntax from `examples/golden/ai_fixtures/` (`to T` not `->`, `@uses(net)`). **Step 4:** PASS. **Step 5:** commit `feat: llm effect and enforced @ai budgets`.

---

### Task 6: Model pin snapshot (L11)

**Files:**
- Create: `contracts/models/known-slugs.v1.json` `{ "slugs": ["openai/gpt-4.1-mini"] }` (include at least 5 real OpenRouter slugs already in `vox-config` bootstrap — `rg OPENROUTER_FREE crates/vox-config`)
- Modify: typeck to load that JSON at compile time (`include_str!` in compiler **or** read via `vox-config` if that edge already exists). Prefer `include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../contracts/models/known-slugs.v1.json"))` from `vox-compiler` to avoid a new crate edge.
- Test: `@ai(model = "gpt-4o-mimi")` errors; a listed slug passes.

- [ ] **Step 1–5:** failing typeck test → JSON file + check → `cargo test -p vox-compiler` → commit `feat: reject unknown @ai model pins against known-slugs snapshot`

---

### Task 7: `vox test` per-test JSON + sourcemap + exit codes (L05 L06 L08 L14)

**Files:**
- Create: `contracts/reports/vox-test-run.v1.json` (JSON Schema, `x-vox-version`)
- Modify: codegen to write `target/generated/.vox-sourcemap.json` as `{ "rust_file:line": { "file": "a.vox", "line": 10 } }` during emit
- Modify: `crates/vox-cli/src/commands/test.rs` — capture cargo JSON (`cargo test -- --format json` is unstable; use `cargo test --message-format json` for build + parse libtest JSON via `CARGO_TERM_VERBOSE` **or** run `cargo test -- --report-time -Z unstable-options --format json` only on nightly — **do not**). Portable approach: `cargo test -- --list` then run each test with `--exact` **only if** count ≤ 32; else parse stdout with a conservative regex for `test {name} ... ok|FAILED` (document the parser). Prefer libtest’s default lines: `test foo::bar ... ok`.
- Clap: if `--coverage` still does not run llvm-cov, **remove the flag**. Same for `--update-snapshots` and `--forall-iterations` unless you implement them in this task (YAGNI: remove).
- Exit: `std::process::exit` codes 2/3/4 as spec §3.6. Map `build::run` failure → 2, cargo test nonzero → 3, `Command` spawn fail → 4.

**Interfaces:**
- Consumes: spec §3.6
- Produces: **one** JSON object on `--json`. Today `--json` emits `BuildLaneEnvelope` (`envelope_version`, `command`, `ok`, …). **Replace or nest** `tests[]` inside that envelope — never print two competing schemas. Update envelope consumers in the same commit.

- [ ] **Step 1: Failing test** in `crates/vox-cli` tests (look for existing `commands/test` tests). A fixture `.vox` with one `@test` that fails; `run` with json; assert `tests[0].status == "failed"` and `source.file` ends with `.vox`.

- [ ] **Step 2: FAIL** (no per-test array).

- [ ] **Step 3: Implement parse + sourcemap remap** (`remap_path(msg, sourcemap)`).

- [ ] **Step 4: PASS.**

- [ ] **Step 5: Commit** `feat: per-test JSON, sourcemaps, honest vox test flags and exit codes`

---

### Task 8a: stub-check default (L09)

**Files:** `crates/vox-cli/Cargo.toml` `default` features; stub-check module.

- [ ] **Step 1:** test `todo_emits_stub_finding_with_default_features` — `vox check --for-llm` on a fixture with `todo!()` reports a finding. Today default features omit `stub-check`.
- [ ] **Step 2:** FAIL.
- [ ] **Step 3:** `default = [..., "stub-check"]`. Mutants stay optional.
- [ ] **Step 4:** PASS.
- [ ] **Step 5:** commit `feat: ship stub-check in the default vox binary`

### Task 8b: vox-lsp in dist + doctor (L15)

**Files:** `contracts/distribution/profiles.v1.yaml`; doctor `--fix-lsp`.

- [ ] **Step 1:** test `default_dist_profile_lists_vox_lsp`.
- [ ] **Step 2:** FAIL (binaries = vox, vox-ml-cli, voxup).
- [ ] **Step 3:** add `vox-lsp`; doctor documents install path.
- [ ] **Step 4:** PASS.
- [ ] **Step 5:** commit `feat: include vox-lsp in distribution profiles`

### Task 8c: repair through llm facade (L16)

**Files:** `crates/vox-cli/src/commands/repair.rs`.

- [ ] **Step 1:** test file does not contain `openrouter` URL; `llm_chat` used. Replay cassette works without OpenRouter key.
- [ ] **Step 2:** FAIL (reqwest to OpenRouter).
- [ ] **Step 3:** `vox_actor_runtime::llm::llm_chat` only.
- [ ] **Step 4:** PASS.
- [ ] **Step 5:** commit `fix: vox repair uses the LLM facade not OpenRouter`

### Task 8d: diag URLs + `vox explain` (L17)

**Files:** diag URL constants; new clap `explain`; `contracts/diagnostics/registry.v1.yaml`.

- [ ] **Step 1:** `explain_unknown_code_exits_1`; known code prints title. URLs use live host (`rg voxlang.org`).
- [ ] **Step 2:** FAIL.
- [ ] **Step 3:** implement; fix wrong domain.
- [ ] **Step 4:** PASS.
- [ ] **Step 5:** commit `feat: vox explain and live diagnostic URLs`

### Task 8e: LLM replay cassette (L10)

**Files:** `vox-actor-runtime` llm; `contracts/config/env-vars.v1.yaml` `VOX_LLM_REPLAY`.

- [ ] **Step 1:** tempfile JSONL request/response; `llm_chat` returns cassette body.
- [ ] **Step 2:** FAIL.
- [ ] **Step 3:** if env set, read cassette (user path or `VOX_CACHE_DIR`).
- [ ] **Step 4:** PASS.
- [ ] **Step 5:** commit `feat: VOX_LLM_REPLAY cassette on the LLM facade`

---

### Task 9a: constrained-gen real mask (L12)

**Files:** `crates/vox-constrained-gen` tests; one mens/populi call site (`rg mask_next`).

- [ ] **Step 1:** `mask_next_rejects_illegal_token` **not** `#[ignore]`. If no API, the test compiles a call that does not exist → FAIL until wired.
- [ ] **Step 2:** FAIL.
- [ ] **Step 3:** wire one sampler **or** export `mask_next` and test it.
- [ ] **Step 4:** PASS on CI (non-ignored).
- [ ] **Step 5:** commit `test: constrained-gen masks at least one illegal token`

### Task 9b: project-wide `vox check` (L13)

**Files:** `crates/vox-cli/src/commands/check.rs` clap.

- [ ] **Step 1:** `check_dir_walks_vox_files` temp dir two `.vox`, one error → nonzero. `--since` documented; implement dir walk first (cap 256).
- [ ] **Step 2:** FAIL (file required).
- [ ] **Step 3:** optional path; walk `*.vox`.
- [ ] **Step 4:** PASS.
- [ ] **Step 5:** commit `feat: vox check on a directory of .vox files`

### Task 9c: native artifact is not `.wasm` (L18)

**Files:** `compile.rs` / `script.rs`. Help text for `--target server|client|fullstack`.

- [ ] **Step 1:** `native_isolation_artifact_has_no_wasm_suffix`. Help/doctor does **not** claim those targets shipped.
- [ ] **Step 2:** FAIL if suffix wrong **or** help lies.
- [ ] **Step 3:** fix name; honesty on roadmap targets.
- [ ] **Step 4:** PASS.
- [ ] **Step 5:** commit `fix: native compile artifact naming and target honesty`

### Task 9d: durable HTTP boot diagnostic (L19)

**Files:** `emit_main_boot.rs` / `http.rs`.

- [ ] **Step 1:** compiling `@durable` HTTP server emits `durable-http-boot-unimplemented` (error), not silent skip. If HTTP already boots, test asserts success and this task is ledger `done` with evidence.
- [ ] **Step 2:** FAIL if silent.
- [ ] **Step 3:** diagnostic only — no full refactor.
- [ ] **Step 4:** PASS.
- [ ] **Step 5:** commit `fix: durable HTTP boot is a hard diagnostic not a silent skip`

### Task 9e: scripts/ check ratchet (L20)

**Files:** `contracts/ci/scripts-check.allow.v1.txt`; `vox ci script-check`.

- [ ] **Step 1:** `scripts/fmt.vox` must `vox check` clean. Allowlist records **today’s** other failures once; later commits may only shrink it.
- [ ] **Step 2:** FAIL if no gate.
- [ ] **Step 3:** implement tighten-only.
- [ ] **Step 4:** PASS.
- [ ] **Step 5:** commit `feat: vox ci script-check ratchet for scripts/*.vox`

---


## Track 2 gate

HARD: `cargo test -p vox-compiler -p vox-codegen -p vox-cli parse_identity_closure emit_move_closure`

HARD: `vox check examples/golden/closures_map.vox` (after Task 3)

HARD: unknown `@ai` type fails typeck
