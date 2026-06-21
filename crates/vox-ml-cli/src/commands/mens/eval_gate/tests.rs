use super::check_run::check_run;
use super::policy::EvalGatePolicy;

fn make_policy_with_per_context() -> EvalGatePolicy {
    let yaml = r#"
version: "1"
per_context:
  target:
    min_parse_rate: 0.80
    min_scope_compliance_rate: 0.95
    block: true
  meta:
    min_parse_rate: 0.30
    block: false
modal_mix:
  max_voice_fraction: 0.30
  block: false
"#;
    serde_yaml::from_str(yaml).expect("parse test policy")
}

#[test]
fn per_context_gate_passes_when_above_threshold() {
    let dir = tempfile::tempdir().expect("tempdir");

    // Write eval_results.json with target slice above threshold
    std::fs::write(
        dir.path().join("eval_results.json"),
        r#"{
                "vox_parse_rate": 0.9,
                "context_breakdown": {
                    "target": { "parse_rate": 0.90, "scope_compliance_rate": 0.97 },
                    "meta":   { "parse_rate": 0.35, "scope_compliance_rate": 0.60 }
                }
            }"#,
    )
    .unwrap();

    let results = check_run(dir.path(), &{
        let p = dir.path().join("policy.yaml");
        let yaml = r#"
version: "1"
per_context:
  target:
    min_parse_rate: 0.80
    min_scope_compliance_rate: 0.95
    block: true
  meta:
    min_parse_rate: 0.30
    block: false
"#;
        std::fs::write(&p, yaml).unwrap();
        p
    })
    .expect("check_run");

    let target_gate = results.iter().find(|r| r.name == "per_context[target]");
    assert!(
        target_gate.is_some(),
        "per_context[target] gate should be present"
    );
    assert!(
        target_gate.unwrap().passed,
        "target gate should pass at 90% parse"
    );

    let meta_gate = results.iter().find(|r| r.name == "per_context[meta]");
    assert!(
        meta_gate.is_some(),
        "per_context[meta] gate should be present"
    );
    assert!(
        meta_gate.unwrap().passed,
        "meta gate should pass at 35% parse"
    );
}

#[test]
fn per_context_gate_fails_blocking_when_below_threshold() {
    let dir = tempfile::tempdir().expect("tempdir");
    // target parse_rate below 0.80 threshold
    std::fs::write(
        dir.path().join("eval_results.json"),
        r#"{"vox_parse_rate":0.5,"context_breakdown":{"target":{"parse_rate":0.50,"scope_compliance_rate":0.99}}}"#,
    )
    .unwrap();
    let policy_path = dir.path().join("policy.yaml");
    std::fs::write(
        &policy_path,
        "version: \"1\"\nper_context:\n  target:\n    min_parse_rate: 0.80\n    block: true\n",
    )
    .unwrap();
    let results = check_run(dir.path(), &policy_path).expect("check_run");
    let gate = results
        .iter()
        .find(|r| r.name == "per_context[target]")
        .expect("gate present");
    assert!(!gate.passed, "target gate should fail below threshold");
    assert!(gate.block, "target gate should be blocking");
}

#[test]
fn modal_mix_gate_passes_when_voice_below_ceiling() {
    let _ = make_policy_with_per_context(); // verify policy parses
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("eval_results.json"),
        r#"{"modal_breakdown":{"text":90,"voice":5}}"#,
    )
    .unwrap();
    let policy_path = dir.path().join("policy.yaml");
    std::fs::write(
        &policy_path,
        "version: \"1\"\nmodal_mix:\n  max_voice_fraction: 0.30\n  block: false\n",
    )
    .unwrap();
    let results = check_run(dir.path(), &policy_path).expect("check_run");
    let gate = results.iter().find(|r| r.name == "modal_mix[voice]");
    assert!(gate.is_some(), "modal_mix[voice] gate present");
    assert!(
        gate.unwrap().passed,
        "5/95 = 5.3% voice < 30% ceiling should pass"
    );
}

