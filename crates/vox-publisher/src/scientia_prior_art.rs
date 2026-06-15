//! Federated prior-art fetch (OpenAlex, Crossref, Semantic Scholar) → [`NoveltyEvidenceBundleV1`].
//!
//! Network I/O is best-effort: failures append traces and continue. Use `offline` for deterministic tests.

use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde_json::Value as JsonValue;
use sha2::{Digest, Sha256};

use crate::scientia_evidence::METADATA_KEY_SCIENTIA_NOVELTY_BUNDLE;
use crate::scientia_finding_ledger::{
    NormalizedPriorArtHit, NoveltyEvidenceBundleV1, NoveltyOverlapSummary, NoveltyQueryTrace,
    NoveltyRecencyBucket, PriorArtSource,
};
use crate::scientia_heuristics::ScientiaHeuristics;

#[derive(Debug, Clone)]
pub struct PriorArtQuery {
    pub title: String,
    pub abstract_text: Option<String>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PriorArtFetchOptions {
    pub mailto_for_crossref: Option<&'static str>,
}

#[must_use]
pub fn query_digest_sha256(query: &PriorArtQuery) -> String {
    let body = serde_json::json!({
        "title": query.title.trim(),
        "abstract": query.abstract_text.as_ref().map(|s| s.trim()),
    });
    let raw = serde_json::to_vec(&body).unwrap_or_default();
    sha256_hex(&raw)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let h = Sha256::digest(bytes);
    h.iter().map(|b| format!("{b:02x}")).collect()
}

fn trace_fp(source: &str, digest: &str) -> String {
    sha256_hex(format!("{source}:{digest}").as_bytes())
}

fn system_time_map_or<T, F: FnOnce(std::time::Duration) -> T>(
    st: SystemTime,
    default: T,
    f: F,
) -> T {
    st.duration_since(UNIX_EPOCH).map_or(default, f)
}

#[must_use]
pub fn now_unix_ms_strict() -> i64 {
    system_time_map_or(SystemTime::now(), 0, |d| {
        i64::try_from(d.as_millis()).unwrap_or(0)
    })
}

fn tokenize(s: &str, min_len: usize) -> HashSet<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t: &&str| t.len() > min_len)
        .map(|s: &str| std::string::ToString::to_string(s))
        .collect()
}

/// Title + abstract snippet used for OpenAlex/Crossref/S2 search and lexical overlap.
#[must_use]
pub fn prior_art_search_text(query: &PriorArtQuery, h: &ScientiaHeuristics) -> String {
    let mut t = query.title.trim().to_string();
    if let Some(a) = query
        .abstract_text
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let cap = h.prior_art_abstract_max_chars;
        if cap > 0 {
            let slice: String = a.chars().take(cap).collect();
            t.push(' ');
            t.push_str(&slice);
        }
    }
    t
}

/// Lexical similarity in \[0,1\] (Jaccard over tokens).
#[must_use]
pub fn title_lexical_score(query_title: &str, hit_title: &str) -> f64 {
    title_lexical_score_with_min_len(query_title, hit_title, 2)
}

/// Lexical score with tunable token length floor (from dynamics seed).
#[must_use]
pub fn title_lexical_score_with_min_len(
    query_title: &str,
    hit_title: &str,
    token_min_len: usize,
) -> f64 {
    let min_len = token_min_len.max(1);
    let a = tokenize(query_title, min_len);
    let b = tokenize(hit_title, min_len);
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    let inter = a.intersection(&b).count();
    let union = a.union(&b).count().max(1);
    (inter as f64 / union as f64).clamp(0.0, 1.0)
}

fn recency_from_years(years: &[Option<i32>]) -> NoveltyRecencyBucket {
    let cy = system_time_map_or(SystemTime::now(), 2026, |d| {
        1970 + (d.as_secs() / 31_536_000) as i32
    });
    let Some(ym) = years.iter().filter_map(|y: &Option<i32>| *y).max() else {
        return NoveltyRecencyBucket::Unknown;
    };
    let age = cy - ym;
    if age <= 1 {
        NoveltyRecencyBucket::VeryRecent
    } else if age <= 4 {
        NoveltyRecencyBucket::Recent
    } else {
        NoveltyRecencyBucket::Stale
    }
}

