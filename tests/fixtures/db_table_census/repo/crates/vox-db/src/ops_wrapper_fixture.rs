// Fixture: `fixture_wrapper_table`'s only vox-db-side reference. Its real
// CRUD lives in a wrapper function whose NAME does not contain the table
// name as a substring — this is the exact blind spot wrapper-call detection
// (Task 1b) exists to catch. The literal-string pass alone would classify
// this table DORMANT; the wrapper-call pass must promote it to LIVE because
// `acquire_widget_lease` is called from crates/other-crate/src/consumer.rs.
pub fn acquire_widget_lease(db: &VoxDb, owner: &str) -> Result<bool> {
    let sql = "INSERT INTO fixture_wrapper_table (owner) VALUES (?1)";
    db.execute(sql, &[owner])
}

// A second pub fn in the same file that must NOT be picked up as a
// candidate for fixture_wrapper_table — its body never mentions the table,
// so the brace-counted body extraction must correctly scope to only the
// function whose body contains the table name.
pub fn unrelated_helper(db: &VoxDb) -> Result<()> {
    let _ = db.execute("SELECT 1", &[]);
    Ok(())
}

// Fixture: `fixture_low_confidence_table`'s only vox-db-side reference is a
// short, generic, underscore-free wrapper name (`list`). It IS called from
// outside vox-db (see consumer.rs), but the wrapper-call pass must flag
// this as low-confidence rather than silently promoting the table to LIVE
// — `list(` is exactly the kind of common-word match that would produce
// noisy false-promotions if auto-trusted.
pub fn list(db: &VoxDb) -> Result<Vec<String>> {
    db.query_all("SELECT owner FROM fixture_low_confidence_table")
}
