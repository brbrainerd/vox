//! `generate_voxdb_schema` (AST) vs `generate_voxdb_schema_from_hir` must stay identical for DB declarations.

use vox_codegen::codegen_ts::{generate_voxdb_schema, generate_voxdb_schema_from_hir};
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;

const VOXDB_FIXTURE: &str = r#"
@table type User {
  name: str
  score: int
}

@index User.user_name on (name)
"#;

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn voxdb_schema_ast_matches_hir() {
    let tokens = lex(VOXDB_FIXTURE);
    let module = parse(tokens).expect("parse");
    let hir = lower_module(&module);
    let from_ast = generate_voxdb_schema(&module);
    let from_hir = generate_voxdb_schema_from_hir(&hir);
    assert_eq!(
        from_ast, from_hir,
        "AST and HIR VoxDB schema emission diverged — update lowering or schema helpers"
    );
}
