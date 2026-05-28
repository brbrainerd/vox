//! Versioned search policy loaded from defaults with `VOX_SEARCH_*` environment overrides.
//!
//! SearXNG `engines` / `language` defaults are embedded from
//! [`contracts/scientia/searxng-query.defaults.v1.yaml`](../../../contracts/scientia/searxng-query.defaults.v1.yaml).

use serde::{Deserialize, Serialize};

use crate::searxng_defaults::embedded_searxng_query_defaults;

/// Policy version mirrored into [`vox_db::SearchDiagnostics::policy_version`] and notes.
pub const SEARCH_POLICY_DEFAULT_VERSION: u32 = 1;

#[inline]
fn default_chunk_vector_fusion_weight() -> f32 {
    0.60
}

#[inline]
fn default_memory_bm25_k1() -> f64 {
    1.2
}

#[inline]
fn default_memory_bm25_b() -> f64 {
    0.75
}

#[inline]
fn default_rrf_k() -> f64 {
    60.0
}

#[inline]
fn default_persist_web_hits() -> bool {
    true
}

/// Tunable retrieval weights and safety rails (replaces ad hoc literals in tool surfaces).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchPolicy {
    /// Monotonic policy schema version.
    pub version: u32,
    /// Weight on the vector leg in memory hybrid fusion (`fuse_hybrid_results`).
    pub memory_vector_fusion_weight: f32,
    /// Weight on the vector leg when fusing ingested `search_document_chunks` lexical + embedding hits.
    #[serde(default = "default_chunk_vector_fusion_weight")]
    pub chunk_vector_fusion_weight: f32,
    /// Trigger automatic verification when coarse evidence quality falls below this threshold.
    pub verification_weak_evidence_threshold: f64,
    /// Weight of top fused score when estimating `evidence_quality`.
    pub evidence_quality_top_weight: f64,
    /// Weight of citation coverage when estimating `evidence_quality`.
    pub evidence_quality_coverage_weight: f64,
    /// Maximum repository files scanned for path-token inventory search per query.
    pub repo_inventory_max_files: usize,
    /// Repository directory names skipped during inventory walks.
    pub repo_inventory_skip_dirs: Vec<String>,
    /// Qdrant HTTP API base (`None` disables sidecar vector path).
    pub qdrant_url: Option<String>,
    /// Named Qdrant collection for repo/doc embeddings mirror.
    pub qdrant_collection: String,
    /// When the collection uses **named** dense vectors, set to the config name (e.g. `default`).
    pub qdrant_vector_name: Option<String>,
    /// Root directory for on-disk Tantivy indices (under `.vox/search/tantivy` by default).
    pub tantivy_index_root: Option<std::path::PathBuf>,
    /// Enable reciprocal rank fusion across corpus hit lists (`VOX_SEARCH_PREFER_RRF`).
    pub prefer_rrf_merge: bool,
    /// Master switch for live web retrieval.
    pub tavily_enabled: bool,
    /// API depth: basic or advanced.
    pub tavily_search_depth: String,
    /// Max results per search call.
    pub tavily_max_results: usize,
    /// Auto-fire when ALL local corpora empty.
    pub tavily_fire_on_empty: bool,
    /// Auto-fire when evidence_quality < threshold (CRAG mode).
    pub tavily_fire_on_weak: bool,
    /// Max credits per session (safety rail).
    pub tavily_credit_budget_per_session: usize,
    /// SearXNG base URL (`None` disables Tier 2).
    pub searxng_url: Option<String>,
    /// Max search results to request from SearXNG/DDG.
    pub searxng_max_results: usize,
    /// Max top hits to deep-scrape for markdown extraction.
    pub searxng_max_urls_to_scrape: usize,
    /// SearXNG `engines=` query parameter (comma-separated engine ids).
    pub searxng_engines: String,
    /// SearXNG `language=` query parameter (short language tag).
    pub searxng_language: String,
    /// Enable Tier 3 DuckDuckGo fallback when SearXNG is unavailable.
    pub duckduckgo_fallback_enabled: bool,
    /// Scraper fetch timeout.
    pub scraper_timeout_ms: u64,
    /// Honor robots.txt (experimental).
    pub scraper_robots_txt_respect: bool,
    /// Minimum text density to accept scraped content (noise reduction).
    pub scraper_min_text_density: f64,
    /// Max iterative search hops for multi-hop research.
    pub web_search_max_hops: u8,
    /// BM25 `k1` for in-process markdown memory ranking (`VOX_SEARCH_BM25_K1`).
    #[serde(default = "default_memory_bm25_k1")]
    pub memory_bm25_k1: f64,
    /// BM25 `b` for in-process markdown memory ranking (`VOX_SEARCH_BM25_B`).
    #[serde(default = "default_memory_bm25_b")]
    pub memory_bm25_b: f64,
    /// Reciprocal rank fusion smoothing constant (`VOX_SEARCH_RRF_K`).
    #[serde(default = "default_rrf_k")]
    pub rrf_k: f64,
    /// When true, web-retrieval hits are mirrored into `search_documents` (async best-effort).
    #[serde(default = "default_persist_web_hits")]
    pub persist_web_hits: bool,
}

