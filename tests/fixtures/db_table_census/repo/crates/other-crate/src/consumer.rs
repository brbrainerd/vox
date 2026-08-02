// Fixture: real usage from OUTSIDE vox-db / vox-db-types.
// References `fixture_live_table` -> should count as an "outside" usage,
// which is the sole signal that makes a table LIVE.
fn read_live(db: &VoxDb) {
    let _rows = db.query("SELECT * FROM fixture_live_table");
}
