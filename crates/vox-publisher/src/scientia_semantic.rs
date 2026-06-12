//! Real semantic similarity for prior-art scoring.
//!
//! vox-publisher defines the pure math + the [`Embedder`] seam; the
//! llm_embed-backed, vox-db-cached implementation lives in the caller
//! (vox-cli), keeping this crate free of LLM/runtime deps. On embed failure
//! callers receive `None` — absence is propagated, never replaced with a
//! fake score.

use sha2::{Digest, Sha256};

/// Cosine similarity in [-1, 1]; 0.0 when either vector is all-zero.
#[must_use]
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f64 = a
        .iter()
        .zip(b.iter())
        .map(|(x, y)| f64::from(*x) * f64::from(*y))
        .sum();
    let norm_a: f64 = a
        .iter()
        .map(|x| f64::from(*x) * f64::from(*x))
        .sum::<f64>()
        .sqrt();
    let norm_b: f64 = b
        .iter()
        .map(|x| f64::from(*x) * f64::from(*x))
        .sum::<f64>()
        .sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    (dot / (norm_a * norm_b)).clamp(-1.0, 1.0)
}

/// Cache key: sha256 over model + NUL + text, lowercase hex.
#[must_use]
pub fn embed_cache_key(text: &str, model: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(model.as_bytes());
    hasher.update(b"\x00");
    hasher.update(text.as_bytes());
    let h = hasher.finalize();
    h.iter().map(|b| format!("{b:02x}")).collect()
}

/// Async embedding provider seam.
#[async_trait::async_trait]
pub trait Embedder: Send + Sync {
    /// Embed `text`; `None` on any failure (offline, no key, provider error).
    async fn embed(&self, text: &str) -> Option<Vec<f32>>;
}

/// Enrich hits' `semantic_score` with cosine(query, hit title) where embeddings
/// are available; leaves `semantic_score: None` where they are not.
pub async fn enrich_semantic_scores(
    query_text: &str,
    hits: &mut [crate::scientia_finding_ledger::NormalizedPriorArtHit],
    embedder: &dyn Embedder,
) {
    if hits.is_empty() {
        return;
    }
    let Some(query_vec) = embedder.embed(query_text).await else {
        return;
    };
    for hit in hits.iter_mut() {
        if hit.title.is_empty() {
            continue;
        }
        if let Some(hit_vec) = embedder.embed(&hit.title).await {
            hit.semantic_score = Some(cosine_similarity(&query_vec, &hit_vec));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_vectors_score_one() {
        let v = vec![0.5_f32, 0.5, 0.5, 0.5];
        assert!((cosine_similarity(&v, &v) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn orthogonal_vectors_score_zero() {
        let a = vec![1.0_f32, 0.0];
        let b = vec![0.0_f32, 1.0];
        assert!((cosine_similarity(&a, &b)).abs() < 1e-9);
    }

    #[test]
    fn zero_vector_scores_zero_no_nan() {
        let zero = vec![0.0_f32, 0.0, 0.0];
        let v = vec![1.0_f32, 2.0, 3.0];
        let s = cosine_similarity(&zero, &v);
        assert!(!s.is_nan(), "must not be NaN");
        assert_eq!(s, 0.0);
    }

    #[test]
    fn cache_key_stable() {
        let k1 = embed_cache_key("hello world", "model-a");
        let k2 = embed_cache_key("hello world", "model-a");
        assert_eq!(k1, k2, "key must be stable");
        assert_eq!(k1.len(), 64, "sha256 hex must be 64 chars");
    }

    #[test]
    fn cache_key_model_scoped() {
        let k1 = embed_cache_key("same text", "model-a");
        let k2 = embed_cache_key("same text", "model-b");
        assert_ne!(k1, k2, "different models must produce different keys");
    }
}
