use vox_lexer::cursor::lex;
use vox_parser::parser::parse;
use vox_typeck::{typecheck_hir, env::TypeEnv, builtins::BuiltinTypes};
use vox_hir::lower_module;

#[test]
fn keyframes_and_theme_lowering() {
    let source = r##"
@keyframes fade_in:
    from:
        opacity: "0"
    to:
        opacity: "1"

@theme default:
    light:
        bg: "#ffffff"
        fg: "#000000"
    dark:
        bg: "#000000"
        fg: "#ffffff"
"##;

    let tokens = lex(source);
    let result = parse(tokens);
    if result.is_err() {
        panic!("Parse failed with errors: {:?}", result.unwrap_err());
    }
    let module = result.unwrap();
    let hir = lower_module(&module);
    let mut env = TypeEnv::new();
    let builtins = BuiltinTypes::register_all(&mut env);
    let _hir_diags = typecheck_hir(&hir, &mut env, &builtins, "");

    // Verify keyframes lowering
    assert_eq!(hir.keyframes.len(), 1, "Expected 1 keyframe");
    let kf = &hir.keyframes[0];
    assert_eq!(kf.name, "fade_in");
    assert_eq!(kf.steps.len(), 2, "Expected 2 keyframe steps");
    assert_eq!(kf.steps[0].selector, "from");
    assert_eq!(kf.steps[1].selector, "to");
    assert_eq!(kf.steps[0].properties, vec![("opacity".to_string(), "0".to_string())]);
    assert_eq!(kf.steps[1].properties, vec![("opacity".to_string(), "1".to_string())]);

    // Verify theme theme
    assert_eq!(hir.themes.len(), 1, "Expected 1 theme");
    let theme = &hir.themes[0];
    assert_eq!(theme.name, "default");
}
