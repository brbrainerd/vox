use vox_lexer::cursor::lex;
use vox_parser::parser::parse;
use vox_typeck::{typecheck_hir, env::TypeEnv, builtins::BuiltinTypes};
use vox_hir::lower_module;
use vox_typeck::typecheck_module;

#[test]
fn multimodal_image_gen_pipeline() {
    let source = "
activity generate_banner(prompt: str) to Result[str]:
    let result = generate_image(prompt, Some(\"1024x1024\"))
    ret result

workflow handle_branding(description: str) to Unit:
    let banner_url = generate_banner(description)
    match banner_url:
        Ok(url) -> print(\"Banner generated: \" + url)
        Error(e) -> print(\"Failed: \" + e)
";

    let tokens = lex(source);
    let module = parse(tokens).expect("Parse failed");
    let hir = lower_module(&module);
    let mut env = TypeEnv::new();
    let builtins = BuiltinTypes::register_all(&mut env);
    let _hir_diags = typecheck_hir(&hir, &mut env, &builtins, "");

    // Verify HIR lowering of activities and workflows
    assert_eq!(hir.activities.len(), 1);
    assert_eq!(hir.workflows.len(), 1);
    assert_eq!(hir.activities[0].name, "generate_banner");

    // Verify type checking (detects generate_image builtin)
    let diagnostics = typecheck_module(&module, source);
    assert!(diagnostics.is_empty(), "Typecheck failed: {:?}", diagnostics);
}
