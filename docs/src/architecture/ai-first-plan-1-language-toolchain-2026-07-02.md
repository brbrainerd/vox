---
title: "AI-Authorship Language/Toolchain Gaps Implementation Plan"
description: "Closes G-L1 (@ai structured_output emits a real JSON-schema body derived from the return typedef, plus a max_iterations re-prompt loop), G-T1 (uniform --json build-lane envelope for vox build/test/run), and G-T7 (vox doctor --diag <id> single-check filter)."
category: "Architecture SSOTs"
status: "roadmap"
last_updated: "2026-07-02"
training_eligible: false
authored: "2026-07-02"
---

# AI-Authorship Language/Toolchain Gaps Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the three highest-leverage gaps blocking machines from authoring Vox code end-to-end: (1) make `@ai(structured_output = T, max_iterations = N)` emit a real JSON-schema-constrained LLM call (today only a name-stub `response_format` is emitted and `max_iterations` is ignored), (2) give the build lane (`vox build`, `vox test`, `vox run --mode script`) the same machine-readable `--json` output `vox check` already has, and (3) let agents run exactly one `vox doctor` check via `--diag <id>` instead of grepping the full run.

**Architecture:** All @ai codegen work lands in `vox-codegen`'s Rust emitter: a new thread-local typedef registry (`ai_schema_ctx`, mirroring the existing `json_as_ctx` precedent) lets `emit_llm_function_body` derive a full JSON Schema from the module's `HirTypeDef`s and embed it in the emitted `LlmConfig.response_format` (`Option<serde_json::Value>` in `vox_actor_runtime::llm::types::LlmConfig`); `max_iterations` becomes an emitted re-prompt loop in the direct-LLM branch. The CLI work reuses the `VOX_CLI_GLOBAL_JSON=1` env contract set by the root `--json` flag (`crates/vox-cli/src/lib.rs:733`) and the `VoxCompilerDiagnosticPayload` type `vox check` already prints; a single `BuildLaneEnvelope` (compact one-line JSON, JSONL-friendly) covers build/test/run. Doctor filtering builds on the existing `[diag id=…]` detail-tag contract and `KNOWN_DIAGNOSIS_IDS` registry in `checks_standard/build_health.rs`.

**Tech Stack:** Rust workspace; crates `vox-codegen`, `vox-compiler` (read-only here), `vox-cli`; `serde_json`, `clap`, `anyhow`, `tokio`. Tests: cargo unit/integration tests (`cargo test -p <crate> …`). Never run `cargo clippy --all-targets` without `--exclude vox-gui`; never `cargo fmt --all` (use `cargo fmt -p <crate>`).

**Scope note (stretch dropped):** The graphify-out → vox-search stretch was researched and found already implemented: `vox_search::execution::route_graphify_structural` routes relational queries to `SearchCorpus::GraphifyStructural`, and execution loads graphs via `vox_config::graphify::load_graphify_corpora` / `load_all_corpora`. The repo's `graphify-out/` currently contains only markdown reports (no graph JSON), so there is no clean code gap to close; omitted per YAGNI.

---

### Task 1: JSON Schema derivation from HIR typedefs (`ai_schema_ctx`)

**Files:**
- Create: `crates/vox-codegen/src/codegen_rust/emit/ai_schema_ctx.rs`
- Modify: `crates/vox-codegen/src/codegen_rust/emit/mod.rs` (add `mod ai_schema_ctx;` after line 13 `mod ai_fixture;`)
- Test: in-module `#[cfg(test)]` tests inside `ai_schema_ctx.rs`

- [ ] Create `crates/vox-codegen/src/codegen_rust/emit/ai_schema_ctx.rs` with the module doc, API stubs, and the unit tests (red phase — `schema_for` stub returns `None` so tests fail):

```rust
//! Thread-local typedef registry for `@ai(structured_output = T)` schema emission.
//!
//! Mirrors the `json_as_ctx` precedent: `emit_lib` registers the module's
//! typedefs for the duration of the emit; `emit_llm_function_body` (which only
//! receives the `HirFn`, not the module) resolves the structured-output type
//! name to a full JSON Schema here. Nested user structs recurse with a depth
//! cap so self-referential types cannot hang codegen.

use std::cell::RefCell;

use vox_compiler::hir::{HirType, HirTypeDef};

thread_local! {
    static MODULE_TYPES: RefCell<Vec<HirTypeDef>> = const { RefCell::new(Vec::new()) };
}

/// RAII guard restoring the previously-registered typedefs on drop.
pub(super) struct AiSchemaGuard(Vec<HirTypeDef>);

impl Drop for AiSchemaGuard {
    fn drop(&mut self) {
        MODULE_TYPES.with(|c| *c.borrow_mut() = std::mem::take(&mut self.0));
    }
}

/// Register `types` as the current module's typedefs for schema lookup.
pub(super) fn enter_module_types(types: &[HirTypeDef]) -> AiSchemaGuard {
    let prev = MODULE_TYPES.with(|c| std::mem::replace(&mut *c.borrow_mut(), types.to_vec()));
    AiSchemaGuard(prev)
}

/// Full JSON Schema for the struct typedef named `type_name`, or `None` when
/// the name is unregistered, a sum type, or fieldless (callers fall back to
/// the legacy name-only `response_format`).
pub(super) fn schema_for(type_name: &str) -> Option<serde_json::Value> {
    let _ = type_name;
    None // implemented in the green step
}
```

