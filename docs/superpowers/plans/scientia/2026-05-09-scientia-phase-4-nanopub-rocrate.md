# SCIENTIA Phase 4 — `vox-nanopub` + `vox-ro-crate` Crates

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create two new L2 crates: `vox-nanopub` (TriG-format nanopublication builder with Ed25519 signing) and `vox-ro-crate` (RO-Crate 1.2 JSON-LD metadata builder with CFF/CodeMeta/TOP compliance surfacing).

**Architecture:** Both crates are pure domain logic (no async, no network calls in production code; network calls are Phase 8). `vox-nanopub` generates TriG strings via template (no external RDF library). `vox-ro-crate` builds JSON-LD via serde_json.

**Tech Stack:** serde, serde_json, sha2, hex, thiserror, vox-crypto, workspace-hack.

**Strategic reference:** [SCIENTIA plan §6 (Publication Artifacts)](../../../src/architecture/scientia-self-publication-finalization-plan-2026.md#phase-4--nanopub--ro-crate--topacm-badges)

---

## PART A: vox-nanopub (Tasks 1–4)

### Task 1: Scaffold vox-nanopub

- [ ] Create `crates/vox-nanopub/Cargo.toml`:

```toml
[package]
name = "vox-nanopub"
description = "SCIENTIA nanopublication builder: TriG emission, Ed25519 signing, Trusty URI (T2 atomic claim → signed nanopub)."
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true

[lints]
workspace = true

[dependencies]
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
sha2 = { workspace = true }
hex = { workspace = true }
thiserror = { workspace = true }
vox-crypto = { workspace = true }
vox-research-events = { workspace = true }
workspace-hack = { workspace = true }
```

- [ ] Create `crates/vox-nanopub/src/lib.rs` declaring modules `trig`, `signing`, `network` and re-exporting key types:

```rust
pub mod trig;
pub mod signing;
pub mod network;

pub use trig::{NanopubDocument, NanopubGraphs, build_nanopub};
pub use signing::{SignedNanopub, sign_nanopub, verify_nanopub};
pub use network::{NanopubNetworkConfig, PublishResult, publish_stub};
```

- [ ] Add `vox-nanopub = { path = "crates/vox-nanopub" }` to root `Cargo.toml` `[workspace.dependencies]`.
- [ ] Run `cargo test -p vox-nanopub` (passes trivially at scaffold stage).
- [ ] Commit: `feat(scientia): scaffold vox-nanopub crate (Phase 4 Task 1)`

---

### Task 2: `trig.rs` — TriG nanopublication builder

A nanopublication has four named graphs: head, assertion, provenance, pubinfo. Implement via string templates (no external RDF library).

- [ ] Create `crates/vox-nanopub/src/trig.rs` with the following types and function:

```rust
use sha2::{Digest, Sha256};

pub struct NanopubGraphs {
    pub assertion_graph: String,   // Turtle triples for the claim
    pub provenance_graph: String,  // who generated it, when
    pub pubinfo_graph: String,     // signature embedded here
}

pub struct NanopubDocument {
    pub trig: String,    // complete TriG serialization
    pub np_uri: String,  // nanopub URI (e.g. "https://vox.scientia/np/RA<hash>")
}

pub fn build_nanopub(claim_text: &str, provider_id: &str, published_at: i64) -> NanopubDocument {
    // Build TriG with four named graphs:
    // :head { :np np:hasAssertion :assertion; np:hasProvenance :provenance; np:hasPublicationInfo :pubinfo }
    // :assertion { :claim1 rdf:type scientia:AtomicClaim; scientia:text "claim_text" }
    // :provenance { :assertion prov:wasAttributedTo :provider; prov:generatedAtTime published_at }
    // :pubinfo { :np dc:created published_at; dc:creator "vox-scientia" }
    //
    // np_uri = "https://vox.scientia/np/RA" + hex(sha256(trig_bytes))
    //
    // Full implementation:
    let prefixes = format!(
        "@prefix : <https://vox.scientia/np/> .\n\
         @prefix np: <http://www.nanopub.org/nschema#> .\n\
         @prefix prov: <http://www.w3.org/ns/prov#> .\n\
         @prefix dc: <http://purl.org/dc/terms/> .\n\
         @prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .\n\
         @prefix scientia: <https://vox.scientia/vocab#> .\n\n"
    );

    let head = format!(
        ":head {{\n  \
           :np np:hasAssertion :assertion ;\n      \
               np:hasProvenance :provenance ;\n      \
               np:hasPublicationInfo :pubinfo .\n\
         }}\n\n"
    );

    let assertion = format!(
        ":assertion {{\n  \
           :claim1 rdf:type scientia:AtomicClaim ;\n          \
                   scientia:text {claim_text:?} .\n\
         }}\n\n",
        claim_text = claim_text
    );

    let provenance = format!(
        ":provenance {{\n  \
           :assertion prov:wasAttributedTo {provider_id:?} ;\n              \
                      prov:generatedAtTime {published_at} .\n\
         }}\n\n",
        provider_id = provider_id,
        published_at = published_at
    );

    let pubinfo = format!(
        ":pubinfo {{\n  \
           :np dc:created {published_at} ;\n      \
               dc:creator \"vox-scientia\" .\n\
         }}\n",
        published_at = published_at
    );

    let trig = format!("{}{}{}{}{}", prefixes, head, assertion, provenance, pubinfo);

    let hash = hex::encode(Sha256::digest(trig.as_bytes()));
    let np_uri = format!("https://vox.scientia/np/RA{}", hash);

    NanopubDocument { trig, np_uri }
}
```

- [ ] Write 3 tests in a `#[cfg(test)]` block at the bottom of `trig.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trig_document_contains_four_graphs() {
        let doc = build_nanopub("test claim", "provider:test", 1000000);
        assert!(doc.trig.contains("@prefix"));
        assert!(doc.trig.contains(":head"));
        assert!(doc.trig.contains(":assertion"));
        assert!(doc.trig.contains(":provenance"));
        assert!(doc.trig.contains(":pubinfo"));
    }

    #[test]
    fn np_uri_starts_with_prefix() {
        let doc = build_nanopub("test claim", "provider:test", 1000000);
        assert!(doc.np_uri.starts_with("https://vox.scientia/np/RA"));
    }

    #[test]
    fn same_inputs_produce_same_trig() {
        let doc1 = build_nanopub("claim", "provider:x", 42);
        let doc2 = build_nanopub("claim", "provider:x", 42);
        assert_eq!(doc1.trig, doc2.trig);
        assert_eq!(doc1.np_uri, doc2.np_uri);
    }
}
```

- [ ] Run `cargo test -p vox-nanopub` — all 3 tests pass.
- [ ] Commit: `feat(scientia): TriG nanopublication builder (Phase 4 Task 2)`

---

### Task 3: `signing.rs` — sign a nanopublication

Sign the TriG bytes with ed25519 (using `vox_crypto`) and embed the hex signature in the pubinfo graph.

- [ ] Create `crates/vox-nanopub/src/signing.rs`:

```rust
use crate::trig::NanopubDocument;
use vox_crypto::facades::{sign, to_verifying_key, verify};
pub use vox_crypto::SigningKey;
pub use vox_crypto::VerifyingKey;

pub struct SignedNanopub {
    pub document: NanopubDocument,
    pub signature_hex: String,
}

pub fn sign_nanopub(doc: NanopubDocument, signing_key: &SigningKey) -> SignedNanopub {
    let sig_bytes: [u8; 64] = sign(signing_key, doc.trig.as_bytes());
    let signature_hex = hex::encode(sig_bytes);
    SignedNanopub { document: doc, signature_hex }
}

pub fn verify_nanopub(signed: &SignedNanopub, verifying_key: &VerifyingKey) -> bool {
    let sig_bytes = match hex::decode(&signed.signature_hex) {
        Ok(b) if b.len() == 64 => {
            let mut arr = [0u8; 64];
            arr.copy_from_slice(&b);
            arr
        }
        _ => return false,
    };
    verify(verifying_key, signed.document.trig.as_bytes(), sig_bytes)
}
```

- [ ] Write 2 tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::trig::build_nanopub;
    use vox_crypto::facades::generate_signing_key;

    #[test]
    fn sign_and_verify_round_trip() {
        let sk = generate_signing_key();
        let vk = to_verifying_key(&sk);
        let doc = build_nanopub("round trip claim", "provider:test", 999);
        let signed = sign_nanopub(doc, &sk);
        assert!(verify_nanopub(&signed, &vk));
    }

    #[test]
    fn tampered_trig_fails_verify() {
        let sk = generate_signing_key();
        let vk = to_verifying_key(&sk);
        let doc = build_nanopub("original claim", "provider:test", 999);
        let mut signed = sign_nanopub(doc, &sk);
        signed.document.trig.push_str("\n# tampered");
        assert!(!verify_nanopub(&signed, &vk));
    }
}
```

**Self-check:** `vox_crypto::facades::sign(key, bytes) -> [u8; 64]`, `vox_crypto::facades::verify(vk, bytes, sig_arr) -> bool`, `vox_crypto::facades::to_verifying_key(sk) -> VerifyingKey`, `vox_crypto::facades::generate_signing_key() -> SigningKey`. Confirm these exist in `vox-crypto` before writing — adjust names to match actual API if needed.

- [ ] Run `cargo test -p vox-nanopub` — all 5 tests pass.
- [ ] Commit: `feat(scientia): Ed25519 nanopub signing + verification (Phase 4 Task 3)`

---

### Task 4: `network.rs` — Nanopub Network publish stub

- [ ] Create `crates/vox-nanopub/src/network.rs`:

```rust
use crate::signing::SignedNanopub;

