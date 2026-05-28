//! Integration tests for synthetic_gen — verify that all live registry entries
//! produce at least one training pair and that output is valid JSONL.
#![allow(missing_docs)]

use vox_bounded_fs::read_utf8_path_capped;
use vox_corpus::corpus::coverage::analyse_str_with_taxonomy;
use vox_corpus::synthetic_gen::{
    A2A_MESSAGE_TYPES, ORCHESTRATOR_TOOLS, SKILL_TOOLS, SyntheticGenConfig, TOOL_REGISTRY_SLIM,
    generate_all,
};

fn default_cfg() -> SyntheticGenConfig {
    SyntheticGenConfig::default()
}

/// Run generate_all in a fresh temp directory so companion files
/// (`synthetic_search.jsonl`, `synthetic_cot_temp.jsonl`) don't collide
/// across parallel test threads.
fn run_to_string(cfg: &SyntheticGenConfig) -> String {
    let tmp_dir = tempfile::tempdir().unwrap();
    let output = tmp_dir.path().join("corpus.jsonl");
    generate_all(cfg, &output).unwrap();
    read_utf8_path_capped(&output).unwrap()
}

#[test]
fn tool_registry_produces_pairs_for_every_entry() {
    let out = run_to_string(&default_cfg());
    for &name in TOOL_REGISTRY_SLIM {
        assert!(
            out.contains(name),
            "tool {name} missing from synthetic output"
        );
    }
}

#[test]
fn a2a_all_message_types_covered() {
    let out = run_to_string(&default_cfg());
    for &msg_type in A2A_MESSAGE_TYPES {
        assert!(
            out.contains(msg_type),
            "A2A type {msg_type} missing from synthetic output"
        );
    }
}

#[test]
fn all_skill_tools_appear_in_output() {
    let out = run_to_string(&default_cfg());
    for &tool in SKILL_TOOLS {
        assert!(
            out.contains(tool),
            "skill tool {tool} missing from synthetic output"
        );
    }
}

#[test]
fn all_orchestrator_tools_appear_in_output() {
    let out = run_to_string(&default_cfg());
    for &tool in ORCHESTRATOR_TOOLS {
        assert!(
            out.contains(tool),
            "orchestrator tool {tool} missing from synthetic output"
        );
    }
}

#[test]
fn every_line_is_valid_jsonl_with_required_fields() {
    let out = run_to_string(&default_cfg());
    let mut count = 0;
    for line in out.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let v: serde_json::Value = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("invalid JSON: {e}\n  Line: {line}"));
        assert!(
            v.get("prompt").and_then(|x| x.as_str()).is_some(),
            "missing prompt"
        );
        assert!(v.get("response").is_some(), "missing response");
        assert!(
            v.get("category").and_then(|x| x.as_str()).is_some(),
            "missing category"
        );
        count += 1;
    }
    assert!(count > 500, "expected >500 pairs, got {count}");
}

#[test]
fn selective_flag_no_tools_excludes_tool_traces() {
    let cfg = SyntheticGenConfig {
        emit_tool_traces: false,
        emit_orchestrator_rows: false,
        emit_skill_rows: false,
        emit_agent_rows: false,
        emit_cli_rows: false,
        emit_script_rows: false,
        emit_routing_decisions: false,
        emit_negative_expanded: false,
        emit_error_recovery: false,
        emit_multi_agent_convos: false,
        emit_telemetry_pairs: false,
        emit_a2a_traces: false,
        emit_workflow_traces: false,
        emit_organic_vox: false,
        augment_after_generate: false,
        ..Default::default()
    };
    let out = run_to_string(&cfg);
    // With all trace generators disabled, vox_build_crate should be absent
    // (it only appears in pure tool_trace rows, not in curated scenarios)
    assert!(
        !out.contains("vox_build_crate"),
        "vox_build_crate should be excluded when all trace generators are disabled"
    );
}

#[test]
fn selective_flag_no_a2a_excludes_a2a_categories() {
    let cfg = SyntheticGenConfig {
        emit_a2a_traces: false,
        ..Default::default()
    };
    let out = run_to_string(&cfg);
    // plan_handoff is A2A only
    let lines_with_a2a: Vec<_> = out
        .lines()
        .filter(|l| l.contains("\"a2a_trace\""))
        .collect();
    // Some tool lines will mention a2a tools but category won't be a2a_trace
    assert!(
        lines_with_a2a.is_empty(),
        "a2a_trace category lines should be absent"
    );
}

#[test]
fn min_phrasings_produces_at_least_that_many_pairs_per_tool() {
    let min = 12usize;
    let cfg = SyntheticGenConfig {
        min_phrasings_per_tool: min,
        emit_a2a_traces: false,
        emit_workflow_traces: false,
        emit_orchestrator_rows: false,
        emit_skill_rows: false,
        emit_agent_rows: false,
        ..Default::default()
    };
    let tmp_dir = tempfile::tempdir().unwrap();
    let tmp_output = tmp_dir.path().join("corpus.jsonl");
    generate_all(&cfg, &tmp_output).unwrap();
    let out = read_utf8_path_capped(&tmp_output).unwrap();
    let count = out
        .lines()
        .filter(|l| l.contains("vox_submit_task"))
        .count();
    assert!(
        count >= min,
        "expected >={min} phrases for vox_submit_task, got {count}"
    );
}

#[test]
fn schema_version_field_present_on_every_row() {
    let out = run_to_string(&default_cfg());
    for line in out.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        assert!(
            v.get("schema_version").is_some(),
            "missing schema_version on line: {line}"
        );
    }
}

#[test]
fn workflow_pairs_contain_vox_snippet_in_response() {
    let cfg = SyntheticGenConfig {
        emit_tool_traces: false,
        emit_a2a_traces: false,
        emit_orchestrator_rows: false,
        emit_skill_rows: false,
        emit_agent_rows: false,
        ..Default::default()
    };
    let out = run_to_string(&cfg);
    let has_workflow_snippet = out.lines().any(|line| {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
            v.get("response")
                .and_then(|r| r.as_str())
                .map(|r| r.contains("workflow"))
                .unwrap_or(false)
        } else {
            false
        }
    });
    assert!(
        has_workflow_snippet,
        "expected at least one workflow response containing 'workflow'"
    );
}

#[test]
fn tool_registry_slim_all_entries_in_orchestrator_tools_are_subset() {
    for &name in ORCHESTRATOR_TOOLS {
        assert!(
            TOOL_REGISTRY_SLIM.contains(&name),
            "ORCHESTRATOR_TOOLS entry {name} not in TOOL_REGISTRY_SLIM"
        );
    }
}

#[test]
fn coverage_analyse_str_with_custom_taxonomy() {
    let jsonl = r#"{"prompt":"x","response":"y","category":"vox_submit_task","schema_version":"vox_dogfood_v1"}"#;
    let taxonomy = &["vox_submit_task", "vox_task_status"];
    let report = analyse_str_with_taxonomy(jsonl, 1, taxonomy);
    assert_eq!(report.covered_types, 1);
    assert_eq!(report.missing_types, vec!["vox_task_status".to_string()]);
}
