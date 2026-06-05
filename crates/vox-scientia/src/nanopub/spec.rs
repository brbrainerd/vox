//! Real `nanopub`-crate wrapper: BUILD a nanopublication from an RDF/TriG assertion,
//! RSA-SIGN it with an ORCID profile, expose the signed TriG + Trusty URI, and
//! VALIDATE it OFFLINE (trusty-hash + RSA-signature check, no network).
//!
//! This is the spike that proves the upstream `nanopub` crate (MIT) can do the full
//! offline build → sign → validate round-trip on Windows. It deliberately performs
//! NO network publishing of any kind.
//!
//! Discovered upstream API (nanopub 0.2.0):
//! - `Nanopub::new(rdf: impl RdfSource) -> Result<Nanopub, NpError>` — parse a TriG nanopub.
//! - `ProfileBuilder::new(private_key).with_orcid(..).with_name(..).build() -> Result<NpProfile, NpError>`.
//! - `Nanopub::sign(profile: &NpProfile) -> Result<Nanopub, NpError>` — RSA-sign + add trusty URI.
//! - `Nanopub::rdf() -> Result<String, NpError>` — serialized (signed) TriG.
//! - `nanopub.info.uri.as_str()` — the trusty URI (`...RA<base64>`).
//! - `Nanopub::check() -> Result<Nanopub, NpError>` — OFFLINE trusty + signature validation.
//! - `nanopub::profile::gen_keys() -> Result<(priv_pem_b64, pub_pem_b64), NpError>` — RSA keypair.

use nanopub::{Nanopub, ProfileBuilder};

/// A signed nanopublication document: the serialized TriG and its Trusty URI.
pub struct SignedNanopubDoc {
    /// Complete signed TriG serialization.
    pub trig: String,
    /// Trusty URI of the signed nanopub (contains the `RA<hash>` artifact code).
    pub trusty_uri: String,
}

/// A signing profile: ORCID identity + display name + an RSA private key (PEM or
/// bare-base64 PKCS#8, as accepted by `nanopub`'s key normalizer).
pub struct NanopubProfile {
    /// ORCID URL, e.g. `https://orcid.org/0000-0002-1267-0234`.
    pub orcid: String,
    /// Human-readable signer name.
    pub name: String,
    /// RSA private key in PEM (PKCS#8 or PKCS#1) form.
    pub rsa_private_key_pem: String,
}

