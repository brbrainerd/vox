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
use secrecy::{ExposeSecret, SecretString};

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
    /// RSA private key as base64-encoded PKCS#8 (the normalized form `nanopub`
    /// emits from [`gen_keys`] and accepts in `ProfileBuilder`). Wrapped in
    /// [`SecretString`] so the signing material can never be `Debug`/`Display`-
    /// logged and is zeroized on drop; expose it only at the `ProfileBuilder` call.
    pub rsa_private_key_b64: SecretString,
}

/// Generate a fresh RSA keypair for nanopub signing, returning
/// `(private_b64, public_b64)` as normalized base64 PKCS#8 strings.
///
/// Thin re-export of the upstream `nanopub` keygen so consumers (e.g. the
/// identity resolver in `vox-cli`) need not depend on the `nanopub` crate
/// directly.
///
/// # Errors
/// Propagates [`nanopub::error::NpError`] if key generation fails.
pub fn gen_keys() -> Result<(String, String), nanopub::error::NpError> {
    nanopub::profile::gen_keys()
}

/// Errors from the build/sign/validate round-trip.
#[derive(Debug, thiserror::Error)]
pub enum NanopubSpecError {
    #[error("nanopub error: {0}")]
    Nanopub(#[from] nanopub::error::NpError),
    #[error(
        "profile ORCID `{profile_orcid}` does not match attributed ORCID `{attributed_to_orcid}`"
    )]
    OrcidMismatch {
        profile_orcid: String,
        attributed_to_orcid: String,
    },
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
    // The provenance graph attributes the assertion to `attributed_to_orcid`,
    // but the RSA signature is produced from `profile`. If these diverge the
    // artifact claims an authorship it did not sign — reject before building.
    if profile.orcid != attributed_to_orcid {
        return Err(NanopubSpecError::OrcidMismatch {
            profile_orcid: profile.orcid.clone(),
            attributed_to_orcid: attributed_to_orcid.to_string(),
        });
    }

    let trig = assemble_unsigned_trig(assertion_ttl, attributed_to_orcid, generated_at_unix);

    let np_profile = ProfileBuilder::new(profile.rsa_private_key_b64.expose_secret().to_string())
        .with_orcid(profile.orcid.clone())
        .with_name(profile.name.clone())
        .build()?;

    let signed = Nanopub::new(trig.as_str())?.sign(&np_profile)?;

    Ok(SignedNanopubDoc {
        trusty_uri: signed.info.uri.as_str().to_string(),
        trig: signed.rdf()?,
    })
}

/// Build the enriched assertion-graph Turtle for a single verified claim, carrying
/// its FULL structure (not just text) under the `scientia:` vocabulary on the
/// `scientia:claim1` subject. The result is the inner assertion body (Turtle
/// without graph braces), suitable to pass as `assertion_ttl` to [`build_and_sign`].
///
/// Reuses the `scientia:` and `xsd:` prefixes that [`build_and_sign`]'s TriG
/// template already declares, so the body is injected directly into the assertion
/// graph. Emits, at minimum:
/// - `scientia:text` (with `"` and `\` escaped in the literal),
/// - when `tuple` is `Some`: `scientia:variableA` / `scientia:relation` / `scientia:variableB`,
/// - `scientia:verifiability`,
/// - `scientia:confidence "<conf>"^^xsd:decimal`,
/// - `scientia:noveltyVerdict`,
/// - one `scientia:closestPriorArt <uri>` per prior-art URI (IRIs in angle brackets).
pub fn assertion_ttl_for_claim(
    text: &str,
    tuple: Option<(&str, &str, &str)>,
    verifiability: &str,
    confidence: f64,
    novelty: &str,
    prior_art_uris: &[&str],
) -> String {
    // Predicate-object lines, joined with " ;\n    " and terminated with " .".
    let mut lines: Vec<String> = Vec::new();

    lines.push(format!("scientia:text \"{}\"", escape_turtle_string(text)));

    if let Some((variable_a, relation, variable_b)) = tuple {
        lines.push(format!(
            "scientia:variableA \"{}\"",
            escape_turtle_string(variable_a)
        ));
        lines.push(format!(
            "scientia:relation \"{}\"",
            escape_turtle_string(relation)
        ));
        lines.push(format!(
            "scientia:variableB \"{}\"",
            escape_turtle_string(variable_b)
        ));
    }

    lines.push(format!(
        "scientia:verifiability \"{}\"",
        escape_turtle_string(verifiability)
    ));

    // xsd:decimal lexical form: a plain decimal with a fractional part.
    lines.push(format!(
        "scientia:confidence \"{}\"^^xsd:decimal",
        format_decimal(confidence)
    ));

    lines.push(format!(
        "scientia:noveltyVerdict \"{}\"",
        escape_turtle_string(novelty)
    ));

    for uri in prior_art_uris {
        // IRIs go in angle brackets; escape `>` / `<` / whitespace conservatively.
        lines.push(format!("scientia:closestPriorArt <{}>", escape_iri(uri)));
    }

    format!("scientia:claim1\n    {} .", lines.join(" ;\n    "))
}