and the tests at the bottom of the same file:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use vox_compiler::ast::span::Span;
    use vox_compiler::hir::DefId;

    fn stub_typedef() -> HirTypeDef {
        HirTypeDef {
            id: DefId(1),
            name: "StubDto".into(),
            variants: vec![],
            fields: vec![
                ("ok".into(), HirType::Named("bool".into())),
                ("score".into(), HirType::Named("int".into())),
                (
                    "tags".into(),
                    HirType::Generic("list".into(), vec![HirType::Named("str".into())]),
                ),
            ],
            is_pub: true,
            span: Span::new(0, 0),
        }
    }

    #[test]
    fn schema_for_struct_maps_scalars_lists_and_closes_object() {
        let td = stub_typedef();
        let _guard = enter_module_types(std::slice::from_ref(&td));
        let schema = schema_for("StubDto").expect("registered struct yields a schema");
        assert_eq!(
            schema,
            serde_json::json!({
                "type": "object",
                "properties": {
                    "ok": {"type": "boolean"},
                    "score": {"type": "integer"},
                    "tags": {"type": "array", "items": {"type": "string"}},
                },
                "required": ["ok", "score", "tags"],
                "additionalProperties": false
            })
        );
    }

    #[test]
    fn schema_for_unknown_type_is_none() {
        let td = stub_typedef();
        let _guard = enter_module_types(std::slice::from_ref(&td));
        assert!(schema_for("NotRegistered").is_none());
    }

    #[test]
    fn schema_for_is_none_outside_guard() {
        assert!(schema_for("StubDto").is_none());
    }

    #[test]
    fn nested_user_struct_recurses_and_option_is_nullable() {
        let inner = HirTypeDef {
            id: DefId(2),
            name: "Inner".into(),
            variants: vec![],
            fields: vec![("n".into(), HirType::Named("int".into()))],
            is_pub: true,
            span: Span::new(0, 0),
        };
        let outer = HirTypeDef {
            id: DefId(3),
            name: "Outer".into(),
            variants: vec![],
            fields: vec![
                ("inner".into(), HirType::Named("Inner".into())),
                (
                    "maybe".into(),
                    HirType::Generic("Option".into(), vec![HirType::Named("str".into())]),
                ),
            ],
            is_pub: true,
            span: Span::new(0, 0),
        };
        let types = vec![inner, outer];
        let _guard = enter_module_types(&types);
        let schema = schema_for("Outer").expect("schema");
        assert_eq!(
            schema["properties"]["inner"],
            serde_json::json!({
                "type": "object",
                "properties": {"n": {"type": "integer"}},
                "required": ["n"],
                "additionalProperties": false
            })
        );
        assert_eq!(
            schema["properties"]["maybe"],
            serde_json::json!({"anyOf": [{"type": "string"}, {"type": "null"}]})
        );
    }
}
```

- [ ] Register the module: in `crates/vox-codegen/src/codegen_rust/emit/mod.rs` add `mod ai_schema_ctx;` directly after `mod ai_fixture;` (line 13).
- [ ] Run: `cargo test -p vox-codegen ai_schema_ctx` — expect the struct/nested tests FAIL (`schema_for` returns `None`; only the unknown-type and outside-guard tests pass).
- [ ] Implement the real derivation, replacing the `schema_for` stub:

```rust
/// Full JSON Schema for the struct typedef named `type_name`, or `None` when
/// the name is unregistered, a sum type, or fieldless (callers fall back to
/// the legacy name-only `response_format`).
pub(super) fn schema_for(type_name: &str) -> Option<serde_json::Value> {
    MODULE_TYPES.with(|c| {
        let types = c.borrow();
        let td = find_struct_typedef(&types, type_name)?;
        Some(schema_for_typedef(td, &types, 0))
    })
}

/// Recursion cap for nested user structs (self-referential types terminate).
const MAX_SCHEMA_DEPTH: u8 = 4;

fn find_struct_typedef<'a>(types: &'a [HirTypeDef], name: &str) -> Option<&'a HirTypeDef> {
    types
        .iter()
        .find(|t| t.name == name && t.variants.is_empty() && !t.fields.is_empty())
}

fn schema_for_typedef(td: &HirTypeDef, types: &[HirTypeDef], depth: u8) -> serde_json::Value {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::with_capacity(td.fields.len());
    for (fname, fty) in &td.fields {
        properties.insert(fname.clone(), schema_for_hir_type(fty, types, depth));
        required.push(serde_json::Value::String(fname.clone()));
    }
    serde_json::json!({
        "type": "object",
        "properties": serde_json::Value::Object(properties),
        "required": required,
        "additionalProperties": false
    })
}

