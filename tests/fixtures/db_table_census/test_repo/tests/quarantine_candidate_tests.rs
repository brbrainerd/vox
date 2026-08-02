// Fixture: synthetic #[test] functions referencing DEAD/DORMANT table names
// from tests/fixtures/db_table_census/repo (fixture_dead_table,
// fixture_dormant_table), used to drive scripts/db-test-census.vox's 1.2a
// fixture test. Not part of the real crate graph — never compiled by cargo.

#[test]
fn test_reads_dead_table() {
    let _rows = db.query("SELECT * FROM fixture_dead_table");
}

#[tokio::test]
async fn test_reads_dormant_table() {
    let _rows = db.query("SELECT * FROM fixture_dormant_table");
}

// Control case: references a LIVE table only — must NOT be flagged.
#[test]
fn test_reads_live_table_only() {
    let _rows = db.query("SELECT * FROM fixture_live_table");
}
