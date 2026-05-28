use super::*;
use crate::publication_worthiness::WorthinessInputs;

#[test]
fn infer_doc_sections_skips_yaml_frontmatter() {
    let md = "---\ntitle: Ignored\n---\n\n## First\nbody\n### Nested\n";
    let hints = markdown::infer_doc_sections_from_markdown(md);
    assert_eq!(hints.len(), 2);
    assert_eq!(hints[0].title, "First");
    assert_eq!(hints[0].heading_level, 2);
    assert_eq!(hints[1].title, "Nested");
    assert_eq!(hints[1].heading_level, 3);
}

#[test]
fn evidence_bumps_epistemic_when_socrates_clean() {
    let evidence = ScientiaEvidenceContext {
        socrates_aggregate: Some(SocratesAggregateSnapshot {
            sample_size: 12,
            parsed_metadata_rows: 10,
            mean_hallucination_risk_proxy: 0.08,
            mean_confidence_estimate: 0.82,
            mean_contradiction_ratio: 0.05,
            answer_count: 8,
            ask_count: 2,
            abstain_count: 0,
        }),
        eval_gate: Some(EvalGateSnapshot {
            passed: true,
            gates_failed: 0,
            gates_total: 3,
        }),
        benchmark: Some(BenchmarkPairSnapshot {
            baseline_run_id: Some("b1".into()),
            candidate_run_id: Some("c1".into()),
            manifest_repo_relative: Some("contracts/eval/benchmark-matrix.json".into()),
            pair_complete: true,
        }),
        discovery_signals: Vec::new(),
        draft_preparation: None,
        candidate_note: None,
        eval_gate_run_dir_repo_relative: None,
        eval_gate_report_repo_relative: None,
        benchmark_pair_report_repo_relative: None,
        human_meaningful_advance: true,
        human_ai_disclosure_complete: true,
        ..Default::default()
    };
    let base = WorthinessInputs {
        red_line_violation_ids: vec![],
        repeated_unresolved_contradiction: false,
        claim_evidence_coverage: 0.92,
        artifact_replayability: 0.88,
        artifact_replayability_measured: None,
        before_after_pair_integrity: 0.5,
        metadata_completeness: 0.9,
        ai_disclosure_compliance: 0.85,
        epistemic: 0.55,
        reproducibility: 0.7,
        novelty: 0.6,
        reliability: 0.6,
        metadata_policy: 0.75,
        meaningful_advance: false,
    };
    let merged = worthiness::apply_scientia_evidence(
        base,
        &evidence,
        &crate::scientia_heuristics::ScientiaHeuristics::default(),
    );
    assert!(merged.meaningful_advance);
    assert_eq!(merged.ai_disclosure_compliance, 1.0);
    assert!(merged.epistemic > 0.65);
    assert!(merged.before_after_pair_integrity >= 0.88);
}

#[test]
fn g2_low_citation_coverage_skips_contradiction_epistemic_shrink() {
    let h = crate::scientia_heuristics::ScientiaHeuristics {
        worthiness_contradiction_coverage_gate: 0.3,
        ..crate::scientia_heuristics::ScientiaHeuristics::default()
    };
    let agg_noisy = SocratesAggregateSnapshot {
        sample_size: 20,
        parsed_metadata_rows: 15,
        mean_hallucination_risk_proxy: 0.1,
        mean_confidence_estimate: 0.8,
        mean_contradiction_ratio: 0.9,
        answer_count: 10,
        ask_count: 5,
        abstain_count: 0,
    };
    let evidence_noisy = ScientiaEvidenceContext {
        socrates_aggregate: Some(agg_noisy.clone()),
        ..Default::default()
    };
    let mut agg_clean = agg_noisy.clone();
    agg_clean.mean_contradiction_ratio = 0.0;
    let evidence_clean = ScientiaEvidenceContext {
        socrates_aggregate: Some(agg_clean),
        ..Default::default()
    };
    let base_low_cov = WorthinessInputs {
        red_line_violation_ids: vec![],
        repeated_unresolved_contradiction: false,
        claim_evidence_coverage: 0.1,
        artifact_replayability: 0.5,
        artifact_replayability_measured: None,
        before_after_pair_integrity: 0.5,
        metadata_completeness: 0.5,
        ai_disclosure_compliance: 1.0,
        epistemic: 0.5,
        reproducibility: 0.5,
        novelty: 0.5,
        reliability: 0.5,
        metadata_policy: 0.5,
        meaningful_advance: false,
    };
    let noisy = worthiness::apply_scientia_evidence(base_low_cov.clone(), &evidence_noisy, &h);
    let clean = worthiness::apply_scientia_evidence(base_low_cov, &evidence_clean, &h);
    assert!(
        (noisy.epistemic - clean.epistemic).abs() < 1e-9,
        "below contradiction_coverage_gate, high vs zero contradiction_ratio must not change epistemic (got noisy={} clean={})",
        noisy.epistemic,
        clean.epistemic
    );
}