/// Build an empty bundle (offline / no hits).
///
/// Both score fields are `None`: no search ran, so we must not fabricate
/// "we searched and found nothing". Callers that consume this bundle through
/// `AtomicNoveltyScorer::score` will receive `InsufficientEvidence`.
#[must_use]
pub fn empty_novelty_bundle(candidate_id: &str, query: &PriorArtQuery) -> NoveltyEvidenceBundleV1 {
    let qd = query_digest_sha256(query);
    NoveltyEvidenceBundleV1 {
        schema_version: 1,
        bundle_id: format!("nb.{}", &qd[..16.min(qd.len())]),
        candidate_id: candidate_id.to_string(),
        computed_at_ms: now_unix_ms_strict(),
        query_digest_sha256: qd,
        sources: vec![],
        normalized_hits: vec![],
        overlap_summary: Some(NoveltyOverlapSummary {
            max_lexical_score: None,
            max_semantic_score: None,
            recency_bucket: NoveltyRecencyBucket::Unknown,
        }),
        query_traces: vec![],
    }
}

async fn http_get_text(client: &reqwest::Client, url: &str) -> Result<(String, i32)> {
    let resp = client.get(url).send().await.context("http get")?;
    let status = resp.status().as_u16() as i32;
    let text = resp.text().await.context("read body")?;
    Ok((text, status))
}

fn openalex_hits(
    v: &JsonValue,
    search_face: &str,
    h: &ScientiaHeuristics,
) -> Vec<NormalizedPriorArtHit> {
    let mut out = Vec::new();
    let Some(arr) = v.get("results").and_then(|x| x.as_array()) else {
        return out;
    };
    let take_n = h.prior_art_results_per_source.clamp(1, 50) as usize;
    for w in arr.iter().take(take_n) {
        let uri = w
            .get("id")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let title = w
            .get("display_name")
            .or_else(|| w.get("title"))
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let year = w
            .get("publication_year")
            .and_then(|x| x.as_i64())
            .map(|y| y as i32);
        let cited = w.get("cited_by_count").and_then(|x| x.as_u64());
        let lex = title_lexical_score_with_min_len(search_face, &title, h.prior_art_token_min_len);
        out.push(NormalizedPriorArtHit {
            source: PriorArtSource::Openalex,
            work_uri: uri,
            title,
            year,
            lexical_score: Some(lex),
            semantic_score: None,
            overlap_note: None,
            cited_by_count: cited,
        });
    }
    out
}

fn crossref_hits(
    v: &JsonValue,
    search_face: &str,
    h: &ScientiaHeuristics,
) -> Vec<NormalizedPriorArtHit> {
    let mut out = Vec::new();
    let items = v
        .pointer("/message/items")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    let take_n = h.prior_art_results_per_source.clamp(1, 50) as usize;
    for w in items.into_iter().take(take_n) {
        let doi = w.get("DOI").and_then(|x| x.as_str()).unwrap_or("").trim();
        let title = w
            .get("title")
            .and_then(|x| x.as_array())
            .and_then(|a| a.first())
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let year = w
            .pointer("/issued/date-parts/0/0")
            .or_else(|| w.pointer("/published-print/date-parts/0/0"))
            .and_then(|x| x.as_i64())
            .map(|y| y as i32);
        let uri = if doi.is_empty() {
            title.clone()
        } else {
            format!("https://doi.org/{doi}")
        };
        let lex = title_lexical_score_with_min_len(search_face, &title, h.prior_art_token_min_len);
        out.push(NormalizedPriorArtHit {
            source: PriorArtSource::Crossref,
            work_uri: uri,
            title,
            year,
            lexical_score: Some(lex),
            semantic_score: None,
            overlap_note: None,
            cited_by_count: None,
        });
    }
    out
}

