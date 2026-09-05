// Requires `runtime` (default-on) plus `postgres` and `mysql`: this test
// exercises live SqlBackend connections for all three backends, and
// vox-sql cfg-gates the Postgres/MySql backends behind those features
// independently of `runtime` (see crates/vox-sql/Cargo.toml and THE TRAP
// note on `BackendKind`). Missing any of the three no-ops this file rather
// than failing to compile.
#![cfg(all(feature = "runtime", feature = "postgres", feature = "mysql"))]

use std::env;

use vox_db::DbConfig;
use vox_sql::build::placeholder_sql;
use vox_sql::{LibsqlBackend, MySqlBackend, PostgresBackend, SqlBackend, SqlRow, SqlValue};

const CREATE_SQL: &str = "CREATE TABLE IF NOT EXISTS vox_p2_items (id INTEGER PRIMARY KEY, name TEXT NOT NULL, qty INTEGER NOT NULL, note TEXT)";
const DELETE_SQL: &str = "DELETE FROM vox_p2_items";
const SELECT_SQL: &str = "SELECT id, name, qty, note FROM vox_p2_items ORDER BY id ASC";

fn fixture_rows() -> Vec<Vec<SqlValue>> {
    vec![
        vec![
            SqlValue::Int(1),
            SqlValue::Text("alpha".to_string()),
            SqlValue::Int(10),
            SqlValue::Null,
        ],
        vec![
            SqlValue::Int(2),
            SqlValue::Text("beta".to_string()),
            SqlValue::Int(20),
            SqlValue::Text("memo".to_string()),
        ],
    ]
}

fn normalized(rows: &[SqlRow]) -> Vec<Vec<SqlValue>> {
    rows.iter()
        .map(|r| {
            r.iter()
                .map(|(_k, v)| normalize_value(v))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>()
}

fn normalize_value(v: &SqlValue) -> SqlValue {
    match v {
        SqlValue::Bool(b) => SqlValue::Int(i64::from(*b)),
        SqlValue::Float(f) => SqlValue::Int(*f as i64),
        other => other.clone(),
    }
}

async fn seed_and_read<B: SqlBackend>(backend: &B) -> Result<Vec<SqlRow>, String> {
    backend
        .execute(CREATE_SQL, &[])
        .await
        .map_err(|e| format!("create failed: {e}"))?;
    backend
        .execute(DELETE_SQL, &[])
        .await
        .map_err(|e| format!("delete failed: {e}"))?;

    let d = backend.dialect();
    let insert_sql = format!(
        "INSERT INTO vox_p2_items (id, name, qty, note) VALUES ({}, {}, {}, {})",
        placeholder_sql(d, 1),
        placeholder_sql(d, 2),
        placeholder_sql(d, 3),
        placeholder_sql(d, 4)
    );

    for row in fixture_rows() {
        backend
            .execute(&insert_sql, &row)
            .await
            .map_err(|e| format!("insert failed: {e}"))?;
    }

    backend
        .query(SELECT_SQL, &[])
        .await
        .map_err(|e| format!("select failed: {e}"))
}

async fn tx_roundtrip<B: SqlBackend>(backend: &B) -> Result<Vec<SqlRow>, String> {
    backend
        .execute(CREATE_SQL, &[])
        .await
        .map_err(|e| format!("create failed: {e}"))?;
    backend
        .execute(DELETE_SQL, &[])
        .await
        .map_err(|e| format!("delete failed: {e}"))?;

    let d = backend.dialect();
    let insert_sql = format!(
        "INSERT INTO vox_p2_items (id, name, qty, note) VALUES ({}, {}, {}, {})",
        placeholder_sql(d, 1),
        placeholder_sql(d, 2),
        placeholder_sql(d, 3),
        placeholder_sql(d, 4)
    );

    // Rollback should discard inserted row.
    backend
        .begin_transaction()
        .await
        .map_err(|e| format!("begin failed: {e}"))?;
    backend
        .execute(
            &insert_sql,
            &[
                SqlValue::Int(10),
                SqlValue::Text("rollback".to_string()),
                SqlValue::Int(1),
                SqlValue::Null,
            ],
        )
        .await
        .map_err(|e| format!("insert in rollback tx failed: {e}"))?;
    backend
        .rollback_transaction()
        .await
        .map_err(|e| format!("rollback failed: {e}"))?;

    // Commit should persist inserted row.
    backend
        .begin_transaction()
        .await
        .map_err(|e| format!("begin failed: {e}"))?;
    backend
        .execute(
            &insert_sql,
            &[
                SqlValue::Int(11),
                SqlValue::Text("commit".to_string()),
                SqlValue::Int(2),
                SqlValue::Null,
            ],
        )
        .await
        .map_err(|e| format!("insert in commit tx failed: {e}"))?;
    backend
        .commit_transaction()
        .await
        .map_err(|e| format!("commit failed: {e}"))?;

    backend
        .query(SELECT_SQL, &[])
        .await
        .map_err(|e| format!("select failed: {e}"))
}

#[tokio::test]
async fn p2_live_backend_differential_optional_env() {
    let libsql = LibsqlBackend::connect_from_config(DbConfig::Memory)
        .await
        .expect("connect libsql memory");
    let baseline = seed_and_read(&libsql)
        .await
        .expect("seed/read libsql baseline");
    let baseline_norm = normalized(&baseline);

    if let Ok(pg_url) = env::var("VOX_P2_POSTGRES_URL") {
        let pg = PostgresBackend::connect(&pg_url)
            .await
            .expect("connect postgres");
        let got = seed_and_read(&pg).await.expect("seed/read postgres");
        assert_eq!(
            normalized(&got),
            baseline_norm,
            "postgres rows should match libsql baseline"
        );

        let tx_rows = tx_roundtrip(&pg).await.expect("tx roundtrip postgres");
        let tx_norm = normalized(&tx_rows);
        assert_eq!(tx_norm.len(), 1, "postgres tx should persist one row");
        assert_eq!(
            tx_norm[0][1],
            SqlValue::Text("commit".to_string()),
            "postgres commit row should persist while rollback row is absent"
        );
    } else {
        eprintln!("p2-live: skipping postgres (set VOX_P2_POSTGRES_URL)");
    }

    if let Ok(my_url) = env::var("VOX_P2_MYSQL_URL") {
        let my = MySqlBackend::connect(&my_url).await.expect("connect mysql");
        let got = seed_and_read(&my).await.expect("seed/read mysql");
        assert_eq!(
            normalized(&got),
            baseline_norm,
            "mysql rows should match libsql baseline"
        );

        let tx_rows = tx_roundtrip(&my).await.expect("tx roundtrip mysql");
        let tx_norm = normalized(&tx_rows);
        assert_eq!(tx_norm.len(), 1, "mysql tx should persist one row");
        assert_eq!(
            tx_norm[0][1],
            SqlValue::Text("commit".to_string()),
            "mysql commit row should persist while rollback row is absent"
        );
    } else {
        eprintln!("p2-live: skipping mysql (set VOX_P2_MYSQL_URL)");
    }

    let libsql_tx_rows = tx_roundtrip(&libsql)
        .await
        .expect("tx roundtrip libsql baseline");
    let libsql_tx_norm = normalized(&libsql_tx_rows);
    assert_eq!(libsql_tx_norm.len(), 1, "libsql tx should persist one row");
    assert_eq!(
        libsql_tx_norm[0][1],
        SqlValue::Text("commit".to_string()),
        "libsql commit row should persist while rollback row is absent"
    );
}
