//! T1.1 contract test: every `dispatch-events.v1.schema.json` fixture validates against
//! the schema, and every T1.1 `OperationKind` variant serializes to a shape the schema
//! accepts. Registered in `contracts/index.yaml` as this schema's `enforced_by`.

use vox_orchestrator_queue::oplog::OperationKind;

fn repo_root() -> std::path::PathBuf {
    // crates/vox-orchestrator-queue -> repo root
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("repo root from CARGO_MANIFEST_DIR")
        .to_path_buf()
}

fn schema_validator() -> jsonschema::Validator {
    let schema_path = repo_root()
        .join("contracts")
        .join("orchestration")
        .join("dispatch-events.v1.schema.json");
    let raw = std::fs::read_to_string(&schema_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", schema_path.display()));
    let schema: serde_json::Value =
        serde_json::from_str(&raw).unwrap_or_else(|e| panic!("parse schema: {e}"));
    jsonschema::validator_for(&schema).expect("compile dispatch-events.v1.schema.json")
}

/// RED: every fixture file under `contracts/orchestration/fixtures/dispatch-events/`
/// must validate against the schema.
#[test]
fn fixtures_validate_against_schema() {
    let validator = schema_validator();
    let fixtures_dir = repo_root()
        .join("contracts")
        .join("orchestration")
        .join("fixtures")
        .join("dispatch-events");
    let entries = std::fs::read_dir(&fixtures_dir)
        .unwrap_or_else(|e| panic!("read_dir {}: {e}", fixtures_dir.display()));

    let mut checked = 0usize;
    for entry in entries {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path:?}: {e}"));
        let instance: serde_json::Value =
            serde_json::from_str(&raw).unwrap_or_else(|e| panic!("parse {path:?}: {e}"));
        if let Err(e) = validator.validate(&instance) {
            panic!("fixture {path:?} failed schema validation: {e}");
        }
        checked += 1;
    }
    assert_eq!(checked, 8, "expected one fixture per T1.1 OperationKind variant");
}

/// RED: every new T1.1 `OperationKind` variant, serialized exactly as `OpLog::record`
/// would persist it (via `serde_json::to_value`), validates against the schema. This
/// pins the externally-tagged wire shape so a future serde/derive change can't silently
/// break the contract.
#[test]
fn t11_variants_serialize_to_schema_valid_shapes() {
    let validator = schema_validator();
    let variants = vec![
        OperationKind::ApprovalRequested {
            approval_id: "AP-000001".into(),
            tool: "vox_run_shell".into(),
            run_id: Some("run-abc123".into()),
        },
        OperationKind::ApprovalResolved {
            approval_id: "AP-000001".into(),
            outcome: "approved".into(),
            resolver: Some("gui".into()),
        },
        OperationKind::FeedbackRequested {
            request_id: "F-000001".into(),
            task_id: Some(7),
            kind: "clarification".into(),
        },
        OperationKind::FeedbackResolved {
            request_id: "F-000001".into(),
            action: "answer".into(),
        },
        OperationKind::TaskDoubted {
            task_id: 7,
            reason: Some("suspect".into()),
        },
        OperationKind::HopperAdmit {
            item_id: "HP-000001".into(),
        },
        OperationKind::HopperAssign {
            item_id: "HP-000001".into(),
            task_id: 42,
        },
        OperationKind::HopperComplete {
            item_id: "HP-000001".into(),
        },
    ];

    for v in variants {
        let value = serde_json::to_value(&v).expect("serialize OperationKind");
        if let Err(e) = validator.validate(&value) {
            panic!("OperationKind {v:?} serialized as {value} failed schema validation: {e}");
        }
    }
}

/// Negative check: an unknown/malformed shape must NOT validate (guards against an
/// over-permissive `additionalProperties: true` schema silently accepting garbage).
#[test]
fn unknown_shape_is_rejected() {
    let validator = schema_validator();
    let bad = serde_json::json!({ "NotARealVariant": { "foo": "bar" } });
    assert!(
        validator.validate(&bad).is_err(),
        "schema must reject an unrecognized variant tag"
    );

    let bad_multi = serde_json::json!({
        "HopperAdmit": { "item_id": "x" },
        "HopperComplete": { "item_id": "x" }
    });
    assert!(
        validator.validate(&bad_multi).is_err(),
        "schema must reject an instance carrying more than one variant tag"
    );
}