#[test]
fn modal_mix_gate_warns_when_voice_exceeds_ceiling() {
    let dir = tempfile::tempdir().expect("tempdir");
    // 40% voice exceeds 30% ceiling
    std::fs::write(
        dir.path().join("eval_results.json"),
        r#"{"modal_breakdown":{"text":60,"voice":40}}"#,
    )
    .unwrap();
    let policy_path = dir.path().join("policy.yaml");
    std::fs::write(
        &policy_path,
        "version: \"1\"\nmodal_mix:\n  max_voice_fraction: 0.30\n  block: false\n",
    )
    .unwrap();
    let results = check_run(dir.path(), &policy_path).expect("check_run");
    let gate = results
        .iter()
        .find(|r| r.name == "modal_mix[voice]")
        .expect("gate present");
    assert!(!gate.passed, "40% voice > 30% ceiling should not pass");
    assert!(!gate.block, "warn-only gate should not block");
}

#[test]
fn policy_deserializes_per_context_and_modal_mix() {
    let policy = make_policy_with_per_context();
    assert!(policy.per_context.contains_key("target"));
    assert!(policy.per_context.contains_key("meta"));
    assert_eq!(policy.per_context["target"].min_parse_rate, 0.80);
    assert!(policy.per_context["target"].block);
    assert!(!policy.per_context["meta"].block);
    assert_eq!(policy.modal_mix.max_voice_fraction, 0.30);
    assert!(!policy.modal_mix.block);
}

#[test]
fn mcp_tool_schema_gate_passes_at_threshold() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("mcp_tool_schema_kpi.json"),
        r#"{"validation_enabled":true,"checks_total":100,"strict_passed":99,"strict_failed":1,"schema_compile_skipped":0,"strict_validity_rate":0.99}"#,
    )
    .unwrap();
    let policy_path = dir.path().join("policy.yaml");
    std::fs::write(
        &policy_path,
        r#"version: "1"
mcp_tool_schema:
  min_strict_validity_rate: 0.99
  block: true
"#,
    )
    .unwrap();
    let results = check_run(dir.path(), &policy_path).expect("check_run");
    let g = results
        .iter()
        .find(|r| r.name == "mcp_tool_schema")
        .expect("gate");
    assert!(g.passed, "{}", g.message);
}

#[test]
fn mcp_tool_schema_gate_fails_below_threshold() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("mcp_tool_schema_kpi.json"),
        r#"{"validation_enabled":true,"checks_total":10,"strict_passed":5,"strict_failed":5,"strict_validity_rate":0.5}"#,
    )
    .unwrap();
    let policy_path = dir.path().join("policy.yaml");
    std::fs::write(
        &policy_path,
        r#"version: "1"
mcp_tool_schema:
  min_strict_validity_rate: 0.99
  block: true
"#,
    )
    .unwrap();
    let results = check_run(dir.path(), &policy_path).expect("check_run");
    let g = results
        .iter()
        .find(|r| r.name == "mcp_tool_schema")
        .expect("gate");
    assert!(!g.passed);
}

#[test]
fn mcp_tool_schema_gate_skipped_when_inactive() {
    let dir = tempfile::tempdir().expect("tempdir");
    let policy_path = dir.path().join("policy.yaml");
    std::fs::write(
        &policy_path,
        r#"version: "1"
mcp_tool_schema:
  min_strict_validity_rate: 0.0
  block: false
"#,
    )
    .unwrap();
    let results = check_run(dir.path(), &policy_path).expect("check_run");
    assert!(
        !results.iter().any(|r| r.name == "mcp_tool_schema"),
        "inactive gate should not emit a row"
    );
}

#[test]
fn pass_at_k_gate_passes_when_above_thresholds() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("benchmark_passatk.json"),
        r#"{"pass_rate_at_1":0.71,"pass_rate_at_k":0.88,"k":4}"#,
    )
    .unwrap();
    let policy_path = dir.path().join("policy.yaml");
    std::fs::write(
        &policy_path,
        r#"version: "1"
pass_at_k:
  min_pass_rate_at_1: 0.70
  min_pass_rate_at_k: 0.85
  block: true
"#,
    )
    .unwrap();
    let results = check_run(dir.path(), &policy_path).expect("check_run");
    let g = results
        .iter()
        .find(|r| r.name == "pass_at_k")
        .expect("gate");
    assert!(g.passed, "{}", g.message);
}

