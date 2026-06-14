use vox_codegen_ts::codegen_ts::emitter::{CodegenOutput, generate};
use vox_compiler::ast::span::Span;
use vox_compiler::hir::{HirConst, HirExpr, HirModule};

fn span() -> Span {
    Span { start: 0, end: 0 }
}

fn empty_module() -> HirModule {
    HirModule::default()
}

fn find_consts_ts(output: &CodegenOutput) -> Option<&str> {
    output
        .files
        .iter()
        .find(|(name, _)| name == "consts.ts")
        .map(|(_, content)| content.as_str())
}

#[test]
fn pub_const_emits_export_const() {
    let mut hir = empty_module();
    hir.consts.push(HirConst {
        name: "MAX".to_string(),
        value: HirExpr::IntLit(3, span()),
        type_ann: None,
        is_pub: true,
        is_deprecated: false,
        is_build_const: false,
        span: span(),
    });

    let output = generate(&hir).expect("codegen should succeed");
    let consts_ts = find_consts_ts(&output).expect("consts.ts should be emitted");

    assert!(
        consts_ts.contains("export const MAX = 3"),
        "expected 'export const MAX = 3' in consts.ts, got:\n{consts_ts}"
    );
}

#[test]
fn private_const_emits_without_export() {
    let mut hir = empty_module();
    hir.consts.push(HirConst {
        name: "LIMIT".to_string(),
        value: HirExpr::IntLit(10, span()),
        type_ann: None,
        is_pub: false,
        is_deprecated: false,
        is_build_const: false,
        span: span(),
    });

    let output = generate(&hir).expect("codegen should succeed");
    let consts_ts = find_consts_ts(&output).expect("consts.ts should be emitted");

    assert!(
        consts_ts.contains("const LIMIT = 10"),
        "expected 'const LIMIT = 10' in consts.ts, got:\n{consts_ts}"
    );
    assert!(
        !consts_ts.contains("export const LIMIT"),
        "private const should not be exported, got:\n{consts_ts}"
    );
}

#[test]
fn no_consts_produces_no_consts_ts() {
    let hir = empty_module();
    let output = generate(&hir).expect("codegen should succeed");
    assert!(
        find_consts_ts(&output).is_none(),
        "consts.ts should not be emitted when hir.consts is empty"
    );
}
