//! Golden test for subscript / index expressions in JSX props and children.
//!
//! Validates the `Expr::Index` AST variant + HIR lowering: `items[0]`,
//! `items[i]`, `items[i + 1]` all reach the emitted TSX intact.
//!
//! Coverage of generic JSX prop forms (member access, conditionals, function
//! refs) is provided by main's `reactive_smoke_test` suite — only the subscript
//! path is exercised here, since it was added on this branch.

fn compile_components(src: &str) -> Vec<(String, String)> {
    let tokens = vox_compiler::lexer::lex(src);
    let module =
        vox_compiler::parser::parse(tokens).unwrap_or_else(|e| panic!("parse failed: {e:?}"));
    let hir = vox_compiler::hir::lower_module(&module);
    let out =
        vox_codegen::codegen_ts::generate(&hir).unwrap_or_else(|e| panic!("codegen failed: {e:?}"));
    out.files.into_iter().collect()
}

fn get_component<'a>(files: &'a [(String, String)], name: &str) -> &'a str {
    let filename = format!("{name}.tsx");
    files
        .iter()
        .find(|(n, _)| n == &filename)
        .map(|(_, c)| c.as_str())
        .unwrap_or_else(|| panic!("{filename} not found in codegen output"))
}

fn read_fixture(name: &str) -> String {
    let path = format!(
        "{}/tests/fixtures/props/{}",
        env!("CARGO_MANIFEST_DIR"),
        name
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn subscript_expressions_pass_through_to_tsx() {
    let src = read_fixture("subscript.vox");
    let files = compile_components(&src);
    let ts = get_component(&files, "Indexed");

    // Literal index
    assert!(
        ts.contains("items[0]"),
        "Indexed.tsx must emit literal `items[0]`. got:\n{ts}"
    );
    // Identifier index
    assert!(
        ts.contains("items[i]"),
        "Indexed.tsx must emit identifier `items[i]`. got:\n{ts}"
    );
    // Arithmetic index expression
    assert!(
        ts.contains("items[i + 1]") || ts.contains("items[(i + 1)]"),
        "Indexed.tsx must emit arithmetic `items[i + 1]`. got:\n{ts}"
    );
}
