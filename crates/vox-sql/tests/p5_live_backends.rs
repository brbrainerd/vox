// Requires `runtime` (default-on) plus `postgres` and `mysql`: this test
// exercises live SqlBackend connections and the migrate module for all
// three backends, and vox-sql cfg-gates the Postgres/MySql backends behind
// those features independently of `runtime` (see crates/vox-sql/Cargo.toml
// and THE TRAP note on `BackendKind`). Missing any of the three no-ops this
// file rather than failing to compile.
#![cfg(all(feature = "runtime", feature = "postgres", feature = "mysql"))]

use std::env;

use vox_ast::decl::{TableDecl, TableField};
use vox_ast::span::Span;
use vox_ast::types::TypeExpr;
use vox_sql::migrate::{AppAutoMigrator, MigrationAction};
use vox_sql::type_map::UnsupportedTypePolicy;
use vox_sql::{AnySqlBackend, MySqlBackend, PostgresBackend, SqlBackend};

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

fn task_table_v1() -> TableDecl {
    TableDecl {
        name: "P5LiveMigrateTask".to_string(),
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

fn task_table_v2() -> TableDecl {
    TableDecl {
        name: "P5LiveMigrateTask".to_string(),
        fields: vec![
            TableField {
                name: "title".to_string(),
                type_ann: named("str"),
                description: None,
                span: sp(),
            },
            // Optional keeps add-column migrations robust across backends with existing rows.
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

async fn assert_migrate_apply_converges(backend: &AnySqlBackend, label: &str) {
    backend
        .execute("DROP TABLE IF EXISTS p5_live_migrate_task", &[])
        .await
        .unwrap_or_else(|e| panic!("{label}: cleanup drop failed: {e}"));

    let migrator = AppAutoMigrator::new(backend);

    let v1 = task_table_v1();
    let v1_tables = vec![&v1];
    let collections = vec![];
    let indexes = vec![];

    let plan_v1 = migrator
        .plan(
            &v1_tables,
            &collections,
            &indexes,
            UnsupportedTypePolicy::JsonText,
        )
        .await
        .unwrap_or_else(|e| panic!("{label}: v1 plan failed: {e}"));
    assert!(
        !plan_v1.is_empty(),
        "{label}: expected create-table action on empty schema"
    );
    let applied_v1 = migrator
        .apply(&plan_v1)
        .await
        .unwrap_or_else(|e| panic!("{label}: v1 apply failed: {e}"));
    assert!(
        applied_v1 > 0,
        "{label}: expected at least one v1 DDL action"
    );

    let replan_v1 = migrator
        .plan(
            &v1_tables,
            &collections,
            &indexes,
            UnsupportedTypePolicy::JsonText,
        )
        .await
        .unwrap_or_else(|e| panic!("{label}: v1 re-plan failed: {e}"));
    assert!(
        replan_v1.auto_actions().is_empty(),
        "{label}: schema should converge after first apply"
    );

    let v2 = task_table_v2();
    let v2_tables = vec![&v2];
    let plan_v2 = migrator
        .plan(
            &v2_tables,
            &collections,
            &indexes,
            UnsupportedTypePolicy::JsonText,
        )
        .await
        .unwrap_or_else(|e| panic!("{label}: v2 plan failed: {e}"));
    assert!(
        plan_v2
            .actions
            .iter()
            .any(|a| matches!(a, MigrationAction::AddColumn { column, .. } if column == "done")),
        "{label}: expected add-column action when schema evolves"
    );
    let applied_v2 = migrator
        .apply(&plan_v2)
        .await
        .unwrap_or_else(|e| panic!("{label}: v2 apply failed: {e}"));
    assert!(
        applied_v2 > 0,
        "{label}: expected at least one v2 DDL action"
    );

    let replan_v2 = migrator
        .plan(
            &v2_tables,
            &collections,
            &indexes,
            UnsupportedTypePolicy::JsonText,
        )
        .await
        .unwrap_or_else(|e| panic!("{label}: v2 re-plan failed: {e}"));
    assert!(
        replan_v2.auto_actions().is_empty(),
        "{label}: schema should converge after add-column apply"
    );

    backend
        .execute("DROP TABLE IF EXISTS p5_live_migrate_task", &[])
        .await
        .unwrap_or_else(|e| panic!("{label}: final cleanup drop failed: {e}"));
}

#[tokio::test]
async fn p5_live_migrate_backends_optional_env() {
    if let Ok(pg_url) = env::var("VOX_P5_POSTGRES_URL") {
        let pg = PostgresBackend::connect(&pg_url)
            .await
            .expect("connect postgres");
        let backend = AnySqlBackend::Postgres(pg);
        assert_migrate_apply_converges(&backend, "postgres").await;
    } else {
        eprintln!("p5-live: skipping postgres (set VOX_P5_POSTGRES_URL)");
    }

    if let Ok(my_url) = env::var("VOX_P5_MYSQL_URL") {
        let my = MySqlBackend::connect(&my_url).await.expect("connect mysql");
        let backend = AnySqlBackend::MySql(my);
        assert_migrate_apply_converges(&backend, "mysql").await;
    } else {
        eprintln!("p5-live: skipping mysql (set VOX_P5_MYSQL_URL)");
    }
}
