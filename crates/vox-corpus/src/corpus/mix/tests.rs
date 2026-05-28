use super::*;

#[test]
fn asr_refine_normalizes_to_training_pair_shape() {
    let raw = r#"{"noisy_text":"hello  wrld","corrected_text":"hello world","rating":4}"#;
    let out = normalize_training_jsonl_line(raw, Some("asr_refine")).expect("ok");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let prompt = v["prompt"].as_str().unwrap();
    assert!(prompt.contains("hello  wrld"));
    assert!(prompt.starts_with("Correct the following noisy"));
    assert_eq!(v["response"].as_str(), Some("hello world"));
    assert_eq!(v["rating"].as_u64(), Some(4));
    assert_eq!(v["category"].as_str(), Some("asr_refine"));
}

#[test]
fn passthrough_without_format() {
    let raw = r#"{"prompt":"a","response":"b"}"#;
    let out = normalize_training_jsonl_line(raw, None).unwrap();
    assert_eq!(out, raw);
}

#[test]
fn tool_trace_normalizes_to_training_pair_shape() {
    let raw = r#"{"task_prompt":"Run fmt","tool_name":"shell","arguments_json":"{\"cmd\":\"cargo fmt\"}","result_json":"{\"ok\":true}","success":true}"#;
    let out = normalize_training_jsonl_line(raw, Some("tool_trace")).expect("ok");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(
        v["prompt"]
            .as_str()
            .unwrap()
            .contains("[vox_tool_supervision]")
    );
    assert!(v["prompt"].as_str().unwrap().contains("Run fmt"));
    let resp = v["response"].as_str().unwrap();
    assert!(resp.contains("shell"));
    assert!(resp.contains("cargo fmt"));
    assert_eq!(v["category"].as_str(), Some("tool_trace"));
}

#[test]
fn tool_trace_uses_followup_when_present() {
    let raw = r#"{"task_prompt":"x","tool_name":"t","arguments_json":"{}","result_json":"{}","success":true,"followup_text":"Ready."}"#;
    let out = normalize_training_jsonl_line(raw, Some("tool_trace")).unwrap();
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["response"].as_str(), Some("Ready."));
}

#[test]
fn speech_to_code_normalizes_to_training_pair_shape() {
    let raw =
        r#"{"refined_transcript":"add a hello function","vox_code":"fn hello() { }","rating":5}"#;
    let out = normalize_training_jsonl_line(raw, Some("speech_to_code")).expect("ok");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let prompt = v["prompt"].as_str().unwrap();
    assert!(prompt.contains("add a hello function"));
    assert!(prompt.starts_with("Given the following spoken"));
    assert_eq!(v["response"].as_str(), Some("fn hello() { }"));
    assert_eq!(v["rating"].as_u64(), Some(5));
    assert_eq!(v["category"].as_str(), Some("speech_to_code"));
}

#[test]
fn speech_to_code_preserves_diagnostics_snapshot() {
    let raw = r#"{"refined_transcript":"fix typo","vox_code":"fn x() { }","diagnostics_snapshot":[{"message":"bad","code":"E001","severity":"error"}]}"#;
    let out = normalize_training_jsonl_line(raw, Some("speech_to_code")).expect("ok");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let snap = v["diagnostics_snapshot"].as_array().expect("snapshot");
    assert_eq!(snap.len(), 1);
    assert_eq!(snap[0]["code"].as_str(), Some("E001"));
}

#[test]
fn strict_rejects_missing_required_source() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cfg_path = dir.path().join("mix.yaml");
    let absent = dir.path().join("nope.jsonl");
    let out_j = dir.path().join("out.jsonl");
    let p_abs = absent.to_string_lossy().replace('\\', "/");
    let p_out = out_j.to_string_lossy().replace('\\', "/");
    std::fs::write(
        &cfg_path,
        format!("sources:\n  - path: \"{p_abs}\"\n    weight: 1\noutput: \"{p_out}\"\n"),
    )
    .unwrap();
    let err = run_mix_with_options(
        &cfg_path,
        None,
        MixRunOptions {
            strict: true,
            write_report: false,
        },
    )
    .expect_err("strict missing");
    let s = format!("{err:#}");
    assert!(s.contains("strict") || s.contains("missing"), "{s}");
}