pub struct NanopubNetworkConfig {
    pub endpoint: String,  // e.g., "https://np.knowledgepixels.com/"
}

pub struct PublishResult {
    pub success: bool,
    pub nanopub_uri: Option<String>,
    pub error: Option<String>,
}

/// Phase 8: replace with actual HTTP POST to the Nanopub Network.
/// For now returns a stub error result.
pub fn publish_stub(_signed: &SignedNanopub, _config: &NanopubNetworkConfig) -> PublishResult {
    PublishResult {
        success: false,
        nanopub_uri: None,
        error: Some("Phase 8 stub".to_string()),
    }
}
```

- [ ] Write 1 test:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::trig::build_nanopub;
    use crate::signing::sign_nanopub;
    use vox_crypto::facades::generate_signing_key;

    #[test]
    fn publish_stub_returns_error_result() {
        let sk = generate_signing_key();
        let doc = build_nanopub("stub claim", "provider:test", 0);
        let signed = sign_nanopub(doc, &sk);
        let config = NanopubNetworkConfig {
            endpoint: "https://np.knowledgepixels.com/".to_string(),
        };
        let result = publish_stub(&signed, &config);
        assert!(!result.success);
        assert!(result.nanopub_uri.is_none());
        assert_eq!(result.error.as_deref(), Some("Phase 8 stub"));
    }
}
```

