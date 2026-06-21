//! B7.0 — Leakage assertion: verifies no tool appears in both training and eval sets.
//!
//! Must run before any gate result is trusted.

use anyhow::{Result, bail};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Minimal split manifest — mirrors what B1.4 eval_split.rs writes.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct SplitManifest {
    /// Tool names (or identifiers) present in the training partition.
    pub train_tools: Vec<String>,
    /// Tool names present in the eval partition.
    pub eval_tools: Vec<String>,
    /// Random seed used for the split.
    #[serde(default)]
    pub seed: u64,
    /// Fraction held out for eval (0.0–1.0).
    #[serde(default)]
    pub eval_frac: f64,
}

/// Load a `SplitManifest` from `split_manifest.json` in `corpus_dir`, or from
/// an explicit path if provided.
pub fn load_split_manifest(path: &Path) -> Result<SplitManifest> {
    let content = vox_bounded_fs::read_utf8_path_capped(path)?;
    let manifest: SplitManifest = serde_json::from_str(&content)?;
    Ok(manifest)
}

// ---------------------------------------------------------------------------
// 3-gram fingerprint helpers (inline — do NOT add vox-similarity dep here)
// ---------------------------------------------------------------------------

/// Normalise a tool name to a canonical form for near-dup comparison.
fn normalise(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_')
        .collect()
}

/// Produce the set of character-3-grams from a string.
fn trigrams(s: &str) -> HashSet<[char; 3]> {
    let chars: Vec<char> = s.chars().collect();
    chars.windows(3).map(|w| [w[0], w[1], w[2]]).collect()
}

/// Jaccard similarity between two sets of 3-grams (0.0–1.0).
fn jaccard(a: &HashSet<[char; 3]>, b: &HashSet<[char; 3]>) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    let inter = a.intersection(b).count() as f64;
    let union = a.union(b).count() as f64;
    if union == 0.0 { 1.0 } else { inter / union }
}

/// Near-dup threshold: two tool names with Jaccard ≥ this are considered duplicates.
const NEAR_DUP_THRESHOLD: f64 = 0.75;

/// Assert that no tool name (exact or near-duplicate) appears in both the
/// training and eval partitions of `manifest`.
///
/// Also reads corpus rows from `corpus_dir` (JSONL files `*.jsonl`) if present
/// and cross-checks tool names from rows against the split.
///
/// Returns `Ok(())` if clean, `Err(...)` describing leakage if not.
pub fn assert_no_leakage(corpus_dir: &Path, manifest: &SplitManifest) -> Result<()> {
    // Step 1: exact intersection of declared tool lists
    let train_set: HashSet<String> = manifest.train_tools.iter().cloned().collect();
    let eval_set: HashSet<String> = manifest.eval_tools.iter().cloned().collect();

    let exact_leaks: Vec<String> = train_set.intersection(&eval_set).cloned().collect();
    if !exact_leaks.is_empty() {
        bail!(
            "leakage detected: {} tool(s) appear in both train and eval splits: {:?}",
            exact_leaks.len(),
            exact_leaks
        );
    }

    // Step 2: near-dup check via 3-gram Jaccard
    let train_grams: Vec<(String, HashSet<[char; 3]>)> = manifest
        .train_tools
        .iter()
        .map(|t| {
            let n = normalise(t);
            let g = trigrams(&n);
            (t.clone(), g)
        })
        .collect();

    let eval_grams: Vec<(String, HashSet<[char; 3]>)> = manifest
        .eval_tools
        .iter()
        .map(|t| {
            let n = normalise(t);
            let g = trigrams(&n);
            (t.clone(), g)
        })
        .collect();

    let mut near_dup_leaks: Vec<(String, String, f64)> = Vec::new();
    for (et, eg) in &eval_grams {
        for (tt, tg) in &train_grams {
            let sim = jaccard(eg, tg);
            if sim >= NEAR_DUP_THRESHOLD && et != tt {
                near_dup_leaks.push((et.clone(), tt.clone(), sim));
            }
        }
    }
    if !near_dup_leaks.is_empty() {
        let details: Vec<String> = near_dup_leaks
            .iter()
            .map(|(e, t, sim)| format!("eval='{}' ~ train='{}' (sim={:.2})", e, t, sim))
            .collect();
        bail!(
            "near-duplicate leakage detected ({} pair(s)):\n{}",
            near_dup_leaks.len(),
            details.join("\n")
        );
    }

    // Step 3: cross-check corpus JSONL rows (if corpus_dir contains JSONL)
    if corpus_dir.exists() {
        let corpus_leaks = check_corpus_rows(corpus_dir, &train_set, &eval_set)?;
        if !corpus_leaks.is_empty() {
            bail!(
                "corpus row leakage: tools found in both training and eval corpus rows: {:?}",
                corpus_leaks
            );
        }
    }

    Ok(())
}

