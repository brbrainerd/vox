//! Regenerator for the weak_test gate's grandfather baseline.
//!
//! `weak_test` is a high-recall ADVISORY heuristic: many of the current findings are
//! false positives (tests asserting via helper fns that panic internally, multi-line
//! macros) or pre-existing shallow tests. The weak_test gate is therefore a
//! *no-NEW-touch-tests* guard — most valuable as a forward check on Phase-3 coverage
//! waves. This regenerator freezes the current findings into
//! `contracts/toestub/weak-test-baseline.v1.json` so the gate passes today and trips
//! only on touch-tests the baseline does not already cover.
//!
//! Ignored by default. Refresh after intentionally accepting or fixing findings:
//!   cargo test -p vox-audit --test regen_weak_test_baseline -- --ignored --nocapture
use std::path::PathBuf;
use vox_audit::core_gates::run_weak_test_gate;
use vox_code_audit::run_context::ToestubTestsMode;
use vox_code_audit::{OutputFormat, Severity, ToestubConfig, ToestubEngine, ToestubRunMode};

fn crates_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

#[test]
#[ignore = "regenerator: writes contracts/toestub/weak-test-baseline.v1.json"]
fn regenerate_weak_test_baseline() {
    let crates = crates_root();
    let repo = crates.parent().unwrap().to_path_buf();
    let config = ToestubConfig {
        roots: vec![crates.clone()],
        min_severity: Severity::Info,
        run_mode: ToestubRunMode::Audit,
        rule_filter: Some(vec!["weak_test".to_string()]),
        tests_mode: ToestubTestsMode::Include,
        format: OutputFormat::Json,
        ..ToestubConfig::default()
    };
    let (result, _) = ToestubEngine::new(config).run_and_report();

    let mut entries: Vec<serde_json::Value> = result
        .findings
        .iter()
        .filter(|f| f.rule_id == "weak_test")
        .map(|f| {
            let p = f.file.to_string_lossy().replace('\\', "/");
            let rel = p.rsplit_once("crates/").map(|(_, r)| r).unwrap_or(p.as_str());
            serde_json::json!({
                "rule_id_prefix": "weak_test",
                "path_glob": format!("**/crates/{rel}"),
                "line": f.line,
                "reason": "Grandfathered advisory weak/touch-test; gate guards NEW touch-tests only.",
                "owner": "robot"
            })
        })
        .collect();
    entries.sort_by(|a, b| {
        (
            a["path_glob"].as_str().unwrap(),
            a["line"].as_u64().unwrap_or(0),
        )
            .cmp(&(
                b["path_glob"].as_str().unwrap(),
                b["line"].as_u64().unwrap_or(0),
            ))
    });

    let n = entries.len();
    let doc = serde_json::json!({ "x-vox-version": 1, "version": 1, "suppressions": entries });
    let out = repo.join("contracts/toestub/weak-test-baseline.v1.json");
    std::fs::write(&out, serde_json::to_string_pretty(&doc).unwrap() + "\n").unwrap();
    println!("wrote {n} weak-test baseline entries to {}", out.display());

    let res = run_weak_test_gate(&crates, Some(out));
    println!("gate with baseline: ok={} detail={:?}", res.ok, res.detail);
    assert!(
        res.ok,
        "freshly-generated baseline must make the weak-test gate pass; got {:?}",
        res.detail
    );
}
