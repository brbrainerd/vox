//! `emit_mobile_setup` snapshot from [`ShellProjectionModule`].

use vox_codegen::codegen_ts::mobile_emit::emit_mobile_setup;
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::lex;
use vox_compiler::parser::parse;
use vox_compiler::shell_projection::project_shell_from_hir;

#[test]
fn shell_projection_round_trip_emits_mobile_ts_snapshot() {
    let src = r#"
@query fn handle_back() to bool { return true }
@query fn handle_link(url: str) to str { return "/" }
@mutation fn store_token(token: str) to str { return token }
@back_button { on_press: handle_back }
@deep_link { scheme: "vox" on_link: handle_link }
@push { on_register: store_token }
"#;
    let m = parse(lex(src)).expect("parse");
    let hir = lower_module(&m);
    let shell = project_shell_from_hir(&hir);
    let out = emit_mobile_setup(&shell).expect("mobile.ts");
    insta::assert_snapshot!("mobile_emit_from_shell_projection", out);
}
