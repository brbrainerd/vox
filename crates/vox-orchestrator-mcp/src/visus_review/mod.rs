//! GUI visual AI adversarial review. Advisory: never gates CI.
//!
//! Deprecated-pending-removal: the legacy `Manifest`/[`run()`] capture-manifest
//! path (screenshots-variants/visual-review specs + `screenshotManifest.ts`) is
//! unreachable from CI as of Task 12 of the Axis frontend review harness plan
//! (`docs/superpowers/plans/2026-07-18-axis-frontend-review-harness.md`) — CI
//! now drives the bounded review-bundle path (`--bundle ...`) instead. The
//! legacy path stays compiled and unit-tested but should be deleted once the
//! bundle path has fully proven out.
pub mod bundle;
pub mod model_select;
pub mod prompt;
pub mod spike;
pub mod types;
pub mod vision_call;
pub use types::*;

/// One rendering defect the model reported for a review-bundle capture.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct Defect {
    #[serde(default)]
    pub severity: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub location: String,
}

/// Parsed model output for a bundle-entry defect review.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct DefectReport {
    #[serde(default)]
    pub score: u32,
    #[serde(default)]
    pub verdict: String,
    #[serde(default)]
    pub defects: Vec<Defect>,
}

/// Slice a model response from its first `{` to its last `}`, tolerating
/// markdown fences and surrounding prose. Shared by `parse_verdict` and
/// `parse_defect_report`.
pub fn extract_json_object(raw: &str) -> Result<&str, String> {
    let start = raw.find('{').ok_or("no JSON object in response")?;
    let end = raw.rfind('}').ok_or("no closing brace")?;
    Ok(&raw[start..=end])
}

/// Extract the JSON verdict object from a model response, tolerating markdown
/// fences and surrounding prose by slicing from the first `{` to the last `}`.
pub fn parse_verdict(raw: &str) -> Result<ReviewVerdict, String> {
    let obj = extract_json_object(raw)?;
    serde_json::from_str(obj).map_err(|e| format!("verdict parse: {e}"))
}

/// Extract the JSON defect-report object from a model response (same fenced-
/// output tolerance as `parse_verdict`).
pub fn parse_defect_report(raw: &str) -> Result<DefectReport, String> {
    let obj = extract_json_object(raw)?;
    serde_json::from_str(obj).map_err(|e| format!("defect report parse: {e}"))
}

