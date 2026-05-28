#![allow(missing_docs)]

//! Contract checks for the broad-wave speech-to-code audit deliverables.

use std::fs;
use std::path::PathBuf;

use serde_json::json;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/")
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

#[test]
fn speech_audit_matrix_schema_and_yaml_validate() {
    let root = workspace_root();
    let schema_path = root.join("contracts/speech-to-code/audit-matrix.schema.json");
    let matrix_path = root.join("contracts/speech-to-code/audit-matrix.v1.yaml");

    let schema_raw = fs::read_to_string(&schema_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", schema_path.display()));
    let matrix_raw = fs::read_to_string(&matrix_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", matrix_path.display()));
    let schema: serde_json::Value = serde_json::from_str(&schema_raw).expect("parse schema json");
    let matrix_yaml: serde_yaml::Value = serde_yaml::from_str(&matrix_raw).expect("parse yaml");
    let matrix_json = serde_json::to_value(matrix_yaml).expect("yaml converts to json");

    jsonschema::validator_for(&schema)
        .expect("compile audit matrix schema")
        .validate(&matrix_json)
        .expect("audit matrix must validate");

    let cells = matrix_json
        .get("cells")
        .and_then(|v| v.as_array())
        .expect("cells array");
    assert!(
        cells.iter().any(|c| c.get("tier") == Some(&json!("must"))
            && c.get("surface") == Some(&json!("editor-webview-mic"))),
        "matrix must include a MUST editor-webview-mic cell"
    );
    assert!(
        cells.iter().any(|c| c.get("tier") == Some(&json!("must"))
            && c.get("surface") == Some(&json!("dashboard-speak-gap"))),
        "matrix must include a MUST dashboard-speak-gap confirmation cell"
    );
    assert!(
        cells.iter().any(|c| c.get("tier") == Some(&json!("should"))
            && c.get("compute") == Some(&json!("cuda"))),
        "matrix must include a SHOULD CUDA runtime decode cell"
    );
}

#[test]
fn committed_speech_canary_kpi_validates_against_contract() {
    let root = workspace_root();
    let schema_path = root.join("contracts/speech-to-code/kpi-baseline.schema.json");
    let canary_path = root.join("contracts/speech-to-code/canary.kpi.json");

    let schema_raw = fs::read_to_string(&schema_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", schema_path.display()));
    let canary_raw = fs::read_to_string(&canary_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", canary_path.display()));
    let schema: serde_json::Value = serde_json::from_str(&schema_raw).expect("parse kpi schema");
    let canary: serde_json::Value = serde_json::from_str(&canary_raw).expect("parse canary kpi");

    jsonschema::validator_for(&schema)
        .expect("compile kpi schema")
        .validate(&canary)
        .expect("committed canary KPI must validate");

    assert_eq!(canary.get("schema_version"), Some(&json!("1")));
    assert!(
        canary.get("wer").and_then(|v| v.as_f64()).is_some(),
        "committed canary KPI must include WER"
    );
    assert!(
        canary.get("cer").and_then(|v| v.as_f64()).is_some(),
        "committed canary KPI must include CER"
    );
}

#[test]
fn speech_audit_docs_are_published_and_indexed() {
    let root = workspace_root();
    let required_docs = [
        "docs/src/architecture/vox-speech-surface-inventory-2026.md",
        "docs/src/architecture/vox-speech-audit-findings-2026.md",
        "docs/src/architecture/vox-speech-improvement-backlog-2026.md",
        "docs/src/architecture/vox-speech-ci-gates-proposal-2026.md",
    ];
    for rel in required_docs {
        let abs = root.join(rel);
        assert!(abs.exists(), "missing speech audit doc: {}", abs.display());
        let raw = fs::read_to_string(&abs).unwrap_or_else(|e| panic!("read {}: {e}", abs.display()));
        assert!(raw.contains("title:"), "{rel} must have frontmatter title");
        assert!(
            raw.contains("last_updated:"),
            "{rel} must have frontmatter last_updated"
        );
    }

    let index_path = root.join("docs/src/architecture/research-index.md");
    let index = fs::read_to_string(&index_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", index_path.display()));
    for rel in required_docs {
        let file_name = rel.rsplit('/').next().expect("file name");
        assert!(
            index.contains(file_name),
            "research-index.md must link {file_name}"
        );
    }
}
