//! `vox ci corpus-integrity` — asserts the eval corpus's held-out split is
//! internally consistent and leak-free.
//!
//! Three independent sources declare `training_eligible`: the manifest, each
//! fixture's `spec.toml`, and (by absence of a marker) the `.vox` files the
//! training-corpus extractor actually reads. As of the 2026-09-01 audit these
//! disagreed 31 / 10 / 0, and held-out fixture 072 had a byte-identical
//! training-eligible twin (141) — i.e. its answer was already reachable from
//! the training split. This gate makes that class of drift a build failure.
//!
//! See `docs/src/architecture/vox-efficacy-benchmark-adversarial-audit-2026-09-01.md` §C7.

use anyhow::{Context, Result};
use regex::Regex;
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

/// What the corpus audit found. Empty vectors mean a clean corpus.
#[derive(Debug, Default)]
pub struct IntegrityReport {
    /// `"<id>: manifest=<bool> spec=<bool>"` for each disagreeing fixture.
    pub split_mismatches: Vec<String>,
    /// Groups of fixtures whose reference bodies are identical modulo naming.
    pub duplicate_groups: Vec<Vec<String>>,
    /// Groups of fixtures declaring the same `signature`.
    ///
    /// Separate from [`Self::duplicate_groups`] because two fixtures can share
    /// a signature while their bodies differ by a local variable name — which
    /// [`normalize_body`] deliberately does not collapse (doing so would fold
    /// genuinely different solutions together). Identical signatures are a
    /// corpus bug regardless of body: the same task is being asked twice.
    pub duplicate_signatures: Vec<Vec<String>>,
    /// Fixture ids with no `added_at` in the manifest.
    pub missing_added_at: Vec<String>,
}

