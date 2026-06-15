//! GUI visual AI adversarial review. Advisory: never gates CI.
pub mod types;
pub use types::*;

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

pub fn run(args: &RunArgs<'_>) -> RunReport {
    let manifest: Manifest = std::fs::read_to_string(args.manifest_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(Manifest {
            total_capture_ms: 0,
            surfaces: vec![],
        });
    let cache: CacheIndex = std::fs::read_to_string(args.cache_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    let mut surfaces = Vec::new();
    let (mut reviewed, mut cached, deferred) = (0usize, 0usize, 0usize);

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
                    surfaces.push(review_surface(args, entry));
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

    RunReport {
        schema_version: 1,
        generated_at: args.now_iso.clone(),
        default_model: String::new(),
        surfaces,
        total_capture_ms: manifest.total_capture_ms,
        total_review_ms: 0,
        surfaces_reviewed: reviewed,
        surfaces_cached: cached,
        surfaces_deferred: deferred,
        spiked: false,
        spike_detail: String::new(),
    }
}

fn review_surface(_args: &RunArgs<'_>, entry: &ManifestEntry) -> SurfaceReport {
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
