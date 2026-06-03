//! Typed, structured result facade for multi-corpus search.
//!
//! Historically `SearchExecution` exposed only pre-formatted `Vec<String>` lines,
//! forcing consumers (GUI, A2A bridges) to hand-parse strings. [`UnifiedHit`] is
//! captured AT SOURCE — where the typed per-corpus hits are flattened into strings —
//! so structured, typed results travel alongside the legacy lines without any
//! string re-parsing.

/// One structured search hit, captured from a typed per-corpus result before
/// it is flattened into a display line.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct UnifiedHit {
    /// Corpus the hit came from: "memory" | "knowledge" | "chunk" | "repo" | "symbol" | "web" | "fused".
    pub source: String,
    /// Best-effort document kind/category (e.g. "doc", "code", "memory", "web"); "" if unknown.
    pub kind: String,
    pub path: Option<String>,
    pub title: Option<String>,
    pub snippet: String,
    pub score: f64,
    pub provenance: Vec<String>,
}

/// Sort `hits` by descending score (stable on ties), so the top hit is first.
pub(crate) fn sort_unified_hits_desc(hits: &mut [UnifiedHit]) {
    hits.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unified_hit_serde_round_trips() {
        let hit = UnifiedHit {
            source: "memory".into(),
            kind: "memory".into(),
            path: Some("docs/foo.md".into()),
            title: Some("Foo".into()),
            snippet: "a snippet".into(),
            score: 0.875,
            provenance: vec!["evidence:hybrid".into(), "bm25:1".into()],
        };
        let json = serde_json::to_string(&hit).expect("serialize");
        let back: UnifiedHit = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(hit, back);
    }

    #[test]
    fn sort_orders_by_score_descending() {
        let mut hits = vec![
            UnifiedHit {
                source: "chunk".into(),
                kind: "doc".into(),
                path: None,
                title: None,
                snippet: "low".into(),
                score: 0.1,
                provenance: vec![],
            },
            UnifiedHit {
                source: "memory".into(),
                kind: "memory".into(),
                path: None,
                title: None,
                snippet: "high".into(),
                score: 0.9,
                provenance: vec![],
            },
            UnifiedHit {
                source: "web".into(),
                kind: "web".into(),
                path: None,
                title: None,
                snippet: "mid".into(),
                score: 0.5,
                provenance: vec![],
            },
        ];
        sort_unified_hits_desc(&mut hits);
        let scores: Vec<f64> = hits.iter().map(|h| h.score).collect();
        assert_eq!(scores, vec![0.9, 0.5, 0.1]);
        assert_eq!(hits[0].snippet, "high");
    }
}