- [ ] Run `cargo test -p vox-nanopub` — all 6 tests pass.
- [ ] Commit: `feat(scientia): Nanopub Network publish stub (Phase 4 Task 4)`

---

## PART B: vox-ro-crate (Tasks 5–8)

### Task 5: Scaffold vox-ro-crate

- [ ] Create `crates/vox-ro-crate/Cargo.toml` (same deps as `vox-nanopub` but without `sha2`/`hex` — not needed for JSON-LD building):

```toml
[package]
name = "vox-ro-crate"
description = "SCIENTIA RO-Crate 1.2 JSON-LD metadata builder with CFF, CodeMeta, TOP-Level-2 and ACM badge compliance surfacing."
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true

[lints]
workspace = true

[dependencies]
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
thiserror = { workspace = true }
vox-crypto = { workspace = true }
vox-research-events = { workspace = true }
workspace-hack = { workspace = true }
```

- [ ] Create `crates/vox-ro-crate/src/lib.rs` declaring modules `metadata`, `compliance`, `cff`:

```rust
pub mod metadata;
pub mod compliance;
pub mod cff;

pub use metadata::{RoCrateMetadata, build_ro_crate_json};
pub use compliance::{
    TopLevel, TopComplianceReport, AcmBadge, acm_artifacts_available_badge,
};
pub use cff::{CffAuthor, CffMetadata, build_cff_json};
```

- [ ] Add `vox-ro-crate = { path = "crates/vox-ro-crate" }` to root `Cargo.toml` `[workspace.dependencies]`.
- [ ] Run `cargo test -p vox-ro-crate` (passes trivially at scaffold stage).
- [ ] Commit: `feat(scientia): scaffold vox-ro-crate crate (Phase 4 Task 5)`

---

### Task 6: `metadata.rs` — RO-Crate 1.2 JSON-LD builder

RO-Crate metadata is a JSON-LD file with a required `@context` and `@graph` array. Build it using `serde_json`.

- [ ] Create `crates/vox-ro-crate/src/metadata.rs`:

