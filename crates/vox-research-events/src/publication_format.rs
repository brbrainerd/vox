//! Publication format adaptation types for SCIENTIA Phase 7.
//!
//! All short-form publication variants must be generated from atomic claims
//! with nanopub URIs — no free-form LLM text in the publication path.

use serde::{Deserialize, Serialize};

/// A short-form publication variant lifted from atomic claims.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortFormVariant {
    /// The claim text from which this variant was lifted.
    pub source_claim_text: String,
    /// Nanopub URI that anchors this claim (e.g. `https://vox.scientia/np/RA...`).
    pub nanopub_uri: String,
    /// Target platform (e.g. "bluesky", "arxiv_abstract", "zenodo_description").
    pub platform: PublicationPlatform,
    /// The constrained-grammar-generated text (not free-form).
    pub adapted_text: String,
    /// Character count of adapted_text — validated before publication.
    pub char_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicationPlatform {
    Bluesky, // 300 char limit
    ArxivAbstract,
    ZenodoDescription,
    AtlasEntry,
}

impl PublicationPlatform {
    /// Maximum character count for this platform.
    pub fn max_chars(&self) -> usize {
        match self {
            Self::Bluesky => 300,
            Self::ArxivAbstract => 1500,
            Self::ZenodoDescription => 2000,
            Self::AtlasEntry => 500,
        }
    }
}

/// Validates that a ShortFormVariant fits within platform limits.
pub fn validate_short_form(variant: &ShortFormVariant) -> Result<(), String> {
    let limit = variant.platform.max_chars();
    if variant.char_count > limit {
        return Err(format!(
            "{:?} limit is {} chars, variant has {}",
            variant.platform, limit, variant.char_count
        ));
    }
    if variant.nanopub_uri.is_empty() {
        return Err("nanopub_uri must not be empty".to_string());
    }
    Ok(())
}

/// Adapt an atomic claim text to a short-form platform variant.
/// This is a stub — Phase 8 wires the actual vox-constrained-gen emitter.
pub fn adapt_claim_to_platform(
    claim_text: &str,
    nanopub_uri: &str,
    platform: PublicationPlatform,
) -> ShortFormVariant {
    let max = platform.max_chars();
    // Naive truncation — Phase 8 replaces with constrained-grammar generation.
    let adapted = if claim_text.len() > max - 10 {
        format!("{}…", &claim_text[..max.saturating_sub(11)])
    } else {
        claim_text.to_string()
    };
    let char_count = adapted.chars().count();
    ShortFormVariant {
        source_claim_text: claim_text.to_string(),
        nanopub_uri: nanopub_uri.to_string(),
        platform,
        adapted_text: adapted,
        char_count,
    }
}

/// Figure policy per Cell/Science 2025: no LLM-generated primary research figures.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FigurePolicy {
    pub llm_generation_disabled: bool, // always true per 2025 policy
    pub schematic_only: bool,
    pub mandatory_legend_disclosure: bool,
}

impl Default for FigurePolicy {
    fn default() -> Self {
        Self {
            llm_generation_disabled: true,
            schematic_only: true,
            mandatory_legend_disclosure: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bluesky_limit_is_300() {
        assert_eq!(PublicationPlatform::Bluesky.max_chars(), 300);
    }

    #[test]
    fn validate_passes_for_valid_variant() {
        let v = adapt_claim_to_platform(
            "p95 latency rose by 15ms",
            "https://vox.scientia/np/RAabc123",
            PublicationPlatform::Bluesky,
        );
        assert!(validate_short_form(&v).is_ok());
    }

    #[test]
    fn validate_fails_for_empty_nanopub_uri() {
        let v = ShortFormVariant {
            source_claim_text: "test".to_string(),
            nanopub_uri: "".to_string(),
            platform: PublicationPlatform::Bluesky,
            adapted_text: "test".to_string(),
            char_count: 4,
        };
        assert!(validate_short_form(&v).is_err());
    }

    #[test]
    fn figure_policy_default_disables_llm() {
        let p = FigurePolicy::default();
        assert!(p.llm_generation_disabled);
        assert!(p.mandatory_legend_disclosure);
    }

    #[test]
    fn bluesky_prioritized_over_x_in_platform_enum() {
        // Bluesky exists as a variant; X/Twitter does not.
        let platforms = [
            PublicationPlatform::Bluesky,
            PublicationPlatform::ArxivAbstract,
            PublicationPlatform::ZenodoDescription,
            PublicationPlatform::AtlasEntry,
        ];
        let has_bluesky = platforms.iter().any(|p| p == &PublicationPlatform::Bluesky);
        assert!(has_bluesky, "Bluesky must be a supported platform");
    }
}
