//! Contract-driven tunables for SCIENTIA discovery, novelty overlap, and preflight (Dynamics).
//!
//! SSOT: [`contracts/scientia/impact-readership-projection.seed.v1.yaml`](../../contracts/scientia/impact-readership-projection.seed.v1.yaml) (`heuristics` + impact sections).

use std::path::Path;

use anyhow::Context;
use serde::Deserialize;

/// Default repo-relative path for the combined dynamics / impact seed.
pub const DYNAMICS_SEED_REL_PATH: &str =
    "contracts/scientia/impact-readership-projection.seed.v1.yaml";

/// Flat runtime heuristics (defaults match pre-contract behavior).
#[derive(Debug, Clone)]
pub struct ScientiaHeuristics {
    pub rank_weight_strong: u32,
    pub rank_weight_supporting: u32,
    pub rank_weight_informational: u32,
    pub rank_bonus_strong_pair: u32,
    /// Max rank_score points subtracted when prior-art overlap is 1.0 (scaled linearly).
    pub rank_novelty_overlap_penalty_max: u32,
    pub novelty_blend_lexical: f64,
    pub novelty_blend_semantic: f64,
    pub novelty_moderate_threshold: f64,
    pub novelty_high_threshold: f64,
    pub preflight_novelty_high_lex_warn: f64,
    pub prior_art_results_per_source: u32,
    pub prior_art_token_min_len: usize,
    pub prior_art_abstract_max_chars: usize,
    pub impact_early_impact_months: u32,
    pub field_bucket_below_upper: u8,
    pub field_bucket_near_upper: u8,
    pub worthiness_epistemic_base: f64,
    pub worthiness_epistemic_r_coef: f64,
    pub worthiness_epistemic_abstract_boost: f64,
    pub worthiness_novelty_base: f64,
    pub worthiness_novelty_r_coef: f64,
    /// Below this `WorthinessInputs::claim_evidence_coverage`, Socrates contradiction shrink is skipped (coverage paradox).
    pub worthiness_contradiction_coverage_gate: f64,
    pub confidence_weight_strong: f64,
    pub confidence_weight_supporting: f64,
    pub confidence_weight_informational: f64,
    pub confidence_contradiction_high: f64,
    pub confidence_contradiction_low: f64,
    pub confidence_repro_sup_strong: f64,
    pub confidence_repro_sup_only: f64,
    pub significance_rank_divisor: f64,
    pub significance_title_len_divisor: f64,
    pub significance_title_boost_max: f64,
    pub evidence_completeness_max: u32,
}

impl Default for ScientiaHeuristics {
    fn default() -> Self {
        Self {
            rank_weight_strong: 10,
            rank_weight_supporting: 4,
            rank_weight_informational: 1,
            rank_bonus_strong_pair: 5,
            rank_novelty_overlap_penalty_max: 12,
            novelty_blend_lexical: 0.55,
            novelty_blend_semantic: 0.45,
            novelty_moderate_threshold: 0.45,
            novelty_high_threshold: 0.75,
            preflight_novelty_high_lex_warn: 0.85,
            prior_art_results_per_source: 8,
            prior_art_token_min_len: 2,
            prior_art_abstract_max_chars: 280,
            impact_early_impact_months: 24,
            field_bucket_below_upper: 40,
            field_bucket_near_upper: 75,
            worthiness_epistemic_base: 0.42,
            worthiness_epistemic_r_coef: 0.5,
            worthiness_epistemic_abstract_boost: 0.06,
            worthiness_novelty_base: 0.35,
            worthiness_novelty_r_coef: 0.38,
            worthiness_contradiction_coverage_gate: 0.3,
            confidence_weight_strong: 1.0,
            confidence_weight_supporting: 0.55,
            confidence_weight_informational: 0.25,
            confidence_contradiction_high: 0.85,
            confidence_contradiction_low: 0.1,
            confidence_repro_sup_strong: 0.35,
            confidence_repro_sup_only: 0.2,
            significance_rank_divisor: 40.0,
            significance_title_len_divisor: 200.0,
            significance_title_boost_max: 0.15,
            evidence_completeness_max: 9,
        }
    }
}

#[derive(Debug, Deserialize, Default)]
struct HeuristicsFile {
    #[serde(default)]
    horizons: HorizonsYaml,
    #[serde(default)]
    field_bucket_percentiles: FieldBucketYaml,
    #[serde(default)]
    heuristics: HeuristicsYaml,
}

#[derive(Debug, Deserialize, Default)]
struct HorizonsYaml {
    #[serde(default)]
    early_impact_months: Option<u32>,
}

