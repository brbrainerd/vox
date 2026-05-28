//! Inputs to the scaffolder and the section-tree shape it produces.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// All the provenance-bound material the scaffolder needs to render an
/// IMRaD skeleton. Producers (vox-cli, vox-orchestrator) populate this from
/// `publication_manifests`, `scientia_claims`, and the RO-Crate metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScaffoldInput {
    /// Working title for the manuscript. Marked `machine_suggested` in the
    /// AI-disclosure block.
    pub title_hint: String,
    /// Authors with optional ORCID. Empty → an `<!-- TODO(author): -->`
    /// block is emitted instead of an auto-filled author list.
    pub authors: Vec<AuthorEntry>,
    /// One row per verified atomic claim. Renders into the Results section.
    pub results_rows: Vec<ResultsRow>,
    /// Verified prior-art citations the human can lean on while writing
    /// `Introduction` / `Discussion`. Surfaced as a TODO-block annotation —
    /// they are *not* assembled into a narrative.
    pub cited_facts: Vec<CitedFact>,
    /// Methods text the human approved at preflight. Empty → emit a methods
    /// TODO block instead of a placeholder claim.
    pub methods_summary: Option<String>,
    /// Free-text limitations from `manual_required` worthiness signals.
    pub limitations: Vec<String>,
    /// Phase 5 — figures with provenance, lifted from the RO-Crate
    /// `mainEntity.figures`. The scaffolder renders these as a `## Figures`
    /// section with traceability footers (path + SHA3 + source script);
    /// captions remain `<!-- TODO(figure-caption): -->` blocks per the
    /// worthiness rubric's "no auto-generated measurement-implying figures"
    /// rule.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub figures: Vec<FigureEntry>,
    /// Pre-built AI-disclosure block (typically from
    /// `vox_scientia::ro_crate::AiDisclosureBlock::build`). Rendered as-is
    /// at the end of the manuscript.
    pub ai_disclosure_markdown: Option<String>,
    /// Competing-interests statement. Required by the worthiness rubric;
    /// `None` results in a TODO block, not an empty string.
    pub competing_interests: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuthorEntry {
    pub name: String,
    pub orcid: Option<String>,
    pub affiliation_ror: Option<String>,
}

/// One figure entry, mirroring `vox_scientia::ro_crate::FigureProvenance`
/// but with the scaffolder's view: caption is always a TODO block, the
/// numbered label is auto-assigned, and the renderer surfaces the
/// provenance footer (hash + source script) so reviewers can replay.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FigureEntry {
    /// Relative path to the figure asset under the RO-Crate root.
    pub path: String,
    /// SHA3-256 of the rendered bytes, hex-encoded.
    pub sha3_256_hex: String,
    /// Path to the script that produced this figure (under the RO-Crate
    /// root). Reviewers re-run this against the manifest's `mainEntity`
    /// environment.
    pub source_script: String,
    /// Optional one-line hint for the caption-writer. Never used as the
    /// final caption — the renderer keeps the caption as a TODO block.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption_hint: Option<String>,
}

/// One verified atomic claim → one results-table row. The Trusty URI binds
/// the row to its signed nanopublication; the renderer surfaces it as a
/// markdown link so reviewers can follow the provenance trail.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResultsRow {
    pub claim_text: String,
    pub trusty_uri: String,
    pub evidence_source: String,
    /// Verifier verdict label (`Supported` / `NotEnoughEvidence` /
    /// `Refuted` per SciFact-Open semantics).
    pub verdict: String,
    /// Optional 95% confidence interval as `(low, high)`.
    pub ci95: Option<(f64, f64)>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CitedFact {
    pub claim_text: String,
    pub citation_key: String,
    pub doi_or_url: String,
}

/// Internal section-tree built from a [`ScaffoldInput`] before rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct SectionTree {
    pub title: String,
    pub authors_markdown: String,
    pub methods_markdown: String,
    pub results_markdown: String,
    pub limitations_markdown: String,
    pub references_markdown: String,
    pub ai_disclosure_markdown: String,
    pub competing_interests_markdown: String,
}

#[derive(Debug, Error)]
pub enum ScaffoldError {
    #[error("scaffold input invalid: {0}")]
    Invalid(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaffold_input_serializes_with_kebab_case_field_names_off_by_default() {
        // serde default is snake_case for field names; we don't rename, so a
        // bare struct serializes with snake_case keys.
        let input = ScaffoldInput {
            title_hint: "t".into(),
            authors: vec![],
            results_rows: vec![],
            cited_facts: vec![],
            methods_summary: None,
            limitations: vec![],
            ai_disclosure_markdown: None,
            competing_interests: None,
            figures: vec![],
        };
        let j = serde_json::to_string(&input).unwrap();
        assert!(j.contains("\"title_hint\""));
        assert!(j.contains("\"results_rows\""));
    }
}
