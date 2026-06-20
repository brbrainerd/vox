//! TDD tests for ClickHouse DDL generation from the taxonomy contract.

use vox_server::{gen_ddl, schema::load_taxonomy};

#[test]
fn gen_ddl_produces_create_table() {
    let t = load_taxonomy().expect("taxonomy must parse");
    let ddl = gen_ddl(&t);
    assert!(ddl.contains("CREATE TABLE IF NOT EXISTS events_raw"));
}

#[test]
fn gen_ddl_has_ttl_180_days() {
    let t = load_taxonomy().expect("taxonomy");
    let ddl = gen_ddl(&t);
    assert!(
        ddl.contains("TTL toDateTime(ts) + INTERVAL 180 DAY"),
        "DDL must have 180-day TTL"
    );
}

#[test]
fn gen_ddl_has_required_primary_columns() {
    let t = load_taxonomy().expect("taxonomy");
    let ddl = gen_ddl(&t);
    assert!(ddl.contains("install_id"), "must have install_id");
    assert!(ddl.contains("event_name"), "must have event_name");
    assert!(ddl.contains("DateTime64(3, 'UTC')"), "must have DateTime64 ts");
}

#[test]
fn gen_ddl_creates_materialized_view_per_category() {
    let t = load_taxonomy().expect("taxonomy");
    let ddl = gen_ddl(&t);
    for cat in &t.categories {
        let view = format!("mv_{}", cat.name);
        assert!(
            ddl.contains(&view),
            "DDL must contain materialized view '{view}' for category '{}'",
            cat.name
        );
    }
}

#[test]
fn gen_ddl_enum_fields_use_low_cardinality() {
    let t = load_taxonomy().expect("taxonomy");
    let ddl = gen_ddl(&t);
    assert!(
        ddl.contains("LowCardinality(Nullable(String))"),
        "enum/hash fields must use LowCardinality(Nullable(String))"
    );
}

#[test]
fn gen_ddl_bool_fields_use_uint8() {
    let t = load_taxonomy().expect("taxonomy");
    let ddl = gen_ddl(&t);
    // 'accepted' in skill_activation and 'recoverable' in error_surface are bool.
    assert!(ddl.contains("Nullable(UInt8)"), "bool fields must use Nullable(UInt8)");
}

#[test]
fn gen_ddl_int_fields_use_nullable_int64() {
    let t = load_taxonomy().expect("taxonomy");
    let ddl = gen_ddl(&t);
    // magnitude_bucket is the int field in default_decision.
    assert!(ddl.contains("Nullable(Int64)"), "int fields must use Nullable(Int64)");
}

#[test]
fn taxonomy_has_expected_categories() {
    let t = load_taxonomy().expect("taxonomy");
    let names: Vec<&str> = t.categories.iter().map(|c| c.name.as_str()).collect();
    for expected in &[
        "command_usage",
        "skill_activation",
        "edit_pattern",
        "harness_usage",
        "error_surface",
        "default_decision",
    ] {
        assert!(names.contains(expected), "taxonomy must have category '{expected}'");
    }
}

#[test]
fn gen_ddl_uses_merge_tree_engine() {
    let t = load_taxonomy().expect("taxonomy");
    let ddl = gen_ddl(&t);
    assert!(ddl.contains("ENGINE = MergeTree()"), "must use MergeTree engine");
}

#[test]
fn ddl_splits_into_executable_statements() {
    use vox_server::schema::{gen_ddl, load_taxonomy, split_statements};
    let tax = load_taxonomy().expect("taxonomy loads");
    let stmts = split_statements(&gen_ddl(&tax));
    // events_raw + at least one materialized view.
    assert!(stmts.len() >= 2, "expected >=2 statements, got {}", stmts.len());
    assert!(
        stmts
            .iter()
            .any(|s| s.contains("CREATE TABLE IF NOT EXISTS events_raw")),
        "one statement must create events_raw"
    );
    assert!(stmts.iter().all(|s| !s.trim().is_empty()), "no empty statements");
    // No bare comment-only fragments survive splitting, and no statement carries a
    // trailing ';' (clickhouse .query() wants one statement, no terminator).
    assert!(stmts.iter().all(|s| !s.trim_start().starts_with("--")));
    assert!(stmts.iter().all(|s| !s.trim_end().ends_with(';')));
}