#[cfg(test)]
mod verdict_tests {
    use super::*;
    #[test]
    fn parses_fenced_json() {
        let raw = "```json\n{\"score\":80,\"verdict\":\"pass_with_notes\",\"findings\":[]}\n```";
        let v = parse_verdict(raw).unwrap();
        assert_eq!(v.score, 80);
        assert_eq!(v.verdict, "pass_with_notes");
    }
    #[test]
    fn errors_on_garbage() {
        assert!(parse_verdict("no json here").is_err());
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ReviewDecision {
    New,
    Changed,
    Cached,
}

/// Cache schema this build reads/writes. A mismatched on-disk cache is
/// discarded wholesale (one-time full re-review) rather than trusted.
pub const CACHE_SCHEMA_VERSION: u32 = 1;

pub fn decide_status(
    cache: &CacheIndex,
    view_key: &str,
    fresh_sha: &str,
    model: &str,
    prompt_version: &str,
) -> ReviewDecision {
    match cache.entries.get(view_key) {
        None => ReviewDecision::New,
        Some(e)
            if e.screenshot_sha256 == fresh_sha
                && e.model == model
                && e.prompt_version == prompt_version =>
        {
            ReviewDecision::Cached
        }
        Some(_) => ReviewDecision::Changed,
    }
}

/// Drop cache entries whose viewKey is absent from the current capture
/// manifest (dead surfaces). No-op on an empty manifest so a missing/unreadable
/// manifest never wipes a good cache.
pub fn prune_dead_views(cache: &mut CacheIndex, manifest: &Manifest) {
    if manifest.surfaces.is_empty() {
        return;
    }
    let live: std::collections::BTreeSet<&str> = manifest
        .surfaces
        .iter()
        .map(|s| s.view_key.as_str())
        .collect();
    cache.entries.retain(|k, _| live.contains(k.as_str()));
}

#[cfg(test)]
mod decide_tests {
    use super::*;
    const M: &str = "google/gemini-3-flash-preview";
    const PV: &str = "2026-07-16.1";
    fn cache_with(view: &str, sha: &str, model: &str, prompt_version: &str) -> CacheIndex {
        let mut c = CacheIndex::default();
        c.entries.insert(
            view.into(),
            CacheEntry {
                screenshot_sha256: sha.into(),
                score: 90,
                verdict: "pass".into(),
                model: model.into(),
                reviewed_at: "t".into(),
                prompt_version: prompt_version.into(),
            },
        );
        c
    }
    #[test]
    fn new_surface_is_new() {
        assert_eq!(
            decide_status(&CacheIndex::default(), "x", "aa", M, PV),
            ReviewDecision::New
        );
    }
    #[test]
    fn same_hash_model_and_prompt_is_cached() {
        assert_eq!(
            decide_status(&cache_with("x", "aa", M, PV), "x", "aa", M, PV),
            ReviewDecision::Cached
        );
    }
    #[test]
    fn different_hash_is_changed() {
        assert_eq!(
            decide_status(&cache_with("x", "aa", M, PV), "x", "bb", M, PV),
            ReviewDecision::Changed
        );
    }
    #[test]
    fn different_model_is_changed_even_with_same_hash() {
        assert_eq!(
            decide_status(&cache_with("x", "aa", "old/model", PV), "x", "aa", M, PV),
            ReviewDecision::Changed
        );
    }
    #[test]
    fn different_prompt_version_is_changed_even_with_same_hash() {
        assert_eq!(
            decide_status(&cache_with("x", "aa", M, "2026-01-01.1"), "x", "aa", M, PV),
            ReviewDecision::Changed
        );
    }
    #[test]
    fn legacy_entry_empty_prompt_version_is_changed() {
        assert_eq!(
            decide_status(&cache_with("x", "aa", M, ""), "x", "aa", M, PV),
            ReviewDecision::Changed
        );
    }
    #[test]
    fn prune_drops_views_absent_from_manifest() {
        let mut c = cache_with("dead-view", "aa", M, PV);
        c.entries
            .extend(cache_with("live-view", "bb", M, PV).entries);
        let manifest = Manifest {
            total_capture_ms: 0,
            surfaces: vec![ManifestEntry {
                view_key: "live-view".into(),
                file: "live-view.png".into(),
                sha256: "bb".into(),
                capture_ms: 1,
            }],
        };
        prune_dead_views(&mut c, &manifest);
        assert!(c.entries.contains_key("live-view"));
        assert!(!c.entries.contains_key("dead-view"));
    }
    #[test]
    fn prune_is_noop_on_empty_manifest() {
        let mut c = cache_with("x", "aa", M, PV);
        prune_dead_views(
            &mut c,
            &Manifest {
                total_capture_ms: 0,
                surfaces: vec![],
            },
        );
        assert_eq!(c.entries.len(), 1);
    }
}

use std::path::Path;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ManifestEntry {
    #[serde(rename = "viewKey")]
    pub view_key: String,
    pub file: String,
    pub sha256: String,
    #[serde(rename = "captureMs")]
    pub capture_ms: u64,
}
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Manifest {
    pub total_capture_ms: u64,
    pub surfaces: Vec<ManifestEntry>,
}

pub struct RunArgs<'a> {
    pub manifest_path: &'a Path,
    pub screens_dir: &'a Path,
    pub cache_path: &'a Path,
    pub report_dir: &'a Path,
    pub now_iso: String,
    pub do_ai: bool,
}

/// Default config used when the on-disk config file is missing/unreadable.
fn default_config() -> VisualReviewConfig {
    VisualReviewConfig {
        schema_version: 1,
        model_preference: vec![
            "google/gemini-3-flash-preview".into(),
            "google/gemini-2.5-flash".into(),
        ],
        escalation_model: "anthropic/claude-opus-4.8".into(),
        per_surface_review_budget_ms: 8_000,
        total_review_budget_ms: 90_000,
        max_concurrent_reviews: 3,
        max_image_edge_px: 2880,
        spike_factor: 1.5,
    }
}

/// One-shot vision review of a single PNG on disk: read bytes + call the
/// vision model + time it. Shared by the legacy `review_surface` path and
/// `run_bundle` (both need the identical fs::read + Instant + vision-call
/// shape, but produce differently-shaped verdicts, so this stays below the
/// verdict-parsing layer rather than reusing `review_surface` itself).
pub async fn review_image(
    png_path: &Path,
    model: &str,
    system: &str,
    user: &str,
) -> Result<(String, vision_call::Usage, u64), String> {
    let png_bytes = std::fs::read(png_path).map_err(|e| format!("read png: {e}"))?;
    let t0 = std::time::Instant::now();
    let res = vision_call::call_vision_model(model, system, user, &png_bytes).await;
    let review_ms = t0.elapsed().as_millis() as u64;
    let (content, usage) = res?;
    Ok((content, usage, review_ms))
}

/// Pick the vision model to review with: registry-backed if the registry
/// loads, else the always-fallback `NullCatalog`. Extracted from `run()` so
/// `run_bundle` shares the exact same selection policy.
pub fn select_review_model(cfg: &VisualReviewConfig) -> String {
    let registry =
        std::panic::catch_unwind(vox_orchestrator::models::ModelRegistry::from_cache).ok();
    match &registry {
        Some(reg) => model_select::choose_vision_model(
            &cfg.model_preference,
            &model_select::RegistryCatalog(reg),
        ),
        None => {
            model_select::choose_vision_model(&cfg.model_preference, &model_select::NullCatalog)
        }
    }
}

pub async fn run(args: &RunArgs<'_>) -> RunReport {
    let manifest: Manifest = std::fs::read_to_string(args.manifest_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(Manifest {
            total_capture_ms: 0,
            surfaces: vec![],
        });
    let mut cache: CacheIndex = std::fs::read_to_string(args.cache_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    if cache.schema_version != CACHE_SCHEMA_VERSION {
        eprintln!(
            "::warning::gui-visual-review: cache schema_version {} != {} — discarding cache (one-time full re-review)",
            cache.schema_version, CACHE_SCHEMA_VERSION
        );
        cache = CacheIndex::default();
    }

    // Config: load the on-disk policy, else fall back to the hardcoded default.
    let cfg: VisualReviewConfig =
        std::fs::read_to_string("contracts/orchestration/visual-review.config.v1.json")
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(default_config);

    // Model selection: try the registry; on any failure use the NullCatalog
    // (always-fallback).
    let model = select_review_model(&cfg);

    let mut surfaces = Vec::new();
    let (mut reviewed, mut cached) = (0usize, 0usize);
    let mut total_review_ms = 0u64;
    let start = std::time::Instant::now();

    for entry in &manifest.surfaces {
        // Per-run time budget: once exhausted, defer remaining surfaces without
        // calling the model. Cached surfaces are cheap, so we only budget-gate
        // surfaces that would otherwise trigger an AI review.
        let decision = decide_status(
            &cache,
            &entry.view_key,
            &entry.sha256,
            &model,
            prompt::PROMPT_VERSION,
        );
        if args.do_ai
            && decision != ReviewDecision::Cached
            && (start.elapsed().as_millis() as u64) >= cfg.total_review_budget_ms
        {
            eprintln!(
                "::warning::gui-visual-review: '{}' deferred (review budget {}ms exhausted)",
                entry.view_key, cfg.total_review_budget_ms
            );
            surfaces.push(SurfaceReport {
                view_key: entry.view_key.clone(),
                screenshot_sha256: entry.sha256.clone(),
                status: "deferred".into(),
                score: None,
                verdict: None,
                findings: vec![],
                model: None,
                prompt_tokens: None,
                completion_tokens: None,
                cost_usd: None,
                review_ms: None,
            });
            continue;
        }
        match decision {
            ReviewDecision::Cached => {
                cached += 1;
                let c = &cache.entries[&entry.view_key];
                surfaces.push(SurfaceReport {
                    view_key: entry.view_key.clone(),
                    screenshot_sha256: entry.sha256.clone(),
                    status: "cached".into(),
                    score: Some(c.score),
                    verdict: Some(c.verdict.clone()),
                    findings: vec![],
                    model: Some(c.model.clone()),
                    prompt_tokens: None,
                    completion_tokens: None,
                    cost_usd: None,
                    review_ms: None,
                });
            }
            ReviewDecision::New | ReviewDecision::Changed => {
                let status = if decision == ReviewDecision::New {
                    "new"
                } else {
                    "changed"
                };
                eprintln!(
                    "::warning::gui-visual-review: surface '{}' {} — appearance review needed",
                    entry.view_key, status
                );
                if args.do_ai {
                    reviewed += 1;
                    let report = review_surface(args, entry, &model).await;
                    if let Some(ms) = report.review_ms {
                        total_review_ms += ms;
                    }
                    if report.status == "reviewed" {
                        cache.entries.insert(
                            entry.view_key.clone(),
                            CacheEntry {
                                screenshot_sha256: entry.sha256.clone(),
                                score: report.score.unwrap_or(0),
                                verdict: report.verdict.clone().unwrap_or_default(),
                                model: report.model.clone().unwrap_or_else(|| model.clone()),
                                reviewed_at: args.now_iso.clone(),
                                prompt_version: prompt::PROMPT_VERSION.to_string(),
                            },
                        );
                    }
                    surfaces.push(report);
                } else {
                    surfaces.push(SurfaceReport {
                        view_key: entry.view_key.clone(),
                        screenshot_sha256: entry.sha256.clone(),
                        status: status.into(),
                        score: None,
                        verdict: None,
                        findings: vec![],
                        model: None,
                        prompt_tokens: None,
                        completion_tokens: None,
                        cost_usd: None,
                        review_ms: None,
                    });
                }
            }
        }
    }

    let deferred = surfaces.iter().filter(|s| s.status == "deferred").count();

    // Persist the updated cache so subsequent runs short-circuit reviewed surfaces.
    if args.do_ai {
        prune_dead_views(&mut cache, &manifest);
        cache.schema_version = CACHE_SCHEMA_VERSION;
        if let Some(parent) = args.cache_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match serde_json::to_string_pretty(&cache) {
            Ok(s) => {
                if let Err(e) = std::fs::write(args.cache_path, s + "\n") {
                    eprintln!("::warning::gui-visual-review: cache write failed: {e}");
                }
            }
            Err(e) => eprintln!("::warning::gui-visual-review: cache serialize failed: {e}"),
        }
    }

    let total_cost_usd: f64 = surfaces.iter().filter_map(|s| s.cost_usd).sum();

    let mut report = RunReport {
        schema_version: 1,
        generated_at: args.now_iso.clone(),
        default_model: model,
        surfaces,
        total_capture_ms: manifest.total_capture_ms,
        total_review_ms,
        surfaces_reviewed: reviewed,
        surfaces_cached: cached,
        surfaces_deferred: deferred,
        spiked: false,
        spike_detail: String::new(),
    };

    // Trailing-median spike detection over the ledger (advisory only). All
    // ledger IO is guarded so any failure merely warns and never aborts.
    let ledger_path = args.report_dir.join("ledger.jsonl");
    let history: Vec<u64> = std::fs::read_to_string(&ledger_path)
        .ok()
        .map(|s| {
            s.lines()
                .filter_map(|l| serde_json::from_str::<spike::LedgerRow>(l).ok())
                .map(|r| r.total_review_ms)
                .collect()
        })
        .unwrap_or_default();
    let (spiked, detail) = spike::is_spike(&history, report.total_review_ms, cfg.spike_factor);
    report.spiked = spiked;
    report.spike_detail = detail;
    if spiked {
        eprintln!(
            "::warning::gui-visual-review: TIME SPIKE — {}",
            report.spike_detail
        );
    }

    // Append this run's row to the ledger (best-effort).
    let row = spike::LedgerRow {
        ts: args.now_iso.clone(),
        surfaces_reviewed: report.surfaces_reviewed,
        total_review_ms: report.total_review_ms,
        total_cost_usd,
        model: report.default_model.clone(),
    };
    match serde_json::to_string(&row) {
        Ok(line) => {
            if let Some(parent) = ledger_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            use std::io::Write;
            match std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&ledger_path)
            {
                Ok(mut f) => {
                    if let Err(e) = writeln!(f, "{line}") {
                        eprintln!("::warning::gui-visual-review: ledger append failed: {e}");
                    }
                }
                Err(e) => eprintln!("::warning::gui-visual-review: ledger open failed: {e}"),
            }
        }
        Err(e) => eprintln!("::warning::gui-visual-review: ledger serialize failed: {e}"),
    }

    report
}

async fn review_surface(args: &RunArgs<'_>, entry: &ManifestEntry, model: &str) -> SurfaceReport {
    use crate::visus_review::prompt;
    let png_path = args.screens_dir.join(&entry.file);
    let res = review_image(
        &png_path,
        model,
        &prompt::system_prompt(),
        &prompt::user_prompt(&entry.view_key),
    )
    .await;
    match res {
        Ok((content, usage, review_ms)) => match parse_verdict(&content) {
            Ok(v) => SurfaceReport {
                view_key: entry.view_key.clone(),
                screenshot_sha256: entry.sha256.clone(),
                status: "reviewed".into(),
                score: Some(v.score),
                verdict: Some(v.verdict),
                findings: v.findings,
                model: Some(model.to_string()),
                prompt_tokens: Some(usage.prompt_tokens),
                completion_tokens: Some(usage.completion_tokens),
                cost_usd: usage.cost_usd,
                review_ms: Some(review_ms),
            },
            Err(e) => failed_surface(entry, &e),
        },
        Err(e) => failed_surface(entry, &e),
    }
}

fn failed_surface(entry: &ManifestEntry, why: &str) -> SurfaceReport {
    eprintln!(
        "::warning::gui-visual-review: '{}' review failed: {}",
        entry.view_key, why
    );
    SurfaceReport {
        view_key: entry.view_key.clone(),
        screenshot_sha256: entry.sha256.clone(),
        status: "deferred".into(),
        score: None,
        verdict: None,
        findings: vec![],
        model: None,
        prompt_tokens: None,
        completion_tokens: None,
        cost_usd: None,
        review_ms: None,
    }
}

/// Default report date for the CLI: today's UTC date. Replaces the historical
/// `--date`-absent behavior of writing a junk `0000-00-00.json` report.
pub fn default_report_date() -> String {
    chrono::Utc::now().format("%Y-%m-%d").to_string()
}

pub fn write_report(
    report_dir: &Path,
    date: &str,
    report: &RunReport,
) -> std::io::Result<std::path::PathBuf> {
    // Refuse placeholder/garbage dates ("0000-00-00" has month 0 and fails the
    // parse) so a stray report file can never be produced again.
    if chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").is_err() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("refusing to write report: {date:?} is not a real YYYY-MM-DD date"),
        ));
    }
    std::fs::create_dir_all(report_dir)?;
    let path = report_dir.join(format!("{date}.json"));
    std::fs::write(&path, serde_json::to_string_pretty(report).unwrap() + "\n")?;
    Ok(path)
}

