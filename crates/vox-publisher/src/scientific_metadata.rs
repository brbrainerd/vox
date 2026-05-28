//! Structured scholarly fields embedded in `crate::publication::PublicationManifest::metadata_json`.
//!
//! Stored under the `scientific_publication` key so community/news metadata can coexist unchanged.
//! All fields are optional at the JSON layer except author `name` when an author entry is present.

use serde::{Deserialize, Serialize};

/// JSON object key for [`ScientificPublicationMetadata`] inside `metadata_json`.
pub const METADATA_KEY_SCIENTIFIC: &str = "scientific_publication";

fn scientific_publication_schema_version_default() -> u32 {
    1
}

/// Normalized metadata for journal/preprint/DOI readiness (Phase 0; backward-compatible).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScientificPublicationMetadata {
    /// Logical schema revision for this `scientific_publication` object (migration + CI drift).
    #[serde(default = "scientific_publication_schema_version_default")]
    pub schema_version: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<ScientificAuthor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license_spdx: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub funding_statement: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub competing_interests_statement: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reproducibility: Option<ReproducibilityAttestation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ethics_and_impact: Option<EthicsAndImpactAttestation>,
}

impl Default for ScientificPublicationMetadata {
    fn default() -> Self {
        Self {
            schema_version: scientific_publication_schema_version_default(),
            authors: Vec::new(),
            license_spdx: None,
            funding_statement: None,
            competing_interests_statement: None,
            reproducibility: None,
            ethics_and_impact: None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScientificAuthor {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orcid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ror: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub affiliation: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReproducibilityAttestation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code_repository_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_repository_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_checksum_note: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EthicsAndImpactAttestation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub broader_impact_statement: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub irb_or_human_subjects_note: Option<String>,
}

/// Build the `metadata_json` string for scientia `publication-prepare` flows.
pub fn build_scientia_metadata_json(
    prepared_by: &str,
    repository_id: Option<&str>,
    scientific: Option<&ScientificPublicationMetadata>,
    scientia_evidence: Option<&crate::scientia_evidence::ScientiaEvidenceContext>,
) -> serde_json::Result<String> {
    let mut root = serde_json::Map::new();
    root.insert(
        "prepared_by".to_string(),
        serde_json::Value::String(prepared_by.to_string()),
    );
    if let Some(id) = repository_id
        && !id.is_empty()
    {
        root.insert("repository_id".to_string(), serde_json::json!(id));
    }
    if let Some(s) = scientific
        && s != &ScientificPublicationMetadata::default()
    {
        root.insert(
            METADATA_KEY_SCIENTIFIC.to_string(),
            serde_json::to_value(s)?,
        );
    }
    if let Some(e) = scientia_evidence {
        root.insert(
            crate::scientia_evidence::METADATA_KEY_SCIENTIA_EVIDENCE.to_string(),
            serde_json::to_value(e)?,
        );
    }
    Ok(serde_json::Value::Object(root).to_string())
}

/// Merge [`crate::scientia_evidence::METADATA_KEY_SCIENTIA_EVIDENCE`] into existing `metadata_json` without dropping sibling keys.
pub fn merge_scientia_evidence_into_metadata_json(
    metadata_json: Option<&str>,
    evidence: &crate::scientia_evidence::ScientiaEvidenceContext,
    prepared_by: Option<&str>,
) -> serde_json::Result<String> {
    let mut map = match metadata_json {
        Some(raw) if !raw.trim().is_empty() => {
            let v: serde_json::Value = serde_json::from_str(raw)?;
            if let Some(o) = v.as_object() {
                o.clone()
            } else {
                serde_json::Map::new()
            }
        }
        _ => serde_json::Map::new(),
    };
    map.insert(
        crate::scientia_evidence::METADATA_KEY_SCIENTIA_EVIDENCE.to_string(),
        serde_json::to_value(evidence)?,
    );
    if let Some(pb) = prepared_by.map(str::trim).filter(|s| !s.is_empty()) {
        map.insert(
            "prepared_by".to_string(),
            serde_json::Value::String(pb.to_string()),
        );
    }
    serde_json::to_string(&serde_json::Value::Object(map))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_metadata_includes_scientific_block() {
        let sci = ScientificPublicationMetadata {
            schema_version: 1,
            authors: vec![ScientificAuthor {
                name: "Ada Lovelace".to_string(),
                orcid: Some("0000-0001-2345-6789".to_string()),
                ror: None,
                affiliation: None,
            }],
            license_spdx: Some("Apache-2.0".to_string()),
            funding_statement: None,
            competing_interests_statement: None,
            reproducibility: None,
            ethics_and_impact: None,
        };
        let s = build_scientia_metadata_json("test", None, Some(&sci), None).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["prepared_by"], "test");
        assert_eq!(v[METADATA_KEY_SCIENTIFIC]["schema_version"], 1);
        assert!(v[METADATA_KEY_SCIENTIFIC]["authors"][0]["name"] == "Ada Lovelace");
    }

    #[test]
    fn digest_changes_when_scientific_block_changes() {
        use crate::publication::PublicationManifest;

        let base = build_scientia_metadata_json("prep", None, None, None).unwrap();
        let mut m1 = PublicationManifest {
            publication_id: "p1".to_string(),
            content_type: "scientia".to_string(),
            source_ref: None,
            title: "t".to_string(),
            author: "a".to_string(),
            abstract_text: None,
            body_markdown: "body".to_string(),
            citations_json: None,
            metadata_json: Some(base),
        };
        let d1 = m1.content_sha3_256();

        let sci = ScientificPublicationMetadata {
            license_spdx: Some("Apache-2.0".to_string()),
            ..Default::default()
        };
        m1.metadata_json =
            Some(build_scientia_metadata_json("prep", None, Some(&sci), None).unwrap());
        let d2 = m1.content_sha3_256();
        assert_ne!(d1, d2);
    }
}
