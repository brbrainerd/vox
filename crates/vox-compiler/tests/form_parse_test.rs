use vox_compiler::ast::decl::Decl;
use vox_compiler::{lexer::cursor::lex, parser::parse};

#[test]
fn form_with_basic_fields_parses() {
    let src = r#"
form Mood {
    field score: int range(1..10) required label("Mood")
    field note: str max_len(280) optional
    on_submit: save_mood
    success_redirect: "/timeline"
}
"#;
    let m = parse(lex(src)).expect("parse");
    let form = m
        .declarations
        .iter()
        .find_map(|d| match d {
            Decl::Form(f) => Some(f),
            _ => None,
        })
        .expect("form decl");
    assert_eq!(form.name, "Mood");
    assert_eq!(form.fields.len(), 2);
    assert_eq!(form.fields[0].name, "score");
    assert!(form.fields[0].required);
    assert_eq!(form.fields[0].label.as_deref(), Some("Mood"));
    assert_eq!(form.on_submit.as_deref(), Some("save_mood"));
    assert_eq!(form.success_redirect.as_deref(), Some("/timeline"));
}

#[test]
fn form_with_hidden_default_field_parses() {
    let src = r#"
form X {
    field at: int default(0) hidden
    on_submit: save
}
"#;
    let m = parse(lex(src)).expect("parse");
    let form = m
        .declarations
        .iter()
        .find_map(|d| match d {
            Decl::Form(f) => Some(f),
            _ => None,
        })
        .expect("form decl");
    assert!(form.fields[0].hidden);
    assert!(form.fields[0].default.is_some());
}
