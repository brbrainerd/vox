use vox_compiler::lexer::lex;
use vox_compiler::parser::parse;
use vox_compiler::typeck::{DiagnosticCategory, TypeckSeverity, typecheck_module};

fn diagnostics_for(source: &str) -> Vec<vox_compiler::typeck::Diagnostic> {
    let tokens = lex(source);
    let module = parse(tokens).unwrap_or_else(|errs| panic!("parse failed: {errs:?}"));
    typecheck_module(&module, source)
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn db_table_query_clause_is_lint_error() {
    let src = r#"
@table type User { name: str active: bool }
@query fn q() to int {
    db.User.query("active = 1")
    return 0
}
"#;
    let diags = diagnostics_for(src);
    assert!(diags.iter().any(|d| {
        d.severity == TypeckSeverity::Error
            && d.category == DiagnosticCategory::Lint
            && d.message.contains(".query(clause)")
    }));
}

#[test]
fn query_decl_rejects_insert_write_ops() {
    let src = r#"
@table type User { name: str active: bool }
@query fn q() to int {
    db.User.insert({ name: "a", active: true })
    return 0
}
"#;
    let diags = diagnostics_for(src);
    assert!(diags.iter().any(|d| {
        d.severity == TypeckSeverity::Error
            && d.category == DiagnosticCategory::Lint
            && d.message.contains("must be read-only")
    }));
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn db_chained_limit_no_longer_reports_unsupported_chain_error() {
    let src = r#"
@table type User { name: str active: bool }
fn q() to int {
    return len(db.User.filter({ active: true }).limit(5))
}
"#;
    let diags = diagnostics_for(src);
    assert!(!diags.iter().any(|d| {
        d.severity == TypeckSeverity::Error
            && d.message
                .contains("db query chaining via '.limit(...)' is not supported yet")
    }));
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn db_select_typechecks_when_non_optional_columns_included() {
    let src = r#"
@table type User { name: str active: bool }
fn q() to int {
    return len(db.User.all().select("name", "active"))
}
"#;
    let diags = diagnostics_for(src);
    assert!(
        !diags.iter().any(|d| d.severity == TypeckSeverity::Error),
        "unexpected errors: {diags:?}"
    );
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn db_select_allows_partial_projection_records() {
    let src = r#"
@table type User { name: str active: bool }
fn q() to int {
    return len(db.User.all().select("name"))
}
"#;
    let diags = diagnostics_for(src);
    assert!(
        !diags.iter().any(|d| d.severity == TypeckSeverity::Error),
        "unexpected errors: {diags:?}"
    );
}
