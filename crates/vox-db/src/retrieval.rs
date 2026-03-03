/// Retrieval mode for hybrid search plans.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetrievalMode {
    Vector,
    FullText,
    Hybrid,
}

/// Query specification for retrieval pipelines.
#[derive(Debug, Clone)]
pub struct RetrievalQuery {
    pub query_text: String,
    pub mode: RetrievalMode,
    pub top_k: usize,
    pub min_score: f32,
}

impl Default for RetrievalQuery {
    fn default() -> Self {
        Self {
            query_text: String::new(),
            mode: RetrievalMode::Hybrid,
            top_k: 8,
            min_score: 0.0,
        }
    }
}

/// Minimal retrieval result metadata suitable for provenance capture.
#[derive(Debug, Clone)]
pub struct RetrievalResult {
    pub chunk_id: String,
    pub source: String,
    pub score: f32,
    pub snippet: String,
}

/// Merge vector/full-text candidates with simple weighted rank fusion.
pub fn fuse_hybrid_results(
    vector_hits: &[RetrievalResult],
    text_hits: &[RetrievalResult],
    vector_weight: f32,
) -> Vec<RetrievalResult> {
    let mut merged: std::collections::HashMap<String, RetrievalResult> =
        std::collections::HashMap::new();
    for hit in vector_hits {
        merged.insert(hit.chunk_id.clone(), hit.clone());
    }
    for hit in text_hits {
        merged
            .entry(hit.chunk_id.clone())
            .and_modify(|existing| {
                existing.score =
                    (existing.score * vector_weight) + (hit.score * (1.0 - vector_weight));
                if existing.snippet.is_empty() {
                    existing.snippet = hit.snippet.clone();
                }
            })
            .or_insert_with(|| hit.clone());
    }
    let mut out: Vec<_> = merged.into_values().collect();
    out.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hybrid_fusion_sorts_by_score() {
        let v = vec![RetrievalResult {
            chunk_id: "a".into(),
            source: "doc".into(),
            score: 0.9,
            snippet: "alpha".into(),
        }];
        let t = vec![RetrievalResult {
            chunk_id: "b".into(),
            source: "doc".into(),
            score: 0.8,
            snippet: "beta".into(),
        }];
        let fused = fuse_hybrid_results(&v, &t, 0.7);
        assert_eq!(fused.len(), 2);
        assert_eq!(fused[0].chunk_id, "a");
    }
}
