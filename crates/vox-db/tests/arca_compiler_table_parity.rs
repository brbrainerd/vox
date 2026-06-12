use vox_codegen::codegen_rust::emit::tables::emit_table_ddl;
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;

fn normalize_sql(sql: &str) -> String {
    sql.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn arca_compiler_table_ddl_parity() {
    let src = r#"
@table(pk: externalKey) type UserProfile {
    externalKey: str
    displayName: str
    active: bool
}
"#;
    let tokens = lex(src);
    let module = parse(tokens).expect("parse");
    let ast_table = module
        .declarations
        .iter()
        .find_map(|decl| match decl {
            vox_compiler::ast::decl::Decl::Table(t) => Some(t),
            _ => None,
        })
        .expect("ast table");
    let hir = lower_module(&module);
    let hir_table = hir.tables.first().expect("hir table");

    let arca = vox_db::ddl::table_to_ddl(ast_table);
    let compiler = emit_table_ddl(hir_table);

    assert_eq!(normalize_sql(&arca), normalize_sql(&compiler));
}
