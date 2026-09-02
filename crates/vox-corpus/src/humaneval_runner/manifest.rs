//! Corpus manifest + per-fixture spec loading, with rolling-window eligibility.
//!
//! The manifest is the SSOT for `training_eligible` (see the header comment in
//! `contracts/eval/humaneval-vox/manifest.v1.yaml`); `vox ci corpus-integrity`
//! enforces that each fixture's `spec.toml` and `.vox` marker agree with it.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// One corpus problem: its prompt, its required signature, and where its tests live.
#[derive(Debug, Clone)]
pub struct Fixture {
    /// Stable id, never recycled (e.g. `"041"`).
    pub id: String,
    /// Directory-name slug (e.g. `"041-nth-prime"`).
    pub slug: String,
    /// `false` = held out of MENS training; only these are valid for external claims.
    pub training_eligible: bool,
    /// ISO date this fixture entered the corpus (rolling-window contamination guard).
    pub added_at: String,
    /// The natural-language task given to the model.
    pub prompt: String,
    /// The exact function signature the solution must provide.
    pub signature: String,
    /// Absolute path to the fixture's `tests.vox`.
    pub tests_path: PathBuf,
}

/// Load every fixture declared by `<corpus_root>/manifest.v1.yaml`, reading each
/// one's prompt and signature from its `spec.toml`.
///
/// Fails loudly on a manifest entry whose files are missing — a silently-skipped
/// fixture would quietly shrink the denominator and inflate every pass rate.
pub fn load_corpus(corpus_root: &Path) -> Result<Vec<Fixture>> {
    let manifest_path = corpus_root.join("manifest.v1.yaml");
    let raw = std::fs::read_to_string(&manifest_path)
        .with_context(|| format!("reading {}", manifest_path.display()))?;
    let doc: serde_yaml::Value = serde_yaml::from_str(&raw)
        .with_context(|| format!("parsing {}", manifest_path.display()))?;

    let entries = doc
        .get("fixtures")
        .and_then(|f| f.as_sequence())
        .context("manifest has no `fixtures:` sequence")?;

    let mut out = Vec::with_capacity(entries.len());
    for entry in entries {
        let s = |key: &str| -> Result<String> {
            entry
                .get(key)
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .with_context(|| format!("fixture entry missing `{key}`"))
        };
        let id = s("id")?;
        let slug = s("slug")?;
        let added_at = s("added_at").with_context(|| {
            format!("fixture {id} has no `added_at` — required for rolling-window scoring")
        })?;
        let training_eligible = entry
            .get("training_eligible")
            .and_then(serde_yaml::Value::as_bool)
            .with_context(|| format!("fixture {id} missing `training_eligible`"))?;

        let spec_rel = entry
            .get("files")
            .and_then(|f| f.get("spec"))
            .and_then(|v| v.as_str())
            .with_context(|| format!("fixture {id} missing `files.spec`"))?;
        let tests_rel = entry
            .get("files")
            .and_then(|f| f.get("tests"))
            .and_then(|v| v.as_str())
            .with_context(|| format!("fixture {id} missing `files.tests`"))?;

        let spec_path = corpus_root.join(spec_rel);
        let spec_raw = std::fs::read_to_string(&spec_path)
            .with_context(|| format!("reading {}", spec_path.display()))?;
        let spec: toml::Value = spec_raw
            .parse()
            .with_context(|| format!("parsing {}", spec_path.display()))?;
        let problem = spec.get("problem").context("spec.toml has no [problem]")?;
        let field = |key: &str| -> Result<String> {
            problem
                .get(key)
                .and_then(|v| v.as_str())
                .map(|v| v.trim().to_string())
                .with_context(|| format!("{} missing [problem].{key}", spec_path.display()))
        };

        out.push(Fixture {
            id,
            slug,
            training_eligible,
            added_at,
            prompt: field("prompt")?,
            signature: field("signature")?,
            tests_path: corpus_root.join(tests_rel),
        });
    }
    Ok(out)
}

/// The held-out subset — the ONLY fixtures valid for an external efficacy claim.
#[must_use]
pub fn held_out(fixtures: &[Fixture]) -> Vec<&Fixture> {
    fixtures.iter().filter(|f| !f.training_eligible).collect()
}

/// Fixtures added strictly after `cutoff` (an ISO `YYYY-MM-DD` date).
///
/// ISO dates compare correctly as strings. Scoring a model only on problems
/// that postdate its training cutoff is the LiveCodeBench contamination-
/// resistance mechanism: a model cannot have memorized a problem that did not
/// exist when it was trained. Prefer a model's `knowledge_cutoff` (from the
/// OpenRouter catalog) over a hand-typed cutoff — see the audit's correction
/// on why an operator-chosen date is a weaker guard.
#[must_use]
pub fn eligible_after<'a>(fixtures: &'a [Fixture], cutoff: &str) -> Vec<&'a Fixture> {
    fixtures
        .iter()
        .filter(|f| f.added_at.as_str() > cutoff)
        .collect()
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

    fn corpus_root() -> std::path::PathBuf {
        repo_root().join("contracts/eval/humaneval-vox")
    }

    #[test]
    fn loads_the_real_corpus_with_prompts_and_dates() {
        let fixtures = load_corpus(&corpus_root()).expect("corpus loads");
        assert_eq!(fixtures.len(), 164, "manifest declares count_current: 164");

        let first = fixtures
            .iter()
            .find(|f| f.id == "001")
            .expect("fixture 001");
        assert_eq!(first.slug, "001-fizzbuzz");
        assert!(first.training_eligible);
        assert_eq!(first.added_at, "2026-05-26");
        assert!(
            first.prompt.contains("FizzBuzz"),
            "prompt is read from the spec.toml, got: {}",
            first.prompt
        );
        assert!(first.signature.starts_with("fn fizzbuzz"));
        assert!(first.tests_path.exists(), "tests.vox path resolves on disk");
    }

    #[test]
    fn held_out_selects_only_non_training_eligible_fixtures() {
        let fixtures = load_corpus(&corpus_root()).expect("corpus loads");
        let ho = held_out(&fixtures);
        assert_eq!(ho.len(), 31, "manifest declares 31 held-out fixtures");
        assert!(
            ho.iter().all(|f| !f.training_eligible),
            "every held-out fixture must be training_eligible: false"
        );
    }

    #[test]
    fn eligible_after_excludes_fixtures_added_on_or_before_the_cutoff() {
        let fixtures = load_corpus(&corpus_root()).expect("corpus loads");
        assert!(
            eligible_after(&fixtures, "2026-09-01").is_empty(),
            "a cutoff on the newest addition date admits nothing after it"
        );
        assert_eq!(eligible_after(&fixtures, "2026-05-01").len(), 164);
        let mid = eligible_after(&fixtures, "2026-05-26");
        assert!(!mid.is_empty(), "the 2026-05-27+ batch is still eligible");
        assert!(mid.iter().all(|f| f.added_at.as_str() > "2026-05-26"));
    }
}