#[test]
fn anti_stub_gate_passes_when_eval_local_report_meets_thresholds() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("eval_local_report.json"),
        r#"{
            "anti_stub_task_success": 0.95,
            "placeholder_event_rate": 0.03,
            "trivial_placeholder_event_rate": 0.02,
            "construct_richness_mean": 0.45
        }"#,
    )
    .unwrap();
    let policy_path = dir.path().join("policy.yaml");
    std::fs::write(
        &policy_path,
        r#"version: "1"
anti_stub:
  min_pass_rate: 0.92
  max_placeholder_event_rate: 0.08
  max_trivial_placeholder_event_rate: 0.08
  min_construct_richness_mean: 0.20
  block: true
"#,
    )
    .unwrap();
    let results = check_run(dir.path(), &policy_path).expect("check_run");
    let g = results
        .iter()
        .find(|r| r.name == "anti_stub")
        .expect("anti_stub gate present");
    assert!(
        g.passed,
        "should pass: all metrics above thresholds. msg={}",
        g.message
    );
    assert!(g.block, "block flag from policy");
}

#[test]
fn anti_stub_gate_fails_blocking_when_pass_rate_below_threshold() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("eval_local_report.json"),
        r#"{
            "anti_stub_task_success": 0.80,
            "placeholder_event_rate": 0.03,
            "trivial_placeholder_event_rate": 0.02,
            "construct_richness_mean": 0.45
        }"#,
    )
    .unwrap();
    let policy_path = dir.path().join("policy.yaml");
    std::fs::write(
        &policy_path,
        r#"version: "1"
anti_stub:
  min_pass_rate: 0.92
  max_placeholder_event_rate: 0.08
  max_trivial_placeholder_event_rate: 0.08
  min_construct_richness_mean: 0.20
  block: true
"#,
    )
    .unwrap();
    let results = check_run(dir.path(), &policy_path).expect("check_run");
    let g = results
        .iter()
        .find(|r| r.name == "anti_stub")
        .expect("anti_stub gate present");
    assert!(!g.passed, "0.80 < 0.92 should fail");
    assert!(g.block, "block: true in policy");
}

#[test]
fn anti_stub_gate_warn_only_when_eval_local_report_missing() {
    let dir = tempfile::tempdir().expect("tempdir");
    // No eval_local_report.json written — simulates post-train pipeline state before eval-local
    let policy_path = dir.path().join("policy.yaml");
    std::fs::write(
        &policy_path,
        r#"version: "1"
anti_stub:
  min_pass_rate: 0.92
  max_placeholder_event_rate: 0.08
  max_trivial_placeholder_event_rate: 0.08
  min_construct_richness_mean: 0.20
  block: false
"#,
    )
    .unwrap();
    let results = check_run(dir.path(), &policy_path).expect("check_run");
    let g = results
        .iter()
        .find(|r| r.name == "anti_stub")
        .expect("anti_stub gate present");
    assert!(!g.passed, "missing file → gate fails");
    assert!(!g.block, "block: false → warning, not hard-block");
}

#[test]
fn anti_stub_gate_hard_blocks_when_full_policy_and_eval_local_report_missing() {
    let dir = tempfile::tempdir().expect("tempdir");
    // No eval_local_report.json — simulates full gate run before eval-local is run by operator
    let policy_path = dir.path().join("policy.yaml");
    std::fs::write(
        &policy_path,
        r#"version: "1"
anti_stub:
  min_pass_rate: 0.92
  block: true
"#,
    )
    .unwrap();
    let results = check_run(dir.path(), &policy_path).expect("check_run");
    let g = results
        .iter()
        .find(|r| r.name == "anti_stub")
        .expect("anti_stub gate present");
    assert!(!g.passed, "missing file → gate fails");
    assert!(g.block, "block: true → hard-block");
    assert!(
        g.message.contains("eval_local_report.json"),
        "message should name the expected file"
    );
}