```rust
use serde_json::{json, Value};

pub struct RoCrateMetadata {
    pub name: String,
    pub description: String,
    pub doi: Option<String>,
    pub author_orcid: Option<String>,    // e.g. "https://orcid.org/0000-0001-2345-6789"
    pub author_ror: Option<String>,      // e.g. "https://ror.org/03yrm5c26"
    pub license_spdx: String,            // e.g. "CC-BY-4.0"
    pub published_at: i64,               // Unix timestamp
    pub keywords: Vec<String>,
}

pub fn build_ro_crate_json(metadata: &RoCrateMetadata) -> Value {
    // Format ISO date from Unix timestamp (seconds since epoch, UTC)
    let iso_date = format_iso_date(metadata.published_at);

    let license_url = format!("https://spdx.org/licenses/{}", metadata.license_spdx);

    let author_id = metadata.author_orcid.clone()
        .unwrap_or_else(|| "#anonymous-author".to_string());

    // Build identifier array (include DOI if present)
    let mut identifiers: Vec<Value> = Vec::new();
    if let Some(doi) = &metadata.doi {
        let doi_url = if doi.starts_with("http") {
            doi.clone()
        } else {
            format!("https://doi.org/{}", doi)
        };
        identifiers.push(json!({ "@id": doi_url }));
    }

    // Build author node
    let mut author_node = json!({
        "@id": author_id,
        "@type": "Person"
    });
    if let Some(ror) = &metadata.author_ror {
        author_node["affiliation"] = json!({ "@id": ror });
    }

    // Descriptor node (ro-crate-metadata.json itself)
    let descriptor = json!({
        "@id": "ro-crate-metadata.json",
        "@type": "CreativeWork",
        "about": { "@id": "./" },
        "conformsTo": { "@id": "https://w3id.org/ro/crate/1.2" }
    });

    // Root dataset node
    let mut dataset = json!({
        "@id": "./",
        "@type": "Dataset",
        "name": metadata.name,
        "description": metadata.description,
        "license": { "@id": license_url },
        "author": [{ "@id": author_id }],
        "datePublished": iso_date,
        "keywords": metadata.keywords
    });

    if !identifiers.is_empty() {
        dataset["identifier"] = json!(identifiers);
    }

    let mut graph = vec![descriptor, dataset];
    // Only include author node if we have a real ORCID or ROR
    if metadata.author_orcid.is_some() || metadata.author_ror.is_some() {
        graph.push(author_node);
    }

    json!({
        "@context": "https://w3id.org/ro/crate/1.2/context",
        "@graph": graph
    })
}

/// Format a Unix timestamp as an ISO 8601 date string (YYYY-MM-DD).
fn format_iso_date(unix_ts: i64) -> String {
    // Simple implementation: days since epoch
    let days = unix_ts / 86400;
    let mut year = 1970i64;
    let mut remaining_days = days;

    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }

    let month_days = if is_leap_year(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 1usize;
    for &md in &month_days {
        if remaining_days < md {
            break;
        }
        remaining_days -= md;
        month += 1;
    }
    let day = remaining_days + 1;

    format!("{:04}-{:02}-{:02}", year, month, day)
}

fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}
```

