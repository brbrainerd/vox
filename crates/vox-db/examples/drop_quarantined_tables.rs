//! One-time upgrade tool: drop the tables Task 4 quarantined
//! (`crates/vox-db/src/migration.rs::QUARANTINE_DROP_TABLES`) from an existing,
//! pre-Task-4 VoxDB file.
//!
//! `migrate_dropping_quarantine` is deliberately not wired into `VoxDb::connect()`/`open()` —
//! it's an explicit, opt-in step. This example is the intended way to invoke it against a real
//! file: back up the file yourself first (this tool does not do that for you), then run
//!
//!   cargo run -p vox-db --example drop_quarantined_tables -- <path-to-store.db>
//!
//! Prints table count and `schema_version` before and after. Aborts (leaving the file untouched)
//! and prints a remediation message if any quarantined table still has rows.

use std::env;

#[tokio::main]
async fn main() {
    let path = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: drop_quarantined_tables <path-to-store.db>");
        std::process::exit(2);
    });

    let db = turso::Builder::new_local(&path)
        .build()
        .await
        .expect("open database file");
    let conn = db.connect().expect("connect");

    let before_tables = table_count(&conn).await;
    let before_version = schema_version(&conn).await;
    println!("Before: {before_tables} tables, schema_version={before_version}");

    match vox_db::migration::migrate_dropping_quarantine(&conn).await {
        Ok(()) => {
            let after_tables = table_count(&conn).await;
            let after_version = schema_version(&conn).await;
            println!("After:  {after_tables} tables, schema_version={after_version}");
            println!(
                "Dropped {} tables.",
                before_tables.saturating_sub(after_tables)
            );
        }
        Err(e) => {
            eprintln!("Aborted, database untouched: {e}");
            std::process::exit(1);
        }
    }
}

async fn table_count(conn: &turso::Connection) -> i64 {
    let mut rows = conn
        .query(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
            (),
        )
        .await
        .expect("count tables");
    rows.next()
        .await
        .expect("next")
        .expect("row")
        .get(0)
        .expect("count")
}

async fn schema_version(conn: &turso::Connection) -> i64 {
    let mut rows = conn
        .query("SELECT COALESCE(MAX(version), 0) FROM schema_version", ())
        .await
        .expect("query schema_version");
    rows.next()
        .await
        .expect("next")
        .expect("row")
        .get(0)
        .expect("version")
}
