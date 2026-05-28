use vox_codegen::codegen_ts::emitter::generate;
use vox_compiler::{hir::lower_module, lexer::cursor::lex, parser::parse};

fn emit(src: &str) -> String {
    let m = parse(lex(src)).expect("parse");
    let hir = lower_module(&m);
    generate(&hir)
        .expect("emit")
        .files
        .iter()
        .map(|(name, content)| format!("--- {name}\n{content}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn form_emits_react_component_with_inputs_and_labels() {
    let src = r#"
@mutation fn save_mood(score: int, note: str) to int { return 1 }
@form Mood {
    field score: int range(1..10) required label("How are you feeling?")
    field note: str max_len(280) optional label("Anything to share?")
    on_submit: save_mood
    success_redirect: "/timeline"
}
"#;
    let ts = emit(src);
    assert!(
        ts.contains("export function Mood("),
        "must export Mood component, got:\n{ts}"
    );
    assert!(
        ts.contains("How are you feeling?"),
        "must include label, got:\n{ts}"
    );
    assert!(
        ts.contains("type=\"number\""),
        "score is int → number input, got:\n{ts}"
    );
    assert!(
        ts.contains("await save_mood("),
        "must await endpoint call, got:\n{ts}"
    );
    assert!(
        ts.contains("navigate("),
        "must trigger redirect, got:\n{ts}"
    );
}

#[test]
fn form_validates_required_field_before_submit() {
    let src = r#"
@mutation fn save(s: int) to int { return 1 }
@form F {
    field s: int required
    on_submit: save
}
"#;
    let ts = emit(src);
    assert!(
        ts.contains("s === undefined") || ts.contains("s === null") || ts.contains("!s"),
        "must check required field, got:\n{ts}"
    );
}

#[test]
fn form_error_message_appears_in_emitted_banner() {
    let src = r#"
@mutation fn save(s: int) to int { return 1 }
@form F {
    field s: int required
    on_submit: save
    error_message: "Could not save."
}
"#;
    let ts = emit(src);
    assert!(
        ts.contains("Could not save."),
        "declared error_message must appear in emitted component, got:\n{ts}"
    );
}