fn s2_hits(
    v: &JsonValue,
    search_face: &str,
    h: &ScientiaHeuristics,
    raw_query_for_fallback_url: &str,
) -> Vec<NormalizedPriorArtHit> {
    let mut out = Vec::new();
    let data = v
        .get("data")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    let take_n = h.prior_art_results_per_source.clamp(1, 50) as usize;
    for w in data.into_iter().take(take_n) {
        let pid = w
            .get("paperId")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let title = w
            .get("title")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let year = w.get("year").and_then(|x| x.as_i64()).map(|y| y as i32);
        let cited = w.get("citationCount").and_then(|x| x.as_u64());
        let uri = if pid.is_empty() {
            reqwest::Url::parse_with_params(
                "https://www.semanticscholar.org/search",
                [("q", raw_query_for_fallback_url)],
            )
            .map(|u| u.to_string())
            .unwrap_or_else(|_| "https://www.semanticscholar.org/".to_string())
        } else {
            format!("https://www.semanticscholar.org/paper/{pid}")
        };
        let lex = title_lexical_score_with_min_len(search_face, &title, h.prior_art_token_min_len);
        out.push(NormalizedPriorArtHit {
            source: PriorArtSource::SemanticScholar,
            work_uri: uri,
            title,
            year,
            lexical_score: Some(lex),
            semantic_score: None,
            overlap_note: None,
            cited_by_count: cited,
        });
    }
    out
}

fn dedupe_hits(hits: Vec<NormalizedPriorArtHit>) -> Vec<NormalizedPriorArtHit> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut out = Vec::new();
    for h in hits {
        let key = format!("{}|{}", h.title.to_lowercase(), h.work_uri.to_lowercase());
        if seen.insert(key) {
            out.push(h);
        }
    }
    out
}