#[cfg(test)]
mod report_date_tests {
    use super::*;

    fn empty_report() -> RunReport {
        RunReport {
            schema_version: 1,
            generated_at: "t".into(),
            default_model: "m".into(),
            surfaces: vec![],
            total_capture_ms: 0,
            total_review_ms: 0,
            surfaces_reviewed: 0,
            surfaces_cached: 0,
            surfaces_deferred: 0,
            spiked: false,
            spike_detail: String::new(),
        }
    }

    #[test]
    fn default_report_date_is_a_real_utc_date() {
        let d = default_report_date();
        assert_ne!(d, "0000-00-00");
        assert!(
            chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d").is_ok(),
            "not a real YYYY-MM-DD date: {d}"
        );
    }

    #[test]
    fn write_report_refuses_the_zero_date_placeholder() {
        let dir = tempfile::tempdir().unwrap();
        let err = write_report(dir.path(), "0000-00-00", &empty_report())
            .expect_err("0000-00-00 must be refused");
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
        assert!(!dir.path().join("0000-00-00.json").exists());
    }

    #[test]
    fn write_report_refuses_non_date_strings() {
        let dir = tempfile::tempdir().unwrap();
        assert!(write_report(dir.path(), "not-a-date", &empty_report()).is_err());
        assert!(write_report(dir.path(), "", &empty_report()).is_err());
    }