fn schema_for_hir_type(ty: &HirType, types: &[HirTypeDef], depth: u8) -> serde_json::Value {
    use serde_json::json;
    if depth >= MAX_SCHEMA_DEPTH {
        return json!({});
    }
    match ty {
        HirType::Named(n) => match n.as_str() {
            "int" => json!({"type": "integer"}),
            "float" => json!({"type": "number"}),
            "bool" => json!({"type": "boolean"}),
            "str" => json!({"type": "string"}),
            other => match find_struct_typedef(types, other) {
                Some(td) => schema_for_typedef(td, types, depth + 1),
                None => json!({}),
            },
        },
        HirType::Generic(outer, args) if outer == "list" || outer == "List" => {
            let items = args
                .first()
                .map(|a| schema_for_hir_type(a, types, depth + 1))
                .unwrap_or_else(|| json!({}));
            json!({"type": "array", "items": items})
        }
        HirType::Generic(outer, args) if outer == "Option" => {
            let inner = args
                .first()
                .map(|a| schema_for_hir_type(a, types, depth + 1))
                .unwrap_or_else(|| json!({}));
            json!({"anyOf": [inner, {"type": "null"}]})
        }
        HirType::Decimal => json!({"type": "string"}),
        _ => json!({}),
    }
}
```

- [ ] Run: `cargo test -p vox-codegen ai_schema_ctx` — expect all 4 tests PASS.
- [ ] Format and commit:
```
cargo fmt -p vox-codegen
git add crates/vox-codegen/src/codegen_rust/emit/ai_schema_ctx.rs crates/vox-codegen/src/codegen_rust/emit/mod.rs
git commit -m "feat(codegen): derive JSON Schema from HIR typedefs for @ai structured_output"
```

---

### Task 2: Emit full `json_schema` response_format through `emit_lib`

**Files:**
- Modify: `crates/vox-codegen/src/codegen_rust/emit/workflow.rs` (`emit_lib`, line 20)
- Modify: `crates/vox-codegen/src/codegen_rust/emit/ai_fixture/llm.rs` (lines 25–31)
- Test: `crates/vox-codegen/tests/ai_structured_output_emit.rs`

- [ ] Add the failing integration test to `crates/vox-codegen/tests/ai_structured_output_emit.rs` (append; note `emit_lib` is already exported from `vox_codegen::codegen_rust::emit`):

```rust
#[test]
fn emit_lib_embeds_full_json_schema_for_structured_output() {
    let src = r#"
        type StubDto {
            ok: bool
            score: int
        }

        @ai(structured_output = StubDto)
        @uses(net)
        fn with_schema(ctx: str) to StubDto {
            return StubDto { ok: true, score: 1 }
        }
    "#;
    let ast = parse(lex(src)).expect("parse");
    let hir = lower_module(&ast);
    let lib = vox_codegen::codegen_rust::emit::emit_lib(&hir);
    assert!(
        lib.contains("\"strict\":true"),
        "expected strict json_schema payload, got:\n{lib}"
    );
    assert!(
        lib.contains("\"name\":\"StubDto\""),
        "expected schema name, got:\n{lib}"
    );
    assert!(
        lib.contains("\"ok\":{\"type\":\"boolean\"}"),
        "expected bool field schema, got:\n{lib}"
    );
    assert!(
        lib.contains("\"score\":{\"type\":\"integer\"}"),
        "expected int field schema, got:\n{lib}"
    );
    assert!(
        lib.contains("\"required\":[\"ok\",\"score\"]"),
        "expected required list in declaration order, got:\n{lib}"
    );
    assert!(
        lib.contains("\"additionalProperties\":false"),
        "expected closed object schema, got:\n{lib}"
    );
    assert!(
        lib.contains("config.response_format = Some(response_format);"),
        "expected response_format wired into LlmConfig, got:\n{lib}"
    );
}
```

- [ ] Run: `cargo test -p vox-codegen --test ai_structured_output_emit emit_lib_embeds_full_json_schema_for_structured_output` — expect FAIL on the `"strict":true` assertion (current emit is name-only).
- [ ] In `crates/vox-codegen/src/codegen_rust/emit/workflow.rs`, register typedefs at the top of `emit_lib` (first line of the function body, line 21):

```rust
pub fn emit_lib(module: &HirModule) -> String {
    // Register typedefs so `@ai(structured_output = T)` emission can embed T's
    // full JSON Schema (guard restores prior state on drop; covers the whole
    // emit including MCP tools/resources and the script lane via emit_script_lib).
    let _ai_schema_guard = super::ai_schema_ctx::enter_module_types(&module.types);
    let mut out = String::new();
```

- [ ] In `crates/vox-codegen/src/codegen_rust/emit/ai_fixture/llm.rs`, replace the name-only emission (lines 25–31) with schema-aware emission (fallback preserves the existing name-only payload so direct `emit_fn` callers without a registered module keep today's output):

```rust
    if let Some(s) = structured_output {
        match super::super::ai_schema_ctx::schema_for(&s.return_type) {
            Some(schema) => {
                let payload = serde_json::json!({
                    "type": "json_schema",
                    "json_schema": {
                        "name": s.return_type,
                        "strict": true,
                        "schema": schema,
                    }
                });
                let payload_src = serde_json::to_string(&payload)
                    .expect("vox codegen: response_format payload serializes");
                out.push_str(&format!(
                    "    let response_format = serde_json::json!({payload_src});\n"
                ));
            }
            None => {
                let schema_name = s.return_type.replace('\\', "\\\\").replace('"', "\\\"");
                out.push_str(&format!(
                    "    let response_format = serde_json::json!({{\"type\":\"json_schema\",\"json_schema\":{{\"name\":\"{}\"}}}});\n",
                    schema_name
                ));
            }
        }
    }
```

- [ ] Run: `cargo test -p vox-codegen --test ai_structured_output_emit` — expect ALL tests PASS (the pre-existing tests, including the name-only fallback assertions in `emit_fn_includes_response_format_for_ai_structured_output`, must still pass).
- [ ] Format and commit:
```
cargo fmt -p vox-codegen
git add crates/vox-codegen/src/codegen_rust/emit/workflow.rs crates/vox-codegen/src/codegen_rust/emit/ai_fixture/llm.rs crates/vox-codegen/tests/ai_structured_output_emit.rs
git commit -m "feat(codegen): emit strict JSON-schema response_format for @ai structured_output"
```

---

### Task 3: Honor `max_iterations` with an emitted re-prompt loop

**Files:**
- Modify: `crates/vox-codegen/src/codegen_rust/emit/ai_fixture/llm.rs` (`emit_direct_llm_body`, lines 285–323)
- Test: `crates/vox-codegen/tests/ai_structured_output_emit.rs`

- [ ] Add the failing tests to `crates/vox-codegen/tests/ai_structured_output_emit.rs`:

```rust
#[test]
fn emit_fn_wraps_structured_output_call_in_reprompt_loop() {
    let src = r#"
        type StubDto {
            ok: bool
        }

        @ai(structured_output = StubDto, max_iterations = 2)
        @uses(net)
        fn with_retry(ctx: str) to StubDto {
            return StubDto { ok: true }
        }
    "#;
    let ast = parse(lex(src)).expect("parse");
    let hir = lower_module(&ast);
    let f = hir
        .functions
        .iter()
        .find(|f| f.name == "with_retry")
        .expect("with_retry");
    let emitted = emit_fn(f, Some(&hir.inferred_types), &[]);
    assert!(
        emitted.contains("for _attempt in 0..2u32"),
        "expected max_iterations-bounded retry loop, got:\n{emitted}"
    );
    assert!(
        emitted.contains("__structured_ok"),
        "expected structured-output validity flag, got:\n{emitted}"
    );
    assert!(
        emitted.contains("prompt.push_str(\"\\nYour previous reply was not a valid JSON object."),
        "expected re-prompt on invalid JSON, got:\n{emitted}"
    );
}

#[test]
fn emit_fn_stays_single_shot_without_structured_output() {
    let src = r#"
        @ai(model = "openrouter/auto")
        @uses(net)
        fn plain(ctx: str) to str {
            return ctx
        }
    "#;
    let ast = parse(lex(src)).expect("parse");
    let hir = lower_module(&ast);
    let f = hir.functions.iter().find(|f| f.name == "plain").expect("plain");
    let emitted = emit_fn(f, Some(&hir.inferred_types), &[]);
    assert!(
        !emitted.contains("__structured_ok"),
        "no retry loop without structured_output, got:\n{emitted}"
    );
}
```

- [ ] Run: `cargo test -p vox-codegen --test ai_structured_output_emit emit_fn_wraps_structured_output_call_in_reprompt_loop` — expect FAIL (no loop emitted today).
- [ ] In `llm.rs`, restructure `emit_direct_llm_body` so the call site is chosen by `structured_output` (the config lines and intent telemetry stay exactly where they are); extract the existing single-shot lines into `emit_llm_call_single_shot` and add the loop variant:

```rust
fn emit_direct_llm_body(
    out: &mut String,
    intent_routed: Option<&vox_compiler::hir::nodes::boilerplate_grafts::HirAiIntentFixture>,
    structured_output: Option<&vox_compiler::hir::nodes::boilerplate_grafts::HirAiStructuredOutput>,
    model_init: &str,
) {
    let _ = model_init; // model variable already emitted before the branch
    out.push_str(
        "    let mut config = vox_actor_runtime::llm::LlmConfig::openrouter(model.clone());\n",
    );
    out.push_str("    config.temperature = Some(0.1);\n");
    if let Some(intent) = intent_routed {
        emit_intent_config(out, intent);
    }
    if structured_output.is_some() {
        out.push_str("    config.response_format = Some(response_format);\n");
    }
    match structured_output {
        Some(s) => emit_llm_call_with_reprompt(out, s.max_iterations.max(1)),
        None => emit_llm_call_single_shot(out),
    }
    if let Some(intent) = intent_routed {
        emit_intent_telemetry(out, intent);
    }
}

/// Single LLM call binding `content` (pre-structured_output behavior, verbatim).
fn emit_llm_call_single_shot(out: &mut String) {
    out.push_str(
        "    let res = vox_actor_runtime::llm::llm_chat(&options, vec![vox_actor_runtime::llm::LlmChatMessage {\n",
    );
    out.push_str("        role: \"user\".to_string(),\n");
    out.push_str("        content: prompt,\n");
    out.push_str("    }], config).await;\n");
    out.push_str("    let content = match res {\n");
    out.push_str("        vox_actor_runtime::ActivityResult::Ok(Ok(resp)) => resp.content,\n");
    out.push_str(
        "        vox_actor_runtime::ActivityResult::Ok(Err(e)) => panic!(\"LLM request failed: {}\", e),\n",
    );
    out.push_str(
        "        vox_actor_runtime::ActivityResult::Failed(e) => panic!(\"LLM activity failed: {:?}\", e),\n",
    );
    out.push_str(
        "        vox_actor_runtime::ActivityResult::Cancelled => panic!(\"LLM activity cancelled\"),\n",
    );
    out.push_str("    };\n");
}

/// `@ai(max_iterations = N)`: retry the call, re-prompting whenever the reply
/// is not parseable JSON (same fence-cleaning as the downstream typed parse).
fn emit_llm_call_with_reprompt(out: &mut String, max_iterations: u32) {
    out.push_str("    let mut content = String::new();\n");
    out.push_str("    let mut __structured_ok = false;\n");
    out.push_str(&format!("    for _attempt in 0..{max_iterations}u32 {{\n"));
    out.push_str(
        "        let res = vox_actor_runtime::llm::llm_chat(&options, vec![vox_actor_runtime::llm::LlmChatMessage {\n",
    );
    out.push_str("            role: \"user\".to_string(),\n");
    out.push_str("            content: prompt.clone(),\n");
    out.push_str("        }], config.clone()).await;\n");
    out.push_str("        let candidate = match res {\n");
    out.push_str("            vox_actor_runtime::ActivityResult::Ok(Ok(resp)) => resp.content,\n");
    out.push_str(
        "            vox_actor_runtime::ActivityResult::Ok(Err(e)) => panic!(\"LLM request failed: {}\", e),\n",
    );
    out.push_str(
        "            vox_actor_runtime::ActivityResult::Failed(e) => panic!(\"LLM activity failed: {:?}\", e),\n",
    );
    out.push_str(
        "            vox_actor_runtime::ActivityResult::Cancelled => panic!(\"LLM activity cancelled\"),\n",
    );
    out.push_str("        };\n");
    out.push_str(
        "        let cleaned = candidate.trim_matches('`').trim_start_matches(\"json\").trim().to_string();\n",
    );
    out.push_str("        content = candidate;\n");
    out.push_str("        if serde_json::from_str::<serde_json::Value>(&cleaned).is_ok() {\n");
    out.push_str("            __structured_ok = true;\n");
    out.push_str("            break;\n");
    out.push_str("        }\n");
    out.push_str(
        "        prompt.push_str(\"\\nYour previous reply was not a valid JSON object. Reply with ONLY a JSON object matching the return-type schema.\\n\");\n",
    );
    out.push_str("    }\n");
    out.push_str(&format!(
        "    if !__structured_ok {{ panic!(\"LLM structured output was not valid JSON after {max_iterations} attempt(s)\"); }}\n"
    ));
}
```

- [ ] Run: `cargo test -p vox-codegen --test ai_structured_output_emit` — expect ALL tests PASS.
- [ ] Run the crate's full suite to catch generated-code snapshot drift: `cargo test -p vox-codegen` — expect PASS. If any golden/snapshot test of emitted `@ai` bodies fails, update the golden per that test's bless instructions (the loop is an intentional emit change for structured_output functions only).
- [ ] Format and commit:
```
cargo fmt -p vox-codegen
git add crates/vox-codegen/src/codegen_rust/emit/ai_fixture/llm.rs crates/vox-codegen/tests/ai_structured_output_emit.rs
git commit -m "feat(codegen): honor @ai max_iterations with emitted JSON re-prompt loop"
```

---

### Task 4: Build-lane JSON envelope + `global_json_enabled` helper

**Files:**
- Modify: `crates/vox-cli/src/pipeline.rs`
- Test: `crates/vox-cli/tests/build_lane_envelope.rs` (create)

- [ ] Create the failing test `crates/vox-cli/tests/build_lane_envelope.rs`:

```rust
//! Shape tests for the build-lane `--json` envelope (`vox --json build/test/run`).

use std::path::{Path, PathBuf};

#[test]
fn build_lane_envelope_reports_errors_and_diagnostics() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/golden_rust_import_lowering.vox");
    let source = std::fs::read_to_string(&fixture).expect("read fixture");
    let file_label = Path::new("tests/fixtures/golden_rust_import_lowering.vox");
    let result =
        vox_cli::pipeline::run_frontend_str(&source, file_label, false).expect("frontend");
    assert!(result.has_errors(), "fixture must produce error diagnostics");

    let raw =
        vox_cli::pipeline::format_build_lane_envelope_json("build", file_label, &result, None);
    assert!(!raw.contains('\n'), "envelope must be single-line JSONL: {raw}");
    let v: serde_json::Value = serde_json::from_str(&raw).expect("valid JSON");
    assert_eq!(v["envelope_version"], 1);
    assert_eq!(v["command"], "build");
    assert_eq!(v["ok"], false);
    assert!(v["error_count"].as_u64().expect("error_count") >= 1);
    assert!(
        !v["diagnostics"].as_array().expect("diagnostics array").is_empty(),
        "diagnostics must carry VoxCompilerDiagnosticPayload entries"
    );
    assert!(
        v.get("exit_code").is_none(),
        "exit_code omitted when None: {raw}"
    );
}