fn finalize_bundle(
    candidate_id: &str,
    query: &PriorArtQuery,
    mut sources: Vec<PriorArtSource>,
    hits: Vec<NormalizedPriorArtHit>,
    traces: Vec<NoveltyQueryTrace>,
) -> NoveltyEvidenceBundleV1 {
    let qd = query_digest_sha256(query);
    let years: Vec<Option<i32>> = hits.iter().map(|h| h.year).collect();
    let max_lex = hits
        .iter()
        .filter_map(|h: &NormalizedPriorArtHit| h.lexical_score)
        .max_by(|a: &f64, b: &f64| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let max_sem = hits
        .iter()
        .filter_map(|h: &NormalizedPriorArtHit| h.semantic_score)
        .max_by(|a: &f64, b: &f64| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    sources.sort_by_key(|s| format!("{s:?}"));
    sources.dedup_by_key(|s| format!("{s:?}"));
    NoveltyEvidenceBundleV1 {
        schema_version: 1,
        bundle_id: format!("nb.{}", &qd[..16.min(qd.len())]),
        candidate_id: candidate_id.to_string(),
        computed_at_ms: now_unix_ms_strict(),
        query_digest_sha256: qd,
        sources,
        normalized_hits: hits,
        overlap_summary: Some(NoveltyOverlapSummary {
            max_lexical_score: max_lex,
            max_semantic_score: max_sem,
            recency_bucket: recency_from_years(&years),
        }),
        query_traces: traces,
    }
}

fn openalex_url(search: &str, per_page: u32) -> Result<String> {
    let pp = per_page.clamp(1, 50).to_string();
    let u = reqwest::Url::parse_with_params(
        "https://api.openalex.org/works",
        [("search", search), ("per_page", pp.as_str())],
    )
    .context("openalex url")?;
    Ok(u.to_string())
}

fn crossref_url(search: &str, mailto: &str, rows: u32) -> Result<String> {
    let rr = rows.clamp(1, 50).to_string();
    let u = reqwest::Url::parse_with_params(
        "https://api.crossref.org/works",
        [
            ("query.bibliographic", search),
            ("rows", rr.as_str()),
            ("mailto", mailto),
        ],
    )
    .context("crossref url")?;
    Ok(u.to_string())
}

fn s2_api_url(search: &str, limit: u32) -> Result<String> {
    let lim = limit.clamp(1, 50).to_string();
    let u = reqwest::Url::parse_with_params(
        "https://api.semanticscholar.org/graph/v1/paper/search",
        [
            ("query", search),
            ("limit", lim.as_str()),
            ("fields", "title,year,paperId,citationCount"),
        ],
    )
    .context("semantic scholar url")?;
    Ok(u.to_string())
}

/// Fetch prior art from selected sources. Uses Crossref polite pool when `mailto` is set.
///
/// Pass `embedder: Some(&impl)` to enrich hits with real cosine semantic scores before
/// computing the bundle overlap summary. Pass `None` to skip embedding (semantic scores
/// will be `None` in the bundle).
pub async fn fetch_prior_art_federated(
    client: &reqwest::Client,
    candidate_id: &str,
    query: &PriorArtQuery,
    mut want: Vec<PriorArtSource>,
    options: PriorArtFetchOptions,
    offline: bool,
    heuristics: &ScientiaHeuristics,
    embedder: Option<&dyn crate::scientia_semantic::Embedder>,
) -> Result<NoveltyEvidenceBundleV1> {
    let qd = query_digest_sha256(query);
    if offline || query.title.trim().is_empty() {
        return Ok(empty_novelty_bundle(candidate_id, query));
    }

    want.retain(|s| *s != PriorArtSource::Manual && *s != PriorArtSource::Other);
    if want.is_empty() {
        want = vec![
            PriorArtSource::Openalex,
            PriorArtSource::Crossref,
            PriorArtSource::SemanticScholar,
        ];
    }

    let search = prior_art_search_text(query, heuristics);
    let title_only = query.title.trim();
    let per = heuristics.prior_art_results_per_source.clamp(1, 50);
    let mail: String = options
        .mailto_for_crossref
        .map(std::string::ToString::to_string)
        .or_else(|| {
            vox_secrets::resolve_secret(vox_secrets::SecretId::VoxScientiaCrossrefMailto)
                .expose()
                .map(std::string::ToString::to_string)
        })
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "opensource@voxlang.org".into());
    let mail_ref = mail.as_str();

    let mut hits = Vec::new();
    let mut traces = Vec::new();
    let mut sources_done = Vec::new();

    for src in want {
        let trace_digest = qd.clone();
        match src {
            PriorArtSource::Openalex => match openalex_url(&search, per) {
                Ok(url) => match http_get_text(client, &url).await {
                    Ok((body, st)) => {
                        traces.push(NoveltyQueryTrace {
                            source: "openalex".to_string(),
                            request_fingerprint_sha256: trace_fp("openalex", &trace_digest),
                            http_status: Some(st),
                            cached: Some(false),
                        });
                        if let Ok(v) = serde_json::from_str::<JsonValue>(&body) {
                            hits.extend(openalex_hits(&v, &search, heuristics));
                            sources_done.push(PriorArtSource::Openalex);
                        }
                    }
                    Err(e) => {
                        traces.push(NoveltyQueryTrace {
                            source: "openalex".to_string(),
                            request_fingerprint_sha256: trace_fp("openalex", &trace_digest),
                            http_status: None,
                            cached: Some(false),
                        });
                        tracing::warn!("openalex prior-art fetch failed: {e:#}");
                    }
                },
                Err(e) => tracing::warn!("openalex url build failed: {e:#}"),
            },
            PriorArtSource::Crossref => match crossref_url(&search, mail_ref, per) {
                Ok(url) => match http_get_text(client, &url).await {
                    Ok((body, st)) => {
                        traces.push(NoveltyQueryTrace {
                            source: "crossref".to_string(),
                            request_fingerprint_sha256: trace_fp("crossref", &trace_digest),
                            http_status: Some(st),
                            cached: Some(false),
                        });
                        if let Ok(v) = serde_json::from_str::<JsonValue>(&body) {
                            hits.extend(crossref_hits(&v, &search, heuristics));
                            sources_done.push(PriorArtSource::Crossref);
                        }
                    }
                    Err(e) => {
                        traces.push(NoveltyQueryTrace {
                            source: "crossref".to_string(),
                            request_fingerprint_sha256: trace_fp("crossref", &trace_digest),
                            http_status: None,
                            cached: Some(false),
                        });
                        tracing::warn!("crossref prior-art fetch failed: {e:#}");
                    }
                },
                Err(e) => tracing::warn!("crossref url build failed: {e:#}"),
            },
            PriorArtSource::SemanticScholar => match s2_api_url(&search, per) {
                Ok(url) => match http_get_text(client, &url).await {
                    Ok((body, st)) => {
                        traces.push(NoveltyQueryTrace {
                            source: "semantic_scholar".to_string(),
                            request_fingerprint_sha256: trace_fp("semantic_scholar", &trace_digest),
                            http_status: Some(st),
                            cached: Some(false),
                        });
                        if let Ok(v) = serde_json::from_str::<JsonValue>(&body) {
                            hits.extend(s2_hits(&v, &search, heuristics, title_only));
                            sources_done.push(PriorArtSource::SemanticScholar);
                        }
                    }
                    Err(e) => {
                        traces.push(NoveltyQueryTrace {
                            source: "semantic_scholar".to_string(),
                            request_fingerprint_sha256: trace_fp("semantic_scholar", &trace_digest),
                            http_status: None,
                            cached: Some(false),
                        });
                        tracing::warn!("semantic_scholar prior-art fetch failed: {e:#}");
                    }
                },
                Err(e) => tracing::warn!("semantic_scholar url build failed: {e:#}"),
            },
            PriorArtSource::Manual | PriorArtSource::Other => {}
        }
    }

    let mut hits = dedupe_hits(hits);
    if let Some(emb) = embedder {
        crate::scientia_semantic::enrich_semantic_scores(&search, &mut hits, emb).await;
    }
    Ok(finalize_bundle(
        candidate_id,
        query,
        sources_done,
        hits,
        traces,
    ))
}

/// Parse embedded novelty bundle from `metadata_json` if present.
#[must_use]
pub fn parse_novelty_bundle_from_metadata_json(
    metadata_json: Option<&str>,
) -> Option<NoveltyEvidenceBundleV1> {
    let raw = metadata_json?;
    let root: JsonValue = serde_json::from_str(raw.trim()).ok()?;
    let b = root.get(METADATA_KEY_SCIENTIA_NOVELTY_BUNDLE)?;
    serde_json::from_value(b.clone()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_query() -> PriorArtQuery {
        PriorArtQuery {
            title: "Test finding".to_string(),
            abstract_text: None,
        }
    }

    #[test]
    fn empty_novelty_bundle_scores_are_none() {
        let bundle = empty_novelty_bundle("cand-1", &test_query());
        let summary = bundle
            .overlap_summary
            .expect("overlap_summary must be Some");
        // Must not fabricate "we searched and found nothing" — both must be None.
        assert!(
            summary.max_lexical_score.is_none(),
            "max_lexical_score must be None in an empty bundle"
        );
        assert!(
            summary.max_semantic_score.is_none(),
            "max_semantic_score must be None in an empty bundle"
        );
    }

    #[test]
    fn empty_novelty_bundle_has_no_hits_and_no_traces() {
        let bundle = empty_novelty_bundle("cand-2", &test_query());
        assert!(bundle.normalized_hits.is_empty());
        assert!(bundle.query_traces.is_empty());
        assert!(bundle.sources.is_empty());
    }

    #[test]
    fn empty_novelty_bundle_recency_is_unknown() {
        let bundle = empty_novelty_bundle("cand-3", &test_query());
        let summary = bundle.overlap_summary.unwrap();
        assert_eq!(summary.recency_bucket, NoveltyRecencyBucket::Unknown);
    }

    #[test]
    fn title_lexical_score_jaccard_exact_values() {
        // Catches: Jaccard over-counting (using intersection/min instead of intersection/union),
        // or failing to dedup tokens / lowercase before comparing.
        assert_eq!(title_lexical_score("alpha beta", "alpha beta"), 1.0);
        assert_eq!(title_lexical_score("alpha beta", "gamma delta"), 0.0);
        // tokens len<=2 dropped at default min_len=2: {alpha,beta,gamma} vs {alpha,beta} → 2/3
        assert!((title_lexical_score("alpha beta gamma", "ALPHA beta") - (2.0 / 3.0)).abs() < 1e-9);
    }

    #[test]
    fn title_lexical_score_with_min_len_filters_short_tokens() {
        // Catches: token_min_len floor not applied (min_len.max(1)) so short tokens leak in,
        // and the all-empty-after-filter case not returning 1.0.
        // min_len=4 drops every token in both → both empty → 1.0
        assert_eq!(
            title_lexical_score_with_min_len("a bb ccc", "x yy zzz", 4),
            1.0
        );
        // min_len=2 keeps "ccc"/"ddd": disjoint single-token sets → 0.0
        assert_eq!(title_lexical_score_with_min_len("ccc", "ddd", 2), 0.0);
    }
}
