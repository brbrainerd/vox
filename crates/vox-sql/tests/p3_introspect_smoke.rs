// Requires the `runtime` feature (default-on): this test exercises the live
// introspect module, which vox-sql's `runtime` feature gate (see
// crates/vox-sql/Cargo.toml) makes optional. `--no-default-features` no-ops
// this file rather than failing to compile.
#![cfg(feature = "runtime")]

use vox_db::DbConfig;
use vox_sql::schema_model::IntrospectedSchema;
use vox_sql::{AnySqlBackend, LibsqlBackend, SqlBackend, SqlValue};

#[tokio::test]
async fn introspect_schema_smoke_for_libsql_memory() {
    let libsql = LibsqlBackend::connect_from_config(DbConfig::Memory)
        .await
        .expect("connect in-memory");
    libsql
        .execute("DROP TABLE IF EXISTS p3_users", &[])
        .await
        .expect("drop table");
    libsql
        .execute(
            "CREATE TABLE IF NOT EXISTS p3_users (id INTEGER PRIMARY KEY, name TEXT NOT NULL, active INTEGER)",
            &[],
        )
        .await
        .expect("create table");
    libsql
        .execute(
            "INSERT INTO p3_users (id, name, active) VALUES (?1, ?2, ?3)",
            &[
                SqlValue::Int(1),
                SqlValue::Text("ada".to_string()),
                SqlValue::Int(1),
            ],
        )
        .await
        .expect("insert row");

    let any = AnySqlBackend::Libsql(libsql);
    let schema: IntrospectedSchema = any.introspect_schema().await.expect("introspect");
    assert_eq!(schema.backend, "sqlite");
    let users = schema
        .tables
        .iter()
        .find(|t| t.name == "p3_users")
        .expect("p3_users table present");
    assert!(users.columns.iter().any(|c| c.name == "id"));
    assert!(users.columns.iter().any(|c| c.name == "name"));
}
