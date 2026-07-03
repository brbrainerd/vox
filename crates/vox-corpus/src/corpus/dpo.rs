use crate::ast_mutator;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
#[cfg(feature = "database")]
use vox_db::VoxDb;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DpoPair {
    pub prompt: String,
    pub chosen: String,
    pub rejected: String,
    pub category: String,
    pub source: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DpoConfig {
    pub input: PathBuf,
    pub output: PathBuf,
    pub limit: usize,
}

pub fn generate_dpo_from_extract(config: &DpoConfig) -> anyhow::Result<usize> {
    use std::fs::File;
    use std::io::{BufRead, BufReader, Write};

    let input_file = File::open(&config.input)?;
    let reader = BufReader::new(input_file);

    let mut out_file = File::create(&config.output)?;
    let mut count = 0;

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let value: serde_json::Value = serde_json::from_str(&line)?;
        let prompt = value
            .get("prompt")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let chosen = value
            .get("response")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let category = value
            .get("category")
            .and_then(|v| v.as_str())
            .unwrap_or("vox_source")
            .to_string();
        let source = value
            .get("source")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        if chosen.is_empty() {
            continue;
        }

        // Generate rejected sample by mutating 'chosen'
        // For Vox files, we use the ast_mutator if it's not a generic prompt
        let rejected =
            if chosen.contains("fn ") || chosen.contains("actor ") || chosen.contains("@") {
                // Try to parse and mutate
                if let Ok(result) = vox_compiler::pipeline::run_frontend_str(&chosen, "<dpo-gen>") {
                    let mutations = ast_mutator::generate_mutations(&chosen, &result.module);
                    if !mutations.is_empty() {
                        ast_mutator::apply_mutations(&chosen, mutations)
                    } else {
                        // Fallback: simple string manipulation
                        chosen.replace("fn ", "function ")
                    }
                } else {
                    chosen.replace("fn ", "function ")
                }
            } else {
                chosen.replace("let ", "var ")
            };

        if rejected == chosen {
            // Skip pairs where mutation failed to change anything
            continue;
        }

        let pair = DpoPair {
            prompt,
            chosen,
            rejected,
            category,
            source,
        };

        let json = serde_json::to_string(&pair)?;
        writeln!(out_file, "{}", json)?;
        count += 1;

        if config.limit > 0 && count >= config.limit {
            break;
        }
    }

    Ok(count)
}

/// Mine PR code-review findings into Rust-review DPO preference pairs (R.2).
///
/// Reads an [`ExternalReviewReplayRow`](crate::external_review_replay::ExternalReviewReplayRow)
/// JSONL file (written by `vox corpus review-export`) and produces `DpoPair` JSONL where:
/// - `chosen`   = the suggested fix from the review finding (`response`).
/// - `rejected` = a *provably worse* variant produced by [`make_worse_variant`].
///
/// Only `sample_kind == "review_fix_pairs"` rows carry a real fix in `response`;
/// `review_antipattern_memory` / `review_regression_challenges` rows are prose and are
/// skipped. Rows for which no meaningful degradation applies are skipped (no no-op pairs).
#[must_use = "the returned count of mined pairs should be checked or logged"]
pub fn review_findings_to_dpo(
    input: &std::path::Path,
    output: &std::path::Path,
) -> anyhow::Result<usize> {
    use crate::external_review_replay::ExternalReviewReplayRow;
    use std::fs::File;
    use std::io::{BufRead, BufReader, Write};

    let reader = BufReader::new(File::open(input)?);
    let mut out = File::create(output)?;
    let mut count = 0;

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        // Typed parse keeps the producer/consumer field contract compiler-checked:
        // a rename in ExternalReviewReplayRow becomes a build break, not silent zero output.
        let row: ExternalReviewReplayRow = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(_) => continue,
        };
        // Only fix-pair findings have an actionable suggested fix in `response`.
        if row.sample_kind != "review_fix_pairs" {
            continue;
        }
        if row.response.trim().is_empty() || row.prompt.trim().is_empty() {
            continue;
        }
        let chosen = row.response.clone();
        // Guard == capability: only the constructs make_worse_variant can degrade.
        let Some(rejected) = make_worse_variant(&chosen) else {
            continue;
        };
        if rejected == chosen {
            continue;
        }

        let pair = DpoPair {
            prompt: row.prompt,
            chosen,
            rejected,
            category: format!("rust_review_{}", row.category),
            source: Some("review_findings".to_string()),
        };
        writeln!(out, "{}", serde_json::to_string(&pair)?)?;
        count += 1;
    }

    Ok(count)
}

