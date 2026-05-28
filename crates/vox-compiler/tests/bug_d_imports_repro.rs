//! Bug D repro: emitted component files reference @endpoint fns and `std.*` builtins
//! as bare identifiers without import statements,
//! per docs/superpowers/plans/language/2026-05-08-codegen-ts-bugs-blocking-tracker.md.

const FIXTURE: &str = r#"
import react.use_state

@query
fn parse_voice(s: str) -> str {
    s
}

@mutation
fn record_event(name: str, payload: str) -> str {
    name
}

component VoicePage() {
    state transcript_raw: str = ""
    view: column() {
        button(on_click={
            let p = parse_voice(transcript_raw)
            let _ = record_event("ev", p)
            let _ms = std.time.now_ms()
        }) { "go" }
        text() { transcript_raw }
    }
}
"#;

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn endpoint_calls_emit_imports() {
    let tokens = vox_compiler::lexer::lex(FIXTURE);
    let module = vox_compiler::parser::parse(tokens).expect("parse");
    let _diags = vox_compiler::typeck::typecheck_module(&module, "bug_d_imports");
    let hir = vox_compiler::hir::lower_module(&module);
    let out = vox_compiler::codegen_ts::generate(&hir).expect("gen");
    let voice = out
        .files
        .iter()
        .find(|(n, _)| n.contains("VoicePage"))
        .map(|(_, b)| b.as_str())
        .expect("VoicePage emitted");
    eprintln!("=== VoicePage ===\n{voice}");
    assert!(
        voice.contains("parse_voice"),
        "fixture should reference parse_voice"
    );
    // Either the endpoint fn is imported or it's a generated local helper.
    let imports_endpoint = voice.contains("import { parse_voice")
        || voice.contains("from \"./vox-client\"")
        || voice.contains("from \"./endpoints\"");
    assert!(
        imports_endpoint,
        "Bug D: VoicePage references @endpoint fns (parse_voice, record_event) but emits no import. body:\n{voice}"
    );
    let std_handled = !voice.contains("std.time.now_ms()") || voice.contains("Date.now()");
    assert!(
        std_handled,
        "Bug D: std.time.now_ms() must be replaced or imported. body:\n{voice}"
    );
}
