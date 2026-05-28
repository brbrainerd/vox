//! Determinism smoke for [`vox_compiler::shell_projection`].

use vox_compiler::hir::lower_module;
use vox_compiler::parser::parse;
use vox_compiler::shell_projection::{canonical_shell_projection_bytes, project_shell_from_hir};

fn lower_src(src: &str) -> vox_compiler::hir::TypedCoreIR_v2 {
    let tokens = vox_compiler::lexer::lex(src);
    let module = parse(tokens).expect("parse");
    lower_module(&module)
}

#[test]
fn shell_projection_canonical_bytes_are_deterministic() {
    let src = r#"
@query fn on_back() to bool { return true }
@back_button {
    on_press: on_back
}
"#;
    let hir = lower_src(src);
    let shell = project_shell_from_hir(&hir);
    let a = canonical_shell_projection_bytes(&shell).expect("a");
    let b = canonical_shell_projection_bytes(&shell).expect("b");
    assert_eq!(a, b);
    assert!(shell.back_button.is_some());
}
