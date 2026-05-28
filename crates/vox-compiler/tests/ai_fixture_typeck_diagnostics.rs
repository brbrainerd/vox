//! Typecheck diagnostics for AI-first fixtures (`vox/ai/*`, `vox/prompt/*`, `vox/subagent/*`, `vox/search/*`).

use vox_compiler::hir::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;
use vox_compiler::typeck::typecheck_hir_module;

fn collect_codes(src: &str) -> Vec<String> {
    let module = parse(lex(src)).expect("parse fixture");
    let mut hir = lower_module(&module);
    typecheck_hir_module(src, &mut hir)
        .into_iter()
        .filter_map(|d| d.code)
        .collect()
}

fn assert_contains_code(src: &str, expected: &str) {
    let codes = collect_codes(src);
    assert!(
        codes.iter().any(|c| c == expected),
        "expected diagnostic `{expected}` in {codes:?}\nsource:\n{src}"
    );
}

fn assert_missing_code(src: &str, forbidden: &str) {
    let codes = collect_codes(src);
    assert!(
        !codes.iter().any(|c| c == forbidden),
        "did not expect `{forbidden}` in {codes:?}\nsource:\n{src}"
    );
}

#[test]
fn unknown_task_category_is_error() {
    let src = r#"
        @ai(task_category = FancyLabel)
        @uses(net)
        fn x() to str { return "" }
    "#;
    assert_contains_code(src, "vox/ai/unknown-task-category");
}

#[test]
fn known_task_category_clean() {
    let src = r#"
        @ai(task_category = CodeGen)
        @uses(net)
        fn x() to str { return "" }
    "#;
    assert_missing_code(src, "vox/ai/unknown-task-category");
}

#[test]
fn invalid_prompt_stage_is_error() {
    let src = r#"
        @prompt(stage = Draft, schema = Blob, redact = [])
        @uses(net)
        fn x() to str { return "" }
    "#;
    assert_contains_code(src, "vox/prompt/invalid-stage");
}

#[test]
fn secret_shaped_redact_warns_without_env_capability() {
    let src = r#"
        @prompt(stage = Planner, schema = Blob, redact = [OPENROUTER_API_KEY])
        @uses(net)
        fn x() to str { return "" }
    "#;
    assert_contains_code(src, "vox/prompt/secret-leakage");
}

#[test]
fn secret_shaped_redact_ok_with_env() {
    let src = r#"
        @prompt(stage = Planner, schema = Blob, redact = [OPENROUTER_API_KEY])
        @uses(net, env)
        fn x() to str { return "" }
    "#;
    assert_missing_code(src, "vox/prompt/secret-leakage");
}

#[test]
fn subagent_chain_depth_error() {
    let src = r#"
        @subagent(policy = parallel, max_depth = 7)
        @uses(net, spawn)
        fn x() to str { return "" }
    "#;
    assert_contains_code(src, "vox/subagent/chain-depth-exceeded");
}

#[test]
fn distributed_policy_warns() {
    let src = r#"
        @subagent(policy = distributed, max_depth = 1)
        @uses(net, spawn)
        fn x() to str { return "" }
    "#;
    assert_contains_code(src, "vox/subagent/distributed-not-wired");
}

#[test]
fn search_corpus_denied() {
    let src = r#"
        @search(corpus = wiki, query = "x", into = str)
        fn x() to str { return "" }
    "#;
    assert_contains_code(src, "vox/search/corpus-denied");
}

#[test]
fn search_memory_key_invalid() {
    let src = r#"
        @search(corpus = memory, query = "tooshort", into = str)
        fn x() to str { return "" }
    "#;
    assert_contains_code(src, "vox/search/memory-key-invalid");
}

#[test]
fn search_web_requires_net() {
    let src = r#"
        @search(corpus = web, query = "wasm spec", into = str)
        fn x() to str { return "" }
    "#;
    assert_contains_code(src, "vox/search/web-policy-denied");
}

#[test]
fn search_web_ok_with_net() {
    let src = r#"
        @search(corpus = web, query = "wasm spec", into = str)
        @uses(net)
        fn x() to str { return "" }
    "#;
    assert_missing_code(src, "vox/search/web-policy-denied");
}