/// Scan JSONL files in `corpus_dir` for rows with a `tool` (or `tool_name`)
/// field, partition into train vs eval buckets via `split` membership, and
/// return any tool names whose rows appear in both buckets.
fn check_corpus_rows(
    corpus_dir: &Path,
    train_set: &HashSet<String>,
    eval_set: &HashSet<String>,
) -> Result<Vec<String>> {
    use std::io::{BufRead, BufReader};

    // Collect tool→{in_train, in_eval} presence
    let mut seen_train: HashSet<String> = HashSet::new();
    let mut seen_eval: HashSet<String> = HashSet::new();

    let rd = std::fs::read_dir(corpus_dir)?;
    for entry in rd.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }
        let file = std::fs::File::open(&path)?;
        let reader = BufReader::new(file);
        for line in reader.lines() {
            let line = line?;
            if line.is_empty() {
                continue;
            }
            let v: serde_json::Value = match serde_json::from_str(&line) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let tool_name = v
                .get("tool")
                .or_else(|| v.get("tool_name"))
                .and_then(|t| t.as_str())
                .map(|s| s.to_string());
            if let Some(tool) = tool_name {
                if train_set.contains(&tool) {
                    seen_train.insert(tool);
                } else if eval_set.contains(&tool) {
                    seen_eval.insert(tool);
                }
            }
        }
    }

    let leaks: Vec<String> = seen_train.intersection(&seen_eval).cloned().collect();
    Ok(leaks)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(train: &[&str], eval: &[&str]) -> SplitManifest {
        SplitManifest {
            train_tools: train.iter().map(|s| s.to_string()).collect(),
            eval_tools: eval.iter().map(|s| s.to_string()).collect(),
            seed: 42,
            eval_frac: 0.2,
        }
    }

    #[test]
    fn clean_corpus_passes() {
        let dir = tempfile::tempdir().unwrap();
        let m = manifest(
            &["read_file", "write_file"],
            &["search_code", "grep_pattern"],
        );
        assert_no_leakage(dir.path(), &m).expect("clean corpus should pass");
    }

    #[test]
    fn exact_leakage_fails() {
        let dir = tempfile::tempdir().unwrap();
        let m = manifest(
            &["read_file", "write_file", "shell_exec"],
            &["search_code", "write_file"], // write_file in both
        );
        let err = assert_no_leakage(dir.path(), &m).unwrap_err();
        assert!(
            err.to_string().contains("leakage detected"),
            "expected leakage error, got: {err}"
        );
    }

    #[test]
    fn near_dup_leakage_fails() {
        let dir = tempfile::tempdir().unwrap();
        // "read_file" and "read_files" are very similar (high Jaccard on 3-grams)
        let m = manifest(
            &["read_file_content"],
            &["read_file_contents"], // near-dup
        );
        let result = assert_no_leakage(dir.path(), &m);
        // These names have high trigram similarity — should fail
        assert!(
            result.is_err(),
            "near-duplicate tool names should trigger leakage check"
        );
    }

    #[test]
    fn distinct_names_pass_near_dup_check() {
        let dir = tempfile::tempdir().unwrap();
        // Completely different names
        let m = manifest(
            &["write_file", "shell_exec"],
            &["search_semantic", "list_branches"],
        );
        assert_no_leakage(dir.path(), &m).expect("distinct names should pass");
    }

    #[test]
    fn corpus_row_leakage_fails() {
        let dir = tempfile::tempdir().unwrap();
        // write a JSONL file with a tool in both partitions
        let jsonl_content = r#"{"tool":"alpha_tool","output":"x"}
{"tool":"beta_tool","output":"y"}
"#;
        std::fs::write(dir.path().join("corpus.jsonl"), jsonl_content).unwrap();
        // manifest says alpha_tool is train-only, but corpus row would appear in eval
        let m = manifest(&["alpha_tool", "gamma_tool"], &["alpha_tool", "delta_tool"]);
        let err = assert_no_leakage(dir.path(), &m).unwrap_err();
        assert!(
            err.to_string().contains("leakage"),
            "corpus row leakage should be detected: {err}"
        );
    }

    #[test]
    fn split_manifest_round_trips() {
        let m = manifest(&["tool_a", "tool_b"], &["tool_c"]);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("split_manifest.json");
        std::fs::write(&path, serde_json::to_string(&m).unwrap()).unwrap();
        let loaded = load_split_manifest(&path).unwrap();
        assert_eq!(loaded.train_tools, m.train_tools);
        assert_eq!(loaded.eval_tools, m.eval_tools);
        assert_eq!(loaded.seed, 42);
    }
}