/// Aggregated SCIENTIA observations that can tune retrieval policy for a run.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SearchPolicyFeedback {
    /// Citation/source precision in the latest window, normalized 0.0..1.0.
    pub citation_precision: f64,
    /// Model self-verification reliability in the latest window, normalized 0.0..1.0.
    pub model_reliability: f64,
    /// Source hit rate in the latest window, normalized 0.0..1.0.
    pub source_hit_rate: f64,
}

impl Default for SearchPolicy {
    fn default() -> Self {
        let searxng_embedded = embedded_searxng_query_defaults();
        Self {
            version: SEARCH_POLICY_DEFAULT_VERSION,
            memory_vector_fusion_weight: 0.55,
            chunk_vector_fusion_weight: 0.60,
            verification_weak_evidence_threshold: 0.55,
            evidence_quality_top_weight: 0.7,
            evidence_quality_coverage_weight: 0.3,
            repo_inventory_max_files: 20_000,
            repo_inventory_skip_dirs: vec![
                ".git".to_string(),
                "target".to_string(),
                "node_modules".to_string(),
                ".next".to_string(),
                ".vox".to_string(),
                "dist".to_string(),
                "build".to_string(),
            ],
            qdrant_url: vox_secrets::resolve_secret(vox_secrets::SecretId::VoxSearchQdrantUrl)
                .expose()
                .filter(|s| !s.trim().is_empty())
                .map(|s| s.trim().to_string()),
            qdrant_collection: vox_secrets::resolve_secret(
                vox_secrets::SecretId::VoxSearchQdrantCollection,
            )
            .expose()
            .unwrap_or("vox_docs")
            .to_string(),
            qdrant_vector_name: vox_secrets::resolve_secret(
                vox_secrets::SecretId::VoxSearchQdrantVectorName,
            )
            .expose()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
            tantivy_index_root: vox_secrets::resolve_secret(
                vox_secrets::SecretId::VoxSearchTantivyRoot,
            )
            .expose()
            .filter(|s| !s.trim().is_empty())
            .map(std::path::PathBuf::from),
            prefer_rrf_merge: parse_truthy_env(vox_secrets::SecretId::VoxSearchPreferRrf),
            tavily_enabled: parse_truthy_env(vox_secrets::SecretId::VoxSearchTavilyEnabled),
            tavily_search_depth: vox_secrets::resolve_secret(
                vox_secrets::SecretId::VoxSearchTavilyDepth,
            )
            .expose()
            .unwrap_or("basic")
            .to_string(),
            tavily_max_results: vox_secrets::resolve_secret(
                vox_secrets::SecretId::VoxSearchTavilyMaxResults,
            )
            .expose()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5),
            tavily_fire_on_empty: match vox_secrets::resolve_secret(
                vox_secrets::SecretId::VoxSearchTavilyOnEmpty,
            )
            .expose()
            {
                Some(v) => {
                    let v = v.trim();
                    v == "1"
                        || v.eq_ignore_ascii_case("true")
                        || v.eq_ignore_ascii_case("yes")
                        || v.eq_ignore_ascii_case("on")
                }
                None => true,
            },
            tavily_fire_on_weak: parse_truthy_env(vox_secrets::SecretId::VoxSearchTavilyOnWeak),
            tavily_credit_budget_per_session: vox_secrets::resolve_secret(
                vox_secrets::SecretId::VoxSearchTavilyBudget,
            )
            .expose()
            .and_then(|v| v.parse().ok())
            .unwrap_or(50),
            searxng_url: vox_secrets::resolve_secret(vox_secrets::SecretId::VoxSearchSearxngUrl)
                .expose()
                .filter(|s| !s.trim().is_empty())
                .map(|s| s.to_string()),
            searxng_max_results: vox_secrets::resolve_secret(
                vox_secrets::SecretId::VoxSearchSearxngMaxResults,
            )
            .expose()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5),
            searxng_max_urls_to_scrape: vox_secrets::resolve_secret(
                vox_secrets::SecretId::VoxSearchSearxngMaxScrape,
            )
            .expose()
            .and_then(|v| v.parse().ok())
            .unwrap_or(3),
            searxng_engines: searxng_embedded.engines.clone(),
            searxng_language: searxng_embedded.language.clone(),
            duckduckgo_fallback_enabled: !parse_falsy_env(
                vox_secrets::SecretId::VoxSearchDdgFallbackDisabled,
            ),
            scraper_timeout_ms: vox_secrets::resolve_secret(
                vox_secrets::SecretId::VoxSearchScraperTimeout,
            )
            .expose()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5000),
            scraper_robots_txt_respect: parse_truthy_env(
                vox_secrets::SecretId::VoxSearchScraperRobotsRespect,
            ),
            scraper_min_text_density: vox_secrets::resolve_secret(
                vox_secrets::SecretId::VoxSearchScraperMinDensity,
            )
            .expose()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.15),
            web_search_max_hops: vox_secrets::resolve_secret(
                vox_secrets::SecretId::VoxSearchMaxHops,
            )
            .expose()
            .and_then(|v| v.parse().ok())
            .unwrap_or(3),
            memory_bm25_k1: vox_secrets::resolve_secret(vox_secrets::SecretId::VoxSearchBm25K1)
                .expose()
                .and_then(|v| v.parse::<f64>().ok())
                .filter(|x| x.is_finite() && *x > 0.0)
                .unwrap_or_else(default_memory_bm25_k1),
            memory_bm25_b: vox_secrets::resolve_secret(vox_secrets::SecretId::VoxSearchBm25B)
                .expose()
                .and_then(|v| v.parse::<f64>().ok())
                .filter(|x| x.is_finite())
                .unwrap_or_else(default_memory_bm25_b),
            rrf_k: vox_secrets::resolve_secret(vox_secrets::SecretId::VoxSearchRrfK)
                .expose()
                .and_then(|v| v.parse::<f64>().ok())
                .filter(|x| x.is_finite() && *x > 0.0)
                .unwrap_or_else(default_rrf_k),
            persist_web_hits: !parse_truthy_env(
                vox_secrets::SecretId::VoxSearchPersistWebHitsDisabled,
            ),
        }
    }
}