#[test]
fn command_result_envelope_carries_exit_code() {
    let raw = vox_cli::pipeline::format_command_result_envelope_json(
        "test",
        Path::new("app.vox"),
        false,
        Some(101),
    );
    let v: serde_json::Value = serde_json::from_str(&raw).expect("valid JSON");
    assert_eq!(v["envelope_version"], 1);
    assert_eq!(v["command"], "test");
    assert_eq!(v["ok"], false);
    assert_eq!(v["exit_code"], 101);
    assert_eq!(v["diagnostics"].as_array().expect("array").len(), 0);
}
```

- [ ] Run: `cargo test -p vox-cli --test build_lane_envelope` — expect COMPILE FAILURE (`format_build_lane_envelope_json` does not exist).
- [ ] Implement in `crates/vox-cli/src/pipeline.rs` (append after `format_check_for_llm_json`, ~line 411):

```rust
/// True when the user passed the root `--json` flag —
/// [`crate::apply_global_opts`] / `run_vox_cli_from_parsed` set
/// `VOX_CLI_GLOBAL_JSON=1` before command dispatch.
#[must_use]
pub fn global_json_enabled() -> bool {
    std::env::var("VOX_CLI_GLOBAL_JSON").ok().as_deref() == Some("1")
}

/// Stable single-line JSON envelope for build-lane commands (`vox build`,
/// `vox test`, `vox run --mode script`). Mirrors [`CheckForLlmEnvelope`]
/// field naming; `command` discriminates the lane. Compact (one line) so
/// multiple envelopes on one stdout stream parse as JSONL.
#[derive(serde::Serialize)]
pub struct BuildLaneEnvelope {
    pub envelope_version: u32,
    pub command: String,
    pub file_path: String,
    pub ok: bool,
    pub error_count: usize,
    pub warning_count: usize,
    pub diagnostics: Vec<vox_compiler::typeck::diagnostics::VoxCompilerDiagnosticPayload>,
    /// Child-process exit code (`vox test`'s `cargo test`); absent for
    /// compile-only envelopes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
}

