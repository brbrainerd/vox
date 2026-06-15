//! Regenerator for the silent-drop gate's grandfather baseline.
//!
//! The `catch_all_swallow` + `cross_crate_dup` detectors are high-recall ADVISORY
//! (Info-severity) heuristics: most of the current findings are legitimate `_ =>`
//! fallbacks or known cross-crate duplication (a separate consolidation effort), not
//! bugs. The silent-drop gate is therefore a *no-NEW-findings* gate: this regenerator
//! freezes the current findings into `contracts/toestub/silent-drop-baseline.v1.json`
//! (line-pinned, CI-portable `**/<crate>/...` globs) so the gate passes today and trips
//! only on findings the baseline does not already cover.
//!
//! Ignored by default. Refresh after intentionally accepting or fixing findings:
//!   cargo test -p vox-audit --test regen_silent_drop_baseline -- --ignored --nocapture
use std::path::PathBuf;
use vox_audit::core_gates::run_silent_drop_gate;
use vox_code_audit::{OutputFormat, Severity, ToestubConfig, ToestubEngine, ToestubRunMode};

const RULES: [&str; 2] = ["vox/catch-all-swallow", "arch/cross-crate-dup"];

fn crates_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

#[test]
#[ignore = "regenerator: writes contracts/toestub/silent-drop-baseline.v1.json"]
fn regenerate_silent_drop_baseline() {
    let crates = crates_root();
    let repo = crates.parent().unwrap().to_path_buf();
    let config = ToestubConfig {
        roots: vec![crates.clone()],
        min_severity: Severity::Info,
        run_mode: ToestubRunMode::Audit,
        rule_filter: Some(RULES.iter().map(|s| s.to_string()).collect()),
        format: OutputFormat::Json,
        ..ToestubConfig::default()
    };
    let (result, _) = ToestubEngine::new(config).run_and_report();

    let mut entries: Vec<serde_json::Value> = result
        .findings
        .iter()
        .filter(|f| RULES.contains(&f.rule_id.as_str()))
        .map(|f| {
            let p = f.file.to_string_lossy().replace('\\', "/");
            // CI-portable glob: match the crate-relative suffix from any absolute root.
            let rel = p.rsplit_once("crates/").map(|(_, r)| r).unwrap_or(p.as_str());
            serde_json::json!({
                "rule_id_prefix": f.rule_id,
                "path_glob": format!("**/crates/{rel}"),
                "line": f.line,
                "reason": "Grandfathered advisory silent-drop finding; gate guards NEW findings only.",
                "owner": "robot"
            })
        })
        .collect();
    entries.sort_by(|a, b| {
        (
            a["rule_id_prefix"].as_str().unwrap(),
            a["path_glob"].as_str().unwrap(),
            a["line"].as_u64().unwrap_or(0),
        )
            .cmp(&(
                b["rule_id_prefix"].as_str().unwrap(),
                b["path_glob"].as_str().unwrap(),
                b["line"].as_u64().unwrap_or(0),
            ))
    });

    let n = entries.len();
    let doc = serde_json::json!({ "x-vox-version": 1, "version": 1, "suppressions": entries });
    let out = repo.join("contracts/toestub/silent-drop-baseline.v1.json");
    std::fs::write(&out, serde_json::to_string_pretty(&doc).unwrap() + "\n").unwrap();
    println!("wrote {n} baseline entries to {}", out.display());

    // Self-verify: the gate must now pass over the same tree with this baseline.
    let res = run_silent_drop_gate(&crates, Some(out));
    println!("gate with baseline: ok={} detail={:?}", res.ok, res.detail);
    assert!(
        res.ok,
        "freshly-generated baseline must make the gate pass (0 remaining); got {:?}",
        res.detail
    );
}
