use vox_codegen_rust::emit::emit_lib;
use vox_codegen_ts::generate as encode_ts;
use vox_typeck::{typecheck_hir, env::TypeEnv, builtins::BuiltinTypes};
use vox_hir::lower_module;
use vox_lexer::cursor::lex;
use vox_parser::parser::parse;
use vox_typeck::typecheck_module;

#[test]
fn test_trait_and_impl_pipeline() {
    let source = r#"
trait Speak:
    fn say_hello(name: str) to str

type Dog =
    | DogStruct(name: str)

impl Speak for Dog:
    fn say_hello(name: str) to str:
        ret "Woof " + name
"#;

    // 1. Lex
    let tokens = lex(source);
    assert!(!tokens.is_empty());

    // 2. Parse
    let ast = parse(tokens).expect("Failed to parse module");

    // 3. Lower to HIR
    let hir = lower_module(&ast);
    let mut env = TypeEnv::new();
    let builtins = BuiltinTypes::register_all(&mut env);
    let _hir_diags = typecheck_hir(&hir, &mut env, &builtins, "");
    assert_eq!(hir.traits.len(), 1);
    assert_eq!(hir.impls.len(), 1);

    // 4. Type Check
    let diagnostics = typecheck_module(&ast, source);
    let errors: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.severity == vox_typeck::diagnostics::Severity::Error)
        .collect();
    assert!(
        errors.is_empty(),
        "Should have no type errors: {:?}",
        errors
    );

    // 5. Rust Codegen
    let rust_code = emit_lib(&hir);
    assert!(rust_code.contains("pub trait Speak {"));
    assert!(rust_code.contains("impl Speak for Dog {"));

    // 6. TS Codegen
    let ts_output = encode_ts(&ast).expect("TS codegen failed");
    let ts_types = ts_output
        .files
        .iter()
        .find(|(name, _)| name == "types.d.ts")
        .unwrap()
        .1
        .clone();
    assert!(ts_types.contains("export interface Speak {"));
    assert!(ts_types.contains("say_hello(name: string): string;"));
}
