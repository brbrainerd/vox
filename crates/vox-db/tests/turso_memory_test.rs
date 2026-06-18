#[tokio::test]
async fn test_turso_memory_directly() {
    let temp_path = std::env::temp_dir().join("test_turso.db").to_string_lossy().to_string();
    let _ = std::fs::remove_file(&temp_path);
    let db = turso::Builder::new_local(&temp_path).build().await.unwrap();
    let conn = db.connect().unwrap();
    
    conn.pragma_update("journal_mode", "WAL").await.unwrap();
    conn.pragma_update("busy_timeout", 5000).await.unwrap();
    conn.pragma_update("synchronous", "NORMAL").await.unwrap();
    conn.pragma_update("foreign_keys", "ON").await.unwrap();
    conn.pragma_update("cache_size", -65536).await.unwrap();
    
    // Create schema_version
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_version (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL DEFAULT (datetime('now'))
        );"
    ).await.unwrap();
    
    let sql = vox_db::schema::baseline_sql();
    println!("baseline_sql length: {}", sql.len());
    match conn.execute_batch(sql).await {
        Ok(_) => println!("execute_batch baseline_sql returned Ok"),
        Err(e) => println!("execute_batch baseline_sql returned Err: {:?}", e),
    }
    
    // Explicitly drop the database object
    println!("Dropping Database object...");
    drop(db);
    
    let mut rows = conn.query("SELECT name FROM sqlite_master WHERE type='table'", ()).await.unwrap();
    println!("--- Tables After drop(db) ---");
    while let Some(row) = rows.next().await.unwrap() {
        let name: String = row.get(0).unwrap();
        println!("TABLE: {}", name);
    }
}