#[derive(Debug, Deserialize, Default)]
struct FieldBucketYaml {
    #[serde(default)]
    below_upper: Option<u8>,
    #[serde(default)]
    near_upper: Option<u8>,
}

#[derive(Debug, Deserialize, Default)]
struct HeuristicsYaml {
    #[serde(default)]
    discovery: DiscoveryYaml,
    #[serde(default)]
    novelty_overlap: NoveltyOverlapYaml,
    #[serde(default)]
    preflight: PreflightYaml,
    #[serde(default)]
    prior_art: PriorArtYaml,
    #[serde(default)]
    worthiness_proxy: WorthinessProxyYaml,
    #[serde(default)]
    confidence: ConfidenceYaml,
    #[serde(default)]
    significance: SignificanceYaml,
    #[serde(default)]
    evidence_completeness: EvidenceCompletenessYaml,
}

#[derive(Debug, Deserialize, Default)]
struct DiscoveryYaml {
    #[serde(default)]
    rank_strength_weights: Option<RankWeightsYaml>,
    #[serde(default)]
    rank_bonus_strong_pair: Option<u32>,
    #[serde(default)]
    rank_novelty_overlap_penalty_max: Option<u32>,
}

#[derive(Debug, Deserialize, Default)]
struct RankWeightsYaml {
    #[serde(default)]
    strong: Option<u32>,
    #[serde(default)]
    supporting: Option<u32>,
    #[serde(default)]
    informational: Option<u32>,
}

#[derive(Debug, Deserialize, Default)]
struct NoveltyOverlapYaml {
    #[serde(default)]
    blend_lexical_weight: Option<f64>,
    #[serde(default)]
    blend_semantic_weight: Option<f64>,
    #[serde(default)]
    moderate_threshold: Option<f64>,
    #[serde(default)]
    high_threshold: Option<f64>,
}

#[derive(Debug, Deserialize, Default)]
struct PreflightYaml {
    #[serde(default)]
    novelty_high_lexical_overlap_warn: Option<f64>,
}

#[derive(Debug, Deserialize, Default)]
struct PriorArtYaml {
    #[serde(default)]
    results_per_source: Option<u32>,
    #[serde(default)]
    token_min_len: Option<usize>,
    #[serde(default)]
    abstract_max_chars: Option<usize>,
}

#[derive(Debug, Deserialize, Default)]
struct WorthinessProxyYaml {
    #[serde(default)]
    epistemic_base: Option<f64>,
    #[serde(default)]
    epistemic_readiness_coef: Option<f64>,
    #[serde(default)]
    epistemic_abstract_boost: Option<f64>,
    #[serde(default)]
    novelty_base: Option<f64>,
    #[serde(default)]
    novelty_readiness_coef: Option<f64>,
    #[serde(default)]
    contradiction_coverage_gate: Option<f64>,
}

#[derive(Debug, Deserialize, Default)]
struct ConfidenceYaml {
    #[serde(default)]
    weight_strong: Option<f64>,
    #[serde(default)]
    weight_supporting: Option<f64>,
    #[serde(default)]
    weight_informational: Option<f64>,
    #[serde(default)]
    contradiction_if_conflicts: Option<f64>,
    #[serde(default)]
    contradiction_if_clean: Option<f64>,
    #[serde(default)]
    reproducibility_with_strong: Option<f64>,
    #[serde(default)]
    reproducibility_without_strong: Option<f64>,
}

#[derive(Debug, Deserialize, Default)]
struct SignificanceYaml {
    #[serde(default)]
    rank_score_divisor: Option<f64>,
    #[serde(default)]
    title_len_divisor: Option<f64>,
    #[serde(default)]
    title_boost_max: Option<f64>,
}

#[derive(Debug, Deserialize, Default)]
struct EvidenceCompletenessYaml {
    #[serde(default)]
    max_checklist_items: Option<u32>,
}

impl ScientiaHeuristics {
    /// Load from repo seed YAML; on parse/missing file errors, logs (tracing) and returns defaults.
    #[must_use]
    pub fn load_from_repo_root(repo_root: &Path) -> Self {
        Self::try_load_from_repo_root(repo_root).unwrap_or_default()
    }

