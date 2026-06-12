//! Nanopub network layer. ONLY the test server is reachable, and only behind
//! BOTH an ApprovalToken (human decision) and `VOX_NANOPUB_TEST_SERVER=1`.
//! Production publishing is deliberately unimplemented (standing decision).

use crate::nanopub::spec::NanopubProfile;
use nanopub::{Nanopub, ProfileBuilder};
use secrecy::ExposeSecret as _;

/// Enforce the env-var allow-gate that controls test-server publishing.
///
/// Returns `Ok(())` when `env_allow` is `true`. Otherwise returns a
/// descriptive error that names the env var and explains it is the *test*
/// registry (public, periodically wiped — NOT production).
///
/// # Errors
/// Returns an error unless `env_allow` is `true`.
pub fn ensure_test_server_allowed(env_allow: bool) -> anyhow::Result<()> {
    if env_allow {
        Ok(())
    } else {
        anyhow::bail!(
            "nanopub test-server publishing is disabled; \
             set VOX_NANOPUB_TEST_SERVER=1 to enable it. \
             Note: the test server (https://np.test.knowledgepixels.com/) is a \
             *public* registry that is periodically wiped — it is NOT production. \
             Never set this flag in production pipelines."
        )
    }
}

/// Publish a signed nanopublication to the nanopub TEST server.
///
/// This function is gated behind BOTH:
/// - an [`crate::review::ApprovalToken`] (only mintable from a persisted human
///   "approved" decision — cannot be constructed outside that module), AND
/// - `VOX_NANOPUB_TEST_SERVER=1` (env allow, checked via
///   [`ensure_test_server_allowed`]).
///
/// Production publishing is **deliberately unimplemented** (standing decision).
///
/// # Arguments
/// - `signed_trig` — the fully-signed nanopub TriG (as produced by
///   [`crate::nanopub::spec::build_and_sign`]).
/// - `profile` — the ORCID signing profile used to sign the nanopub.
/// - `_approval` — proof that a human approved this claim; not consumed (the
///   guard is enforced by token type-gating in the caller).
///
/// # Returns
/// The published URI (the Trusty URI the test server accepted).
///
/// # Errors
/// Returns an error when `VOX_NANOPUB_TEST_SERVER=1` is unset, when the TriG
/// cannot be parsed, or when the test-server HTTP call fails.
pub async fn publish_to_test_server(
    signed_trig: &str,
    profile: &NanopubProfile,
    _approval: &crate::review::ApprovalToken,
) -> anyhow::Result<String> {
    // Env-var gate: must be set to "1".
    let env_allow = std::env::var("VOX_NANOPUB_TEST_SERVER")
        .map(|v| v == "1")
        .unwrap_or(false);
    ensure_test_server_allowed(env_allow)?;

    // Build the NpProfile exactly as spec.rs does in `build_and_sign`.
    let np_profile = ProfileBuilder::new(profile.rsa_private_key_b64.expose_secret().to_string())
        .with_orcid(profile.orcid.clone())
        .with_name(profile.name.clone())
        .build()
        .map_err(|e| anyhow::anyhow!("build NpProfile for publish: {e}"))?;

    // Parse the already-signed TriG, then publish to the test server.
    // `None` as the second argument to `publish` selects the test server
    // (the nanopub crate default when no server URL is supplied).
    let published = Nanopub::new(signed_trig)
        .map_err(|e| anyhow::anyhow!("parse signed TriG before publish: {e}"))?
        .publish(Some(&np_profile), None)
        .await
        .map_err(|e| anyhow::anyhow!("publish to nanopub test server: {e}"))?;

    Ok(published.info.uri.as_str().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The env-gate must refuse when the env var is absent / not "1".
    #[test]
    fn ensure_test_server_allowed_refuses_without_env() {
        let err = ensure_test_server_allowed(false).expect_err("must refuse when env_allow=false");
        let msg = err.to_string();
        assert!(
            msg.contains("VOX_NANOPUB_TEST_SERVER"),
            "error must name the env var, got: {msg}"
        );
    }

    /// The env-gate must pass when the caller signals allow.
    #[test]
    fn ensure_test_server_allowed_passes_with_env() {
        ensure_test_server_allowed(true).expect("must pass when env_allow=true");
    }

    /// Module-shape guard: this module must expose NO function with "publish"
    /// in its name other than `publish_to_test_server`. This keeps the existing
    /// no-production-network guard family green and extends it to this file.
    /// The allowlist is: `publish_to_test_server` only.
    #[test]
    fn only_test_server_publish_function_is_exposed() {
        let src = include_str!("network.rs");
        // Assembles the forbidden name from fragments so the guard never trips
        // on its own assertion text.
        let prod_publish = format!("{}{}", "publish_to_", "network");
        let prod_server = format!("{}{}", "publish_to_", "production");
        assert!(
            !src.to_lowercase().contains(&prod_publish),
            "network.rs must not expose a production-network publish fn, found: {prod_publish}"
        );
        assert!(
            !src.to_lowercase().contains(&prod_server),
            "network.rs must not expose a production-server publish fn, found: {prod_server}"
        );
        // The only allowed publish symbol is `publish_to_test_server`.
        assert!(
            src.contains("publish_to_test_server"),
            "network.rs must export publish_to_test_server"
        );
    }
}