#[test]
fn file_hydration_inlines_eval_gate_from_repo_relative_path() {
    let dir = tempfile::tempdir().unwrap();
    let report_path = dir.path().join("reports/eval_gate.json");
    std::fs::create_dir_all(report_path.parent().unwrap()).unwrap();
    std::fs::write(
        &report_path,
        r#"{"passed":true,"gates_failed":0,"gates_total":4}"#,
    )
    .unwrap();
    let meta = r#"{"repository_id":"r1","scientia_evidence":{"eval_gate_report_repo_relative":"reports/eval_gate.json"}}"#;
    let out = enrich_metadata_json_with_repo_files(Some(meta), dir.path())
        .unwrap()
        .unwrap();
    let ev = parse_scientia_evidence(Some(&out)).expect("evidence");
    let g = ev.eval_gate.as_ref().unwrap();
    assert!(g.passed);
    assert_eq!(g.gates_total, 4);
}

#[test]
fn file_hydration_skips_when_sidecar_missing() {
    let dir = tempfile::tempdir().unwrap();
    let meta = r#"{"scientia_evidence":{"eval_gate_report_repo_relative":"nope/missing.json"}}"#;
    assert!(
        enrich_metadata_json_with_repo_files(Some(meta), dir.path())
            .unwrap()
            .is_none()
    );
}

#[test]
fn candidate_context_defaults_capture_discovery_signals_and_prep() {
    let scientific = ScientificPublicationMetadata::default();
    let mut evidence = ScientiaEvidenceContext {
        eval_gate: Some(EvalGateSnapshot {
            passed: true,
            gates_failed: 0,
            gates_total: 5,
        }),
        benchmark: Some(BenchmarkPairSnapshot {
            baseline_run_id: Some("baseline-1".into()),
            candidate_run_id: Some("candidate-1".into()),
            manifest_repo_relative: Some("reports/bench.json".into()),
            pair_complete: true,
        }),
        human_meaningful_advance: true,
        ..Default::default()
    };
    populate_candidate_context_defaults(
        Some("docs/src/adr/013-openclaw-ws-native-strategy.md"),
        None,
        None,
        Some(&scientific),
        &mut evidence,
    );
    assert!(
        evidence
            .discovery_signals
            .iter()
            .any(|s| s.code == "eval_gate_passed"
                && s.strength == signals::DiscoverySignalStrength::Strong)
    );
    assert!(
        evidence
            .discovery_signals
            .iter()
            .any(|s| s.code == "adr_writeup_present")
    );
    let prep = evidence.draft_preparation.as_ref().expect("draft prep");
    assert!(prep.abstract_needed);
    assert!(prep.citations_needed);
    assert!(prep.reproducibility_details_needed);
    assert_eq!(
        prep.recommended_scholarly_venue.as_deref(),
        Some("arxiv_assist")
    );
    assert!(
        evidence
            .candidate_note
            .as_ref()
            .is_some_and(|n| n.contains("structured signals"))
    );
}
