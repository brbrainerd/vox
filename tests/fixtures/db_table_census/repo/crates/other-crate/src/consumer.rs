// Fixture: real usage from OUTSIDE vox-db / vox-db-types.
// References `fixture_live_table` -> should count as an "outside" usage,
// which is the sole signal that makes a table LIVE.
fn read_live(db: &VoxDb) {
    let _rows = db.query("SELECT * FROM fixture_live_table");
}

// Fixture: real outside usage via a wrapper method call whose name never
// mentions the underlying table literally (see Task 1b wrapper-call
// detection pass / crates/vox-db/src/ops_wrapper_fixture.rs). The
// literal-string pass alone cannot see this call site.
fn lease_widget(db: &VoxDb) {
    let _ok = db.acquire_widget_lease("tester");
}

// Fixture: calls a short/generic wrapper name from outside vox-db, to prove
// the wrapper-call pass distinguishes "found a call site" from "confident
// enough to auto-promote" (see ops_wrapper_fixture.rs).
fn read_low_confidence(db: &VoxDb) {
    let _owners = db.list();
}
