//! T1.3 RED: `orch_daemon_method`'s Rust constants and
//! `contracts/orchestration/orch-daemon-rpc-methods.schema.json`'s `method`
//! enum must name exactly the same set of `orch.*` methods. Both sources are
//! parsed/greped programmatically here (never hand-copied into this test) so
//! adding a method constant without updating the schema — or vice versa — is
//! a compile-time-adjacent (test) failure rather than a silent drift, mirroring
//! the T1.1 `dispatch_events_contract` fixture-parity pattern in
//! `crates/vox-orchestrator-queue/tests/dispatch_events_contract.rs`.

use std::collections::BTreeSet;

fn repo_root() -> std::path::PathBuf {
    // crates/vox-foundation -> repo root
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("repo root from CARGO_MANIFEST_DIR")
        .to_path_buf()
}

/// Every `pub const XXX: &str = "orch.something";` value inside the
/// `orch_daemon_method` module of `crates/vox-foundation/src/protocol.rs`.
/// Parsed with a small line-oriented scanner scoped to that module (so a
/// `dei_method` or other module's constants never leak in), not a hand-copied
/// list — a new `orch.*` constant is picked up automatically.
fn method_constants_from_protocol_rs() -> BTreeSet<String> {
    let path = repo_root()
        .join("crates")
        .join("vox-foundation")
        .join("src")
        .join("protocol.rs");
    let src = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path:?}: {e}"));

    let mut in_module = false;
    let mut depth = 0i32;
    let mut out = BTreeSet::new();

    for line in src.lines() {
        let trimmed = line.trim();
        if !in_module {
            if trimmed.starts_with("pub mod orch_daemon_method") {
                in_module = true;
                depth = trimmed.matches('{').count() as i32 - trimmed.matches('}').count() as i32;
            }
            continue;
        }

        depth += trimmed.matches('{').count() as i32 - trimmed.matches('}').count() as i32;
        if depth <= 0 {
            break; // left the module
        }

        if let Some(eq_pos) = trimmed.find('=') {
            if trimmed.starts_with("pub const") {
                let rhs = trimmed[eq_pos + 1..].trim().trim_end_matches(';').trim();
                if let Some(value) = rhs.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
                    out.insert(value.to_string());
                }
            }
        }
    }

    assert!(
        !out.is_empty(),
        "scanner found zero orch_daemon_method constants in protocol.rs — parser likely broken"
    );
    out
}

/// Every entry in the schema's `properties.method.enum` array, parsed as JSON
/// (not hand-copied).
fn method_enum_from_schema() -> BTreeSet<String> {
    let path = repo_root()
        .join("contracts")
        .join("orchestration")
        .join("orch-daemon-rpc-methods.schema.json");
    let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path:?}: {e}"));
    let schema: serde_json::Value =
        serde_json::from_str(&raw).unwrap_or_else(|e| panic!("parse {path:?}: {e}"));

    let enum_arr = schema["properties"]["method"]["enum"]
        .as_array()
        .unwrap_or_else(|| panic!("{path:?}: properties.method.enum must be an array"));

    enum_arr
        .iter()
        .map(|v| {
            v.as_str()
                .unwrap_or_else(|| panic!("{path:?}: enum entry {v} is not a string"))
                .to_string()
        })
        .collect()
}

#[test]
fn schema_method_enum_matches_protocol_rs_constants() {
    let rust_methods = method_constants_from_protocol_rs();
    let schema_methods = method_enum_from_schema();

    let missing_from_schema: Vec<&String> = rust_methods.difference(&schema_methods).collect();
    let extra_in_schema: Vec<&String> = schema_methods.difference(&rust_methods).collect();

    assert!(
        missing_from_schema.is_empty(),
        "orch_daemon_method constants present in protocol.rs but MISSING from \
         orch-daemon-rpc-methods.schema.json's method enum: {missing_from_schema:?}"
    );
    assert!(
        extra_in_schema.is_empty(),
        "orch-daemon-rpc-methods.schema.json's method enum has entries with NO \
         matching orch_daemon_method constant in protocol.rs (stale/typo?): {extra_in_schema:?}"
    );
}