/// Produce a *provably worse* Rust variant of `src` for a DPO `rejected` sample,
/// or `None` if no meaningful degradation applies (caller skips the row).
///
/// Each transform makes the code strictly worse so the preference signal is correct:
/// 1. `expr?;`        → `expr.unwrap();`  (panics instead of propagating `Err`)
/// 2. `.expect("..")` → `.unwrap()`       (drops the diagnostic message)
/// 3. strip `-> Type` from a fn signature (body now type-mismatches)
fn make_worse_variant(src: &str) -> Option<String> {
    if src.contains("?;") {
        let v = src.replacen("?;", ".unwrap();", 1);
        if v != src {
            return Some(v);
        }
    }
    if let Some(v) = downgrade_expect_to_unwrap(src) {
        return Some(v);
    }
    if src.contains("->") {
        let v = strip_return_type(src);
        if v != src {
            return Some(v);
        }
    }
    None
}

/// Replace the first `.expect("msg")` with `.unwrap()` (balanced-paren aware).
fn downgrade_expect_to_unwrap(src: &str) -> Option<String> {
    let start = src.find(".expect(")?;
    let open = start + ".expect".len(); // index of '('
    let bytes = src.as_bytes();
    let mut depth = 0i32;
    let mut end = None;
    for (i, &b) in bytes.iter().enumerate().skip(open) {
        match b {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(i);
                    break;
                }
            }
            _ => {}
        }
    }
    let end = end?;
    let mut produced = String::with_capacity(src.len());
    produced.push_str(&src[..start]);
    produced.push_str(".unwrap()");
    produced.push_str(&src[end + 1..]);
    Some(produced)
}

