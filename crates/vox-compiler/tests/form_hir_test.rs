use vox_compiler::{
    hir::lower::lower_module, lexer::cursor::lex, parser::parse, typeck::typecheck_module,
};

#[test]
fn form_lowered_with_correct_field_count() {
    let src = r#"
@mutation fn save_x(s: int, n: str) to int { return 1 }
@form X {
    field s: int required
    field n: str optional
    on_submit: save_x
}
"#;
    let m = parse(lex(src)).expect("parse");
    let hir = lower_module(&m);
    assert_eq!(hir.forms.len(), 1);
    assert_eq!(hir.forms[0].name, "X");
    assert_eq!(hir.forms[0].fields.len(), 2);
}

#[test]
fn form_with_field_type_mismatch_errors() {
    let src = r#"
@mutation fn save_x(s: str) to int { return 1 }
@form X {
    field s: int required
    on_submit: save_x
}
"#;
    let m = parse(lex(src)).expect("parse");
    let ds = typecheck_module(&m, src);
    let hit = ds
        .iter()
        .find(|d| d.code.as_deref() == Some("lint.form.field_type_mismatch"));
    assert!(
        hit.is_some(),
        "Expected lint.form.field_type_mismatch, got: {:?}",
        ds
    );
}

#[test]
fn form_with_unknown_endpoint_errors() {
    let src = r#"
@form X {
    field s: int required
    on_submit: nonexistent
}
"#;
    let m = parse(lex(src)).expect("parse");
    let ds = typecheck_module(&m, src);
    let hit = ds
        .iter()
        .find(|d| d.code.as_deref() == Some("lint.form.unknown_endpoint"));
    assert!(
        hit.is_some(),
        "Expected lint.form.unknown_endpoint, got: {:?}",
        ds
    );
}