#[test]
fn incremental_skip_works() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cfg_path = dir.path().join("mix.yaml");
    let src_path = dir.path().join("src.jsonl");
    let out_path = dir.path().join("out.jsonl");

    std::fs::write(&src_path, r#"{"prompt":"a","response":"b"}"#).unwrap();

    let p_src = src_path.to_string_lossy().replace('\\', "/");
    let p_out = out_path.to_string_lossy().replace('\\', "/");

    std::fs::write(
        &cfg_path,
        format!("sources:\n  - path: \"{p_src}\"\n    weight: 1\noutput: \"{p_out}\"\n"),
    )
    .unwrap();

    // First run — produces report and output
    run_mix_with_options(
        &cfg_path,
        None,
        MixRunOptions {
            strict: true,
            write_report: true,
        },
    )
    .expect("first run");
    assert!(out_path.is_file());
    assert!(dir.path().join("out.mix_report.json").is_file());

    // Measure time for second run (should be skip)
    let start = std::time::Instant::now();
    run_mix_with_options(
        &cfg_path,
        None,
        MixRunOptions {
            strict: true,
            write_report: true,
        },
    )
    .expect("second run");
    let elapsed = start.elapsed();

    // On a fast system, a skip should be < 10ms for a tiny file, but even on CI it should be very fast.
    assert!(
        elapsed.as_millis() < 500,
        "skip took {}ms",
        elapsed.as_millis()
    );
}

#[test]
fn mix_emits_once_with_weight_stamp_by_default() {
    // Default semantics (post-2026-05-23 fix): weight is stamped onto each
    // row as `mix_weight` and each row is emitted exactly once. Previously
    // weight=2 produced 2 literal copies, which destroyed uniqueness for
    // SFT corpora (audit measured 99.2% duplication on 6× weight).
    let dir = tempfile::tempdir().expect("tempdir");
    let cfg_path = dir.path().join("mix.yaml");
    let s1_path = dir.path().join("s1.jsonl");
    let s2_path = dir.path().join("s2.jsonl");
    let out_path = dir.path().join("out.jsonl");

    std::fs::write(
        &s1_path,
        "{\"lane\":\"vox_codegen\",\"prompt\":\"s1\",\"response\":\"r1\"}\n",
    )
    .unwrap();
    std::fs::write(
        &s2_path,
        "{\"lane\":\"vox_codegen\",\"prompt\":\"s2\",\"response\":\"r2\"}\n",
    )
    .unwrap();

    let p1 = s1_path.to_string_lossy().replace('\\', "/");
    let p2 = s2_path.to_string_lossy().replace('\\', "/");
    let p_out = out_path.to_string_lossy().replace('\\', "/");

    std::fs::write(
        &cfg_path,
        format!("sources:\n  - path: \"{p1}\"\n    weight: 2\n  - path: \"{p2}\"\n    weight: 1\noutput: \"{p_out}\"\ninclude_lanes: [\"vox_codegen\"]\n"),
    ).unwrap();

    run_mix_with_options(
        &cfg_path,
        None,
        MixRunOptions {
            strict: true,
            write_report: true,
        },
    )
    .expect("run");

    let mixed = std::fs::read_to_string(&out_path).unwrap();
    let lines: Vec<&str> = mixed.lines().filter(|l| !l.is_empty()).collect();

    // New semantics: each source emits once → 2 lines, not 3.
    assert_eq!(lines.len(), 2, "expected emit-once: got {lines:#?}");
    let l0: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    let l1: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
    assert_eq!(l0["prompt"].as_str(), Some("s1"));
    assert_eq!(l1["prompt"].as_str(), Some("s2"));
    // weight=2 is stamped; weight=1 (default) is omitted to keep diffs minimal.
    assert_eq!(l0["mix_weight"].as_f64(), Some(2.0));
    assert!(
        l1.get("mix_weight").is_none() || l1["mix_weight"].as_f64() == Some(1.0),
        "weight=1 should be no-op or 1.0, got {}",
        l1["mix_weight"]
    );
}

