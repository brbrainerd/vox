//! GUI visual AI adversarial review. Advisory: never gates CI.
pub mod model_select;
pub mod prompt;
pub mod types;
pub mod vision_call;
pub use types::*;

/// Extract the JSON verdict object from a model response, tolerating markdown
/// fences and surrounding prose by slicing from the first `{` to the last `}`.
pub fn parse_verdict(raw: &str) -> Result<ReviewVerdict, String> {
    let start = raw.find('{').ok_or("no JSON object in response")?;
    let end = raw.rfind('}').ok_or("no closing brace")?;
    serde_json::from_str(&raw[start..=end]).map_err(|e| format!("verdict parse: {e}"))
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

pub fn decide_status(cache: &CacheIndex, view_key: &str, fresh_sha: &str) -> ReviewDecision {
    match cache.entries.get(view_key) {
        None => ReviewDecision::New,
        Some(e) if e.screenshot_sha256 == fresh_sha => ReviewDecision::Cached,
        Some(_) => ReviewDecision::Changed,
    }
}

#[cfg(test)]
mod decide_tests {
    use super::*;
    fn cache_with(view: &str, sha: &str) -> CacheIndex {
        let mut c = CacheIndex::default();
        c.entries.insert(
            view.into(),
            CacheEntry {
                screenshot_sha256: sha.into(),
                score: 90,
                verdict: "pass".into(),
                model: "m".into(),
                reviewed_at: "t".into(),
            },
        );
        c
    }
    #[test]
    fn new_surface_is_new() {
        assert_eq!(
            decide_status(&CacheIndex::default(), "x", "aa"),
            ReviewDecision::New
        );
    }
    #[test]
    fn same_hash_is_cached() {
        assert_eq!(
            decide_status(&cache_with("x", "aa"), "x", "aa"),
            ReviewDecision::Cached
        );
    }
    #[test]
    fn different_hash_is_changed() {
        assert_eq!(
            decide_status(&cache_with("x", "aa"), "x", "bb"),
            ReviewDecision::Changed
        );
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

    // Config: load the on-disk policy, else fall back to the hardcoded default.
    let cfg: VisualReviewConfig =
        std::fs::read_to_string("contracts/orchestration/visual-review.config.v1.json")
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(default_config);

    // Model selection: try the registry; on any failure use the NullCatalog
    // (always-fallback). `ModelRegistry` is re-exported at
    // `vox_orchestrator::models::ModelRegistry`.
    let registry =
        std::panic::catch_unwind(vox_orchestrator::models::ModelRegistry::from_cache).ok();
    let model = match &registry {
        Some(reg) => model_select::choose_vision_model(
            &cfg.model_preference,
            &model_select::RegistryCatalog(reg),
        ),
        None => {
            model_select::choose_vision_model(&cfg.model_preference, &model_select::NullCatalog)
        }
    };

    let mut surfaces = Vec::new();
    let (mut reviewed, mut cached, deferred) = (0usize, 0usize, 0usize);
    let mut total_review_ms = 0u64;

    for entry in &manifest.surfaces {
        let decision = decide_status(&cache, &entry.view_key, &entry.sha256);
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

    let deferred = deferred + surfaces.iter().filter(|s| s.status == "deferred").count();

    // Persist the updated cache so subsequent runs short-circuit reviewed surfaces.
    if args.do_ai {
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

    RunReport {
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
    }
}

async fn review_surface(args: &RunArgs<'_>, entry: &ManifestEntry, model: &str) -> SurfaceReport {
    use crate::visus_review::{prompt, vision_call};
    let png = match std::fs::read(args.screens_dir.join(&entry.file)) {
        Ok(b) => b,
        Err(e) => return failed_surface(entry, &format!("read png: {e}")),
    };
    let t0 = std::time::Instant::now();
    let res = vision_call::call_vision_model(
        model,
        &prompt::system_prompt(),
        &prompt::user_prompt(&entry.view_key),
        &png,
    )
    .await;
    let review_ms = t0.elapsed().as_millis() as u64;
    match res {
        Ok((content, usage)) => match parse_verdict(&content) {
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

pub fn write_report(
    report_dir: &Path,
    date: &str,
    report: &RunReport,
) -> std::io::Result<std::path::PathBuf> {
    std::fs::create_dir_all(report_dir)?;
    let path = report_dir.join(format!("{date}.json"));
    std::fs::write(&path, serde_json::to_string_pretty(report).unwrap() + "\n")?;
    Ok(path)
}
