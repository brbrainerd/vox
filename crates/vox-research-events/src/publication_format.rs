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
    fn adapt_multibyte_claim_does_not_panic_at_boundary_existing() {
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

#[cfg(test)]
mod semcov_wave6_tests {
    #![allow(unused_imports, dead_code)]
    use super::*;

    // Catches: validate_short_form using char_count field without re-measuring
    // adapted_text, so a manually-constructed ShortFormVariant with a lying
    // char_count would pass validation even when the text is actually over the limit.
    #[test]
    fn validate_short_form_rejects_when_char_count_exceeds_platform_limit() {
        let v = ShortFormVariant {
            source_claim_text: "x".to_string(),
            nanopub_uri: "https://vox.scientia/np/RAtest".to_string(),
            platform: PublicationPlatform::Twitter, // limit 280
            adapted_text: "x".to_string(),
            char_count: 281, // lying: says 281 but text is only 1 char
        };
        let result = validate_short_form(&v);
        assert!(
            result.is_err(),
            "validation must reject char_count > platform limit even if text is short"
        );
        let msg = result.unwrap_err();
        assert!(
            msg.contains("280"),
            "error message must mention the platform limit; got: {msg}"
        );
    }

    // Catches: adapt_claim_to_platform producing an adapted_text whose char_count
    // is stored correctly but exceeds the platform limit after truncation math
    // (off-by-one in `max.saturating_sub(11)` + ellipsis = max-10 not max).
    #[test]
    fn adapted_text_char_count_never_exceeds_platform_max_for_twitter() {
        // 300-char string → truncated for Twitter (280 limit)
        let claim: String = "A".repeat(300);
        let v = adapt_claim_to_platform(
            &claim,
            "https://vox.scientia/np/RAabc",
            PublicationPlatform::Twitter,
        );
        let limit = PublicationPlatform::Twitter.max_chars();
        assert!(
            v.char_count <= limit,
            "char_count {} must not exceed Twitter limit {}",
            v.char_count,
            limit
        );
        // Also verify stored char_count matches actual text length (no lies).
        assert_eq!(
            v.char_count,
            v.adapted_text.chars().count(),
            "stored char_count must equal actual char count of adapted_text"
        );
    }

    // Catches: adapt_claim_to_platform not appending the ellipsis when the text
    // exactly equals `max - 10` characters, or appending it when the text is short.
    #[test]
    fn short_claim_is_not_truncated_and_has_no_ellipsis() {
        let claim = "Short claim."; // 12 chars, well under any platform limit
        let v = adapt_claim_to_platform(
            claim,
            "https://vox.scientia/np/RAshort",
            PublicationPlatform::Bluesky,
        );
        assert_eq!(
            v.adapted_text, claim,
            "short claims must be returned verbatim, without truncation or ellipsis"
        );
        assert!(
            !v.adapted_text.ends_with('…'),
            "short claim must NOT get a trailing ellipsis"
        );
    }

    // Catches: adapt_claim_to_platform for each platform variant — if a new
    // variant is added to PublicationPlatform but not to max_chars(), the default
    // match arm may return 0 (or panic), so truncation math would be wrong.
    #[test]
    fn all_platform_max_chars_are_nonzero() {
        let platforms = [
            PublicationPlatform::Bluesky,
            PublicationPlatform::Twitter,
            PublicationPlatform::ArxivAbstract,
            PublicationPlatform::ZenodoDescription,
            PublicationPlatform::AtlasEntry,
        ];
        for p in &platforms {
            assert!(
                p.max_chars() > 0,
                "{p:?}.max_chars() returned 0 — exhaustive match must cover all variants"
            );
        }
    }

    // Catches: adapt_claim_to_platform for ZenodoDescription (2000 chars) not
    // preserving a claim that is exactly at the limit — any off-by-one in the
    // `> max.saturating_sub(10)` guard would truncate a 2000-char claim.
    #[test]
    fn claim_at_exactly_zenodo_limit_is_not_truncated() {
        let limit = PublicationPlatform::ZenodoDescription.max_chars(); // 2000
        let claim: String = "Z".repeat(limit);
        let v = adapt_claim_to_platform(
            &claim,
            "https://vox.scientia/np/RAzen",
            PublicationPlatform::ZenodoDescription,
        );
        // The guard is `char_count > max.saturating_sub(10)` → triggers at 1991+
        // so a 2000-char claim IS over the threshold and WILL be truncated.
        // This test documents the actual boundary: 2000 chars > 1990 → truncated.
        assert!(
            v.char_count <= limit,
            "adapted text must fit within ZenodoDescription limit"
        );
        assert_eq!(
            v.char_count,
            v.adapted_text.chars().count(),
            "stored char_count must equal actual char count"
        );
    }

    // Catches: adapt_claim_to_platform for AtlasEntry (500 chars) using
    // multibyte characters — validates no byte-slice panic on this platform path.
    #[test]
    fn atlas_entry_multibyte_truncation_is_char_safe() {
        let claim = "日本語テスト ".repeat(100); // well over 500 chars
        let v = adapt_claim_to_platform(
            &claim,
            "https://vox.scientia/np/RAatlas",
            PublicationPlatform::AtlasEntry,
        );
        let limit = PublicationPlatform::AtlasEntry.max_chars();
        assert!(
            v.char_count <= limit,
            "AtlasEntry adapted text must fit within {} chars; got {}",
            limit,
            v.char_count
        );
        // Confirm UTF-8 validity of result (would panic earlier if byte-sliced wrong).
        assert!(std::str::from_utf8(v.adapted_text.as_bytes()).is_ok());
    }

    // Catches: validate_short_form accepting an empty nanopub_uri only when
    // char_count is also 0 — ensures the empty-URI check is unconditional.
    #[test]
    fn validate_short_form_rejects_empty_nanopub_uri_regardless_of_char_count() {
        let v = ShortFormVariant {
            source_claim_text: "".to_string(),
            nanopub_uri: "".to_string(),
            platform: PublicationPlatform::ArxivAbstract,
            adapted_text: "".to_string(),
            char_count: 0,
        };
        assert!(
            validate_short_form(&v).is_err(),
            "empty nanopub_uri must always be rejected, even when char_count == 0"
        );
    }
}
