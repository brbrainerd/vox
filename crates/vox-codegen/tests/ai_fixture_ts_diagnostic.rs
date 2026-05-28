//! AI fixtures + TypeScript codegen: diagnostic emission and strict gate.

use vox_codegen::codegen_ts::{CodegenOptions, generate_with_options};
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;

#[test]
fn ts_emit_records_missing_ai_lowering_diagnostic() {
    let src = r#"
        @prompt(stage = Planner, schema = Blob, redact = [])
        @uses(net)
        fn demo() to str { return "" }
    "#;
    let module = parse(lex(src)).expect("parse");
    let hir = lower_module(&module);
    let out = generate_with_options(
        &hir,
        CodegenOptions {
            strict_ai: false,
            ..Default::default()
        },
    )
    .expect("codegen");
    assert!(
        out.diagnostics
            .iter()
            .any(|d| d.code == "vox/codegen/missing-ts-ai-lowering"),
        "expected TS AI lowering diagnostic, got {:?}",
        out.diagnostics
    );
}

#[test]
fn ts_strict_ai_errors_when_fixtures_present() {
    let src = r#"
        @ai(task_category = CodeGen)
        @uses(net)
        fn demo() to str { return "" }
    "#;
    let module = parse(lex(src)).expect("parse");
    let hir = lower_module(&module);
    let err = match generate_with_options(
        &hir,
        CodegenOptions {
            strict_ai: true,
            ..Default::default()
        },
    ) {
        Err(e) => e,
        Ok(_) => panic!("strict AI should fail codegen"),
    };
    assert!(
        err.contains("vox/codegen/missing-ts-ai-lowering"),
        "unexpected error: {err}"
    );
}
