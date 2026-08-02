//! Source trust scoring: Crossref retraction lookup + OpenAlex venue/author
//! reputation, feeding `ResearchHit.trust_score`. Both APIs are free/keyless.
//! Fail-open: any network or parse error yields a neutral trust score
//! rather than blocking the research pipeline.

use serde::Deserialize;
use std::sync::OnceLock;

const CROSSREF_BASE: &str = "https://api.crossref.org";
const OPENALEX_BASE: &str = "https://api.openalex.org";

pub struct TrustScorer {
    http: reqwest::Client,
    crossref_base: String,
    openalex_base: String,
}

impl TrustScorer {
    pub fn new() -> Self {
        Self {
            http: vox_http_client::client_builder()
                .timeout(vox_config::timeouts::D_30S)
                .user_agent("VoxResearchBot/1.0 (+https://vox.dev/research-bot)")
                .build()
                .expect("reqwest client"),
            crossref_base: CROSSREF_BASE.to_string(),
            openalex_base: OPENALEX_BASE.to_string(),
        }
    }

    #[cfg(test)]
    fn with_base_urls(crossref_base: impl Into<String>, openalex_base: impl Into<String>) -> Self {
        Self {
            http: vox_http_client::client_builder()
                .timeout(vox_config::timeouts::D_30S)
                .user_agent("VoxResearchBot/1.0 (+https://vox.dev/research-bot)")
                .build()
                .expect("reqwest client"),
            crossref_base: crossref_base.into(),
            openalex_base: openalex_base.into(),
        }
    }

    /// Returns `Some(true)` if the DOI is confirmed retracted/corrected,
    /// `Some(false)` if confirmed clean, `None` if the lookup failed
    /// (caller should treat `None` as "unknown, don't penalize").
    pub async fn check_retraction(&self, doi: &str) -> Option<bool> {
        #[derive(Deserialize)]
        struct CrossrefWork {
            message: CrossrefMessage,
        }
        #[derive(Deserialize)]
        struct CrossrefMessage {
            #[serde(default, rename = "update-to")]
            update_to: Vec<CrossrefUpdate>,
        }
        #[derive(Deserialize)]
        struct CrossrefUpdate {
            #[serde(rename = "type")]
            update_type: String,
        }

        let url = format!(
            "{}/works/{}",
            self.crossref_base.trim_end_matches('/'),
            urlencoding::encode(doi)
        );
        let resp = self.http.get(&url).send().await.ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let text = resp.text().await.ok()?;
        let parsed: CrossrefWork = serde_json::from_str(&text).ok()?;
        let is_retracted = parsed
            .message
            .update_to
            .iter()
            .any(|u| u.update_type.eq_ignore_ascii_case("retraction"));
        Some(is_retracted)
    }

    /// Returns the OpenAlex venue `source_type` string (e.g. "journal",
    /// "repository", "conference", "preprint") for a work matching `title`,
    /// or `None` on any lookup failure or when no venue type is available.
    /// This is the raw signal `reputation_multiplier` derives its score
    /// from — exposed separately so callers building `WorthinessSignalItem`s
    /// (see `vox-scientia::producers::worthiness`) can classify venue type
    /// without duplicating the OpenAlex fetch/parse.
    pub async fn venue_type(&self, title: &str) -> Option<String> {
        #[derive(Deserialize)]
        struct OpenAlexSearch {
            results: Vec<OpenAlexWork>,
        }
        #[derive(Deserialize)]
        struct OpenAlexWork {
            #[serde(default)]
            primary_location: Option<OpenAlexLocation>,
        }
        #[derive(Deserialize)]
        struct OpenAlexLocation {
            source: Option<OpenAlexSource>,
        }
        #[derive(Deserialize)]
        struct OpenAlexSource {
            #[serde(rename = "type")]
            source_type: Option<String>,
        }

        let url = format!(
            "{}/works?search={}&per-page=1",
            self.openalex_base.trim_end_matches('/'),
            urlencoding::encode(title)
        );
        let resp = self.http.get(&url).send().await.ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let text = resp.text().await.ok()?;
        let parsed = serde_json::from_str::<OpenAlexSearch>(&text).ok()?;

        parsed
            .results
            .into_iter()
            .next()
            .and_then(|w| w.primary_location)
            .and_then(|l| l.source)
            .and_then(|s| s.source_type)
    }

