// Fixture: real per-table usage from *inside* vox-db (not a declaration file).
// References `fixture_dormant_table` -> should count as a vox_db-side usage.
fn read_dormant(db: &VoxDb) {
    let _rows = db.query("SELECT * FROM fixture_dormant_table");
}