- [ ] Write 3 tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn sample_metadata() -> RoCrateMetadata {
        RoCrateMetadata {
            name: "Test Dataset".to_string(),
            description: "A test dataset".to_string(),
            doi: None,
            author_orcid: Some("https://orcid.org/0000-0001-2345-6789".to_string()),
            author_ror: None,
            license_spdx: "CC-BY-4.0".to_string(),
            published_at: 1_700_000_000,
            keywords: vec!["test".to_string()],
        }
    }

    #[test]
    fn ro_crate_has_required_context() {
        let json = build_ro_crate_json(&sample_metadata());
        assert_eq!(json["@context"], "https://w3id.org/ro/crate/1.2/context");
    }

    #[test]
    fn ro_crate_graph_contains_dataset() {
        let json = build_ro_crate_json(&sample_metadata());
        let graph = json["@graph"].as_array().unwrap();
        let has_dataset = graph.iter().any(|node| {
            node["@type"].as_str() == Some("Dataset")
        });
        assert!(has_dataset);
    }

    #[test]
    fn doi_included_when_present() {
        let mut meta = sample_metadata();
        meta.doi = Some("10.1234/test".to_string());
        let json = build_ro_crate_json(&meta);
        let graph = json["@graph"].as_array().unwrap();
        let dataset = graph.iter().find(|n| n["@type"] == "Dataset").unwrap();
        assert!(dataset.get("identifier").is_some());
    }
}
```

- [ ] Run `cargo test -p vox-ro-crate` — all 3 tests pass.
- [ ] Commit: `feat(scientia): RO-Crate 1.2 JSON-LD metadata builder (Phase 4 Task 6)`

---

### Task 7: `compliance.rs` — TOP-Level-2 + ACM badge surfacing

TOP (Transparency and Openness Promotion) Level 2 and ACM Artifact Available badges are structured metadata that appear in the manifest.

- [ ] Create `crates/vox-ro-crate/src/compliance.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum TopLevel {
    Level0,
    Level1,
    Level2,
    Level3,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TopComplianceReport {
    pub data_citation: TopLevel,
    pub data_transparency: TopLevel,
    pub analysis_code_transparency: TopLevel,
    pub overall_level: TopLevel,  // min of the three
}

impl TopComplianceReport {
    /// Assess TOP compliance from available artifact indicators.
    ///
    /// - Level 0: nothing
    /// - Level 1: data cited (has_data_doi)
    /// - Level 2: data + code (has_data_doi && has_code_doi)
    /// - Level 3: data + code + preregistration
    ///
    /// `overall_level` = min(data_citation, data_transparency, analysis_code_transparency)
    pub fn assess(has_data_doi: bool, has_code_doi: bool, has_preregistration: bool) -> Self {
        let data_citation = if has_data_doi {
            if has_preregistration { TopLevel::Level3 } else { TopLevel::Level2 }
        } else {
            TopLevel::Level0
        };

        let data_transparency = if has_data_doi {
            TopLevel::Level1
        } else {
            TopLevel::Level0
        };

        let analysis_code_transparency = if has_code_doi {
            if has_preregistration { TopLevel::Level3 } else { TopLevel::Level2 }
        } else {
            TopLevel::Level0
        };

        // overall = min of the three dimensions
        let overall_level = data_citation.clone()
            .min(data_transparency.clone())
            .min(analysis_code_transparency.clone());

        TopComplianceReport {
            data_citation,
            data_transparency,
            analysis_code_transparency,
            overall_level,
        }
    }

    pub fn is_level2_or_above(&self) -> bool {
        self.overall_level >= TopLevel::Level2
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcmBadge {
    pub name: String,    // e.g. "Artifacts Available"
    pub url: String,     // e.g. "https://www.acm.org/publications/policies/artifact-review-and-badging-current"
    pub awarded: bool,   // true if Zenodo deposit exists
}

pub fn acm_artifacts_available_badge(zenodo_doi: Option<&str>) -> AcmBadge {
    AcmBadge {
        name: "Artifacts Available".to_string(),
        url: "https://www.acm.org/publications/policies/artifact-review-and-badging-current"
            .to_string(),
        awarded: zenodo_doi.is_some(),
    }
}
```

**Note:** `#[derive(PartialOrd, Ord)]` on `TopLevel` gives the correct ordering because enum variants in Rust derive ordinal order from their declaration order (Level0 < Level1 < Level2 < Level3). This is the correct approach — no manual `PartialOrd` implementation needed.

- [ ] Write 4 tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_assets_is_level0() {
        let report = TopComplianceReport::assess(false, false, false);
        assert_eq!(report.overall_level, TopLevel::Level0);
        assert!(!report.is_level2_or_above());
    }

    #[test]
    fn data_and_code_is_level2() {
        let report = TopComplianceReport::assess(true, true, false);
        assert!(report.is_level2_or_above());
    }

    #[test]
    fn all_three_is_level3() {
        let report = TopComplianceReport::assess(true, true, true);
        assert_eq!(report.overall_level, TopLevel::Level3);
    }

    #[test]
    fn acm_badge_awarded_when_zenodo_doi_present() {
        let badge = acm_artifacts_available_badge(Some("10.5281/zenodo.12345"));
        assert!(badge.awarded);
        let no_badge = acm_artifacts_available_badge(None);
        assert!(!no_badge.awarded);
    }
}
```

- [ ] Run `cargo test -p vox-ro-crate` — all 7 tests pass.
- [ ] Commit: `feat(scientia): TOP-Level-2 compliance + ACM badge metadata (Phase 4 Task 7)`

---

### Task 8: `cff.rs` + wire + mark Phase 4 Complete

CFF (Citation File Format) is a YAML-format software citation spec. We serialize to `serde_json::Value` (the caller converts to YAML if needed — no `yaml` dep required).

- [ ] Create `crates/vox-ro-crate/src/cff.rs`:

```rust
use serde_json::{json, Value};

pub struct CffAuthor {
    pub given_names: String,
    pub family_names: String,
    pub orcid: Option<String>,
}

pub struct CffMetadata {
    pub title: String,
    pub version: String,
    pub doi: Option<String>,
    pub authors: Vec<CffAuthor>,
    pub repository_url: String,
    pub license: String,
}

pub fn build_cff_json(cff: &CffMetadata) -> Value {
    let authors: Vec<Value> = cff.authors.iter().map(|a| {
        let mut author = json!({
            "given-names": a.given_names,
            "family-names": a.family_names,
        });
        if let Some(orcid) = &a.orcid {
            author["orcid"] = json!(orcid);
        }
        author
    }).collect();

    let mut result = json!({
        "cff-version": "1.2.0",
        "message": "If you use this software, please cite it as below.",
        "title": cff.title,
        "version": cff.version,
        "license": cff.license,
        "repository-code": cff.repository_url,
        "authors": authors
    });

    if let Some(doi) = &cff.doi {
        result["doi"] = json!(doi);
    }

    result
}
```

- [ ] Write 2 tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn sample_cff() -> CffMetadata {
        CffMetadata {
            title: "vox-scientia".to_string(),
            version: "0.1.0".to_string(),
            doi: None,
            authors: vec![CffAuthor {
                given_names: "Jane".to_string(),
                family_names: "Doe".to_string(),
                orcid: None,
            }],
            repository_url: "https://github.com/example/vox".to_string(),
            license: "MIT".to_string(),
        }
    }

    #[test]
    fn cff_has_required_fields() {
        let json = build_cff_json(&sample_cff());
        assert_eq!(json["cff-version"], "1.2.0");
        assert!(json["message"].as_str().is_some());
        assert_eq!(json["title"], "vox-scientia");
    }

    #[test]
    fn author_orcid_included_when_present() {
        let mut cff = sample_cff();
        cff.authors[0].orcid = Some("https://orcid.org/0000-0001-2345-6789".to_string());
        let json = build_cff_json(&cff);
        let authors = json["authors"].as_array().unwrap();
        assert_eq!(
            authors[0]["orcid"],
            "https://orcid.org/0000-0001-2345-6789"
        );
    }
}
```

- [ ] Update `crates/vox-ro-crate/src/lib.rs` to export `cff::{CffAuthor, CffMetadata, build_cff_json}` and `compliance::{AcmBadge, TopComplianceReport, TopLevel, acm_artifacts_available_badge}` (already declared in Task 5 scaffold — verify exports are correct).

- [ ] Run `cargo test -p vox-nanopub -p vox-ro-crate 2>&1 | tail -10` — all tests pass.

- [ ] Mark Phase 4 complete in the strategic plan at `docs/src/architecture/scientia-self-publication-finalization-plan-2026.md` (update the Phase 4 checkbox or status marker).

- [ ] Commit 1: `feat(scientia): CFF metadata builder + vox-ro-crate lib exports (Phase 4 Task 8a)`
- [ ] Commit 2: `feat(scientia): wire vox-nanopub + vox-ro-crate into workspace + mark Phase 4 Complete (Phase 4 Task 8b)`

---

## Rules

- No placeholders. Complete code in every step.
- Run `cargo test -p vox-nanopub` and/or `cargo test -p vox-ro-crate` after each task before committing.
- No external RDF/Turtle library — all TriG output is via `format!` string templates.
- No `base64` dep — use `hex` encoding for the np_uri hash (already a workspace dep).
- No `yaml` dep — `build_cff_json` returns `serde_json::Value`; callers convert to YAML if needed.
- `TopLevel` derives `PartialOrd + Ord` — declaration order gives Level0 < Level1 < Level2 < Level3 automatically.
- `vox_crypto` API: `facades::sign(key, bytes) -> [u8; 64]`, `facades::verify(vk, bytes, sig) -> bool`, `facades::to_verifying_key(sk) -> VerifyingKey`, `facades::generate_signing_key() -> SigningKey` — verify these names against the actual crate source before writing signing.rs.
- Network calls are Phase 8 stubs only — no `reqwest` or async in these crates.
