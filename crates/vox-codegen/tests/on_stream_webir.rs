use vox_codegen::web_ir::{BehaviorNode, lower::lower_hir_to_web_ir};
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
fn on_stream_lowers_to_streamsub_behavior() {
    let hir = lower_module(&parse(lex(SRC)).expect("parse"));
    let web = lower_hir_to_web_ir(&hir);
    let found = web.behavior_nodes.iter().any(|b| {
        matches!(
            b, BehaviorNode::StreamSub { channel, binding, .. }
                if channel == "orch_status" && binding == "s"
        )
    });
    assert!(found, "expected a BehaviorNode::StreamSub for orch_status");
}
