//! Importance ranking for date-scoped CodeRabbit sweeps.
//!
//! Scores candidate files by `recency + churn + graph centrality`, selects the most
//! important (`--top N`), and lets the planner emit the highest-importance PRs first.
//!
//! Centrality comes from the AST code graph via [`vox_graph_reader`]. Verified
//! 2026-06-29: the graph covers ~83% of code files (`.rs/.ts/.js/.py`) but only ~42%
//! of all tracked files (docs/configs are not extractable; refresh via
//! `vox graphify rebuild`). So a file with **no** node
//! is imputed at the **median** of covered candidates — absence is neutral, never a
//! penalty (zeroing would wrongly sink the majority of files).

use std::collections::HashMap;
use std::path::Path;

use super::semantic_planner::SemanticChunk;

/// Relative weights for the three importance signals. Equal by default.
#[derive(Debug, Clone, Copy)]
pub struct RankWeights {
    pub recency: f64,
    pub churn: f64,
    pub centrality: f64,
}

impl Default for RankWeights {
    fn default() -> Self {
        Self {
            recency: 1.0,
            churn: 1.0,
            centrality: 1.0,
        }
    }
}

impl RankWeights {
    /// True when no weight diverges from the default (used to decide whether ranking runs).
    #[allow(clippy::float_cmp)]
    pub fn is_default(self) -> bool {
        self.recency == 1.0 && self.churn == 1.0 && self.centrality == 1.0
    }

    /// Parse `"r,c,g"` (any missing component defaults to 1.0). Non-numeric → 1.0.
    pub fn parse(s: &str) -> Self {
        let mut it = s.split(',').map(|p| p.trim().parse::<f64>().unwrap_or(1.0));
        RankWeights {
            recency: it.next().unwrap_or(1.0),
            churn: it.next().unwrap_or(1.0),
            centrality: it.next().unwrap_or(1.0),
        }
    }
}

/// `"<file>::<symbol>"` → `"<file>"`, stripping a leading `.claude/worktrees/<seg>/`.
pub(crate) fn file_of_node(id: &str) -> String {
    let file = id.split("::").next().unwrap_or(id);
    if let Some(rest) = file.strip_prefix(".claude/worktrees/") {
        if let Some((_, tail)) = rest.split_once('/') {
            return tail.to_string();
        }
    }
    file.to_string()
}

fn norm(map: &HashMap<String, f64>, key: &str, max: f64) -> f64 {
    if max <= 0.0 {
        0.0
    } else {
        map.get(key).copied().unwrap_or(0.0) / max
    }
}

/// Per-file importance score for every file in `files`.
///
/// Uncovered centrality is imputed at the median normalized centrality of the covered
/// candidates (neutral). When `centrality` is `None` the term is omitted entirely.
pub fn score_map(
    files: &[String],
    recency: &HashMap<String, f64>,
    churn: &HashMap<String, u64>,
    centrality: Option<&HashMap<String, f64>>,
    w: RankWeights,
) -> HashMap<String, f64> {
    let churn_f: HashMap<String, f64> = churn.iter().map(|(k, v)| (k.clone(), *v as f64)).collect();
    let rmax = recency.values().copied().fold(0.0, f64::max);
    let cmax = churn_f.values().copied().fold(0.0, f64::max);
    let gmax = centrality
        .map(|g| g.values().copied().fold(0.0, f64::max))
        .unwrap_or(0.0);
    // Median normalized centrality over the candidate files that ARE covered.
    let cmed_norm = match centrality {
        Some(g) if gmax > 0.0 => {
            let mut v: Vec<f64> = files
                .iter()
                .filter_map(|f| g.get(f))
                .map(|x| x / gmax)
                .collect();
            if v.is_empty() {
                0.0
            } else {
                v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                v[v.len() / 2]
            }
        }
        _ => 0.0,
    };
    let mut out = HashMap::with_capacity(files.len());
    for f in files {
        let mut s = w.recency * norm(recency, f, rmax) + w.churn * norm(&churn_f, f, cmax);
        if let Some(g) = centrality {
            let cov = g.get(f).map(|v| v / gmax).unwrap_or(cmed_norm);
            s += w.centrality * cov;
        }
        out.insert(f.clone(), s);
    }
    out
}

/// Sort `files` in place by descending score (stable tie-break by path). Shared by
/// `rank_files` and the `semantic-submit` selection path so both use one ordering.
pub(crate) fn sort_files_by_score(files: &mut [String], score: &HashMap<String, f64>) {
    files.sort_by(|a, b| {
        let (sa, sb) = (
            score.get(a).copied().unwrap_or(0.0),
            score.get(b).copied().unwrap_or(0.0),
        );
        sb.partial_cmp(&sa)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.cmp(b))
    });
}