/// Strip the `-> Type` return annotation from fn signature lines. Uses the LAST
/// `->` before the body `{` so function-type params (`impl Fn() -> i32`) are not
/// mistaken for the return arrow.
fn strip_return_type(src: &str) -> String {
    src.lines()
        .map(|line| {
            let t = line.trim_start();
            let is_sig = t.starts_with("fn ")
                || t.starts_with("pub fn ")
                || t.starts_with("async fn ")
                || t.starts_with("pub async fn ")
                || t.starts_with("pub(crate) fn ")
                || t.starts_with("const fn ")
                || t.starts_with("unsafe fn ");
            if is_sig
                && let Some(brace) = line.find('{')
            {
                let head = &line[..brace];
                if let Some(arrow) = head.rfind("->") {
                    let before = head[..arrow].trim_end();
                    return format!("{} {}", before, &line[brace..]);
                }
            }
            line.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Export DPO preference pairs from VoxDB (corrections vs original failures).
#[cfg(feature = "database")]
pub async fn export_dogfood_dpo(db: &VoxDb, limit: i64, output: &PathBuf) -> anyhow::Result<usize> {
    use std::fs::File;
    use std::io::Write;

    let pairs = db.get_training_data(limit).await?;
    let mut out_file = File::create(output)?;
    let mut count = 0;

    for pair in pairs {
        if let Some(preferred) = pair.correction.as_ref().filter(|c: &&String| !c.is_empty()) {
            let preferred_str: &str = preferred.as_str();
            let dpo = DpoPair {
                prompt: pair.prompt,
                chosen: preferred_str.to_string(),
                rejected: pair.response,
                category: "agents_dogfood_dpo".to_string(),
                source: Some("vox_db".to_string()),
            };
            let json = serde_json::to_string(&dpo)?;
            writeln!(out_file, "{}", json)?;
            count += 1;
        }
    }

    Ok(count)
}

#[cfg(test)]
mod review_dpo_tests {
    use super::*;
    use crate::external_review_replay::ExternalReviewReplayRow;

    fn fix_row(response: &str, sample_kind: &str) -> String {
        let row = ExternalReviewReplayRow {
            prompt: "Fix the issue in".to_string(),
            response: response.to_string(),
            category: "correctness".to_string(),
            severity: "high".to_string(),
            placement_kind: "inline".to_string(),
            source_id: "r1".to_string(),
            repository_id: "vox/vox".to_string(),
            pr_number: 42,
            file_path: Some("src/lib.rs".to_string()),
            line_start: Some(10),
            correctness_state: "accepted".to_string(),
            sample_kind: sample_kind.to_string(),
        };
        serde_json::to_string(&row).unwrap()
    }

    fn mine(jsonl: &str) -> Vec<DpoPair> {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("findings.jsonl");
        std::fs::write(&input, jsonl).unwrap();
        let output = dir.path().join("out.jsonl");
        let _ = review_findings_to_dpo(&input, &output).unwrap();
        std::fs::read_to_string(&output)
            .unwrap()
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    #[test]
    fn mines_return_type_strip_as_worse() {
        let pairs = mine(&format!(
            "{}\n",
            fix_row(
                "fn process(data: &[u8]) -> Vec<u8> { data.to_vec() }",
                "review_fix_pairs"
            )
        ));
        assert_eq!(pairs.len(), 1);
        let p = &pairs[0];
        assert_eq!(p.source.as_deref(), Some("review_findings"));
        assert_ne!(p.chosen, p.rejected);
        assert!(p.rejected.contains("fn process"), "fn name preserved");
        assert!(
            !p.rejected.contains("-> Vec<u8>"),
            "return type stripped → type error"
        );
        assert!(p.category.starts_with("rust_review_"));
    }

    #[test]
    fn try_operator_downgraded_to_unwrap() {
        let pairs = mine(&format!(
            "{}\n",
            fix_row("let v = parse(s)?;", "review_fix_pairs")
        ));
        assert_eq!(pairs.len(), 1);
        assert!(
            pairs[0].rejected.contains(".unwrap();"),
            "? → .unwrap() (panics)"
        );
        assert!(!pairs[0].rejected.contains("?;"));
    }

    #[test]
    fn expect_downgraded_to_unwrap_not_upgraded() {
        let pairs = mine(&format!(
            "{}\n",
            fix_row(
                "let v = thing.expect(\"clear message\");",
                "review_fix_pairs"
            )
        ));
        assert_eq!(pairs.len(), 1);
        // The rejected sample must be WORSE: unwrap() drops the message.
        assert!(pairs[0].rejected.contains(".unwrap()"));
        assert!(
            !pairs[0].rejected.contains(".expect("),
            "must not keep/prefer expect"
        );
    }

    #[test]
    fn fn_type_param_does_not_corrupt_signature() {
        // The param arrow (Fn() -> i32) must NOT be mistaken for the return arrow.
        let pairs = mine(&format!(
            "{}\n",
            fix_row(
                "fn run(cb: impl Fn() -> i32) -> i32 { cb() }",
                "review_fix_pairs"
            )
        ));
        assert_eq!(pairs.len(), 1);
        assert!(
            pairs[0].rejected.contains("impl Fn() -> i32"),
            "param arrow preserved"
        );
        assert!(pairs[0].rejected.contains("fn run"), "fn intact");
    }

    #[test]
    fn skips_prose_fix_rows() {
        let pairs = mine(&format!(
            "{}\n",
            fix_row(
                "The function should have a doc comment explaining the purpose.",
                "review_fix_pairs"
            )
        ));
        assert!(pairs.is_empty(), "pure prose has no degradable construct");
    }

    #[test]
    fn skips_non_fix_sample_kinds() {
        let pairs = mine(&format!(
            "{}\n",
            fix_row("fn good() -> i32 { 1 }", "review_antipattern_memory")
        ));
        assert!(
            pairs.is_empty(),
            "antipattern/regression rows are prose, not fixes"
        );
    }

    #[test]
    fn strip_return_type_handles_modifiers() {
        let src = "pub async fn compute(x: i32) -> i32 {\n    x * 2\n}";
        let result = strip_return_type(src);
        assert!(!result.contains("-> i32"));
        assert!(result.contains("pub async fn compute"));
    }
}
