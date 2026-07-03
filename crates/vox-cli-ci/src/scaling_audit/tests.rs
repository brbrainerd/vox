use super::enforce_toestub_rust_parse_budget;
use serde_json::json;

#[test]
fn budget_skips_when_field_absent() {
    enforce_toestub_rust_parse_budget(&json!({ "findings": [] }), 0).unwrap();
}

#[test]
fn budget_allows_at_cap() {
    enforce_toestub_rust_parse_budget(&json!({ "rust_parse_failures": 2 }), 2).unwrap();
}

#[test]
fn budget_rejects_over_cap() {
    let e = enforce_toestub_rust_parse_budget(&json!({ "rust_parse_failures": 5 }), 3).unwrap_err();
    assert!(e.to_string().contains("rust_parse_failures=5"), "{e}");
}
