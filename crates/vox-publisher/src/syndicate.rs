//! Approval-gated short-form syndication seam (SCIENTIA SSOT §7).
//!
//! `adapt_claim_to_platform` / [`ShortFormVariant`] previously had zero
//! non-test callers. This module is the single seam that lifts an atomic claim
//! into a platform-constrained short-form variant — and it refuses to do so
//! without a valid [`ApprovalToken`] bound to the claim's content digest.
//!
//! The token can only be minted via
//! [`vox_scientia::review::mint_from_decision`] from a persisted, human
//! `"approved"` decision row, so there is no way to syndicate un-reviewed text.

use anyhow::{Result, bail};
use sha3::{Digest, Sha3_256};
use vox_research_events::publication_format::{
    PublicationPlatform, ShortFormVariant, adapt_claim_to_platform, validate_short_form,
};
use vox_scientia::review::ApprovalToken;

/// SHA3-256 hex digest of the claim text. Matches the
/// `content_sha3_256` convention the P2 review path binds a token to.
fn content_digest(claim: &str) -> String {
    let mut hasher = Sha3_256::new();
    hasher.update(claim.as_bytes());
    let out = hasher.finalize();
    let mut hex = String::with_capacity(out.len() * 2);
    for b in out {
        use std::fmt::Write;
        let _ = write!(hex, "{b:02x}");
    }
    hex
}

/// Lift an approved atomic `claim` into a platform-constrained
/// [`ShortFormVariant`], preserving its Trusty/nanopub URI.
///
/// Refuses (returns `Err`) unless `token`'s `bound_digest` equals the SHA3-256
/// digest of `claim` — i.e. the human approval is bound to *this exact* content.
/// An edit to the claim invalidates the prior approval (the digest changes),
/// matching the nanopub-build replay protection.
///
/// On success the returned variant has passed [`validate_short_form`] (fits the
/// platform char limit and carries a non-empty nanopub URI).
pub fn syndicate(
    claim: &str,
    nanopub_uri: &str,
    channel: PublicationPlatform,
    token: &ApprovalToken,
) -> Result<ShortFormVariant> {
    let expected = content_digest(claim);
    if token.bound_digest() != expected {
        bail!(
            "ApprovalToken digest does not match claim content: token is bound to a \
             different (or stale) artifact version; refusing to syndicate"
        );
    }

    let variant = adapt_claim_to_platform(claim, nanopub_uri, channel);
    validate_short_form(&variant)
        .map_err(|e| anyhow::anyhow!("short-form validation failed: {e}"))?;

    // The Trusty/nanopub URI must survive adaptation byte-for-byte.
    debug_assert_eq!(variant.nanopub_uri, nanopub_uri);
    Ok(variant)
}

// Token-minting tests require a real `ApprovalToken`, whose ONLY public
// construction path is `mint_from_decision` over a `vox_db::store::ReviewDecisionRow`.
// vox-db is feature-gated (`scholarly-external-jobs`), so these tests are too.
// The "no token => cannot call" guarantee is structural: `syndicate` requires
// `&ApprovalToken` by signature, and `ApprovalToken` has no public constructor.
#[cfg(all(test, feature = "scholarly-external-jobs"))]
mod tests {
    use super::*;
    use vox_db::store::ReviewDecisionRow;
    use vox_scientia::review::mint_from_decision;

    fn mint(bound_digest: &str) -> ApprovalToken {
        let row = ReviewDecisionRow {
            claim_id: 1,
            publication_id: "pub-1".into(),
            bound_digest: bound_digest.into(),
            decision: "approved".into(),
            actor: "tester".into(),
            reason: None,
            model_fingerprints_json: None,
            decided_at_ms: 1_000,
        };
        mint_from_decision(&row).expect("approved row must mint a token")
    }

    #[test]
    fn syndicate_refuses_token_bound_to_different_content() {
        let claim = "p95 latency rose by 15ms";
        let wrong = super::content_digest("a totally different claim");
        let token = mint(&wrong);
        let res = syndicate(
            claim,
            "https://vox.scientia/np/RAabc123",
            PublicationPlatform::Twitter,
            &token,
        );
        assert!(res.is_err(), "must refuse a token bound to other content");
    }

    #[test]
    fn syndicate_preserves_trusty_uri_byte_identical() {
        let claim = "café résumé 日本語 p95 latency rose by 15ms";
        let uri = "https://vox.scientia/np/RAabc123def456";
        let token = mint(&super::content_digest(claim));
        let v = syndicate(claim, uri, PublicationPlatform::Twitter, &token).unwrap();
        assert_eq!(v.nanopub_uri, uri);
        assert_eq!(v.nanopub_uri.as_bytes(), uri.as_bytes());
        assert!(v.char_count <= PublicationPlatform::Twitter.max_chars());
    }
}
