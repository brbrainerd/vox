//! Reciprocal rank fusion across correlated corpus lists.

use std::collections::HashMap;

/// Stable deduplication key for a formatted retrieval line (see `execution` formatters).
pub(crate) fn rrf_dedup_key(line: &str) -> String {
    let t = line.trim_start();
    if let Some(rest) = t.strip_prefix("[chunk:") {
        let id = rest
            .split(|c: char| c.is_whitespace() || c == ']')
            .next()
            .unwrap_or("");
        return format!("chunk:{id}");
    }
    if let Some(rest) = t.strip_prefix("[repo:") {
        let id = rest.split(']').next().unwrap_or("");
        return format!("repo:{id}");
    }
    if let Some(rest) = t.strip_prefix("[node:") {
        let id = rest.split(']').next().unwrap_or("");
        return format!("node:{id}");
    }
    if let Some(rest) = t.strip_prefix("[tantivy:") {
        let id = rest.split_whitespace().next().unwrap_or("");
        return format!("tantivy:{id}");
    }
    if let Some(rest) = t.strip_prefix("[qdrant:") {
        let id = rest.split_whitespace().next().unwrap_or("");
        return format!("qdrant:{id}");
    }
    if let Some(rest) = t.strip_prefix('[')
        && let Some((head, _)) = rest.split_once(']')
    {
        return format!("bracket:{head}");
    }
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    line.hash(&mut h);
    format!("opaque:{}", h.finish())
}

/// Merge ordered hit lists using RRF; each list is ranked by position (first = rank 1).
/// `rrf_k` is the smoothing constant (often ~60); must be ≥ 1.0.
pub(crate) fn rrf_merge_line_lists(lists: &[Vec<String>], limit: usize, rrf_k: f64) -> Vec<String> {
    let k = rrf_k.max(1.0);
    let mut scores: HashMap<String, f64> = HashMap::new();
    let mut line_for_key: HashMap<String, String> = HashMap::new();
    for list in lists {
        for (rank, line) in list.iter().enumerate() {
            let key = rrf_dedup_key(line);
            let contrib = 1.0 / (k + (rank + 1) as f64);
            *scores.entry(key.clone()).or_insert(0.0) += contrib;
            line_for_key.entry(key).or_insert_with(|| line.clone());
        }
    }
    let mut keys: Vec<String> = scores.keys().cloned().collect();
    keys.sort_by(|a, b| {
        scores[b]
            .partial_cmp(&scores[a])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    keys.into_iter()
        .filter_map(|k| line_for_key.get(&k).cloned())
        .take(limit.max(1))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rrf_prefers_hits_that_rank_well_in_multiple_lists() {
        let a = vec!["[chunk:x] first".into(), "[chunk:y] second".into()];
        let b = vec!["[chunk:x] only".into()];
        let out = rrf_merge_line_lists(&[a, b], 2, 60.0);
        assert_eq!(out.len(), 2);
        assert!(out[0].contains("chunk:x"), "x should lead: {:?}", out);
    }

    #[test]
    fn dedup_key_stable_for_repo_and_nodes() {
        assert_eq!(
            rrf_dedup_key("[repo:crates/foo] snippet"),
            "repo:crates/foo"
        );
        assert_eq!(rrf_dedup_key("[node:abc] label"), "node:abc");
    }
}
