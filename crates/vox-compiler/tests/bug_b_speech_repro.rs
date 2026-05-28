//! Bug B repro: `Speech.transcribe_microphone()` lowers to `mobile.transcribe_microphone()`
//! per docs/superpowers/plans/language/2026-05-08-codegen-ts-bugs-blocking-tracker.md.

const FIXTURE: &str = r#"
import react.use_state

component VoicePage() {
    state transcript_raw: str = ""
    view: column() {
        button(on_click={transcript_raw = Speech.transcribe_microphone()}) { "rec" }
        text() { transcript_raw }
    }
}
"#;

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn speech_transcribe_microphone_emits_speech_namespace() {
    let tokens = vox_compiler::lexer::lex(FIXTURE);
    let module = vox_compiler::parser::parse(tokens).expect("parse");
    let _diags = vox_compiler::typeck::typecheck_module(&module, "bug_b_speech");
    let hir = vox_compiler::hir::lower_module(&module);
    let out = vox_compiler::codegen_ts::generate(&hir).expect("gen");
    for (name, body) in &out.files {
        if name.contains("VoicePage") {
            eprintln!("=== {name} ===\n{body}");
        }
    }
    let voice_body = out
        .files
        .iter()
        .find(|(n, _)| n.contains("VoicePage"))
        .map(|(_, b)| b.as_str())
        .unwrap_or("");
    assert!(
        !voice_body.contains("mobile.transcribe_microphone"),
        "Bug B: emit lowers Speech to mobile.\nbody:\n{voice_body}"
    );
}