    #[test]
    fn write_report_accepts_a_real_date() {
        let dir = tempfile::tempdir().unwrap();
        let p =
            write_report(dir.path(), "2026-07-16", &empty_report()).expect("real date accepted");
        assert!(p.exists());
        assert!(p.ends_with("2026-07-16.json"));
    }
}

// ---------------------------------------------------------------------------
// Review-bundle analysis (Phase C): frontier-resumable, priority-ordered,
// browser-scoped-pruned AI defect analysis over `bundle::load_bundle` output.
// ---------------------------------------------------------------------------

use std::collections::BTreeSet;

/// One entry in `bundle-report.v1.json`'s `entries` array.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BundleReportEntry {
    pub id: String,
    pub surface: String,
    pub state: String,
    pub viewport: String,
    pub browser: String,
    pub theme: String,
    /// "reviewed" | "cached" | "deferred"
    pub status: String,
    pub score: Option<u32>,
    pub verdict: Option<String>,
    pub defects: Vec<Defect>,
    pub programmatic: ProgrammaticFindings,
}

/// Programmatic (non-AI) findings the capture harness already measured for
/// this entry — reported regardless of whether the entry got an AI review.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProgrammaticFindings {
    pub axe_serious_critical: usize,
    pub axe_total: usize,
    pub console_errors: usize,
    pub icon_issues: usize,
    pub overflow_px: i64,
    pub state_ok: bool,
}