#[test]
fn run_eval_gate_writes_gate_receipt_json_on_pass() {
    use super::run_gate::run_eval_gate;
    let dir = tempfile::tempdir().expect("tempdir");

    // Write enough artifacts for a clean pass (no trainer artifacts → trainer gates skip)
    // Write eval_local_report.json so anti_stub gate can read fields
    std::fs::write(
        dir.path().join("eval_local_report.json"),
        r#"{"anti_stub_task_success":0.95,"placeholder_event_rate":0.02,"trivial_placeholder_event_rate":0.01,"construct_richness_mean":0.40}"#,
    )
    .unwrap();
    let policy_path = dir.path().join("policy.yaml");
    std::fs::write(
        &policy_path,
        r#"version: "1"
anti_stub:
  min_pass_rate: 0.92
  max_placeholder_event_rate: 0.08
  max_trivial_placeholder_event_rate: 0.08
  min_construct_richness_mean: 0.20
  block: true
"#,
    )
    .unwrap();

    let code = run_eval_gate(dir.path().to_path_buf(), Some(policy_path)).expect("run");
    assert_eq!(code, 0, "gate should pass");

    let receipt_path = dir.path().join("gate_receipt.json");
    assert!(receipt_path.exists(), "gate_receipt.json must be written");
    let text = std::fs::read_to_string(&receipt_path).unwrap();
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["schema"], "vox_mens_gate_receipt_v1");
    assert!(
        v["overall_passed"].as_bool().unwrap_or(false),
        "overall_passed should be true"
    );
    let gates = v["gates"].as_array().expect("gates array");
    assert!(!gates.is_empty(), "gates list should not be empty");
}

#[test]
fn run_eval_gate_writes_gate_receipt_json_on_fail() {
    use super::run_gate::run_eval_gate;
    let dir = tempfile::tempdir().expect("tempdir");
    // eval_local_report.json with bad metrics → anti_stub fails blocking
    std::fs::write(
        dir.path().join("eval_local_report.json"),
        r#"{"anti_stub_task_success":0.50}"#,
    )
    .unwrap();
    let policy_path = dir.path().join("policy.yaml");
    std::fs::write(
        &policy_path,
        r#"version: "1"
anti_stub:
  min_pass_rate: 0.92
  block: true
"#,
    )
    .unwrap();
    let code = run_eval_gate(dir.path().to_path_buf(), Some(policy_path)).expect("run");
    assert_eq!(code, 1, "gate should fail");
    let v: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.path().join("gate_receipt.json")).unwrap(),
    )
    .unwrap();
    assert!(
        !v["overall_passed"].as_bool().unwrap_or(true),
        "overall_passed false on fail"
    );
}

#[test]
fn pass_at_k_gate_fails_on_regression_drop() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("benchmark_passatk.json"),
        r#"{"pass_rate_at_1":0.50,"pass_rate_at_k":0.70}"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("baseline_passatk.json"),
        r#"{"pass_rate_at_1":0.62,"pass_rate_at_k":0.82}"#,
    )
    .unwrap();
    let policy_path = dir.path().join("policy.yaml");
    std::fs::write(
        &policy_path,
        r#"version: "1"
pass_at_k:
  max_regression_drop: 0.05
  baseline_file: baseline_passatk.json
  block: true
"#,
    )
    .unwrap();
    let results = check_run(dir.path(), &policy_path).expect("check_run");
    let g = results
        .iter()
        .find(|r| r.name == "pass_at_k")
        .expect("gate");
    assert!(!g.passed, "{}", g.message);
}

#[test]
fn rust_compile_and_clippy_gates_work() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("eval_results.json"),
        r#"{"rust_compile_rate":0.95,"clippy_clean_rate":0.90}"#,
    )
    .unwrap();
    let policy_path = dir.path().join("policy.yaml");
    std::fs::write(
        &policy_path,
        r#"version: "1"
rust_compile_rate:
  min_pct: 0.90
  block: true
clippy_clean_rate:
  min_pct: 0.85
  block: false
"#,
    )
    .unwrap();
    let results = check_run(dir.path(), &policy_path).expect("check_run");

    let compile_gate = results
        .iter()
        .find(|r| r.name == "rust_compile_rate")
        .expect("compile gate present");
    assert!(compile_gate.passed, "should pass");
    assert!(compile_gate.block, "should be blocking");

    let clippy_gate = results
        .iter()
        .find(|r| r.name == "clippy_clean_rate")
        .expect("clippy gate present");
    assert!(clippy_gate.passed, "should pass");
    assert!(!clippy_gate.block, "should not be blocking");
}