/// `files` sorted by descending importance (stable tie-break by path).
pub fn rank_files(
    files: &[String],
    recency: &HashMap<String, f64>,
    churn: &HashMap<String, u64>,
    centrality: Option<&HashMap<String, f64>>,
    w: RankWeights,
) -> Vec<String> {
    let score = score_map(files, recency, churn, centrality, w);
    let mut v = files.to_vec();
    sort_files_by_score(&mut v, &score);
    v
}

/// Re-order planner chunks by descending mean file score (highest-importance PRs first).
pub(crate) fn reorder_chunks_by_score(chunks: &mut [SemanticChunk], score: &HashMap<String, f64>) {
    let agg = |c: &SemanticChunk| -> f64 {
        if c.files.is_empty() {
            return 0.0;
        }
        c.files.iter().map(|f| score.get(f).copied().unwrap_or(0.0)).sum::<f64>() / c.files.len() as f64
    };
    chunks.sort_by(|a, b| {
        agg(b)
            .partial_cmp(&agg(a))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.order.cmp(&b.order))
    });
}

/// File-aggregated node degree from the AST graph cache. `None` on any failure / zero matches.
pub fn load_file_centrality(repo: &Path) -> Option<HashMap<String, f64>> {
    let path = repo.join(".vox/cache/graphify/repo-code-graph/graph.json");
    let text = std::fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let reader = vox_graph_reader::GraphifyReader::from_value(value).ok()?;
    let n = reader.node_count();
    let mut by_file: HashMap<String, f64> = HashMap::new();
    for (id, deg) in reader.god_nodes(n) {
        *by_file.entry(file_of_node(&id)).or_insert(0.0) += deg as f64;
    }
    if by_file.is_empty() {
        None
    } else {
        Some(by_file)
    }
}

/// Log how much of the candidate set actually gets a centrality signal.
pub fn log_centrality_coverage(candidates: &[String], central: Option<&HashMap<String, f64>>) {
    if let Some(g) = central {
        let hit = candidates.iter().filter(|f| g.contains_key(*f)).count();
        let pct = if candidates.is_empty() {
            0.0
        } else {
            100.0 * hit as f64 / candidates.len() as f64
        };
        eprintln!(
            "[semantic-submit] centrality covers {hit}/{} candidate files ({pct:.0}%); \
             uncovered imputed at median. Run `vox graphify rebuild` to refresh coverage.",
            candidates.len()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn churn_dominates_and_degrades_without_graph() {
        let recency: HashMap<String, f64> = [("a.rs".into(), 1.0), ("b.rs".into(), 1.0)].into();
        let churn: HashMap<String, u64> = [("a.rs".into(), 10), ("b.rs".into(), 100)].into();
        let files = vec!["a.rs".to_string(), "b.rs".to_string()];
        let ranked = rank_files(&files, &recency, &churn, None, RankWeights::default());
        assert_eq!(ranked[0], "b.rs");
    }

    #[test]
    fn missing_centrality_is_imputed_neutral_not_zero() {
        let recency: HashMap<String, f64> = [("covered".into(), 1.0), ("bare".into(), 1.0)].into();
        let churn: HashMap<String, u64> = [("covered".into(), 1), ("bare".into(), 1)].into();
        let central: HashMap<String, f64> = [("covered".into(), 100.0)].into();
        let files = vec!["covered".to_string(), "bare".to_string()];
        let ranked = rank_files(&files, &recency, &churn, Some(&central), RankWeights::default());
        // "bare" imputed to the median (100/gmax = 1.0) → identical score → tie by name.
        assert_eq!(ranked, vec!["bare".to_string(), "covered".to_string()]);
    }

    #[test]
    fn file_part_strips_symbol_and_worktree() {
        assert_eq!(file_of_node("crates/x/a.rs::foo"), "crates/x/a.rs");
        assert_eq!(
            file_of_node(".claude/worktrees/w1/crates/x/a.rs::foo"),
            "crates/x/a.rs"
        );
    }

    #[test]
    fn reorder_chunks_by_aggregate_score_desc() {
        let score: HashMap<String, f64> =
            [("a".into(), 10.0), ("b".into(), 1.0), ("c".into(), 5.0)].into();
        let mut chunks = vec![
            SemanticChunk { order: 1, name: "low".into(), files: vec!["b".into()] },
            SemanticChunk { order: 2, name: "high".into(), files: vec!["a".into()] },
            SemanticChunk { order: 3, name: "mid".into(), files: vec!["c".into()] },
        ];
        reorder_chunks_by_score(&mut chunks, &score);
        assert_eq!(chunks[0].name, "high");
        assert_eq!(chunks[2].name, "low");
    }

    #[test]
    fn weights_parse_partial() {
        let w = RankWeights::parse("2,3");
        assert_eq!(w.recency, 2.0);
        assert_eq!(w.churn, 3.0);
        assert_eq!(w.centrality, 1.0);
    }
}