/// Envelope for a lane that just ran the frontend (diagnostics attached).
#[must_use]
pub fn format_build_lane_envelope_json(
    command: &str,
    file: &Path,
    result: &FrontendResult,
    exit_code: Option<i32>,
) -> String {
    use vox_compiler::typeck::diagnostics::VoxCompilerDiagnosticPayload;
    let file_path = file.to_string_lossy().to_string();
    let diagnostics: Vec<VoxCompilerDiagnosticPayload> = result
        .diagnostics
        .iter()
        .map(|d| VoxCompilerDiagnosticPayload::from_diagnostic(d, &file_path, &result.source))
        .collect();
    let env = BuildLaneEnvelope {
        envelope_version: 1,
        command: command.to_string(),
        file_path,
        ok: !result.has_errors(),
        error_count: result.error_count(),
        warning_count: result.warning_count(),
        diagnostics,
        exit_code,
    };
    serde_json::to_string(&env).unwrap_or_default()
}

/// Envelope for lane results with no [`FrontendResult`] at hand (e.g.
/// `vox test` after `cargo test` — the preceding build envelope already
/// carried the diagnostics).
#[must_use]
pub fn format_command_result_envelope_json(
    command: &str,
    file: &Path,
    ok: bool,
    exit_code: Option<i32>,
) -> String {
    let env = BuildLaneEnvelope {
        envelope_version: 1,
        command: command.to_string(),
        file_path: file.to_string_lossy().to_string(),
        ok,
        error_count: 0,
        warning_count: 0,
        diagnostics: Vec::new(),
        exit_code,
    };
    serde_json::to_string(&env).unwrap_or_default()
}
```

- [ ] Run: `cargo test -p vox-cli --test build_lane_envelope` — expect both tests PASS.
- [ ] Format and commit:
```
cargo fmt -p vox-cli
git add crates/vox-cli/src/pipeline.rs crates/vox-cli/tests/build_lane_envelope.rs
git commit -m "feat(cli): build-lane JSON envelope + global_json_enabled helper"
```

---

### Task 5: `vox build` honors global `--json`

**Files:**
- Modify: `crates/vox-cli/src/diagnostics.rs` (add `vox_note!` macro)
- Modify: `crates/vox-cli/src/commands/build.rs` (lines 47–48 and all `println!` sites inside `run()`)
- Modify: `crates/vox-cli/src/commands/check.rs` (line 49, use the shared helper)

- [ ] Add the note macro to `crates/vox-cli/src/diagnostics.rs` (append at the end, before the tests module):

```rust
// ── JSON-mode progress notes ─────────────────────────────────────────────────

