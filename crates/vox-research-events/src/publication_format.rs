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
    Twitter, // 280 char limit
    ArxivAbstract,
    ZenodoDescription,
    AtlasEntry,
}

impl PublicationPlatform {
    /// Maximum character count for this platform.
    pub fn max_chars(&self) -> usize {
        match self {
            Self::Bluesky => 300,
            Self::Twitter => 280,
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
    // Char-safe: count by Unicode scalar values, never slice on a byte index,
    // which would panic on a multibyte boundary (e.g. "café résumé 日本語").
    let char_count = claim_text.chars().count();
    let adapted = if char_count > max.saturating_sub(10) {
        // Reserve room for the single-char "…" ellipsis so the result fits.
        let take = max.saturating_sub(11);
        let head: String = claim_text.chars().take(take).collect();
        format!("{head}…")
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
    fn bluesky_and_twitter_both_supported() {
        // Bluesky remains the prioritized short-form platform (300 chars);
        // Twitter is now also supported as a 280-char variant.
        let platforms = [
            PublicationPlatform::Bluesky,
            PublicationPlatform::Twitter,
            PublicationPlatform::ArxivAbstract,
            PublicationPlatform::ZenodoDescription,
            PublicationPlatform::AtlasEntry,
        ];
        assert!(
            platforms.iter().any(|p| p == &PublicationPlatform::Bluesky),
            "Bluesky must be a supported platform"
        );
        assert!(
            platforms.iter().any(|p| p == &PublicationPlatform::Twitter),
            "Twitter must be a supported platform"
        );
        assert_eq!(PublicationPlatform::Twitter.max_chars(), 280);
    }

    #[test]
    fn adapt_multibyte_claim_does_not_panic_at_boundary() {
        // A multibyte claim padded past the Twitter limit; truncating with a
        // byte slice on a char budget would panic on the multibyte boundary.
        let base = "café résumé 日本語… ";
        let claim = base.repeat(40); // far exceeds 280 chars
        let v = adapt_claim_to_platform(
            &claim,
            "https://vox.scientia/np/RAabc123",
            PublicationPlatform::Twitter,
        );
        // Char-correct: must fit the platform limit and be valid UTF-8.
        assert!(v.char_count <= PublicationPlatform::Twitter.max_chars());
        assert_eq!(v.char_count, v.adapted_text.chars().count());
        assert!(v.adapted_text.ends_with('…'));
    }
}
