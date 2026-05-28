#![allow(missing_docs)]

//! AgentOS: ACI envelope attaches valid `aci` metadata; guardrail blocks catastrophic shell patterns.

use std::fs;
use std::path::PathBuf;

use serde_json::json;
use vox_orchestrator::agentos::guardrail_kernel::preflight_mcp_tool;
use vox_orchestrator_mcp::aci::{ACI_TOOL_RESPONSE_SCHEMA_RELPATH, attach_aci_envelope};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/")
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

#[test]
fn agentos_benchmark_manifest_matches_repo_paths() {
    let root = workspace_root();
    let manifest_path = root.join("contracts/benchmarks/agentos-aci-suite.v1.yaml");
    let raw = fs::read_to_string(&manifest_path).expect("read benchmark manifest");
    let schema_line = raw
        .lines()
        .find_map(|l| {
            let l = l.trim();
            l.strip_prefix("schema_under_test:").map(str::trim)
        })
        .expect("schema_under_test in manifest");
    assert_eq!(schema_line, ACI_TOOL_RESPONSE_SCHEMA_RELPATH);
    assert!(root.join(ACI_TOOL_RESPONSE_SCHEMA_RELPATH).is_file());
    for id in [
        "aci-schema-instance-valid-minimal",
        "guardrail-destructive-shell-denied",
        "aci-envelope-attach-validates",
    ] {
        assert!(raw.contains(id), "manifest missing case id {id}");
    }
}

#[test]
fn aci_tool_response_validates_against_contract_schema() {
    let root = workspace_root();
    let schema_path = root.join(ACI_TOOL_RESPONSE_SCHEMA_RELPATH);
    let schema_src = fs::read_to_string(&schema_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", schema_path.display()));
    let schema_val: serde_json::Value =
        serde_json::from_str(&schema_src).expect("parse ACI schema");
    let validator = jsonschema::validator_for(&schema_val).expect("compile ACI schema");

    let out = attach_aci_envelope(
        "vox_git_status",
        r#"{"success":true,"data":{"ok":true}}"#,
        true,
        None,
    )
    .expect("attach ACI envelope");
    let v: serde_json::Value = serde_json::from_str(&out).expect("parse wrapped JSON");
    validator
        .validate(&v)
        .expect("attached payload must match ACI schema");
    assert_eq!(v["aci"]["tool"], "vox_git_status");
    assert!(v["aci"]["side_effects"].as_array().is_some());
}

#[test]
fn guardrail_preflight_blocks_destructive_shell() {
    assert!(
        preflight_mcp_tool(
            "vox_run_shell",
            &json!({ "command": "rm -rf /tmp/x", "user_approval": true }),
        )
        .is_err()
    );
}
