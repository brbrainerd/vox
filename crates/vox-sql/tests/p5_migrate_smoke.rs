// Requires the `runtime` feature (default-on): this test exercises live
// LibsqlBackend connections and the migrate module, which vox-sql's
// `runtime` feature gate (see crates/vox-sql/Cargo.toml) makes optional.
// `--no-default-features` no-ops this file rather than failing to compile.
#![cfg(feature = "runtime")]

use vox_ast::decl::{TableDecl, TableField};
use vox_ast::span::Span;
use vox_ast::types::TypeExpr;
use vox_db::DbConfig;
use vox_sql::migrate::AppAutoMigrator;
use vox_sql::type_map::UnsupportedTypePolicy;
use vox_sql::{AnySqlBackend, LibsqlBackend};

fn sp() -> Span {
    Span { start: 0, end: 0 }
}

fn named(name: &str) -> TypeExpr {
    TypeExpr::Named {
        name: name.to_string(),
        span: sp(),
    }
}

fn task_table() -> TableDecl {
    TableDecl {
        name: "Task".to_string(),
        fields: vec![TableField {
            name: "title".to_string(),
            type_ann: named("str"),
            description: None,
            span: sp(),
        }],
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

#[tokio::test]
async fn p5_app_migrator_libsql_smoke() {
    let libsql = LibsqlBackend::connect_from_config(DbConfig::Memory)
        .await
        .expect("connect in-memory");
    let backend = AnySqlBackend::Libsql(libsql);
    let migrator = AppAutoMigrator::new(&backend);

    let table = task_table();
    let tables = vec![&table];
    let collections = vec![];
    let indexes = vec![];

    let plan = migrator
        .plan(
            &tables,
            &collections,
            &indexes,
            UnsupportedTypePolicy::JsonText,
        )
        .await
        .expect("plan");
    assert!(
        !plan.is_empty(),
        "expected create-table action on empty schema"
    );

    let applied = migrator.apply(&plan).await.expect("apply");
    assert!(applied > 0, "expected at least one DDL action");

    let plan2 = migrator
        .plan(
            &tables,
            &collections,
            &indexes,
            UnsupportedTypePolicy::JsonText,
        )
        .await
        .expect("re-plan");
    assert!(
        plan2.auto_actions().is_empty(),
        "schema should converge with no further auto actions"
    );
}
