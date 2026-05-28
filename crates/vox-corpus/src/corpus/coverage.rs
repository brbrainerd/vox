//! Corpus coverage analysis for the Mens training pipeline.
//!
//! Scans a JSONL corpus file, counts `{"category": ...}` entries per construct
//! type, and compares against the full taxonomy passed by the caller (e.g.
//! `vox_cli::training::TAXONOMY`).
//!
//! ## Usage
//! ```rust,no_run
//! use vox_corpus::corpus::coverage::{CoverageReport, analyse_jsonl_with_taxonomy};
//! let taxonomy = &["function", "actor"];
//! let report = analyse_jsonl_with_taxonomy(std::path::Path::new("mens/data/train.jsonl"), 5, taxonomy)
//!     .expect("coverage analysis");
//! println!("{}", report.summary());
//! ```

use std::collections::HashMap;
use std::path::Path;

use anyhow::Context;
use serde_json::Value;

// Removed DEFAULT_TAXONOMY, it is now explicitly managed externally

/// Coverage analysis results for a JSONL corpus file.
#[derive(Debug, Clone)]
pub struct CoverageReport {
    /// Total number of parseable pairs in the corpus.
    pub total_pairs: usize,
    /// Number of taxonomy types with at least one pair.
    pub covered_types: usize,
    /// Total taxonomy types (always `TAXONOMY.len()`).
    pub total_types: usize,
    /// Taxonomy types with zero pairs in the corpus.
    pub missing_types: Vec<String>,
    /// Taxonomy types below the minimum threshold.
    pub underrepresented_types: Vec<(String, usize)>,
    /// Pair counts per category (includes non-taxonomy categories).
    pub counts: HashMap<String, usize>,
    /// Minimum pairs across covered taxonomy types (0 if nothing covered).
    pub min_covered_count: usize,
    /// Maximum pairs across covered taxonomy types.
    pub max_covered_count: usize,
    /// Balance score: 1.0 = perfectly balanced, 0.0 = totally skewed.
    /// Computed as `1.0 - (std_dev / mean)` clamped to [0, 1].
    pub balance_score: f64,
    /// Fraction of taxonomy types covered: `covered_types / total_types`.
    pub coverage_ratio: f64,
    /// Minimum pairs per category threshold used in analysis.
    pub min_pairs_threshold: usize,
}