/// In-memory result of `run_bundle`. `total_surfaces`/`reviewed`/`cached`/
/// `deferred`/`defects_found` are flattened here (rather than nested under a
/// `totals` field) so callers don't have to reach through another layer; the
/// on-disk `bundle-report.v1.json` nests them under `"totals"` per the plan's
/// schema.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BundleReport {
    pub schema_version: u32,
    pub generated_at: String,
    pub entries: Vec<BundleReportEntry>,
    pub total_surfaces: usize,
    pub reviewed: usize,
    pub cached: usize,
    pub deferred: usize,
    pub defects_found: usize,
}

pub struct BundleRunArgs<'a> {
    pub bundle_dir: &'a Path,
    pub cache_path: &'a Path,
    pub report_dir: &'a Path,
    pub now_iso: String,
    pub do_ai: bool,
    /// Total wall-clock budget for AI reviews this run, in milliseconds.
    /// Default 1_800_000 (30 minutes) at the call site.
    pub total_budget_ms: u64,
    /// Optional hard cap on the number of AI reviews performed this run.
    pub max_reviews: Option<usize>,
    /// Browsers eligible for AI review this run (default `["chromium"]`).
    /// Entries whose browser isn't in this list still get their programmatic
    /// findings reported, just deferred for AI review.
    pub browsers: Vec<String>,
}

/// Extract the browser id from a capture id: the segment after the last
/// `--`, skipping over a trailing `theme-...` segment if present (a
/// `theme-` segment is a viewport/theme detail, not the browser).
fn bundle_key_browser(key: &str) -> Option<String> {
    let mut parts: Vec<&str> = key.split("--").collect();
    while let Some(last) = parts.last() {
        if last.starts_with("theme-") {
            parts.pop();
        } else {
            break;
        }
    }
    parts.last().map(|s| s.to_string())
}

