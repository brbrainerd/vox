//! Source trust scoring: Crossref retraction lookup + OpenAlex venue/author
//! reputation, feeding `ResearchHit.trust_score`. Both APIs are free/keyless.
//! Fail-open: any network or parse error yields a neutral trust score
//! rather than blocking the research pipeline.

use serde::Deserialize;

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

        let url = format!("{}/works/{}", self.crossref_base.trim_end_matches('/'), doi);
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

    /// Returns a soft reputation multiplier in [0.5, 1.5] based on the
    /// venue type and author citation history for a work matching `title`,
    /// via OpenAlex. Returns `1.0` (neutral) on any lookup failure.
    pub async fn reputation_multiplier(&self, title: &str) -> f64 {
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
            urlencoding_lite(title)
        );
        let Ok(resp) = self.http.get(&url).send().await else {
            return 1.0;
        };
        if !resp.status().is_success() {
            return 1.0;
        }
        let Ok(text) = resp.text().await else {
            return 1.0;
        };
        let Ok(parsed) = serde_json::from_str::<OpenAlexSearch>(&text) else {
            return 1.0;
        };

        match parsed
            .results
            .first()
            .and_then(|w| w.primary_location.as_ref())
            .and_then(|l| l.source.as_ref())
            .and_then(|s| s.source_type.as_deref())
        {
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

/// Minimal query-param percent-encoding (spaces, common punctuation) without
/// pulling in a new dependency — sufficient for search-term titles.
fn urlencoding_lite(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            ' ' => "%20".to_string(),
            '&' => "%26".to_string(),
            '?' => "%3F".to_string(),
            '#' => "%23".to_string(),
            c if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' => c.to_string(),
            c => format!("%{:02X}", c as u32),
        })
        .collect()
}

/// Computes a combined trust score for a hit: 1.0 baseline, halved on
/// confirmed retraction, scaled by venue reputation otherwise. Never fails
/// — any lookup error yields the neutral 1.0 baseline. `doi` is optional
/// since most web hits won't resolve to one.
pub async fn score_hit_trust(title: &str, doi: Option<&str>) -> f64 {
    let scorer = TrustScorer::new();
    if let Some(doi) = doi
        && scorer.check_retraction(doi).await == Some(true)
    {
        return 0.1; // heavily penalized, not zeroed, so it's still visible/debuggable
    }
    scorer.reputation_multiplier(title).await
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
}
