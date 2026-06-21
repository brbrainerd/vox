use vox_compiler::hir::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;

const SRC: &str = r#"
component Live() {
    state status: str = ""
    on stream(orch_status) as s: { status = s }
    view: text { status }
}
"#;

#[test]
fn on_stream_lowers_to_hir_member() {
    let hir = lower_module(&parse(lex(SRC)).expect("parse"));
    let rc = hir
        .components
        .iter()
        .find(|c| c.name == "Live")
        .expect("Live");
    let found = rc.members.iter().any(|m| {
        matches!(
            m, vox_compiler::hir::HirReactiveMember::OnStream(s)
                if s.channel == "orch_status" && s.binding == "s"
        )
    });
    assert!(found, "expected lowered HirReactiveMember::OnStream");
}