/// Print a human progress note: stdout normally, **stderr** when `json` is
/// true — under global `--json`, stdout is reserved for machine envelopes
/// (single-line JSON, JSONL-parseable), matching the module contract above.
#[macro_export]
macro_rules! vox_note {
    ($json:expr, $($arg:tt)*) => {
        if $json { eprintln!($($arg)*) } else { println!($($arg)*) }
    };
}
```

- [ ] In `crates/vox-cli/src/commands/build.rs`, replace the frontend invocation (lines 47–55) with JSON-aware plumbing:

```rust
    let json = crate::pipeline::global_json_enabled();
    let frontend = crate::pipeline::run_frontend(file, json).await?;
    if json {
        // Single-line envelope on stdout (JSONL); mirrors `vox check --output-format json`
        // diagnostic payloads. Emitted for success AND failure so agents always get one.
        println!(
            "{}",
            crate::pipeline::format_build_lane_envelope_json("build", file, &frontend, None)
        );
    } else {
        crate::pipeline::print_diagnostics(&frontend, file, false);
    }
    if frontend.has_errors() {
        anyhow::bail!(
            "Build failed with {} error(s) and {} warning(s)",
            frontend.error_count(),
            frontend.warning_count()
        );
    }
```

- [ ] Still in `build.rs::run()`, mechanically replace every remaining `println!(` with `crate::vox_note!(json, ` (closing parens unchanged). Sites: the four `"  wrote {}"` loops (Rust files ×2, TS files ×2), the SSG-shell `"  wrote {}"` loops (×2), the `emit_ir` `"  wrote {}"` prints (×4), `"  kept existing {}"`, `"  removed stale {}"` (×2), the three v0-component prints, the mobile-target Capacitor notice, and the four `"Build complete…"` summaries. Do NOT touch `eprintln!` sites or the `#[cfg(test)]` modules. Verify no stdout print remains in the function:
```
rg -n "println!" crates/vox-cli/src/commands/build.rs
```
Expected: zero bare `println!` lines inside `run()` (test modules have none).
- [ ] In `crates/vox-cli/src/commands/check.rs` line 47–49, switch to the shared helper (behavior identical):

```rust
    let json = args.output_format == "json"
        || args.for_llm
        || crate::pipeline::global_json_enabled();
```

- [ ] Run: `cargo check -p vox-cli` — expect clean compile; then `cargo test -p vox-cli --test build_lane_envelope --test check_diagnostics_json_golden` — expect PASS (check's JSON output path is unchanged).
- [ ] Smoke-verify from the repo root (fixture has known type errors; expect exactly one single-line JSON envelope with `"command":"build","ok":false` on stdout and a non-zero exit):
```
cargo run -p vox-cli -- --json build crates/vox-cli/tests/fixtures/golden_rust_import_lowering.vox
```
- [ ] Format and commit:
```
cargo fmt -p vox-cli
git add crates/vox-cli/src/diagnostics.rs crates/vox-cli/src/commands/build.rs crates/vox-cli/src/commands/check.rs
git commit -m "feat(cli): vox build emits build-lane JSON envelope under global --json"
```

---

### Task 6: `vox test` and `vox run --mode script` honor global `--json`

**Files:**
- Modify: `crates/vox-cli/src/commands/test.rs` (`run_once`)
- Modify: `crates/vox-cli/src/commands/runtime/run/script.rs` (`compile`, lines 253–266)

- [ ] In `crates/vox-cli/src/commands/test.rs::run_once`, make the two progress prints JSON-aware and emit a result envelope after `cargo test` (replace lines 28 and 44, and the status handling at lines 66–72):

```rust
    let json = crate::pipeline::global_json_enabled();
    crate::vox_note!(json, "Building for tests: {}...", file.display());
```
```rust
    crate::vox_note!(json, "Running tests in {}...", generated_dir.display());
```
```rust
    let status = cmd.status().context("Failed to execute cargo test")?;

    if json {
        // The preceding build::run already emitted the `"command":"build"`
        // envelope (with diagnostics) on stdout; this closes the JSONL stream.
        println!(
            "{}",
            crate::pipeline::format_command_result_envelope_json(
                "test",
                file,
                status.success(),
                status.code(),
            )
        );
    }

    if !status.success() {
        anyhow::bail!("Tests failed with exit code: {:?}", status.code());
    }

    Ok(())
```

- [ ] In `crates/vox-cli/src/commands/runtime/run/script.rs::compile`, honor JSON mode for frontend diagnostics (replace lines 253–266). On success the script's own stdout is the program output, so the envelope is emitted only on compile failure:

```rust
    let json = crate::pipeline::global_json_enabled();
    let result: crate::pipeline::FrontendResult =
        crate::pipeline::run_frontend_with_options(file, json, &pipeline_opts).await?;

    if !result.module.has_entrypoint() {
        anyhow::bail!(
            "No `fn main()` found in {}. Script files must contain a top-level main function.",
            file.display()
        );
    }

    if result.has_errors() {
        if json {
            println!(
                "{}",
                crate::pipeline::format_build_lane_envelope_json("run", file, &result, None)
            );
        } else {
            crate::pipeline::print_diagnostics(&result, file, false);
        }
        anyhow::bail!("Type checking failed");
    }
```

- [ ] Run: `cargo check -p vox-cli` — expect clean compile.
- [ ] Run: `cargo test -p vox-cli --test build_lane_envelope` — expect PASS (envelope shape unchanged).
- [ ] Format and commit:
```
cargo fmt -p vox-cli
git add crates/vox-cli/src/commands/test.rs crates/vox-cli/src/commands/runtime/run/script.rs
git commit -m "feat(cli): vox test and vox run script lane honor global --json"
```

---

### Task 7: `vox doctor --diag <id>` — flag plumbing

**Files:**
- Modify: `crates/vox-cli/src/cli_args.rs` (`DoctorArgs`, line 395)
- Modify: `crates/vox-cli/src/cli_dispatch/lanes.rs` (`run_doctor_command`, line 8)
- Modify: `crates/vox-cli/src/commands/diagnostics/doctor/mod.rs` (`run` signature + its two `#[cfg(test)]` call sites)

- [ ] Add the flag to `DoctorArgs` in `crates/vox-cli/src/cli_args.rs` (after the `tier` field, line 424):

```rust
    /// Run and report ONLY the build-health check that can produce this
    /// `[diag id=…]` (e.g. `sccache.pathological`, `docker.wsl_wedged`).
    /// Exits non-zero when that diagnosis fires. Unknown ids list the registry.
    #[arg(long, value_name = "ID")]
    pub diag: Option<String>,
```

- [ ] Extend `doctor::run` in `crates/vox-cli/src/commands/diagnostics/doctor/mod.rs` with a trailing parameter (keeps under the workspace `too-many-arguments-threshold = 12`):

```rust
pub async fn run(
    compile_target: Option<&str>,
    auto_heal: bool,
    test_health: bool,
    build_perf: bool,
    scope: bool,
    json: bool,
    probe: bool,
    fix_cuda_path: bool,
    tier: &str,
    diag: Option<&str>,
) -> Result<()> {
```
and for this task only, consume it inertly right after the signature so the crate compiles while Task 8 lands the behavior:
```rust
    let _ = diag; // wired in the --diag dispatch (next commit)
```

- [ ] Update the two existing tests at the bottom of `doctor/mod.rs` to pass the new argument (append `, None` — i.e. `run(None, false, false, true, false, false, false, false, "full", None)` in both `extended_doctor_flags_require_codex_build` and `build_perf_runs_when_codex_enabled`).
- [ ] Thread it in `crates/vox-cli/src/cli_dispatch/lanes.rs::run_doctor_command` (add as the last argument of the `commands::diagnostics::doctor::run(...)` call):

```rust
        args.diag.as_deref(),
```

- [ ] Run: `cargo check -p vox-cli` — expect clean compile; `cargo test -p vox-cli extended_doctor_flags_require_codex_build` — expect PASS.
- [ ] Format and commit:
```
cargo fmt -p vox-cli
git add crates/vox-cli/src/cli_args.rs crates/vox-cli/src/cli_dispatch/lanes.rs crates/vox-cli/src/commands/diagnostics/doctor/mod.rs
git commit -m "feat(cli): add --diag flag plumbing to vox doctor"
```

---

### Task 8: `--diag` dispatch — map diag ids to single checks, exit non-zero when fired

**Files:**
- Modify: `crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/build_health.rs`
- Modify: `crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/mod.rs`
- Modify: `crates/vox-cli/src/commands/diagnostics/doctor/mod.rs`
- Test: unit tests in `build_health.rs` tests module + `doctor/mod.rs` tests module

- [ ] Add the failing mapping test to the existing `#[cfg(test)] mod tests` in `checks_standard/build_health.rs`:

```rust
    #[test]
    fn every_known_diag_id_maps_to_a_check() {
        for id in KNOWN_DIAGNOSIS_IDS {
            assert!(
                check_kind_for_diag(id).is_some(),
                "diag id `{id}` has no --diag check mapping"
            );
        }
        assert_eq!(check_kind_for_diag("nope.unknown"), None);
    }
```

and the failing CLI-behavior test to the `#[cfg(test)] mod tests` in `doctor/mod.rs`:

```rust
    #[tokio::test]
    async fn diag_flag_rejects_unknown_id() {
        let err = run(
            None, false, false, false, false, false, false, false, "full",
            Some("bogus.id"),
        )
        .await
        .expect_err("unknown diag id should error");
        let s = err.to_string();
        assert!(s.contains("unknown diag id"), "unexpected message: {s}");
        assert!(
            s.contains("sccache.pathological"),
            "error should list the known-id registry: {s}"
        );
    }
```

- [ ] Run: `cargo test -p vox-cli every_known_diag_id_maps_to_a_check` — expect COMPILE FAILURE (`check_kind_for_diag` missing).
- [ ] Implement the pure mapping + executor in `build_health.rs` (after `parse_diag_id`, ~line 416):

```rust
/// Which check-runner covers a diagnosis id. Pure; the tests enforce that
/// every entry of [`KNOWN_DIAGNOSIS_IDS`] maps (add here when adding an id).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DiagCheckKind {
    Toolchain,
    CompileProbe,
    Docker,
    Sccache,
    Schema,
    Linker,
}

pub(crate) fn check_kind_for_diag(id: &str) -> Option<DiagCheckKind> {
    match id {
        "toolchain.rustc_shadowed" | "toolchain.rustup_shadowed" | "toolchain.rustc_absent"
        | "toolchain.rustup_absent" => Some(DiagCheckKind::Toolchain),
        "toolchain.compile_failed" | "toolchain.compile_timeout" => {
            Some(DiagCheckKind::CompileProbe)
        }
        "docker.wsl_wedged" | "docker.daemon_down" | "docker.absent" => Some(DiagCheckKind::Docker),
        "sccache.pathological" | "sccache.shadowed_shim" => Some(DiagCheckKind::Sccache),
        "vox.schema_drift" => Some(DiagCheckKind::Schema),
        "linker.lld_missing" => Some(DiagCheckKind::Linker),
        _ => None,
    }
}

/// Run only the check-set that can produce `kind`'s diagnoses.
pub(crate) async fn run_check_for_diag(kind: DiagCheckKind, checks: &mut Vec<Check>) {
    match kind {
        DiagCheckKind::Toolchain => toolchain_integrity(checks).await,
        DiagCheckKind::CompileProbe => compile_probe(checks).await,
        DiagCheckKind::Docker => docker_health(checks).await,
        DiagCheckKind::Sccache => sccache_guard(checks).await,
        DiagCheckKind::Schema => schema_health(checks).await,
        DiagCheckKind::Linker => linker_health(checks).await,
    }
}
```

- [ ] Expose it from `checks_standard/mod.rs` (append at the end of the file):

```rust
pub(crate) use build_health::parse_diag_id;

/// Stable diag-id registry, for `--diag` unknown-id error messages.
pub(crate) fn known_diag_ids() -> &'static [&'static str] {
    build_health::KNOWN_DIAGNOSIS_IDS
}

/// Run only the check-set covering `id`. Returns `false` for unregistered ids.
pub(crate) async fn run_diag_check(id: &str, checks: &mut Vec<Check>) -> bool {
    let Some(kind) = build_health::check_kind_for_diag(id) else {
        return false;
    };
    build_health::run_check_for_diag(kind, checks).await;
    true
}
```

- [ ] Wire the branch into `doctor/mod.rs::run` — replace the `let _ = diag;` placeholder from Task 7 with this block, placed immediately after the `fix_cuda_path` early-return (line 36) and before the `probe` combination checks:

```rust
    if let Some(id) = diag {
        if probe || build_perf || scope || test_health || auto_heal || compile_target.is_some() {
            anyhow::bail!(
                "`--diag` runs a single build-health check and cannot be combined with \
                 --probe, --build-perf, --scope, --test-health, --auto-heal, or --compile-target"
            );
        }
        let mut checks: Vec<common::Check> = Vec::new();
        if !checks_standard::run_diag_check(id, &mut checks).await {
            anyhow::bail!(
                "unknown diag id `{id}` — known ids:\n  {}",
                checks_standard::known_diag_ids().join("\n  ")
            );
        }
        output::print_results(&checks, false, json);
        let fired = checks
            .iter()
            .any(|c| !c.pass && checks_standard::parse_diag_id(&c.detail) == Some(id));
        if fired {
            anyhow::bail!("doctor: diagnosis `{id}` fired — apply the FIX above and re-run");
        }
        return Ok(());
    }
```

- [ ] Run: `cargo test -p vox-cli every_known_diag_id_maps_to_a_check diag_flag_rejects_unknown_id parses_diag_id_from_tag` — expect PASS.
- [ ] Smoke-verify (runs only the linker check on Windows / no-op pass elsewhere; expect a filtered report and exit 0 on a healthy machine):
```
cargo run -p vox-cli -- doctor --diag linker.lld_missing
```
- [ ] Format and commit:
```
cargo fmt -p vox-cli
git add crates/vox-cli/src/commands/diagnostics/doctor/
git commit -m "feat(cli): vox doctor --diag <id> runs a single registered check with exit semantics"
```

---

### Task 9: Changelog entries

**Files:**
- Modify: `CHANGELOG.md` (the `## [Unreleased]` section, line 14)

- [ ] Replace the `(Empty — v0.6.0 just tagged.  Next-release content lands here.)` placeholder under `## [Unreleased]` with:

```markdown
### Added

- **`@ai(structured_output = T)` now emits a real JSON-schema-constrained LLM call** — the generated `LlmConfig.response_format` carries `{"type":"json_schema","json_schema":{"name":…,"strict":true,"schema":{…}}}` derived from `T`'s typedef (scalars, `list[…]`, `Option[…]`, nested structs; closed objects with `required`), and `max_iterations = N` now emits a bounded re-prompt loop that retries when the reply is not valid JSON.
- **Uniform `--json` across the build lane** — `vox --json build`, `vox --json test`, and `vox --json run` (script lane) emit a stable single-line `BuildLaneEnvelope` (`envelope_version`, `command`, `ok`, `error_count`, `warning_count`, `diagnostics` using the same `VoxCompilerDiagnosticPayload` as `vox check`, optional `exit_code`) on stdout; human progress notes move to stderr so stdout stays JSONL-parseable.
- **`vox doctor --diag <id>`** — run and report only the build-health check that can produce a given `[diag id=…]` (e.g. `sccache.pathological`); exits non-zero when the diagnosis fires, and unknown ids list the registered-id registry.
```

- [ ] Run: `cargo check -p vox-cli` — expect clean (sanity that nothing else changed).
- [ ] Commit:
```
git add CHANGELOG.md
git commit -m "docs(changelog): @ai structured_output codegen, build-lane --json, doctor --diag"
```

---

### Critical Files for Implementation

- `crates/vox-codegen/src/codegen_rust/emit/ai_fixture/llm.rs`
- `crates/vox-codegen/src/codegen_rust/emit/workflow.rs`
- `crates/vox-cli/src/pipeline.rs`
- `crates/vox-cli/src/commands/build.rs`
- `crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/build_health.rs`
