use crate::memory_hybrid::HybridSearchHit;
use regex::Regex;
use std::collections::HashSet;

/// Logic for Corrective RAG (CRAG) expansion and multi-hop query generation.
pub struct CragRouter;

impl CragRouter {
    /// Generates refined sub-queries based on initial research hits and the original query.
    /// Used for iterative multi-hop research loops.
    pub fn expand_queries_from_partial_evidence(
        original_query: &str,
        hits: &[HybridSearchHit],
    ) -> Vec<String> {
        let mut refined_queries = Vec::new();
        let mut seen_queries = HashSet::new();
        let mut seen_concepts = HashSet::new();

        let push_query = |refined_queries: &mut Vec<String>,
                          seen_queries: &mut HashSet<String>,
                          query: String| {
            if seen_queries.insert(query.to_ascii_lowercase()) {
                refined_queries.push(query);
            }
        };

        // 1. Extract proper nouns or capitalized technical terms from snippets
        // This is a heuristic until Lane G (Research Expert) takes over.
        let concept_regex = Regex::new(r"([A-Z][a-z]{3,}(?:\s[A-Z][a-z]{3,})*)").unwrap();

        for hit in hits.iter().take(5) {
            for cap in concept_regex.captures_iter(&hit.content_snippet) {
                let concept = cap[1].to_string();
                if !original_query
                    .to_lowercase()
                    .contains(&concept.to_lowercase())
                    && seen_concepts.insert(concept.clone())
                {
                    push_query(
                        &mut refined_queries,
                        &mut seen_queries,
                        format!("{} {}", original_query, concept),
                    );
                }
                if refined_queries.len() >= 3 {
                    break;
                }
            }
            if refined_queries.len() >= 3 {
                break;
            }
        }

        // 2. Generate gap-oriented follow-ups from weak or contradictory evidence.
        if hits.iter().any(|hit| hit.potential_contradiction) {
            push_query(
                &mut refined_queries,
                &mut seen_queries,
                format!("{} conflicting evidence source comparison", original_query),
            );
        }
        let avg_score = if hits.is_empty() {
            0.0
        } else {
            hits.iter().map(|hit| hit.score).sum::<f64>() / hits.len() as f64
        };
        if avg_score < 0.55 {
            push_query(
                &mut refined_queries,
                &mut seen_queries,
                format!("{} primary source evidence", original_query),
            );
            push_query(
                &mut refined_queries,
                &mut seen_queries,
                format!("{} independent corroborating sources", original_query),
            );
        }

        // 3. If no new concepts found, try broad free-source refinements.
        if refined_queries.is_empty() {
            push_query(
                &mut refined_queries,
                &mut seen_queries,
                format!("{} latest developments", original_query),
            );
            push_query(
                &mut refined_queries,
                &mut seen_queries,
                format!("{} detailed comparison", original_query),
            );
        }

        refined_queries.truncate(3);
        refined_queries
    }

    /// Determines if a research pass should continue based on quality score.
    pub fn should_continue(current_quality: f64, target_quality: f64, hops_remaining: u8) -> bool {
        hops_remaining > 0 && current_quality < target_quality
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(score: f64, snippet: &str, contradiction: bool) -> HybridSearchHit {
        HybridSearchHit {
            path: "https://example.test/doc".to_string(),
            title: "Doc".to_string(),
            content_snippet: snippet.to_string(),
            score,
            provenance: vec!["WebResearch".to_string()],
            potential_contradiction: contradiction,
        }
    }

    #[test]
    fn crag_expands_weak_evidence_with_primary_and_corroborating_queries() {
        let queries = CragRouter::expand_queries_from_partial_evidence(
            "deep research citation grounding",
            &[hit(0.2, "small weak snippet", false)],
        );

        assert!(
            queries
                .iter()
                .any(|q| q.contains("primary source evidence"))
        );
        assert!(
            queries
                .iter()
                .any(|q| q.contains("independent corroborating sources"))
        );
    }

    #[test]
    fn crag_expands_contradictions_with_comparison_query() {
        let queries = CragRouter::expand_queries_from_partial_evidence(
            "deep research citation grounding",
            &[hit(0.9, "Source Alpha disagrees with Source Beta", true)],
        );

        assert!(
            queries
                .iter()
                .any(|q| q.contains("conflicting evidence source comparison"))
        );
    }
}