#[test]
fn mix_physical_repeats_opt_in_restores_legacy_duplication() {
    // Setting `physical_repeats: true` per-source restores the pre-fix
    // behavior: weight=N emits each row N times. Kept as an escape hatch
    // for downstream consumers that don't honor `mix_weight`.
    let dir = tempfile::tempdir().expect("tempdir");
    let cfg_path = dir.path().join("mix.yaml");
    let s1_path = dir.path().join("s1.jsonl");
    let out_path = dir.path().join("out.jsonl");
    std::fs::write(
        &s1_path,
        "{\"lane\":\"vox_codegen\",\"prompt\":\"s1\",\"response\":\"r1\"}\n",
    )
    .unwrap();
    let p1 = s1_path.to_string_lossy().replace('\\', "/");
    let p_out = out_path.to_string_lossy().replace('\\', "/");
    std::fs::write(
        &cfg_path,
        format!("sources:\n  - path: \"{p1}\"\n    weight: 3\n    physical_repeats: true\noutput: \"{p_out}\"\ninclude_lanes: [\"vox_codegen\"]\n"),
    )
    .unwrap();
    run_mix_with_options(
        &cfg_path,
        None,
        MixRunOptions {
            strict: true,
            write_report: true,
        },
    )
    .expect("run");
    let mixed = std::fs::read_to_string(&out_path).unwrap();
    let n = mixed.lines().filter(|l| !l.is_empty()).count();
    assert_eq!(n, 3, "physical_repeats:true with weight=3 → 3 lines");
}

#[test]
fn lane_override_promotes_documentation_category_above_generic_codegen() {
    // Regression test for 2026-05-23 audit: validated_mixed.jsonl tags
    // every row `lane: vox_codegen` regardless of its category, so an
    // `include_lanes: [vox_docs_qa]` filter never emitted documentation
    // rows. The fix: when the row's lane is the generic catch-all
    // (`vox_codegen`) AND the category-derived lane is more specific
    // (e.g. `documentation` → `vox_docs_qa`), promote the specific lane.
    let raw = r#"{"lane":"vox_codegen","category":"documentation","prompt":"p","response":"r"}"#;
    let (out, lane) = enrich_lane_metadata(raw).expect("enrich ok");
    assert_eq!(lane, "vox_docs_qa", "specific category should win");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["lane"].as_str(), Some("vox_docs_qa"));
}

#[test]
fn lane_override_preserves_explicit_non_default_lane() {
    // If the row already has an explicit non-default lane (e.g. an
    // upstream tagger really meant `vox_tooling`), trust it even if the
    // category would also imply something else.
    let raw = r#"{"lane":"vox_tooling","category":"documentation","prompt":"p","response":"r"}"#;
    let (_out, lane) = enrich_lane_metadata(raw).expect("enrich ok");
    assert_eq!(lane, "vox_tooling", "explicit specific lane wins");
}

#[test]
fn stamp_mix_weight_skips_default_weight() {
    let raw = r#"{"prompt":"p","response":"r"}"#;
    // weight=1.0 → no-op, returns the input unchanged.
    assert_eq!(stamp_mix_weight(raw, 1.0).unwrap(), raw);
    // weight=2.5 → adds the field.
    let out = stamp_mix_weight(raw, 2.5).unwrap();
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["mix_weight"].as_f64(), Some(2.5));
}
