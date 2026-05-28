//! Golden fixtures for wire-format v1 shapes (see `docs/src/architecture/wire-format-v1-ssot.md`).

#[test]
fn error_envelope_example_has_required_fields() {
    let raw = include_str!("golden/wire-format/error-envelope.example.json");
    let v: serde_json::Value = serde_json::from_str(raw).expect("valid JSON");
    assert_eq!(v["ok"], false);
    assert!(
        v["code"].as_str().is_some_and(|s| !s.is_empty()),
        "code present"
    );
    assert!(
        v["message"].as_str().is_some_and(|s| !s.is_empty()),
        "message present"
    );
    assert!(v.get("request_id").is_some());
    assert!(v.get("details").is_some());
}