    pub fn try_load_from_repo_root(repo_root: &Path) -> anyhow::Result<Self> {
        let path = repo_root.join(DYNAMICS_SEED_REL_PATH);
        if !path.is_file() {
            tracing::debug!(
                target: "vox.publisher.scientia_heuristics",
                path = %path.display(),
                "dynamics seed missing; using defaults"
            );
            return Ok(Self::default());
        }
        let raw = vox_bounded_fs::read_utf8_path_capped(&path)
            .with_context(|| format!("read {}", path.display()))?;
        let f: HeuristicsFile =
            serde_yaml::from_str(&raw).with_context(|| format!("parse {}", path.display()))?;
        Ok(Self::from_yaml(f))
    }

    fn from_yaml(f: HeuristicsFile) -> Self {
        let mut h = Self::default();
        if let Some(m) = f.horizons.early_impact_months {
            h.impact_early_impact_months = m;
        }
        if let Some(v) = f.field_bucket_percentiles.below_upper {
            h.field_bucket_below_upper = v;
        }
        if let Some(v) = f.field_bucket_percentiles.near_upper {
            h.field_bucket_near_upper = v;
        }
        let y = &f.heuristics;
        if let Some(ref w) = y.discovery.rank_strength_weights {
            if let Some(v) = w.strong {
                h.rank_weight_strong = v;
            }
            if let Some(v) = w.supporting {
                h.rank_weight_supporting = v;
            }
            if let Some(v) = w.informational {
                h.rank_weight_informational = v;
            }
        }
        if let Some(v) = y.discovery.rank_bonus_strong_pair {
            h.rank_bonus_strong_pair = v;
        }
        if let Some(v) = y.discovery.rank_novelty_overlap_penalty_max {
            h.rank_novelty_overlap_penalty_max = v.min(100);
        }
        let no = &y.novelty_overlap;
        if let Some(v) = no.blend_lexical_weight {
            h.novelty_blend_lexical = v;
        }
        if let Some(v) = no.blend_semantic_weight {
            h.novelty_blend_semantic = v;
        }
        if let Some(v) = no.moderate_threshold {
            h.novelty_moderate_threshold = v;
        }
        if let Some(v) = no.high_threshold {
            h.novelty_high_threshold = v;
        }
        if let Some(v) = y.preflight.novelty_high_lexical_overlap_warn {
            h.preflight_novelty_high_lex_warn = v;
        }
        let pa = &y.prior_art;
        if let Some(v) = pa.results_per_source {
            h.prior_art_results_per_source = v.clamp(1, 50);
        }
        if let Some(v) = pa.token_min_len {
            h.prior_art_token_min_len = v;
        }
        if let Some(v) = pa.abstract_max_chars {
            h.prior_art_abstract_max_chars = v;
        }
        let wp = &y.worthiness_proxy;
        if let Some(v) = wp.epistemic_base {
            h.worthiness_epistemic_base = v;
        }
        if let Some(v) = wp.epistemic_readiness_coef {
            h.worthiness_epistemic_r_coef = v;
        }
        if let Some(v) = wp.epistemic_abstract_boost {
            h.worthiness_epistemic_abstract_boost = v;
        }
        if let Some(v) = wp.novelty_base {
            h.worthiness_novelty_base = v;
        }
        if let Some(v) = wp.novelty_readiness_coef {
            h.worthiness_novelty_r_coef = v;
        }
        if let Some(v) = wp.contradiction_coverage_gate {
            h.worthiness_contradiction_coverage_gate = v.clamp(0.0, 1.0);
        }
        let c = &y.confidence;
        if let Some(v) = c.weight_strong {
            h.confidence_weight_strong = v;
        }
        if let Some(v) = c.weight_supporting {
            h.confidence_weight_supporting = v;
        }
        if let Some(v) = c.weight_informational {
            h.confidence_weight_informational = v;
        }
        if let Some(v) = c.contradiction_if_conflicts {
            h.confidence_contradiction_high = v;
        }
        if let Some(v) = c.contradiction_if_clean {
            h.confidence_contradiction_low = v;
        }
        if let Some(v) = c.reproducibility_with_strong {
            h.confidence_repro_sup_strong = v;
        }
        if let Some(v) = c.reproducibility_without_strong {
            h.confidence_repro_sup_only = v;
        }
        let s = &y.significance;
        if let Some(v) = s.rank_score_divisor {
            h.significance_rank_divisor = v.max(1e-6);
        }
        if let Some(v) = s.title_len_divisor {
            h.significance_title_len_divisor = v.max(1.0);
        }
        if let Some(v) = s.title_boost_max {
            h.significance_title_boost_max = v;
        }
        if let Some(v) = y.evidence_completeness.max_checklist_items {
            h.evidence_completeness_max = v.max(1);
        }
        h
    }
}
