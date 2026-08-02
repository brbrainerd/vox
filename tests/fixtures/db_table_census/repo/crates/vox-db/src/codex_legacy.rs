// Fixture: stand-in for the real crates/vox-db/src/codex_legacy.rs — bulk
// JSONL import/export that mentions nearly every table name generically.
// The census tool must exclude this file from the usage signal, so a table
// that ONLY appears here (fixture_excluded_table) still classifies as DEAD.
fn export_all(db: &VoxDb) {
    let _ = db.query("SELECT * FROM fixture_excluded_table");
}