/// Drop cached entries whose id is absent from this run's live set — but
/// only when that entry's browser also had at least one live entry in this
/// run. This keeps e.g. a firefox cache entry alive across a chromium-only
/// run, since firefox simply wasn't exercised this time.
pub fn prune_bundle_cache(
    cache: &mut CacheIndex,
    live_ids: &BTreeSet<String>,
    live_browsers: &BTreeSet<String>,
) {
    cache.entries.retain(|k, _| {
        if live_ids.contains(k) {
            return true;
        }
        !matches!(bundle_key_browser(k), Some(b) if live_browsers.contains(&b))
    });
}

fn severity_rank(s: &str) -> u8 {
    match s {
        "critical" => 0,
        "major" => 1,
        "minor" => 2,
        _ => 3,
    }
}

fn render_bundle_digest(report: &BundleReport) -> String {
    use std::collections::BTreeMap;
    use std::fmt::Write as _;
    let mut out = String::new();
    let _ = writeln!(out, "# GUI Visual Review Bundle Digest\n");
    let _ = writeln!(out, "Generated: {}\n", report.generated_at);
    let _ = writeln!(out, "| Total | Reviewed | Cached | Deferred | Defects |");
    let _ = writeln!(out, "|---|---|---|---|---|");
    let _ = writeln!(
        out,
        "| {} | {} | {} | {} | {} |\n",
        report.total_surfaces,
        report.reviewed,
        report.cached,
        report.deferred,
        report.defects_found
    );

    let mut by_surface: BTreeMap<&str, Vec<&BundleReportEntry>> = BTreeMap::new();
    for e in &report.entries {
        by_surface.entry(e.surface.as_str()).or_default().push(e);
    }
    for (surface, entries) in &by_surface {
        let _ = writeln!(out, "## {surface}\n");
        for e in entries {
            let _ = writeln!(
                out,
                "- **{}** [{} / {}] status={} score={:?} verdict={:?}",
                e.id, e.browser, e.viewport, e.status, e.score, e.verdict
            );
            let mut defects = e.defects.clone();
            defects.sort_by_key(|d| severity_rank(&d.severity));
            for d in &defects {
                let _ = writeln!(
                    out,
                    "  - ({}) {}: {} — {}",
                    d.severity, d.kind, d.description, d.location
                );
            }
        }
        let _ = writeln!(out);
    }
    out
}

