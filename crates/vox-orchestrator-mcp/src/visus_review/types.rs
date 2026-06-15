use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Deserialize)]
pub struct VisualReviewConfig {
    pub schema_version: u32,
    pub model_preference: Vec<String>,
    pub escalation_model: String,
    pub per_surface_review_budget_ms: u64,
    pub total_review_budget_ms: u64,
    pub max_concurrent_reviews: usize,
    pub max_image_edge_px: u32,
    pub spike_factor: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub principle: String,
    pub severity: String,
    pub region: String,
    pub critique: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewVerdict {
    pub score: u32,
    pub verdict: String,
    pub findings: Vec<Finding>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SurfaceReport {
    pub view_key: String,
    pub screenshot_sha256: String,
    pub status: String,
    pub score: Option<u32>,
    pub verdict: Option<String>,
    pub findings: Vec<Finding>,
    pub model: Option<String>,
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub cost_usd: Option<f64>,
    pub review_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunReport {
    pub schema_version: u32,
    pub generated_at: String,
    pub default_model: String,
    pub surfaces: Vec<SurfaceReport>,
    pub total_capture_ms: u64,
    pub total_review_ms: u64,
    pub surfaces_reviewed: usize,
    pub surfaces_cached: usize,
    pub surfaces_deferred: usize,
    pub spiked: bool,
    pub spike_detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub screenshot_sha256: String,
    pub score: u32,
    pub verdict: String,
    pub model: String,
    pub reviewed_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CacheIndex {
    #[serde(default = "default_schema")]
    pub schema_version: u32,
    #[serde(default)]
    pub entries: BTreeMap<String, CacheEntry>,
}
fn default_schema() -> u32 {
    1
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn config_parses_model_preference_and_budgets() {
        let json = r#"{ "schema_version":1, "model_preference":["google/gemini-3-flash-preview","google/gemini-2.5-flash"], "escalation_model":"anthropic/claude-opus-4.8", "per_surface_review_budget_ms":8000, "total_review_budget_ms":90000, "max_concurrent_reviews":3, "max_image_edge_px":2880, "spike_factor":1.5 }"#;
        let cfg: VisualReviewConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.model_preference[0], "google/gemini-3-flash-preview");
        assert_eq!(cfg.total_review_budget_ms, 90_000);
        assert_eq!(cfg.spike_factor, 1.5);
    }
    #[test]
    fn cache_roundtrips() {
        let mut idx = CacheIndex::default();
        idx.entries.insert(
            "dashboard".into(),
            CacheEntry {
                screenshot_sha256: "a".repeat(64),
                score: 82,
                verdict: "pass_with_notes".into(),
                model: "google/gemini-3-flash-preview".into(),
                reviewed_at: "2026-06-15T00:00:00Z".into(),
            },
        );
        let s = serde_json::to_string(&idx).unwrap();
        let back: CacheIndex = serde_json::from_str(&s).unwrap();
        assert_eq!(back.entries["dashboard"].score, 82);
    }
}