/// Escape a string for use inside a Turtle double-quoted literal (`"..."`).
/// Handles backslash, double-quote, and the control characters that the oxttl
/// parser rejects unescaped in a single-line literal.
fn escape_turtle_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out
}

/// Escape a URI for use inside a Turtle IRI reference (`<...>`). Per the Turtle
/// grammar, the delimiters `<`, `>`, `"`, `{`, `}`, `|`, `^`, backtick, backslash,
/// and whitespace are not allowed bare in an IRIREF and must be percent- or
/// backslash-handled. We percent-encode the disallowed set so the IRI stays valid.
fn escape_iri(uri: &str) -> String {
    let mut out = String::with_capacity(uri.len());
    for ch in uri.chars() {
        match ch {
            '<' => out.push_str("%3C"),
            '>' => out.push_str("%3E"),
            '"' => out.push_str("%22"),
            '{' => out.push_str("%7B"),
            '}' => out.push_str("%7D"),
            '|' => out.push_str("%7C"),
            '^' => out.push_str("%5E"),
            '`' => out.push_str("%60"),
            '\\' => out.push_str("%5C"),
            c if c.is_whitespace() || (c as u32) <= 0x20 => {
                for b in c.to_string().bytes() {
                    out.push_str(&format!("%{b:02X}"));
                }
            }
            other => out.push(other),
        }
    }
    out
}

/// Format an `f64` confidence as an `xsd:decimal` lexical form. `xsd:decimal`
/// forbids exponents and requires at least one digit on each side of the point,
/// so we always emit a fractional part.
fn format_decimal(value: f64) -> String {
    // `{:?}` on f64 never uses exponent form for finite values in this range and
    // preserves a round-trippable representation; ensure a decimal point exists.
    let mut s = format!("{value}");
    if !s.contains('.') {
        s.push_str(".0");
    }
    s
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
            rsa_private_key_b64: throwaway_rsa_private_key().into(),
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
    fn enriched_assertion_carries_structure() {
        let ttl = assertion_ttl_for_claim(
            "p95 latency increased by 12ms",
            Some(("p95_latency_ms", "increased_by", "12 ms")),
            "numeric",
            0.91,
            "possibly_novel",
            &["https://doi.org/10.1/x"],
        );

        assert!(ttl.contains("scientia:text"), "missing text: {ttl}");
        assert!(ttl.contains("scientia:relation"), "missing relation: {ttl}");
        assert!(
            ttl.contains("scientia:confidence"),
            "missing confidence: {ttl}"
        );
        assert!(
            ttl.contains("scientia:noveltyVerdict"),
            "missing noveltyVerdict: {ttl}"
        );
        assert!(
            ttl.contains("closestPriorArt"),
            "missing closestPriorArt: {ttl}"
        );
    }

    #[test]
    fn enriched_nanopub_validates_offline() {
        let profile = NanopubProfile {
            orcid: "https://orcid.org/0000-0002-1267-0234".to_string(),
            name: "Vox Scientia Test".to_string(),
            rsa_private_key_b64: throwaway_rsa_private_key().into(),
        };

        let assertion = assertion_ttl_for_claim(
            "p95 latency increased by 12ms",
            Some(("p95_latency_ms", "increased_by", "12 ms")),
            "numeric",
            0.91,
            "possibly_novel",
            &["https://doi.org/10.1/x"],
        );

        let signed = build_and_sign(
            &assertion,
            "https://orcid.org/0000-0002-1267-0234",
            1_700_000_000,
            &profile,
        )
        .expect("build_and_sign should succeed on enriched assertion");

        validate_offline(&signed.trig)
            .expect("offline validation should pass for enriched nanopub");
    }

    #[test]
    fn tampered_trig_fails_offline_validation() {
        let profile = NanopubProfile {
            orcid: "https://orcid.org/0000-0002-1267-0234".to_string(),
            name: "Vox Scientia Test".to_string(),
            rsa_private_key_b64: throwaway_rsa_private_key().into(),
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