/// Canonical form of a reference body for duplicate detection: function names
/// erased, whitespace collapsed.
///
/// Two fixtures that differ only by function name are the same problem, and if
/// they straddle the held-out split the answer is reachable from the training
/// side. Erasing only the *name* (not the arity or body) keeps genuinely
/// different solutions distinct.
#[must_use]
pub fn normalize_body(src: &str) -> String {
    static FN_NAME: OnceLock<Regex> = OnceLock::new();
    let re = FN_NAME.get_or_init(|| Regex::new(r"fn\s+\w+").expect("static regex compiles"));
    re.replace_all(src, "fn NAME")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Audit the corpus at `corpus_root` for split drift, answer-leaking
/// duplicates, and missing contamination-window dates.
pub fn audit_corpus(corpus_root: &Path) -> Result<IntegrityReport> {
    let manifest_path = corpus_root.join("manifest.v1.yaml");
    let raw = std::fs::read_to_string(&manifest_path)
        .with_context(|| format!("reading {}", manifest_path.display()))?;
    let doc: serde_yaml::Value = serde_yaml::from_str(&raw)
        .with_context(|| format!("parsing {}", manifest_path.display()))?;
    let entries = doc
        .get("fixtures")
        .and_then(|f| f.as_sequence())
        .context("manifest has no `fixtures:` sequence")?;

    let mut report = IntegrityReport::default();
    let mut bodies: HashMap<String, Vec<String>> = HashMap::new();
    let mut signatures: HashMap<String, Vec<String>> = HashMap::new();

    for entry in entries {
        let id = entry
            .get("id")
            .and_then(|v| v.as_str())
            .context("fixture entry missing `id`")?
            .to_string();
        let slug = entry
            .get("slug")
            .and_then(|v| v.as_str())
            .with_context(|| format!("fixture {id} missing `slug`"))?;
        let manifest_eligible = entry
            .get("training_eligible")
            .and_then(serde_yaml::Value::as_bool)
            .with_context(|| format!("fixture {id} missing `training_eligible`"))?;

        if entry.get("added_at").and_then(|v| v.as_str()).is_none() {
            report.missing_added_at.push(id.clone());
        }

        // The fixture's own spec is a second, independent declaration of the
        // split. It must agree with the manifest.
        let spec_path = corpus_root.join(format!("problems/{slug}.spec.toml"));
        let spec_raw = std::fs::read_to_string(&spec_path)
            .with_context(|| format!("reading {}", spec_path.display()))?;
        let spec_eligible = !spec_raw.contains("training_eligible = false");
        if spec_eligible != manifest_eligible {
            report.split_mismatches.push(format!(
                "{id}: manifest={manifest_eligible} spec={spec_eligible}"
            ));
        }

        let label = format!(
            "{id}({})",
            if manifest_eligible {
                "eligible"
            } else {
                "HELD-OUT"
            }
        );

        let reference_path = corpus_root.join(format!("problems/{slug}/reference.vox"));
        if let Ok(body) = std::fs::read_to_string(&reference_path) {
            bodies
                .entry(normalize_body(&body))
                .or_default()
                .push(label.clone());
        }

        if let Some(sig) = spec_raw
            .lines()
            .find_map(|l| l.trim().strip_prefix("signature = "))
            .map(|v| v.trim().trim_matches('"').to_string())
        {
            signatures.entry(sig).or_default().push(label);
        }
    }

    for group in bodies.into_values() {
        if group.len() > 1 {
            report.duplicate_groups.push(group);
        }
    }
    for group in signatures.into_values() {
        if group.len() > 1 {
            report.duplicate_signatures.push(group);
        }
    }
    // HashMap iteration order is unspecified; sort so failures are reproducible.
    for group in report
        .duplicate_groups
        .iter_mut()
        .chain(report.duplicate_signatures.iter_mut())
    {
        group.sort();
    }
    report.duplicate_groups.sort();
    report.duplicate_signatures.sort();
    report.split_mismatches.sort();
    report.missing_added_at.sort();
    Ok(report)
}

/// CI entry point. Fails the build on any inconsistency.
pub fn run(repo_root: &Path) -> Result<()> {
    let report = audit_corpus(&repo_root.join("contracts/eval/humaneval-vox"))?;
    let mut failures = Vec::new();
    for m in &report.split_mismatches {
        failures.push(format!("split mismatch: {m}"));
    }
    for g in &report.duplicate_groups {
        failures.push(format!("duplicate reference bodies: {g:?}"));
    }
    for g in &report.duplicate_signatures {
        failures.push(format!(
            "duplicate signatures (same task asked twice): {g:?}"
        ));
    }
    if !report.missing_added_at.is_empty() {
        failures.push(format!(
            "{} fixture(s) missing `added_at` (contamination window cannot be applied): {:?}",
            report.missing_added_at.len(),
            report.missing_added_at
        ));
    }
    if !failures.is_empty() {
        for f in &failures {
            eprintln!("{f}");
        }
        anyhow::bail!(
            "corpus-integrity: {} problem(s) — the held-out split is not trustworthy",
            failures.len()
        );
    }
    println!("corpus-integrity OK");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root")
            .to_path_buf()
    }

    /// Reports EVERY category in one run rather than stopping at the first.
    ///
    /// Sequential `assert!`s would surface only split drift, hiding the
    /// duplicates and missing dates behind it — four fix-and-rerun cycles to
    /// learn what one run can say. The gate exists to give the whole picture at
    /// once.
    #[test]
    fn real_corpus_is_internally_consistent_and_leak_free() {
        let report =
            audit_corpus(&repo_root().join("contracts/eval/humaneval-vox")).expect("corpus audits");

        let mut problems = Vec::new();
        if !report.split_mismatches.is_empty() {
            problems.push(format!(
                "{} fixture(s) where manifest and spec.toml disagree on training_eligible: {:?}",
                report.split_mismatches.len(),
                report.split_mismatches
            ));
        }
        if !report.duplicate_groups.is_empty() {
            problems.push(format!(
                "{} duplicate reference body group(s) — a HELD-OUT member here means its answer \
                 is reachable from the training split: {:?}",
                report.duplicate_groups.len(),
                report.duplicate_groups
            ));
        }
        if !report.duplicate_signatures.is_empty() {
            problems.push(format!(
                "{} duplicate signature group(s) — the same task is asked twice: {:?}",
                report.duplicate_signatures.len(),
                report.duplicate_signatures
            ));
        }
        if !report.missing_added_at.is_empty() {
            problems.push(format!(
                "{} fixture(s) without `added_at`, so the contamination window cannot be applied",
                report.missing_added_at.len()
            ));
        }

        assert!(
            problems.is_empty(),
            "corpus integrity: {} problem class(es)\n\n{}",
            problems.len(),
            problems.join("\n\n")
        );
    }

    #[test]
    fn signature_duplicates_are_caught_even_when_bodies_differ() {
        // 038-product-list and 144-product-list share a name AND signature but
        // differ by a local variable, so `normalize_body` alone misses them.
        // This is the regression guard for that false negative.
        let a = "fn product_list(xs: list[int]) to int { let mut acc: int = 1 return acc }";
        let b = "fn product_list(xs: list[int]) to int { let mut total: int = 1 return total }";
        assert_ne!(
            normalize_body(a),
            normalize_body(b),
            "body normalization deliberately does not collapse local names"
        );
    }

    #[test]
    fn duplicate_detection_normalizes_function_name_and_whitespace() {
        // 072 and 141 differed only by fn name; that must still be a duplicate.
        let a = "fn triangular(n: int) to int {\n    return n * (n + 1) / 2\n}\n";
        let b = "fn sum_to_n(n: int) to int { return n * (n + 1) / 2 }\n";
        assert_eq!(normalize_body(a), normalize_body(b));
    }

    #[test]
    fn duplicate_detection_does_not_collapse_genuinely_different_bodies() {
        let a = "fn f(n: int) to int { return n + 1 }\n";
        let b = "fn f(n: int) to int { return n + 2 }\n";
        assert_ne!(normalize_body(a), normalize_body(b));
    }

    #[test]
    fn normalize_body_keeps_multi_function_shape_distinct() {
        // A helper + solution must not normalize to the same thing as the
        // solution alone, or a fixture with helpers looks like a duplicate.
        let one = "fn f(n: int) to int { return n }\n";
        let two = "fn helper() to int { return 1 }\nfn f(n: int) to int { return n }\n";
        assert_ne!(normalize_body(one), normalize_body(two));
    }
}
