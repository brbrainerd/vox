//! Multi-hop web research loop shared by orchestrator CRAG paths.

use std::collections::HashSet;

use tracing::{info, warn};

use crate::crag::CragRouter;
use crate::memory_hybrid::HybridSearchHit;
use crate::policy::SearchPolicy;
use crate::web_dispatcher::WebSearchDispatcher;

/// Target distinct-source count for the citation leg (same **8** bucket scale as full `execute_search_plan` citation coverage).
const WEB_CRAG_COVERAGE_BUCKETS: f64 = 8.0;

/// Evidence-quality estimate for CRAG continuation using the same top-score + coverage weights as [`SearchPolicy`] / [`execute_search_plan`](crate::execution::execute_search_plan).
#[must_use]
pub fn web_research_crag_quality(
    policy: &SearchPolicy,
    top_score: f64,
    unique_source_count: usize,
) -> f64 {
    let top = top_score.clamp(0.0, 1.0);
    let citation_coverage = if unique_source_count == 0 {
        0.0
    } else {
        (unique_source_count as f64 / WEB_CRAG_COVERAGE_BUCKETS).min(1.0)
    };
    ((top * policy.evidence_quality_top_weight)
        + (citation_coverage * policy.evidence_quality_coverage_weight))
        .clamp(0.0, 1.0)
}

/// Run up to `policy.web_search_max_hops` dispatcher rounds, deduping URLs and expanding queries via [`CragRouter`].
///
/// Returns formatted evidence lines (legacy orchestrator shape: `[autonomous_research:…]`).
pub async fn run_multi_hop_web_research(
    policy: &SearchPolicy,
    initial_queries: &[String],
    quality_target: f64,
    anchor_query: &str,
) -> Vec<String> {
    let mut research_results = Vec::new();
    let mut hops_remaining = policy.web_search_max_hops;
    let mut active_queries: Vec<String> = initial_queries.to_vec();
    let mut visited_urls = HashSet::new();
    let mut running_top_score = 0.0_f64;

    while hops_remaining > 0 && !active_queries.is_empty() {
        let mut hop_hits: Vec<HybridSearchHit> = Vec::new();
        info!(
            hop = policy.web_search_max_hops - hops_remaining + 1,
            query_count = active_queries.len(),
            "starting research hop"
        );

        for query in &active_queries {
            match WebSearchDispatcher::search(query, policy).await {
                Ok(hits) => {
                    for hit in hits {
                        if visited_urls.insert(hit.path.clone()) {
                            running_top_score = running_top_score.max(hit.score.clamp(0.0, 1.0));
                            let engine = hit
                                .provenance
                                .iter()
                                .find_map(|p| p.strip_prefix("engine:"))
                                .unwrap_or("unknown");

                            research_results.push(format!(
                                "[autonomous_research:{}] {} (score: {:.3}; engine: {}) - {}",
                                hit.path,
                                hit.title,
                                hit.score,
                                engine,
                                hit.content_snippet.replace('\n', " ")
                            ));
                            hop_hits.push(hit);
                        }
                    }
                }
                Err(e) => {
                    warn!(query = %query, error = %e, "research query failed");
                }
            }
        }

        let current_quality =
            web_research_crag_quality(policy, running_top_score, visited_urls.len());

        if !CragRouter::should_continue(current_quality, quality_target, hops_remaining) {
            break;
        }

        active_queries = CragRouter::expand_queries_from_partial_evidence(anchor_query, &hop_hits);
        hops_remaining -= 1;
    }

    research_results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_research_crag_quality_respects_policy_weights() {
        let policy = SearchPolicy::default();
        let q0 = web_research_crag_quality(&policy, 0.0, 0);
        assert!(q0.abs() < f64::EPSILON);

        let q_mid = web_research_crag_quality(&policy, 1.0, 4);
        assert!(
            q_mid > 0.6 && q_mid <= 1.0,
            "expected blended quality in (0.6, 1], got {q_mid}"
        );
    }

    #[tokio::test]
    async fn multi_hop_returns_empty_when_initial_queries_empty() {
        let policy = SearchPolicy::default();
        let out = run_multi_hop_web_research(&policy, &[], 1.0, "anchor").await;
        assert!(out.is_empty());
    }
}