/// Errors from the build/sign/validate round-trip.
#[derive(Debug, thiserror::Error)]
pub enum NanopubSpecError {
    #[error("nanopub error: {0}")]
    Nanopub(#[from] nanopub::error::NpError),
}

/// Build a full nanopublication TriG around `assertion_ttl` (the inner assertion
/// triples, Turtle body without graph braces), attributing it to
/// `attributed_to_orcid` and stamping `generated_at_unix`, then RSA-sign it with
/// `profile`. Returns the signed TriG and Trusty URI. No network access.
pub fn build_and_sign(
    assertion_ttl: &str,
    attributed_to_orcid: &str,
    generated_at_unix: i64,
    profile: &NanopubProfile,
) -> Result<SignedNanopubDoc, NanopubSpecError> {
    let trig = assemble_unsigned_trig(assertion_ttl, attributed_to_orcid, generated_at_unix);

    let np_profile = ProfileBuilder::new(profile.rsa_private_key_pem.clone())
        .with_orcid(profile.orcid.clone())
        .with_name(profile.name.clone())
        .build()?;

    let signed = Nanopub::new(trig.as_str())?.sign(&np_profile)?;

    Ok(SignedNanopubDoc {
        trusty_uri: signed.info.uri.as_str().to_string(),
        trig: signed.rdf()?,
    })
}

/// Validate a signed nanopub TriG OFFLINE: re-derives the trusty hash and verifies
/// the embedded RSA signature against the embedded public key. No network access.
pub fn validate_offline(trig: &str) -> Result<(), NanopubSpecError> {
    Nanopub::new(trig)?.check()?;
    Ok(())
}

/// Assemble an unsigned nanopub TriG using the upstream temp namespace. The lib
/// rewrites this temp namespace to the trusty URI during signing.
fn assemble_unsigned_trig(
    assertion_ttl: &str,
    attributed_to_orcid: &str,
    generated_at_unix: i64,
) -> String {
    let iso_created = chrono::DateTime::from_timestamp(generated_at_unix, 0)
        .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).expect("epoch is a valid time"))
        .to_rfc3339();

    format!(
        "@prefix : <http://purl.org/nanopub/temp/mynanopub#> .\n\
         @prefix xsd: <http://www.w3.org/2001/XMLSchema#> .\n\
         @prefix dc: <http://purl.org/dc/terms/> .\n\
         @prefix pav: <http://purl.org/pav/> .\n\
         @prefix prov: <http://www.w3.org/ns/prov#> .\n\
         @prefix np: <http://www.nanopub.org/nschema#> .\n\
         @prefix npx: <http://purl.org/nanopub/x/> .\n\
         @prefix scientia: <https://vox.scientia/vocab#> .\n\n\
         :Head {{\n  \
           : np:hasAssertion :assertion ;\n    \
             np:hasProvenance :provenance ;\n    \
             np:hasPublicationInfo :pubinfo ;\n    \
             a np:Nanopublication .\n\
         }}\n\n\
         :assertion {{\n  {assertion_ttl}\n}}\n\n\
         :provenance {{\n  \
           :assertion prov:wasAttributedTo <{orcid}> ;\n    \
             prov:generatedAtTime \"{created}\"^^xsd:dateTime .\n\
         }}\n\n\
         :pubinfo {{\n  \
           : dc:created \"{created}\"^^xsd:dateTime ;\n    \
             dc:creator \"vox-scientia\" .\n\
         }}\n",
        assertion_ttl = assertion_ttl,
        orcid = attributed_to_orcid,
        created = iso_created,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Produce a throwaway RSA private-key PEM via the upstream keygen.
    /// `gen_keys()` returns normalized base64 PKCS#8; `ProfileBuilder` accepts it.
    fn throwaway_rsa_private_key() -> String {
        let (private_key, _public_key) =
            nanopub::profile::gen_keys().expect("RSA keygen should succeed");
        private_key
    }

    #[test]
    fn build_sign_validate_round_trip() {
        let profile = NanopubProfile {
            orcid: "https://orcid.org/0000-0002-1267-0234".to_string(),
            name: "Vox Scientia Test".to_string(),
            rsa_private_key_pem: throwaway_rsa_private_key(),
        };

        let assertion = "scientia:claim1 scientia:text \"mosquitoes transmit malaria\" .";

        let signed = build_and_sign(
            assertion,
            "https://orcid.org/0000-0002-1267-0234",
            1_700_000_000,
            &profile,
        )
        .expect("build_and_sign should succeed");

        // Trusty URI must be non-empty and carry the RA artifact code.
        assert!(
            !signed.trusty_uri.is_empty(),
            "trusty URI should be non-empty"
        );
        assert!(
            signed.trusty_uri.contains("RA"),
            "trusty URI should contain the RA artifact code, got: {}",
            signed.trusty_uri
        );

        // Signed TriG should be non-empty and carry a signature triple.
        assert!(!signed.trig.is_empty(), "signed TriG should be non-empty");

        // Offline validation of the signed TriG must pass (trusty + RSA signature).
        validate_offline(&signed.trig).expect("offline validation should pass");
    }

    #[test]
    fn tampered_trig_fails_offline_validation() {
        let profile = NanopubProfile {
            orcid: "https://orcid.org/0000-0002-1267-0234".to_string(),
            name: "Vox Scientia Test".to_string(),
            rsa_private_key_pem: throwaway_rsa_private_key(),
        };

        let signed = build_and_sign(
            "scientia:claim1 scientia:text \"original\" .",
            "https://orcid.org/0000-0002-1267-0234",
            1_700_000_000,
            &profile,
        )
        .expect("build_and_sign should succeed");

        // Corrupt the assertion text; the trusty hash / signature must no longer match.
        let tampered = signed.trig.replace("original", "tampered");
        assert_ne!(
            tampered, signed.trig,
            "replacement should have changed the TriG"
        );
        assert!(
            validate_offline(&tampered).is_err(),
            "tampered TriG must fail offline validation"
        );
    }
}