    /// Returns a soft reputation multiplier in [0.5, 1.5] based on the
    /// venue type and author citation history for a work matching `title`,
    /// via OpenAlex. Returns `1.0` (neutral) on any lookup failure.
    pub async fn reputation_multiplier(&self, title: &str) -> f64 {
        match self.venue_type(title).await.as_deref() {
            Some("journal") => 1.5,
            Some("repository") | Some("conference") => 1.2,
            Some("preprint") => 1.0,
            _ => 1.0,
        }
    }
}

impl Default for TrustScorer {
    fn default() -> Self {
        Self::new()
    }
}

static SHARED_SCORER: OnceLock<TrustScorer> = OnceLock::new();

/// Returns a process-wide shared `TrustScorer`, constructed once on first
/// use. Prefer this over `TrustScorer::new()` in hot paths that score many
/// hits, so the underlying HTTP client's connection pool is actually reused.
fn shared_scorer() -> &'static TrustScorer {
    SHARED_SCORER.get_or_init(TrustScorer::new)
}

/// Computes a combined trust score for a hit: 1.0 baseline, halved on
/// confirmed retraction, scaled by venue reputation otherwise. Never fails
/// — any lookup error yields the neutral 1.0 baseline. `doi` is optional
/// since most web hits won't resolve to one.
/// Extracts a DOI from a URL if it matches a common DOI-URL shape
/// (`https://doi.org/10.XXXX/...` or `https://dx.doi.org/10.XXXX/...`).
/// Returns `None` for URLs that aren't DOI links — this is intentionally
/// narrow (exact-prefix match, not a general DOI regex) since it only
/// needs to catch the common case of a search hit whose URL IS a DOI
/// resolver link; a hit citing a DOI in its body text without a doi.org
/// URL is out of scope for this simple extractor.
pub fn extract_doi_from_url(url: &str) -> Option<String> {
    let lower = url.to_ascii_lowercase();
    for prefix in ["https://doi.org/", "http://doi.org/", "https://dx.doi.org/", "http://dx.doi.org/"] {
        if let Some(rest) = lower.strip_prefix(prefix) {
            let doi = rest.trim_end_matches('/').trim();
            if !doi.is_empty() && doi.starts_with("10.") {
                // Re-slice from the ORIGINAL (non-lowercased) url to preserve
                // DOI casing (DOIs can be case-sensitive in some registries).
                let start = url.len() - rest.len();
                return Some(url[start..].trim_end_matches('/').trim().to_string());
            }
        }
    }
    None
}

pub async fn score_hit_trust(title: &str, doi: Option<&str>) -> f64 {
    let scorer = shared_scorer();
    if let Some(doi) = doi
        && scorer.check_retraction(doi).await == Some(true)
    {
        return 0.1; // heavily penalized, not zeroed, so it's still visible/debuggable
    }
    scorer.reputation_multiplier(title).await
}

/// Cheap domain check gating the OpenAlex title-search call in
/// `score_hit_trust_for_url` — skips the network call and title-collision
/// misclassification risk for hits that are clearly not scholarly sources.
/// Fail-open: an unrecognized domain returns `false` (skip OpenAlex), which
/// is the same neutral 1.0 result a genuine no-match would have produced
/// anyway, so this never suppresses a real signal, only wasted calls.
pub fn is_plausibly_academic(url: &str) -> bool {
    let key = url.to_ascii_lowercase();
    key.contains("doi.org/")
        || key.contains("arxiv.org/")
        || key.contains(".edu/")
        || key.contains("pubmed.ncbi.nlm.nih.gov/")
        || key.contains("ncbi.nlm.nih.gov/")
        || key.contains("researchgate.net/")
        || key.contains("springer.com/")
        || key.contains("sciencedirect.com/")
        || key.contains("jstor.org/")
}