/// Run a bounded, resumable AI defect analysis over a review bundle
/// (`bundle::load_bundle` output). Never panics on missing/malformed input:
/// an empty/unreadable bundle dir simply produces an empty report.
pub async fn run_bundle(args: &BundleRunArgs<'_>) -> BundleReport {
    let (entries, _skipped) = bundle::load_bundle(args.bundle_dir).unwrap_or_default();

    let mut cache: CacheIndex = std::fs::read_to_string(args.cache_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    if cache.schema_version != CACHE_SCHEMA_VERSION {
        cache = CacheIndex::default();
    }

    let cfg: VisualReviewConfig =
        std::fs::read_to_string("contracts/orchestration/visual-review.config.v1.json")
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(default_config);
    let model = select_review_model(&cfg);

    let browsers_allowed: BTreeSet<String> = args.browsers.iter().cloned().collect();

    let decisions: Vec<ReviewDecision> = entries
        .iter()
        .map(|e| decide_status(&cache, &e.id, &e.sha256, &model, prompt::PROMPT_VERSION))
        .collect();

    // Frontier candidates: not cached, browser-eligible, AI enabled.
    let mut candidates: Vec<usize> = (0..entries.len())
        .filter(|&i| {
            args.do_ai
                && decisions[i] != ReviewDecision::Cached
                && browsers_allowed.contains(&entries[i].browser)
        })
        .collect();
    // Priority: compact viewport first, then non-default state, then
    // chromium before firefox (and any other browser), tie-broken by id for
    // determinism.
    candidates.sort_by_key(|&i| {
        let e = &entries[i];
        let viewport_rank = if e.viewport == "compact" { 0 } else { 1 };
        let state_rank = if e.state == "default" { 1 } else { 0 };
        let browser_rank = if e.browser == "chromium" { 0 } else { 1 };
        (viewport_rank, state_rank, browser_rank, e.id.clone())
    });

    let mut ai_status: Vec<Option<&'static str>> = vec![None; entries.len()];
    let mut ai_score: Vec<Option<u32>> = vec![None; entries.len()];
    let mut ai_verdict: Vec<Option<String>> = vec![None; entries.len()];
    let mut ai_defects: Vec<Vec<Defect>> = vec![Vec::new(); entries.len()];

    let start = std::time::Instant::now();
    for (attempt, i) in candidates.into_iter().enumerate() {
        if let Some(max) = args.max_reviews {
            if attempt >= max {
                break;
            }
        }
        if (start.elapsed().as_millis() as u64) >= args.total_budget_ms {
            eprintln!(
                "::warning::gui-visual-review-bundle: total budget {}ms exhausted — remaining entries deferred",
                args.total_budget_ms
            );
            break;
        }
        let e = &entries[i];
        let png_path = args.bundle_dir.join(&e.file);
        let system = prompt::defect_system_prompt();
        let user = prompt::defect_user_prompt(e);
        match review_image(&png_path, &model, &system, &user).await {
            Ok((content, _usage, _ms)) => match parse_defect_report(&content) {
                Ok(dr) => {
                    ai_status[i] = Some("reviewed");
                    ai_score[i] = Some(dr.score);
                    ai_verdict[i] = Some(dr.verdict.clone());
                    ai_defects[i] = dr.defects.clone();
                    cache.entries.insert(
                        e.id.clone(),
                        CacheEntry {
                            screenshot_sha256: e.sha256.clone(),
                            score: dr.score,
                            verdict: dr.verdict,
                            model: model.clone(),
                            reviewed_at: args.now_iso.clone(),
                            prompt_version: prompt::PROMPT_VERSION.to_string(),
                        },
                    );
                }
                Err(e2) => {
                    eprintln!(
                        "::warning::gui-visual-review-bundle: '{}' defect-report parse failed: {e2}",
                        e.id
                    );
                    ai_status[i] = Some("deferred");
                }
            },
            Err(e2) => {
                eprintln!(
                    "::warning::gui-visual-review-bundle: '{}' review failed: {e2}",
                    e.id
                );
                ai_status[i] = Some("deferred");
            }
        }
    }

    let (mut reviewed_n, mut cached_n, mut deferred_n, mut defects_found) =
        (0usize, 0usize, 0usize, 0usize);
    let mut report_entries = Vec::with_capacity(entries.len());
    for (i, e) in entries.iter().enumerate() {
        let status = if let Some(s) = ai_status[i] {
            s.to_string()
        } else if decisions[i] == ReviewDecision::Cached {
            "cached".to_string()
        } else {
            "deferred".to_string()
        };
        match status.as_str() {
            "reviewed" => reviewed_n += 1,
            "cached" => cached_n += 1,
            _ => deferred_n += 1,
        }
        let (score, verdict, defects) = match status.as_str() {
            "reviewed" => (ai_score[i], ai_verdict[i].clone(), ai_defects[i].clone()),
            "cached" => {
                let c = &cache.entries[&e.id];
                (Some(c.score), Some(c.verdict.clone()), Vec::new())
            }
            _ => (None, None, Vec::new()),
        };
        defects_found += defects.len();

        let axe_serious_critical = e
            .axe_violations
            .iter()
            .filter(|v| matches!(v["impact"].as_str(), Some("serious") | Some("critical")))
            .count();
        let overflow_px = e
            .overflow
            .get("scrollHostHorizontalOverflowPx")
            .and_then(|v| v.as_i64())
            .or_else(|| {
                e.overflow
                    .get("bodyHorizontalOverflowPx")
                    .and_then(|v| v.as_i64())
            })
            .unwrap_or(0);

        report_entries.push(BundleReportEntry {
            id: e.id.clone(),
            surface: e.surface.clone(),
            state: e.state.clone(),
            viewport: e.viewport.clone(),
            browser: e.browser.clone(),
            theme: e.theme.clone(),
            status,
            score,
            verdict,
            defects,
            programmatic: ProgrammaticFindings {
                axe_serious_critical,
                axe_total: e.axe_violations.len(),
                console_errors: e.console_errors.len(),
                icon_issues: e.icon_issues.len(),
                overflow_px,
                state_ok: e.state_ok,
            },
        });
    }

    if args.do_ai {
        let live_ids: BTreeSet<String> = entries.iter().map(|e| e.id.clone()).collect();
        let live_browsers: BTreeSet<String> = entries.iter().map(|e| e.browser.clone()).collect();
        prune_bundle_cache(&mut cache, &live_ids, &live_browsers);
        cache.schema_version = CACHE_SCHEMA_VERSION;
        if let Some(parent) = args.cache_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match serde_json::to_string_pretty(&cache) {
            Ok(s) => {
                if let Err(e) = std::fs::write(args.cache_path, s + "\n") {
                    eprintln!("::warning::gui-visual-review-bundle: cache write failed: {e}");
                }
            }
            Err(e) => eprintln!("::warning::gui-visual-review-bundle: cache serialize failed: {e}"),
        }
    }

    let report = BundleReport {
        schema_version: 1,
        generated_at: args.now_iso.clone(),
        entries: report_entries,
        total_surfaces: entries.len(),
        reviewed: reviewed_n,
        cached: cached_n,
        deferred: deferred_n,
        defects_found,
    };

    let _ = std::fs::create_dir_all(args.report_dir);
    let report_json = serde_json::json!({
        "schema_version": report.schema_version,
        "generated_at": report.generated_at,
        "entries": report.entries,
        "totals": {
            "total_surfaces": report.total_surfaces,
            "reviewed": report.reviewed,
            "cached": report.cached,
            "deferred": report.deferred,
            "defects_found": report.defects_found,
        }
    });
    match serde_json::to_string_pretty(&report_json) {
        Ok(s) => {
            if let Err(e) = std::fs::write(args.report_dir.join("bundle-report.v1.json"), s + "\n")
            {
                eprintln!("::warning::gui-visual-review-bundle: report write failed: {e}");
            }
        }
        Err(e) => eprintln!("::warning::gui-visual-review-bundle: report serialize failed: {e}"),
    }
    if let Err(e) = std::fs::write(
        args.report_dir.join("bundle-digest.md"),
        render_bundle_digest(&report),
    ) {
        eprintln!("::warning::gui-visual-review-bundle: digest write failed: {e}");
    }

    if report.defects_found > 0 || report.deferred > 0 {
        eprintln!(
            "::warning::gui-visual-review-bundle: {} defects found across {} surfaces, {} deferred",
            report.defects_found, report.total_surfaces, report.deferred
        );
    }

    report
}

#[cfg(test)]
mod bundle_run_tests {
    use super::*;

    #[test]
    fn bundle_cache_key_is_the_capture_id() {
        let mut c = CacheIndex::default();
        c.entries.insert(
            "chat--default--wide--chromium".into(),
            CacheEntry {
                screenshot_sha256: "aa".into(),
                score: 90,
                verdict: "pass".into(),
                model: "m".into(),
                reviewed_at: "t".into(),
                prompt_version: crate::visus_review::prompt::PROMPT_VERSION.into(),
            },
        );
        let pv = crate::visus_review::prompt::PROMPT_VERSION;
        assert_eq!(
            decide_status(&c, "chat--default--wide--chromium", "aa", "m", pv),
            ReviewDecision::Cached
        );
        assert_eq!(
            decide_status(&c, "chat--default--wide--chromium", "bb", "m", pv),
            ReviewDecision::Changed
        );
        assert_eq!(
            decide_status(&c, "chat--default--laptop--chromium", "aa", "m", pv),
            ReviewDecision::New
        );
    }

    #[test]
    fn defect_report_parses_fenced_model_output() {
        let raw = "```json\n{\"score\": 40, \"verdict\": \"fail\", \"defects\": [{\"severity\":\"critical\",\"kind\":\"occlusion\",\"description\":\"HUD covers the composer\",\"location\":\"bottom center\"}]}\n```";
        let d = parse_defect_report(raw).unwrap();
        assert_eq!(d.defects.len(), 1);
        assert_eq!(d.defects[0].kind, "occlusion");
    }

    #[test]
    fn bundle_prune_is_browser_scoped() {
        let mut c = CacheIndex::default();
        for id in ["a--default--wide--chromium", "b--default--wide--firefox"] {
            c.entries.insert(
                id.into(),
                CacheEntry {
                    screenshot_sha256: "s".into(),
                    score: 90,
                    verdict: "pass".into(),
                    model: "m".into(),
                    reviewed_at: "t".into(),
                    prompt_version: crate::visus_review::prompt::PROMPT_VERSION.into(),
                },
            );
        }
        // Live run contains only chromium entries, and 'a' is gone.
        let live_ids: std::collections::BTreeSet<String> =
            ["c--default--wide--chromium".to_string()].into();
        let live_browsers: std::collections::BTreeSet<String> = ["chromium".to_string()].into();
        prune_bundle_cache(&mut c, &live_ids, &live_browsers);
        assert!(
            !c.entries.contains_key("a--default--wide--chromium"),
            "stale chromium key pruned"
        );
        assert!(
            c.entries.contains_key("b--default--wide--firefox"),
            "firefox key survives a chromium-only run"
        );
    }

    #[tokio::test]
    async fn run_bundle_no_ai_writes_reports_and_leaves_cache_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let report_dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("entries-chromium-w0.jsonl"),
            "{\"id\":\"a--default--wide--chromium\",\"surface\":\"a\",\"state\":\"default\",\"viewport\":\"wide\",\"browser\":\"chromium\",\"file\":\"a.png\",\"sha256\":\"1\"}\n").unwrap();
        let cache_path = dir.path().join("bundle-cache.v1.json");
        let args = BundleRunArgs {
            bundle_dir: dir.path(),
            cache_path: &cache_path,
            report_dir: report_dir.path(),
            now_iso: "t".into(),
            do_ai: false,
            total_budget_ms: 1000,
            max_reviews: None,
            browsers: vec!["chromium".into()],
        };
        let report = run_bundle(&args).await;
        assert!(report_dir.path().join("bundle-report.v1.json").exists());
        assert!(report_dir.path().join("bundle-digest.md").exists());
        assert!(!cache_path.exists(), "cache persisted only when do_ai");
        assert_eq!(report.total_surfaces, 1);
    }
}
