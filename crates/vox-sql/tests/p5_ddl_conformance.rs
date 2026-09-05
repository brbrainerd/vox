// Requires `postgres` and `mysql`: this test asserts cross-dialect DDL
// conformance using `BackendKind::Postgres`/`BackendKind::MySql`, which
// vox-sql cfg-gates behind those features independently of `runtime` (see
// crates/vox-sql/Cargo.toml and THE TRAP note on `BackendKind`).
#![cfg(all(feature = "postgres", feature = "mysql"))]

use vox_ast::decl::{IndexDecl, TableDecl, TableField};
use vox_ast::span::Span;
use vox_ast::types::TypeExpr;
use vox_sql::BackendKind;
use vox_sql::ddl::{add_column_ddl, index_to_ddl, table_to_ddl};
use vox_sql::type_map::UnsupportedTypePolicy;

fn sp() -> Span {
    Span { start: 0, end: 0 }
}

fn named(name: &str) -> TypeExpr {
    TypeExpr::Named {
        name: name.to_string(),
        span: sp(),
    }
}

fn option(inner: &str) -> TypeExpr {
    TypeExpr::Generic {
        name: "Option".to_string(),
        args: vec![named(inner)],
        span: sp(),
    }
}

fn task_table() -> TableDecl {
    TableDecl {
        name: "Task".to_string(),
        fields: vec![
            TableField {
                name: "title".to_string(),
                type_ann: named("str"),
                description: None,
                span: sp(),
            },
            TableField {
                name: "done".to_string(),
                type_ann: option("bool"),
                description: None,
                span: sp(),
            },
        ],
        description: None,
        json_layout: None,
        auth_provider: None,
        roles: vec![],
        cors: None,
        is_pub: false,
        is_deprecated: false,
        primary_key: None,
        is_extern: false,
        source: None,
        span: sp(),
    }
}

#[test]
fn p5_table_ddl_conformance_across_backends() {
    let t = task_table();
    let sqlite =
        table_to_ddl(BackendKind::Libsql, &t, UnsupportedTypePolicy::Reject).expect("sqlite ddl");
    assert!(sqlite.contains("_id INTEGER PRIMARY KEY AUTOINCREMENT"));
    assert!(sqlite.contains("title TEXT NOT NULL"));

    let pg =
        table_to_ddl(BackendKind::Postgres, &t, UnsupportedTypePolicy::Reject).expect("pg ddl");
    assert!(pg.contains("_id BIGSERIAL PRIMARY KEY"));
    assert!(pg.contains("done BOOLEAN"));

    let my = table_to_ddl(BackendKind::MySql, &t, UnsupportedTypePolicy::Reject).expect("my ddl");
    assert!(my.contains("_id BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY"));
    assert!(my.contains("done TINYINT(1)"));
}

#[test]
fn p5_add_column_ddl_conformance_across_backends() {
    let sqlite = add_column_ddl(
        BackendKind::Libsql,
        "task",
        "priority",
        &named("int"),
        UnsupportedTypePolicy::Reject,
    )
    .expect("sqlite add col");
    assert!(sqlite.contains("ADD COLUMN priority INTEGER NOT NULL"));

    let pg = add_column_ddl(
        BackendKind::Postgres,
        "task",
        "priority",
        &named("int"),
        UnsupportedTypePolicy::Reject,
    )
    .expect("pg add col");
    assert!(pg.contains("ADD COLUMN priority BIGINT NOT NULL"));
}

#[test]
fn p5_index_ddl_is_stable() {
    let idx = IndexDecl {
        table_name: "Task".to_string(),
        index_name: "by_title".to_string(),
        columns: vec!["title".to_string()],
        span: sp(),
    };
    let ddl = index_to_ddl(BackendKind::Postgres, &idx);
    assert_eq!(
        ddl,
        "CREATE INDEX IF NOT EXISTS idx_task_by_title ON task (title);"
    );
}
