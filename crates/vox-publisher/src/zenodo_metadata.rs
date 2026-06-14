//! Zenodo REST [`metadata`](https://developers.zenodo.org/) object builder from a [`crate::publication::PublicationManifest`].
//!
//! Suitable for `POST /api/deposit/depositions` (new draft) or for `.zenodo.json` communities/workflows.
//! License ids follow Zenodo’s vocabulary where possible; unknown SPDX values are passed lowercased.

use crate::publication::PublicationManifest;
use crate::publication_preflight::parse_scientific_from_metadata_json;
use crate::scientific_metadata::ScientificPublicationMetadata;
use crate::zenodo_api_types::{
    ZenodoCreator, ZenodoDepositionCreateBody, ZenodoDepositionMetadata, ZenodoRelatedIdentifier,
};

/// Derive keywords from a title string.
/// Lowercases tokens, strips non-alphanumeric edges, keeps tokens longer than 3 chars, deduplicates
/// preserving order, and caps at 8.
#[must_use]
pub fn keywords_from_title(title: &str) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    for token in title.split_whitespace() {
        let lower = token.to_lowercase();
        let stripped: String = lower
            .trim_matches(|c: char| !c.is_alphanumeric())
            .to_string();
        if stripped.len() > 3 && seen.insert(stripped.clone()) {
            result.push(stripped);
            if result.len() == 8 {
                break;
            }
        }
    }
    result
}

