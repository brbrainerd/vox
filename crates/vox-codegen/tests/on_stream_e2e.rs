use vox_codegen::codegen_ts::{CodegenOptions, generate_with_options};
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;

// A Dashboard-like surface: a live orchestrator status panel expressed entirely in
// .vox via `on stream`. Proves the blocked:reactive-streams surface is now expressible.
const SRC: &str = r#"
component LiveDashboard() {
    state agents: str = "0"
    on stream(orch_status) as s: { agents = s }
    view: text { agents }
}
"#;

#[test]
fn live_dashboard_emits_component_and_channel_runtime() {
    let hir = lower_module(&parse(lex(SRC)).expect("parse"));
    let out = generate_with_options(&hir, CodegenOptions::default()).expect("emit");

    let comp = out
        .files
        .iter()
        .find(|(n, _)| n == "LiveDashboard.tsx")
        .expect("LiveDashboard.tsx component");
    assert!(
        comp.1.contains("voxChannel.subscribe(\"orch_status\")")
            || comp.1.contains("voxChannel.subscribe(\"orch_status\","),
        "component must subscribe to orch_status; got:\n{}",
        comp.1
    );
    assert!(
        !comp.1.contains("@tauri-apps"),
        "component must not import @tauri-apps"
    );

    // The channel runtime is emitted alongside (because a StreamSub exists).
    let runtime = out.files.iter().find(|(n, _)| n == "vox-channel.ts");
    assert!(
        runtime.is_some(),
        "vox-channel.ts must be emitted when a stream is used"
    );
    let rt = &runtime.unwrap().1;
    assert!(
        rt.contains("__TAURI_INTERNALS__"),
        "runtime must guard the Tauri transport"
    );
    for line in rt.lines() {
        let l = line.trim_start();
        if l.starts_with("import ") {
            assert!(
                !l.contains("@tauri-apps"),
                "runtime has a static tauri import: {line}"
            );
        }
    }
}