impl SearchPolicy {
    /// Environment overrides merged onto [`Default::default`].
    #[must_use]
    pub fn from_env() -> Self {
        let mut p = Self::default();
        if let Some(v) =
            vox_secrets::resolve_secret(vox_secrets::SecretId::VoxSearchPolicyVersion).expose()
            && let Ok(n) = v.parse::<u32>()
        {
            p.version = n;
        }
        if let Some(v) =
            vox_secrets::resolve_secret(vox_secrets::SecretId::VoxSearchMemoryVectorWeight).expose()
            && let Ok(w) = v.parse::<f32>()
        {
            p.memory_vector_fusion_weight = w.clamp(0.0, 1.0);
        }
        if let Some(v) =
            vox_secrets::resolve_secret(vox_secrets::SecretId::VoxSearchChunkVectorWeight).expose()
            && let Ok(w) = v.parse::<f32>()
        {
            p.chunk_vector_fusion_weight = w.clamp(0.0, 1.0);
        }
        if let Some(v) = vox_secrets::resolve_secret(
            vox_secrets::SecretId::VoxSearchVerificationQualityThreshold,
        )
        .expose()
            && let Ok(t) = v.parse::<f64>()
        {
            p.verification_weak_evidence_threshold = t.clamp(0.0, 1.0);
        }
        if let Some(v) =
            vox_secrets::resolve_secret(vox_secrets::SecretId::VoxSearchRepoMaxFiles).expose()
            && let Ok(n) = v.parse::<usize>()
        {
            p.repo_inventory_max_files = n.max(100);
        }
        if let Some(raw) =
            vox_secrets::resolve_secret(vox_secrets::SecretId::VoxSearchRepoSkipDirs).expose()
        {
            let dirs: Vec<String> = raw
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if !dirs.is_empty() {
                p.repo_inventory_skip_dirs = dirs;
            }
        }
        if vox_secrets::resolve_secret(vox_secrets::SecretId::VoxSearchTavilyEnabled)
            .expose()
            .is_some()
        {
            p.tavily_enabled = parse_truthy_env(vox_secrets::SecretId::VoxSearchTavilyEnabled);
        }
        if let Some(v) =
            vox_secrets::resolve_secret(vox_secrets::SecretId::VoxSearchTavilyDepth).expose()
        {
            p.tavily_search_depth = v.to_string();
        }
        if let Some(v) =
            vox_secrets::resolve_secret(vox_secrets::SecretId::VoxSearchTavilyMaxResults).expose()
            && let Ok(n) = v.parse::<usize>()
        {
            p.tavily_max_results = n;
        }
        if vox_secrets::resolve_secret(vox_secrets::SecretId::VoxSearchTavilyOnEmpty)
            .expose()
            .is_some()
        {
            p.tavily_fire_on_empty =
                parse_truthy_env(vox_secrets::SecretId::VoxSearchTavilyOnEmpty);
        }
        if vox_secrets::resolve_secret(vox_secrets::SecretId::VoxSearchTavilyOnWeak)
            .expose()
            .is_some()
        {
            p.tavily_fire_on_weak = parse_truthy_env(vox_secrets::SecretId::VoxSearchTavilyOnWeak);
        }
        if let Some(v) =
            vox_secrets::resolve_secret(vox_secrets::SecretId::VoxSearchTavilyBudget).expose()
            && let Ok(n) = v.parse::<usize>()
        {
            p.tavily_credit_budget_per_session = n;
        }
        if let Some(v) =
            vox_secrets::resolve_secret(vox_secrets::SecretId::VoxSearchMaxHops).expose()
            && let Ok(n) = v.parse::<u8>()
        {
            p.web_search_max_hops = n;
        }
        if let Some(v) =
            vox_secrets::resolve_secret(vox_secrets::SecretId::VoxSearchBm25K1).expose()
            && let Ok(x) = v.parse::<f64>()
            && x.is_finite()
            && x > 0.0
        {
            p.memory_bm25_k1 = x;
        }
        if let Some(v) = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxSearchBm25B).expose()
            && let Ok(x) = v.parse::<f64>()
            && x.is_finite()
        {
            p.memory_bm25_b = x;
        }
        if let Some(v) = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxSearchRrfK).expose()
            && let Ok(x) = v.parse::<f64>()
            && x.is_finite()
            && x > 0.0
        {
            p.rrf_k = x;
        }
        if vox_secrets::resolve_secret(vox_secrets::SecretId::VoxSearchPersistWebHitsDisabled)
            .expose()
            .is_some()
        {
            p.persist_web_hits =
                !parse_truthy_env(vox_secrets::SecretId::VoxSearchPersistWebHitsDisabled);
        }
        if let Some(v) =
            vox_secrets::resolve_secret(vox_secrets::SecretId::VoxSearchSearxngEngines).expose()
        {
            if let Some(norm) = normalize_searxng_engines_csv(v) {
                p.searxng_engines = norm;
            } else {
                tracing::warn!(
                    raw = %v,
                    "VOX_SEARCH_SEARXNG_ENGINES ignored (allowed: ASCII alnum, comma, hyphen, underscore)"
                );
            }
        }
        if let Some(v) =
            vox_secrets::resolve_secret(vox_secrets::SecretId::VoxSearchSearxngLanguage).expose()
        {
            if let Some(norm) = normalize_searxng_language_tag(v) {
                p.searxng_language = norm;
            } else {
                tracing::warn!(
                    raw = %v,
                    "VOX_SEARCH_SEARXNG_LANGUAGE ignored (allowed: ASCII alnum and hyphen, max 16 chars)"
                );
            }
        }
        p
    }

    /// Apply SCIENTIA feedback while preserving the free/no-paid-service default.
    #[must_use]
    pub fn with_scientia_feedback(mut self, feedback: SearchPolicyFeedback) -> Self {
        let citation_precision = feedback.citation_precision.clamp(0.0, 1.0);
        let model_reliability = feedback.model_reliability.clamp(0.0, 1.0);
        let source_hit_rate = feedback.source_hit_rate.clamp(0.0, 1.0);

        if citation_precision < 0.65 || source_hit_rate < 0.5 {
            self.verification_weak_evidence_threshold =
                (self.verification_weak_evidence_threshold + 0.10).min(0.85);
            self.web_search_max_hops = self.web_search_max_hops.saturating_add(1).min(5);
        }
        if model_reliability < 0.6 {
            self.evidence_quality_coverage_weight =
                (self.evidence_quality_coverage_weight + 0.10).min(0.6);
            self.evidence_quality_top_weight =
                (1.0 - self.evidence_quality_coverage_weight).max(0.4);
        }
        if citation_precision >= 0.85 && source_hit_rate >= 0.8 {
            self.verification_weak_evidence_threshold =
                (self.verification_weak_evidence_threshold - 0.05).max(0.45);
        }
        self
    }

    /// Effective fusion weight clamped to `[0, 1]`.
    #[must_use]
    pub fn clamped_memory_vector_weight(&self) -> f32 {
        self.memory_vector_fusion_weight.clamp(0.0, 1.0)
    }

    /// Effective chunk hybrid fusion weight on the vector leg, clamped to `[0, 1]`.
    ///
    /// Note: [`Self::from_env`] may clamp weights when parsing secrets; this helper clamps again
    /// for programmatic mutations (tests / dynamic policy).
    #[must_use]
    pub fn clamped_chunk_vector_weight(&self) -> f32 {
        self.chunk_vector_fusion_weight.clamp(0.0, 1.0)
    }

    /// BM25 `k1` clamped to a sane interval for memory markdown ranking.
    #[must_use]
    pub fn clamped_memory_bm25_k1(&self) -> f64 {
        self.memory_bm25_k1.clamp(0.1, 3.0)
    }

    /// BM25 `b` clamped to `[0, 1]`.
    #[must_use]
    pub fn clamped_memory_bm25_b(&self) -> f64 {
        self.memory_bm25_b.clamp(0.0, 1.0)
    }

    /// RRF smoothing constant `k` clamped for numerical stability.
    #[must_use]
    pub fn clamped_rrf_k(&self) -> f64 {
        self.rrf_k.clamp(1.0, 500.0)
    }

    /// Sanitized `engines=` value for SearXNG requests.
    #[must_use]
    pub fn searxng_engines_csv(&self) -> &str {
        self.searxng_engines.as_str()
    }

    /// Sanitized `language=` value for SearXNG requests.
    #[must_use]
    pub fn searxng_language_tag(&self) -> &str {
        self.searxng_language.as_str()
    }
}