/// Build the typed POST body for a new Zenodo deposit draft.
#[must_use]
pub fn zenodo_deposition_create_body(manifest: &PublicationManifest) -> ZenodoDepositionCreateBody {
    let scientific: Option<ScientificPublicationMetadata> =
        parse_scientific_from_metadata_json(manifest.metadata_json.as_deref())
            .ok()
            .flatten();

    // Parse the raw metadata_json once for scientia.* keys.
    let meta_val: Option<serde_json::Value> = manifest
        .metadata_json
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok());

    let creators: Vec<ZenodoCreator> = if let Some(ref sci) = scientific {
        if sci.authors.is_empty() {
            vec![ZenodoCreator {
                name: manifest.author.clone(),
                affiliation: None,
                orcid: None,
            }]
        } else {
            sci.authors
                .iter()
                .map(|a| {
                    let affiliation = a
                        .affiliation
                        .as_deref()
                        .and_then(|s| {
                            let t = s.trim();
                            (!t.is_empty()).then_some(t)
                        })
                        .map(std::string::ToString::to_string);
                    let orcid = a.orcid.as_deref().and_then(|oid: &str| {
                        let t = oid.trim();
                        if t.is_empty() {
                            return None;
                        }
                        let uri = if t.starts_with("http") {
                            t.to_string()
                        } else {
                            format!("https://orcid.org/{t}")
                        };
                        Some(uri)
                    });
                    ZenodoCreator {
                        name: a.name.clone(),
                        affiliation,
                        orcid,
                    }
                })
                .collect()
        }
    } else {
        vec![ZenodoCreator {
            name: manifest.author.clone(),
            affiliation: None,
            orcid: None,
        }]
    };

    let description = manifest
        .abstract_text
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .map(std::string::ToString::to_string)
        .unwrap_or_else(|| {
            let body = manifest.body_markdown.trim();
            if body.len() <= 4000 {
                body.to_string()
            } else {
                format!("{}…", body.chars().take(3990).collect::<String>())
            }
        });

    let license = scientific
        .as_ref()
        .and_then(|s| s.license_spdx.as_deref())
        .and_then(|s| {
            let t = s.trim().to_lowercase();
            (!t.is_empty()).then_some(t)
        })
        .unwrap_or_else(|| "notspecified".to_string());

    // publication_date: today (deterministic archive record).
    let publication_date = Some(chrono::Utc::now().format("%Y-%m-%d").to_string());

    // keywords: explicit scientia.keywords wins, else derive from title.
    let keywords: Vec<String> = meta_val
        .as_ref()
        .and_then(|v| v.get("scientia"))
        .and_then(|s| s.get("keywords"))
        .and_then(|kw| kw.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|k| k.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| keywords_from_title(&manifest.title));

    // related_identifiers: reproducibility repos + nanopubs + swhid.
    let mut related_identifiers: Vec<ZenodoRelatedIdentifier> = Vec::new();
    if let Some(ref sci) = scientific
        && let Some(ref repro) = sci.reproducibility
    {
        if let Some(ref url) = repro.code_repository_url
            && !url.trim().is_empty()
        {
            related_identifiers.push(ZenodoRelatedIdentifier {
                identifier: url.clone(),
                relation: "isSupplementTo".to_string(),
                resource_type: Some("software".to_string()),
            });
        }
        if let Some(ref url) = repro.data_repository_url
            && !url.trim().is_empty()
        {
            related_identifiers.push(ZenodoRelatedIdentifier {
                identifier: url.clone(),
                relation: "isSupplementTo".to_string(),
                resource_type: Some("dataset".to_string()),
            });
        }
    }
    if let Some(ref val) = meta_val {
        if let Some(arr) = val
            .get("scientia")
            .and_then(|s| s.get("nanopub_uris"))
            .and_then(|v| v.as_array())
        {
            for uri in arr.iter().filter_map(|v| v.as_str()) {
                if !uri.trim().is_empty() {
                    related_identifiers.push(ZenodoRelatedIdentifier {
                        identifier: uri.to_string(),
                        relation: "isSupplementTo".to_string(),
                        resource_type: None,
                    });
                }
            }
        }
        if let Some(swhid) = val
            .get("scientia")
            .and_then(|s| s.get("swhid"))
            .and_then(|v| v.as_str())
            && !swhid.trim().is_empty()
        {
            related_identifiers.push(ZenodoRelatedIdentifier {
                identifier: swhid.to_string(),
                relation: "isIdenticalTo".to_string(),
                resource_type: Some("software".to_string()),
            });
        }
    }

    // version: scientia.version string if present.
    let version: Option<String> = meta_val
        .as_ref()
        .and_then(|v| v.get("scientia"))
        .and_then(|s| s.get("version"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(str::to_string);

    ZenodoDepositionCreateBody {
        metadata: ZenodoDepositionMetadata {
            title: manifest.title.clone(),
            upload_type: "publication".to_string(),
            publication_type: "article".to_string(),
            description,
            creators,
            access_right: "open".to_string(),
            license,
            publication_date,
            keywords,
            related_identifiers,
            version,
        },
    }
}

/// Build the JSON envelope for a new Zenodo deposit draft (compat for callers expecting [`serde_json::Value`]).
#[must_use]
pub fn zenodo_deposition_metadata(manifest: &PublicationManifest) -> serde_json::Value {
    serde_json::to_value(zenodo_deposition_create_body(manifest)).unwrap_or_else(|_| {
        serde_json::json!({ "metadata": { "title": manifest.title, "upload_type": "publication" } })
    })
}

/// Pretty JSON for a sidecar `.zenodo.json` file (same envelope as [`zenodo_deposition_metadata`]).
pub fn zenodo_json_pretty(manifest: &PublicationManifest) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(&zenodo_deposition_create_body(manifest))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scientific_metadata::{
        ReproducibilityAttestation, ScientificAuthor, ScientificPublicationMetadata,
    };

    #[test]
    fn zenodo_includes_creators_and_license() {
        let sci = ScientificPublicationMetadata {
            authors: vec![ScientificAuthor {
                name: "A".to_string(),
                orcid: Some("0000-0002-1825-0097".to_string()),
                affiliation: Some("U".to_string()),
                ror: None,
            }],
            license_spdx: Some("Apache-2.0".to_string()),
            ..Default::default()
        };
        let meta =
            crate::scientific_metadata::build_scientia_metadata_json("x", None, Some(&sci), None)
                .unwrap();
        let m = PublicationManifest {
            publication_id: "p".to_string(),
            content_type: "scientia".to_string(),
            source_ref: None,
            title: "T".to_string(),
            author: "A".to_string(),
            abstract_text: Some("Abs".to_string()),
            body_markdown: "b".to_string(),
            citations_json: None,
            metadata_json: Some(meta),
        };
        let v = zenodo_deposition_metadata(&m);
        assert_eq!(v["metadata"]["title"], "T");
        assert_eq!(v["metadata"]["license"], "apache-2.0");
        assert!(
            v["metadata"]["creators"][0]["orcid"]
                .as_str()
                .unwrap()
                .contains("orcid.org")
        );
    }

    #[test]
    fn zenodo_json_pretty_round_trips() {
        let m = PublicationManifest {
            publication_id: "p".to_string(),
            content_type: "scientia".to_string(),
            source_ref: None,
            title: "T2".to_string(),
            author: "B".to_string(),
            abstract_text: None,
            body_markdown: "body".to_string(),
            citations_json: None,
            metadata_json: None,
        };
        let s = zenodo_json_pretty(&m).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["metadata"]["title"], "T2");
    }

    #[test]
    fn body_includes_publication_date_and_keywords() {
        let m = PublicationManifest {
            publication_id: "p".to_string(),
            content_type: "scientia".to_string(),
            source_ref: None,
            title: "Neural Networks and Deep Learning".to_string(),
            author: "A".to_string(),
            abstract_text: Some("Abs".to_string()),
            body_markdown: "b".to_string(),
            citations_json: None,
            metadata_json: None,
        };
        let v = zenodo_deposition_metadata(&m);
        let date = v["metadata"]["publication_date"].as_str().unwrap();
        assert_eq!(date.len(), 10, "date must be YYYY-MM-DD");
        assert_eq!(date.chars().filter(|&c| c == '-').count(), 2);
        let kws = v["metadata"]["keywords"].as_array().unwrap();
        assert!(!kws.is_empty(), "keywords should be derived from title");
    }

    #[test]
    fn explicit_keywords_win_over_derivation() {
        let meta = serde_json::json!({
            "scientia": { "keywords": ["alpha", "beta"] }
        })
        .to_string();
        let m = PublicationManifest {
            publication_id: "p".to_string(),
            content_type: "scientia".to_string(),
            source_ref: None,
            title: "Something Completely Different".to_string(),
            author: "A".to_string(),
            abstract_text: None,
            body_markdown: "b".to_string(),
            citations_json: None,
            metadata_json: Some(meta),
        };
        let v = zenodo_deposition_metadata(&m);
        let kws: Vec<&str> = v["metadata"]["keywords"]
            .as_array()
            .unwrap()
            .iter()
            .map(|k| k.as_str().unwrap())
            .collect();
        assert_eq!(kws, vec!["alpha", "beta"]);
    }

    #[test]
    fn related_identifiers_carry_repos_nanopubs_and_swhid() {
        let sci = ScientificPublicationMetadata {
            authors: vec![ScientificAuthor {
                name: "A".to_string(),
                ..Default::default()
            }],
            reproducibility: Some(ReproducibilityAttestation {
                code_repository_url: Some("https://github.com/org/repo".to_string()),
                data_repository_url: None,
                artifact_checksum_note: None,
            }),
            ..Default::default()
        };
        let sci_json = crate::scientific_metadata::build_scientia_metadata_json(
            "test",
            None,
            Some(&sci),
            None,
        )
        .unwrap();
        // Merge scientia block with nanopub_uris and swhid.
        let mut root: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(&sci_json).unwrap();
        root.insert(
            "scientia".to_string(),
            serde_json::json!({
                "nanopub_uris": ["https://w3id.org/np/RAxyz"],
                "swhid": "swh:1:snp:abc"
            }),
        );
        let meta = serde_json::Value::Object(root).to_string();

        let m = PublicationManifest {
            publication_id: "p".to_string(),
            content_type: "scientia".to_string(),
            source_ref: None,
            title: "T".to_string(),
            author: "A".to_string(),
            abstract_text: None,
            body_markdown: "b".to_string(),
            citations_json: None,
            metadata_json: Some(meta),
        };
        let body = zenodo_deposition_create_body(&m);
        let ids = &body.metadata.related_identifiers;
        // code_repository_url + nanopub + swhid = 3
        assert_eq!(ids.len(), 3, "expected 3 related identifiers, got {ids:?}");
        let code = ids
            .iter()
            .find(|r| r.identifier.contains("github"))
            .unwrap();
        assert_eq!(code.relation, "isSupplementTo");
        assert_eq!(code.resource_type.as_deref(), Some("software"));
        let nanopub = ids.iter().find(|r| r.identifier.contains("w3id")).unwrap();
        assert_eq!(nanopub.relation, "isSupplementTo");
        assert!(nanopub.resource_type.is_none());
        let swhid = ids.iter().find(|r| r.identifier.contains("swh")).unwrap();
        assert_eq!(swhid.relation, "isIdenticalTo");
        assert_eq!(swhid.resource_type.as_deref(), Some("software"));
    }

    #[test]
    fn keywords_from_title_dedups_and_caps() {
        let kws = keywords_from_title("Alpha beta Alpha gamma delta epsilon zeta theta iota");
        // "alpha" should appear once despite two occurrences; max 8
        assert!(kws.len() <= 8);
        assert_eq!(kws.iter().filter(|k| k.as_str() == "alpha").count(), 1);
        // "beta" len=4 > 3, should be included
        assert!(kws.contains(&"beta".to_string()));
    }
}