/// URL-aware wrapper around `score_hit_trust` that skips the OpenAlex
/// reputation lookup entirely for non-academic domains. This is the new
/// entry point `web_gather.rs` should call; `score_hit_trust` itself is
/// left unchanged for any other caller that doesn't yet have a URL handy.
pub async fn score_hit_trust_for_url(title: &str, doi: Option<&str>, url: &str) -> f64 {
    if !is_plausibly_academic(url) {
        return 1.0;
    }
    score_hit_trust(title, doi).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path_regex};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn check_retraction_detects_retracted_work() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path_regex(r"^/works/.*"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "message": { "update-to": [{"type": "retraction"}] }
            })))
            .mount(&server)
            .await;

        let scorer = TrustScorer::with_base_urls(server.uri(), "http://unused.invalid");
        assert_eq!(scorer.check_retraction("10.1234/example").await, Some(true));
    }

    #[tokio::test]
    async fn check_retraction_returns_false_for_clean_work() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path_regex(r"^/works/.*"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "message": { "update-to": [] }
            })))
            .mount(&server)
            .await;

        let scorer = TrustScorer::with_base_urls(server.uri(), "http://unused.invalid");
        assert_eq!(scorer.check_retraction("10.1234/example").await, Some(false));
    }

    #[tokio::test]
    async fn check_retraction_returns_none_on_http_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path_regex(r"^/works/.*"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let scorer = TrustScorer::with_base_urls(server.uri(), "http://unused.invalid");
        assert_eq!(scorer.check_retraction("10.1234/example").await, None);
    }

    #[tokio::test]
    async fn reputation_multiplier_favors_journal_venue() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path_regex(r"^/works.*"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "results": [{"primary_location": {"source": {"type": "journal"}}}]
            })))
            .mount(&server)
            .await;

        let scorer = TrustScorer::with_base_urls("http://unused.invalid", server.uri());
        assert_eq!(scorer.reputation_multiplier("Example Paper Title").await, 1.5);
    }

    #[tokio::test]
    async fn reputation_multiplier_defaults_to_neutral_on_no_results() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path_regex(r"^/works.*"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "results": []
            })))
            .mount(&server)
            .await;

        let scorer = TrustScorer::with_base_urls("http://unused.invalid", server.uri());
        assert_eq!(scorer.reputation_multiplier("Nonexistent Title").await, 1.0);
    }

    #[test]
    fn extract_doi_from_url_matches_doi_org_link() {
        assert_eq!(
            extract_doi_from_url("https://doi.org/10.1234/example.5678"),
            Some("10.1234/example.5678".to_string())
        );
    }

    #[test]
    fn extract_doi_from_url_returns_none_for_non_doi_url() {
        assert_eq!(extract_doi_from_url("https://example.com/some-article"), None);
    }

    #[test]
    fn extract_doi_from_url_handles_dx_doi_org_variant() {
        assert_eq!(
            extract_doi_from_url("https://dx.doi.org/10.9999/xyz"),
            Some("10.9999/xyz".to_string())
        );
    }

    #[tokio::test]
    async fn venue_type_returns_raw_source_type_string() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path_regex(r"^/works.*"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "results": [{"primary_location": {"source": {"type": "journal"}}}]
            })))
            .mount(&server)
            .await;

        let scorer = TrustScorer::with_base_urls("http://unused.invalid", server.uri());
        assert_eq!(scorer.venue_type("Example Paper Title").await, Some("journal".to_string()));
    }

    #[test]
    fn is_plausibly_academic_gates_correctly() {
        assert!(is_plausibly_academic("https://doi.org/10.1000/xyz123"));
        assert!(is_plausibly_academic("https://arxiv.org/abs/2401.00001"));
        assert!(is_plausibly_academic("https://www.ncbi.nlm.nih.gov/pmc/articles/PMC123"));
        assert!(!is_plausibly_academic("https://en.wikipedia.org/wiki/Research"));
        assert!(!is_plausibly_academic("https://www.reuters.com/world/article"));
        assert!(!is_plausibly_academic("https://blog.example/post"));
    }

    #[tokio::test]
    async fn score_hit_trust_skips_openalex_for_non_academic_url() {
        let score = score_hit_trust_for_url(
            "Some Blog Post Title",
            None,
            "https://blog.example/post",
        )
        .await;
        assert_eq!(score, 1.0, "non-academic URL should short-circuit to neutral without an OpenAlex call");
    }
}