/// Restrict `engines=` to characters SearXNG engine bundles use (prevents query injection).
fn normalize_searxng_engines_csv(raw: &str) -> Option<String> {
    let t = raw.trim();
    if t.is_empty() || t.len() > 256 {
        return None;
    }
    if !t
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, ',' | '_' | '-'))
    {
        return None;
    }
    Some(t.to_string())
}

fn normalize_searxng_language_tag(raw: &str) -> Option<String> {
    let t = raw.trim();
    if t.is_empty() || t.len() > 16 {
        return None;
    }
    if !t.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return None;
    }
    Some(t.to_string())
}

fn parse_truthy_env(id: vox_secrets::SecretId) -> bool {
    match vox_secrets::resolve_secret(id).expose() {
        Some(v) => {
            let v = v.trim();
            v == "1"
                || v.eq_ignore_ascii_case("true")
                || v.eq_ignore_ascii_case("yes")
                || v.eq_ignore_ascii_case("on")
        }
        None => false,
    }
}

fn parse_falsy_env(id: vox_secrets::SecretId) -> bool {
    match vox_secrets::resolve_secret(id).expose() {
        Some(v) => {
            let v = v.trim();
            v == "0"
                || v.eq_ignore_ascii_case("false")
                || v.eq_ignore_ascii_case("no")
                || v.eq_ignore_ascii_case("off")
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn searxng_engine_csv_accepts_baseline() {
        let s = normalize_searxng_engines_csv("google,bing,ddg").expect("baseline engines");
        assert_eq!(s, "google,bing,ddg");
        assert!(normalize_searxng_engines_csv("bad;injection").is_none());
    }

    #[test]
    fn searxng_language_tag_accepts_en_us() {
        assert_eq!(
            normalize_searxng_language_tag("en-US").as_deref(),
            Some("en-US")
        );
        assert!(normalize_searxng_language_tag("en_US").is_none());
    }

    #[test]
    fn scientia_feedback_tightens_weak_source_policy_without_tavily() {
        let policy = SearchPolicy {
            tavily_enabled: false,
            ..SearchPolicy::default()
        };
        let adjusted = policy.clone().with_scientia_feedback(SearchPolicyFeedback {
            citation_precision: 0.3,
            model_reliability: 0.5,
            source_hit_rate: 0.2,
        });

        assert!(
            adjusted.verification_weak_evidence_threshold
                > policy.verification_weak_evidence_threshold
        );
        assert!(adjusted.web_search_max_hops > policy.web_search_max_hops);
        assert!(!adjusted.tavily_enabled);
    }
}
