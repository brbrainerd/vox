use vox_codegen::codegen_ts::{CodegenOptions, generate_with_options};
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

fn live_tsx() -> String {
    let hir = lower_module(&parse(lex(SRC)).expect("parse"));
    let out = generate_with_options(&hir, CodegenOptions::default()).expect("emit");
    out.files
        .iter()
        .find(|(name, _)| name == "Live.tsx")
        .map(|(_, body)| body.clone())
        .expect("Live.tsx emitted")
}

#[test]
fn on_stream_emits_subscribe_useeffect_without_tauri_import() {
    let tsx = live_tsx();
    assert!(
        tsx.contains("voxChannel.subscribe(\"orch_status\""),
        "expected a voxChannel.subscribe call; got:\n{tsx}"
    );
    assert!(
        tsx.contains("useEffect("),
        "subscription must be inside a useEffect"
    );
    // Auto-cleanup: the effect returns an unsubscribe.
    assert!(tsx.contains("return () =>"), "effect must return a cleanup");
    // Transport-neutral: the component never imports @tauri-apps directly.
    assert!(
        !tsx.contains("@tauri-apps"),
        "emitted component must not import @tauri-apps; got:\n{tsx}"
    );
    // The runtime is imported from the generated module.
    assert!(
        tsx.contains("vox-channel") || tsx.contains("./vox-channel"),
        "component must import the voxChannel runtime"
    );
}

#[test]
fn unknown_channel_emits_diagnostic_comment() {
    let src = r#"
component Bad() {
    state x: str = ""
    on stream(not_a_real_channel) as s: { x = s }
    view: text { x }
}
"#;
    let hir = lower_module(&parse(lex(src)).expect("parse"));
    let out = generate_with_options(&hir, CodegenOptions::default()).expect("emit");
    let tsx = out
        .files
        .iter()
        .find(|(n, _)| n == "Bad.tsx")
        .map(|(_, b)| b.clone())
        .expect("Bad.tsx");
    assert!(
        tsx.contains("vox/web/unknown-channel"),
        "unknown channel must produce a diagnostic comment; got:\n{tsx}"
    );
    assert!(
        !tsx.contains("voxChannel.subscribe(\"not_a_real_channel\""),
        "unknown channel must not emit a subscribe call"
    );
}