impl CoverageReport {
    /// Human-readable summary for stderr logging.
    #[must_use]
    pub fn summary(&self) -> String {
        let pct = self.coverage_ratio * 100.0;
        let missing_str = if self.missing_types.is_empty() {
            "none".to_string()
        } else {
            self.missing_types.join(", ")
        };
        let under_str = if self.underrepresented_types.is_empty() {
            "none".to_string()
        } else {
            self.underrepresented_types
                .iter()
                .map(|(k, v)| format!("{k}({v})"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        format!(
            "[coverage] total={} types={}/{} ({pct:.1}%) balance={:.3}\n\
             [coverage] missing: {missing_str}\n\
             [coverage] under (<{}): {under_str}",
            self.total_pairs,
            self.covered_types,
            self.total_types,
            self.balance_score,
            self.min_pairs_threshold,
        )
    }

    /// True if every taxonomy type meets the minimum threshold.
    #[must_use]
    pub fn is_sufficient(&self) -> bool {
        self.missing_types.is_empty() && self.underrepresented_types.is_empty()
    }
}

/// Analyse a JSONL file against a custom taxonomy slice.
pub fn analyse_jsonl_with_taxonomy(
    path: &Path,
    min_pairs_per_category: usize,
    taxonomy: &[&str],
) -> anyhow::Result<CoverageReport> {
    let content = vox_bounded_fs::read_utf8_path_capped(path)
        .with_context(|| format!("read corpus for coverage analysis: {}", path.display()))?;
    let counts = count_categories_from_str(&content);
    Ok(build_report(counts, min_pairs_per_category, taxonomy))
}

/// Analyse a JSONL string against a custom taxonomy slice.
#[must_use]
pub fn analyse_str_with_taxonomy(
    jsonl: &str,
    min_pairs_per_category: usize,
    taxonomy: &[&str],
) -> CoverageReport {
    let counts = count_categories_from_str(jsonl);
    build_report(counts, min_pairs_per_category, taxonomy)
}

fn count_categories_from_str(jsonl: &str) -> HashMap<String, usize> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for line in jsonl.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Try to parse the category field
        if let Ok(Value::Object(map)) = serde_json::from_str::<Value>(trimmed) {
            // Support both "category" and "category" prefixed with "rust_" (from extract_rs)
            if let Some(cat) = map.get("category").and_then(|v| v.as_str()) {
                // Normalise rust_ prefixed categories to their base
                let base = if let Some(stripped) = cat.strip_prefix("rust_") {
                    stripped
                } else {
                    cat
                };
                *counts.entry(base.to_string()).or_insert(0) += 1;
            }
        }
    }
    counts
}

fn build_report(
    counts: HashMap<String, usize>,
    min_pairs_per_category: usize,
    taxonomy: &[&str],
) -> CoverageReport {
    let total_pairs: usize = counts.values().sum();
    let total_types = taxonomy.len();

    let mut missing_types = Vec::new();
    let mut underrepresented_types = Vec::new();
    let mut covered_types = 0usize;
    let mut taxonomy_counts: Vec<usize> = Vec::with_capacity(total_types);

    for &t in taxonomy {
        let count = counts.get(t).copied().unwrap_or(0);
        taxonomy_counts.push(count);
        if count == 0 {
            missing_types.push(t.to_string());
        } else {
            covered_types += 1;
            if count < min_pairs_per_category {
                underrepresented_types.push((t.to_string(), count));
            }
        }
    }

    let min_covered_count = taxonomy_counts
        .iter()
        .filter(|&&c| c > 0)
        .copied()
        .min()
        .unwrap_or(0);
    let max_covered_count = taxonomy_counts.iter().copied().max().unwrap_or(0);

    // Balance score: 1 - CV (coefficient of variation), clamped to [0,1].
    // A perfectly uniform distribution has CV=0 → score=1.
    let balance_score = if covered_types < 2 {
        0.0
    } else {
        let covered_counts: Vec<f64> = taxonomy_counts
            .iter()
            .filter(|&&c| c > 0)
            .map(|&c| c as f64)
            .collect();
        let mean = covered_counts.iter().sum::<f64>() / covered_counts.len() as f64;
        let variance = covered_counts
            .iter()
            .map(|&c| (c - mean).powi(2))
            .sum::<f64>()
            / covered_counts.len() as f64;
        let std_dev = variance.sqrt();
        let cv = if mean > 0.0 { std_dev / mean } else { 1.0 };
        (1.0 - cv).clamp(0.0, 1.0)
    };

    underrepresented_types.sort_by_key(|(_, c)| *c);

    CoverageReport {
        total_pairs,
        covered_types,
        total_types,
        missing_types,
        underrepresented_types,
        counts,
        min_covered_count,
        max_covered_count,
        balance_score,
        coverage_ratio: covered_types as f64 / total_types as f64,
        min_pairs_threshold: min_pairs_per_category,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_jsonl(categories: &[&str]) -> String {
        categories
            .iter()
            .map(|c| format!(r#"{{"prompt":"x","response":"y","category":"{c}"}}"#))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn counts_categories_correctly() {
        let taxonomy = &["function", "actor", "table", "workflow"];
        let jsonl = make_jsonl(&["function", "function", "actor", "table"]);
        let report = analyse_str_with_taxonomy(&jsonl, 1, taxonomy);
        assert_eq!(report.total_pairs, 4);
        assert_eq!(*report.counts.get("function").unwrap(), 2);
        assert_eq!(*report.counts.get("actor").unwrap(), 1);
    }

    #[test]
    fn detects_missing_types() {
        let taxonomy = &["function", "actor", "table", "workflow"];
        let jsonl = make_jsonl(&["function"]);
        let report = analyse_str_with_taxonomy(&jsonl, 1, taxonomy);
        assert!(report.missing_types.contains(&"actor".to_string()));
        assert!(report.missing_types.contains(&"workflow".to_string()));
        assert!(!report.missing_types.contains(&"function".to_string()));
    }

    #[test]
    fn detects_underrepresented() {
        let taxonomy = &["function", "actor", "table", "workflow"];
        // "function" has 1 pair, threshold is 5 → underrepresented
        let jsonl = make_jsonl(&["function"]);
        let report = analyse_str_with_taxonomy(&jsonl, 5, taxonomy);
        let under_keys: Vec<&str> = report
            .underrepresented_types
            .iter()
            .map(|(k, _)| k.as_str())
            .collect();
        assert!(
            under_keys.contains(&"function"),
            "function should be underrepresented"
        );
    }

    #[test]
    fn perfect_balance_score() {
        let taxonomy = &["function", "actor", "table", "workflow"];
        let jsonl = make_jsonl(&["function", "actor", "table"]);
        let report = analyse_str_with_taxonomy(&jsonl, 1, taxonomy);
        assert!(
            (report.balance_score - 1.0).abs() < 1e-9,
            "expected balance=1.0 for uniform, got {}",
            report.balance_score
        );
    }

    #[test]
    fn coverage_ratio_full() {
        let all: Vec<&str> = vec!["function", "actor", "table", "workflow"];
        let jsonl = make_jsonl(&all);
        let report = analyse_str_with_taxonomy(&jsonl, 1, &all);
        assert_eq!(report.coverage_ratio, 1.0);
        assert!(report.missing_types.is_empty());
    }

    #[test]
    fn rust_prefix_normalised() {
        let taxonomy = &["parser"];
        let jsonl = r#"{"prompt":"x","response":"y","category":"rust_parser"}"#;
        let report = analyse_str_with_taxonomy(jsonl, 1, taxonomy);
        // "parser" is in taxonomy and should be counted in counts correctly
        assert_eq!(report.total_pairs, 1);
    }

    #[test]
    fn summary_is_nonempty() {
        let taxonomy = &["function", "actor", "table", "workflow"];
        let jsonl = make_jsonl(&["function"]);
        let report = analyse_str_with_taxonomy(&jsonl, 1, taxonomy);
        let s = report.summary();
        assert!(
            s.contains("[coverage]"),
            "summary should contain [coverage]: {s}"
        );
    }

    #[test]
    fn is_sufficient_only_when_all_covered() {
        let all: Vec<&str> = vec!["function", "actor", "table", "workflow"];
        // Repeat each 5 times so threshold of 5 is met
        let rows: Vec<&str> = all
            .iter()
            .flat_map(|t| std::iter::repeat_n(*t, 5))
            .collect();
        let jsonl = make_jsonl(&rows);
        let report = analyse_str_with_taxonomy(&jsonl, 5, &all);
        assert!(
            report.is_sufficient(),
            "should be sufficient: {:?}",
            report.missing_types
        );
    }
}
