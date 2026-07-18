//! Advisory GUI visual review CLI. ALWAYS exits 0 — never gates CI.
use std::path::Path;
use vox_orchestrator_mcp::visus_review::{
    BundleRunArgs, RunArgs, default_report_date, run, run_bundle, write_report,
};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let a: Vec<String> = std::env::args().collect();
    let get = |k: &str| {
        a.iter()
            .position(|x| x == k)
            .and_then(|i| a.get(i + 1))
            .cloned()
    };
    let do_ai = a.iter().any(|x| x == "--ai");

    if let Some(bundle_dir) = get("--bundle") {
        let cache = get("--cache")
            .unwrap_or_else(|| "contracts/reports/gui-visual-review/bundle-cache.v1.json".into());
        let report_dir =
            get("--report-dir").unwrap_or_else(|| "contracts/reports/gui-visual-review".into());
        let now = get("--now").unwrap_or_default();
        let total_budget_ms = get("--total-budget-ms")
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(1_800_000);
        let max_reviews = get("--max-reviews").and_then(|s| s.parse::<usize>().ok());
        let browsers = get("--browsers")
            .map(|s| s.split(',').map(|b| b.trim().to_string()).collect())
            .unwrap_or_else(|| vec!["chromium".to_string()]);
        let args = BundleRunArgs {
            bundle_dir: Path::new(&bundle_dir),
            cache_path: Path::new(&cache),
            report_dir: Path::new(&report_dir),
            now_iso: now,
            do_ai,
            total_budget_ms,
            max_reviews,
            browsers,
        };
        let report = run_bundle(&args).await;
        eprintln!(
            "gui-visual-review-bundle: {} reviewed, {} cached, {} deferred, {} defects across {} surfaces",
            report.reviewed,
            report.cached,
            report.deferred,
            report.defects_found,
            report.total_surfaces
        );
        if report.defects_found > 0 || report.deferred > 0 {
            eprintln!(
                "::warning::gui-visual-review-bundle: {} defects found, {} entries deferred",
                report.defects_found, report.deferred
            );
        }
        std::process::exit(0);
    }

    let manifest =
        get("--manifest").unwrap_or_else(|| "crates/vox-gui/ui/e2e/screens/manifest.json".into());
    let screens = get("--screens").unwrap_or_else(|| "crates/vox-gui/ui/e2e/screens".into());
    let cache = get("--cache")
        .unwrap_or_else(|| "contracts/reports/gui-visual-review/cache.v1.json".into());
    let report_dir =
        get("--report-dir").unwrap_or_else(|| "contracts/reports/gui-visual-review".into());
    let date = get("--date").unwrap_or_else(default_report_date);
    let now = get("--now").unwrap_or_default();
    let args = RunArgs {
        manifest_path: Path::new(&manifest),
        screens_dir: Path::new(&screens),
        cache_path: Path::new(&cache),
        report_dir: Path::new(&report_dir),
        now_iso: now,
        do_ai,
    };
    let report = run(&args).await;
    match write_report(Path::new(&report_dir), &date, &report) {
        Ok(p) => eprintln!("gui-visual-review: wrote {}", p.display()),
        Err(e) => eprintln!("::warning::gui-visual-review: report write failed: {e}"),
    }
    eprintln!(
        "gui-visual-review: {} reviewed, {} cached, {} deferred{}",
        report.surfaces_reviewed,
        report.surfaces_cached,
        report.surfaces_deferred,
        if report.spiked { " (TIME SPIKE)" } else { "" }
    );
    std::process::exit(0);
}
