//! Advisory GUI visual review CLI. ALWAYS exits 0 — never gates CI.
use std::path::Path;
use vox_orchestrator_mcp::visus_review::{RunArgs, run, write_report};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let a: Vec<String> = std::env::args().collect();
    let get = |k: &str| {
        a.iter()
            .position(|x| x == k)
            .and_then(|i| a.get(i + 1))
            .cloned()
    };
    let manifest =
        get("--manifest").unwrap_or_else(|| "crates/vox-gui/ui/e2e/screens/manifest.json".into());
    let screens = get("--screens").unwrap_or_else(|| "crates/vox-gui/ui/e2e/screens".into());
    let cache = get("--cache")
        .unwrap_or_else(|| "contracts/reports/gui-visual-review/cache.v1.json".into());
    let report_dir =
        get("--report-dir").unwrap_or_else(|| "contracts/reports/gui-visual-review".into());
    let date = get("--date").unwrap_or_else(|| "0000-00-00".into());
    let now = get("--now").unwrap_or_default();
    let do_ai = a.iter().any(|x| x == "--ai");
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
