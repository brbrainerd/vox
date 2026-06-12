//! Deterministic archive-metadata autofill. Pure planner + JSON applier.
//! NEVER overwrites a present value; every fill carries provenance
//! origin "autofill:<rule>". LLM-generated content is out of scope by design.

use serde::{Deserialize, Serialize};

use crate::publication::PublicationManifest;
use crate::scientia_discovery::FieldProvenanceEntry;
use crate::scientific_metadata::{
    METADATA_KEY_SCIENTIFIC, ReproducibilityAttestation, ScientificAuthor,
    ScientificPublicationMetadata,
};

/// Minimal view of the caller-supplied user identity (no DB coupling).
#[derive(Debug, Clone)]
pub struct UserIdentityView {
    pub user_id: String,
    pub orcid_id: Option<String>,
}

/// One field-fill proposed by the autofill planner.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedFill {
    pub field: String,
    pub value: serde_json::Value,
    pub origin: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

/// The full autofill plan: zero or more fills + fields that require human input.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AutofillPlan {
    pub fills: Vec<PlannedFill>,
    pub human_only_remaining: Vec<String>,
}

/// JSON key for the `scientia.*` block inside `metadata_json`.
const METADATA_KEY_SCIENTIA: &str = "scientia";

/// Parse the `scientia.*` sub-object from `metadata_json` (may be absent).
fn parse_scientia_block(meta: Option<&str>) -> serde_json::Map<String, serde_json::Value> {
    meta.and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
        .and_then(|v| {
            v.get(METADATA_KEY_SCIENTIA)
                .and_then(|b| b.as_object().cloned())
        })
        .unwrap_or_default()
}

/// Parse the `scientific_publication` sub-object from `metadata_json`.
fn parse_scientific_block(meta: Option<&str>) -> ScientificPublicationMetadata {
    meta.and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
        .and_then(|v| {
            v.get(METADATA_KEY_SCIENTIFIC)
                .and_then(|b| serde_json::from_value(b.clone()).ok())
        })
        .unwrap_or_default()
}

/// Determine the "today" date string (YYYY-MM-DD) – pure for tests (injectable).
fn today_ymd() -> String {
    let now = chrono::Utc::now().date_naive();
    format!("{}", now.format("%Y-%m-%d"))
}

/// Deterministic autofill planner.
///
/// Returns a plan of proposed field fills. The plan does NOT modify anything;
/// call [`apply_autofill`] to materialise it into a new `metadata_json` string.
///
/// Rules (each fires only when the field is absent or empty):
/// - `publication_date` (scientia block) ← today YYYY-MM-DD, "autofill:today"
/// - `license_spdx` (scientific block)  ← `repo_license_spdx`, "autofill:repo_license"
/// - `authors` (scientific block)       ← `[{name: manifest.author, orcid: identity.orcid_id}]`
///                                         when authors vec is empty, "autofill:user_identity"
/// - `authors[0].orcid` (scientific)    ← identity.orcid_id when author exists but orcid absent
/// - `reproducibility.code_repository_url` ← `git_remote_url`, "autofill:git_remote"
/// - `keywords` (scientia block)        ← derived from title, "autofill:title_keywords"
/// - `abstract_text`                    ← lead paragraph (≥200-char body), "autofill:lead_paragraph"
///
/// `human_only_remaining` captures fields no rule can fill automatically.
#[must_use]
pub fn compute_autofill(
    manifest: &PublicationManifest,
    identity: Option<&UserIdentityView>,
    repo_license_spdx: Option<&str>,
    git_remote_url: Option<&str>,
) -> AutofillPlan {
    let scientia = parse_scientia_block(manifest.metadata_json.as_deref());
    let scientific = parse_scientific_block(manifest.metadata_json.as_deref());

    let mut fills: Vec<PlannedFill> = Vec::new();
    let mut human_only: Vec<String> = Vec::new();

    // ── publication_date ────────────────────────────────────────────────────
    let has_pub_date = scientia
        .get("publication_date")
        .and_then(|v| v.as_str())
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    if !has_pub_date {
        fills.push(PlannedFill {
            field: "publication_date".into(),
            value: serde_json::Value::String(today_ymd()),
            origin: "autofill:today".into(),
            notes: None,
        });
    }

    // ── license_spdx ────────────────────────────────────────────────────────
    if scientific
        .license_spdx
        .as_deref()
        .map(str::trim)
        .is_none_or(|s| s.is_empty())
    {
        if let Some(lic) = repo_license_spdx.filter(|s| !s.trim().is_empty()) {
            fills.push(PlannedFill {
                field: "license_spdx".into(),
                value: serde_json::Value::String(lic.to_string()),
                origin: "autofill:repo_license".into(),
                notes: None,
            });
        }
    }

    // ── authors / authors[0].orcid ───────────────────────────────────────────
    if scientific.authors.is_empty() {
        // No authors at all — bootstrap from manifest.author + identity
        if let Some(ident) = identity {
            fills.push(PlannedFill {
                field: "authors".into(),
                value: serde_json::json!([{
                    "name": manifest.author,
                    "orcid": ident.orcid_id,
                }]),
                origin: "autofill:user_identity".into(),
                notes: None,
            });
        }
    } else {
        // Authors exist but first author has no ORCID
        let first_has_orcid = scientific.authors[0]
            .orcid
            .as_deref()
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false);
        if !first_has_orcid {
            if let Some(ident) = identity {
                if let Some(ref orcid) = ident.orcid_id {
                    fills.push(PlannedFill {
                        field: "authors[0].orcid".into(),
                        value: serde_json::Value::String(orcid.clone()),
                        origin: "autofill:user_identity".into(),
                        notes: None,
                    });
                }
            }
        }
    }

    // ── reproducibility.code_repository_url ─────────────────────────────────
    let has_repo_url = scientific
        .reproducibility
        .as_ref()
        .and_then(|r| r.code_repository_url.as_deref())
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    if !has_repo_url {
        if let Some(url) = git_remote_url.filter(|s| !s.trim().is_empty()) {
            fills.push(PlannedFill {
                field: "reproducibility.code_repository_url".into(),
                value: serde_json::Value::String(url.to_string()),
                origin: "autofill:git_remote".into(),
                notes: None,
            });
        }
    }

    // ── keywords ────────────────────────────────────────────────────────────
    let has_keywords = scientia
        .get("keywords")
        .and_then(|v| v.as_array())
        .map(|a| !a.is_empty())
        .unwrap_or(false);
    if !has_keywords && !manifest.title.trim().is_empty() {
        let kws = crate::zenodo_metadata::keywords_from_title(&manifest.title);
        if !kws.is_empty() {
            fills.push(PlannedFill {
                field: "keywords".into(),
                value: serde_json::to_value(&kws).unwrap_or(serde_json::Value::Array(vec![])),
                origin: "autofill:title_keywords".into(),
                notes: None,
            });
        }
    }

    // ── abstract_text ────────────────────────────────────────────────────────
    let abstract_empty = manifest
        .abstract_text
        .as_deref()
        .map(str::trim)
        .is_none_or(|s| s.is_empty());
    if abstract_empty && manifest.body_markdown.len() >= 200 {
        // Take the first paragraph (up to first blank line), capped at 1500 chars.
        let para = extract_lead_paragraph(&manifest.body_markdown, 1500);
        if !para.is_empty() {
            fills.push(PlannedFill {
                field: "abstract_text".into(),
                value: serde_json::Value::String(para),
                origin: "autofill:lead_paragraph".into(),
                notes: Some("review recommended".into()),
            });
        }
    }

    // ── human_only_remaining ────────────────────────────────────────────────
    if scientific
        .funding_statement
        .as_deref()
        .is_none_or(str::is_empty)
    {
        human_only.push("funding_statement".into());
    }
    if scientific
        .competing_interests_statement
        .as_deref()
        .is_none_or(str::is_empty)
    {
        human_only.push("competing_interests_statement".into());
    }
    if scientific
        .ethics_and_impact
        .as_ref()
        .map(|e| {
            e.broader_impact_statement
                .as_deref()
                .is_none_or(str::is_empty)
        })
        .unwrap_or(true)
    {
        human_only.push("ethics_and_impact".into());
    }

    AutofillPlan {
        fills,
        human_only_remaining: human_only,
    }
}

/// Extract the lead paragraph from `body`, up to `cap` characters.
/// A "paragraph" ends at the first blank line (`\n\n` or `\r\n\r\n`).
fn extract_lead_paragraph(body: &str, cap: usize) -> String {
    let end = body
        .find("\n\n")
        .or_else(|| body.find("\r\n\r\n"))
        .unwrap_or(body.len());
    let para = &body[..end.min(body.len())];
    para.chars().take(cap).collect()
}

/// Apply the autofill plan to `metadata_json`, returning a new JSON string.
///
/// Field writes:
/// - `publication_date`, `keywords`  → `metadata_json.scientia.<field>`
/// - `license_spdx`, `authors`, `authors[0].orcid`, `reproducibility.code_repository_url`
///   → `metadata_json.scientific_publication.<field>`
/// - `abstract_text` → `manifest.abstract_text` (mutated in-place via the provided `&mut`)
///
/// Provenance entries are appended to `metadata_json.scientia.field_provenance`
/// (same shape as [`FieldProvenanceEntry`]).
///
/// # Errors
/// Returns an error if the existing `metadata_json` is malformed JSON.
pub fn apply_autofill(
    metadata_json: Option<&str>,
    abstract_text: &mut Option<String>,
    plan: &AutofillPlan,
) -> anyhow::Result<String> {
    if plan.fills.is_empty() {
        // Nothing to apply; return existing metadata_json or an empty object.
        return Ok(metadata_json
            .filter(|s| !s.trim().is_empty())
            .unwrap_or("{}")
            .to_string());
    }

    let mut root: serde_json::Value = metadata_json
        .filter(|s| !s.trim().is_empty())
        .map(|s| serde_json::from_str(s))
        .transpose()
        .map_err(|e| anyhow::anyhow!("metadata_json parse error: {e}"))?
        .unwrap_or_else(|| serde_json::json!({}));

    // Ensure sub-objects exist.
    if root.get(METADATA_KEY_SCIENTIFIC).is_none() {
        root[METADATA_KEY_SCIENTIFIC] = serde_json::json!({
            "schema_version": 1
        });
    }
    if root.get(METADATA_KEY_SCIENTIA).is_none() {
        root[METADATA_KEY_SCIENTIA] = serde_json::json!({});
    }

    let mut provenance_entries: Vec<serde_json::Value> = root
        .get(METADATA_KEY_SCIENTIA)
        .and_then(|v| v.get("field_provenance"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    for fill in &plan.fills {
        match fill.field.as_str() {
            // ── scientia block ────────────────────────────────────────────
            "publication_date" | "keywords" => {
                root[METADATA_KEY_SCIENTIA][&fill.field] = fill.value.clone();
            }

            // ── scientific_publication block ─────────────────────────────
            "license_spdx" => {
                root[METADATA_KEY_SCIENTIFIC]["license_spdx"] = fill.value.clone();
            }
            "authors" => {
                root[METADATA_KEY_SCIENTIFIC]["authors"] = fill.value.clone();
            }
            "authors[0].orcid" => {
                if let Some(arr) = root[METADATA_KEY_SCIENTIFIC]["authors"].as_array_mut() {
                    if let Some(first) = arr.first_mut() {
                        first["orcid"] = fill.value.clone();
                    }
                }
            }
            "reproducibility.code_repository_url" => {
                // Ensure reproducibility object exists.
                if root[METADATA_KEY_SCIENTIFIC]
                    .get("reproducibility")
                    .is_none()
                {
                    root[METADATA_KEY_SCIENTIFIC]["reproducibility"] = serde_json::json!({});
                }
                root[METADATA_KEY_SCIENTIFIC]["reproducibility"]["code_repository_url"] =
                    fill.value.clone();
            }

            // ── manifest field (out-of-band) ─────────────────────────────
            "abstract_text" => {
                if let Some(s) = fill.value.as_str() {
                    *abstract_text = Some(s.to_string());
                }
            }

            _ => {
                // Unknown field — write into the scientia block as a fallback.
                root[METADATA_KEY_SCIENTIA][&fill.field] = fill.value.clone();
            }
        }

        provenance_entries.push(serde_json::to_value(FieldProvenanceEntry {
            field: fill.field.clone(),
            origin: fill.origin.clone(),
            notes: fill.notes.clone(),
        })?);
    }

    root[METADATA_KEY_SCIENTIA]["field_provenance"] = serde_json::Value::Array(provenance_entries);

    Ok(serde_json::to_string(&root)?)
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn bare_manifest() -> PublicationManifest {
        PublicationManifest {
            publication_id: "test-pub-1".into(),
            content_type: "scientia".into(),
            source_ref: None,
            title: "Deterministic Novelty Detection in Vox".into(),
            author: "Ada Lovelace".into(),
            abstract_text: None,
            body_markdown: "body text".into(),
            citations_json: None,
            metadata_json: None,
        }
    }

    fn identity_with_orcid() -> UserIdentityView {
        UserIdentityView {
            user_id: "local-user".into(),
            orcid_id: Some("https://orcid.org/0000-0001-2345-6789".into()),
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // T1: fills missing date, license, authors, repo url
    // ─────────────────────────────────────────────────────────────────────────
    #[test]
    fn fills_missing_date_license_creators() {
        let manifest = bare_manifest();
        let identity = identity_with_orcid();
        let plan = compute_autofill(
            &manifest,
            Some(&identity),
            Some("MIT"),
            Some("https://github.com/org/repo"),
        );

        let fields: Vec<&str> = plan.fills.iter().map(|f| f.field.as_str()).collect();
        assert!(
            fields.contains(&"publication_date"),
            "expected publication_date fill"
        );
        assert!(
            fields.contains(&"license_spdx"),
            "expected license_spdx fill"
        );
        assert!(fields.contains(&"authors"), "expected authors fill");
        assert!(
            fields.contains(&"reproducibility.code_repository_url"),
            "expected code_repository_url fill"
        );
        assert!(fields.contains(&"keywords"), "expected keywords fill");

        for fill in &plan.fills {
            assert!(
                fill.origin.starts_with("autofill:"),
                "origin must start with 'autofill:'; got {:?}",
                fill.origin
            );
        }

        // Check license value
        let lic = plan
            .fills
            .iter()
            .find(|f| f.field == "license_spdx")
            .unwrap();
        assert_eq!(lic.value, serde_json::Value::String("MIT".into()));

        // Check authors bootstrap
        let auth = plan.fills.iter().find(|f| f.field == "authors").unwrap();
        assert_eq!(auth.value[0]["name"], "Ada Lovelace");
        assert_eq!(
            auth.value[0]["orcid"],
            "https://orcid.org/0000-0001-2345-6789"
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // T2: never overwrites existing values
    // ─────────────────────────────────────────────────────────────────────────
    #[test]
    fn never_overwrites_existing() {
        let existing_sci = ScientificPublicationMetadata {
            license_spdx: Some("Apache-2.0".into()),
            authors: vec![ScientificAuthor {
                name: "Charles Babbage".into(),
                orcid: Some("https://orcid.org/0000-0000-0000-0001".into()),
                ror: None,
                affiliation: None,
            }],
            ..Default::default()
        };
        let metadata_json = serde_json::json!({
            METADATA_KEY_SCIENTIFIC: serde_json::to_value(&existing_sci).unwrap()
        })
        .to_string();

        let mut manifest = bare_manifest();
        manifest.metadata_json = Some(metadata_json);

        let identity = identity_with_orcid();
        let plan = compute_autofill(&manifest, Some(&identity), Some("MIT"), None);

        // No license fill (already Apache-2.0)
        assert!(
            plan.fills.iter().all(|f| f.field != "license_spdx"),
            "must not overwrite existing license"
        );
        // No authors fill (already present with orcid)
        assert!(
            plan.fills.iter().all(|f| f.field != "authors"),
            "must not overwrite existing authors"
        );
        assert!(
            plan.fills.iter().all(|f| f.field != "authors[0].orcid"),
            "must not overwrite existing orcid"
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // T3: abstract_from_lead_paragraph_only_when_long_body
    // ─────────────────────────────────────────────────────────────────────────
    #[test]
    fn abstract_from_lead_paragraph_only_when_long_body() {
        let short_body = "Too short.";
        let mut m_short = bare_manifest();
        m_short.body_markdown = short_body.into();
        let plan_short = compute_autofill(&m_short, None, None, None);
        assert!(
            plan_short.fills.iter().all(|f| f.field != "abstract_text"),
            "must not fill abstract when body < 200 chars"
        );

        let long_body = format!(
            "{}\n\n{}",
            "This is the lead paragraph. ".repeat(10),
            "Second paragraph content here."
        );
        let mut m_long = bare_manifest();
        m_long.body_markdown = long_body.clone();
        let plan_long = compute_autofill(&m_long, None, None, None);
        let abs_fill = plan_long.fills.iter().find(|f| f.field == "abstract_text");
        assert!(
            abs_fill.is_some(),
            "must fill abstract for body >= 200 chars"
        );
        if let Some(fill) = abs_fill {
            // Must not contain the second paragraph
            assert!(
                !fill
                    .value
                    .as_str()
                    .unwrap_or("")
                    .contains("Second paragraph")
            );
            assert_eq!(fill.notes.as_deref(), Some("review recommended"));
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // T4: apply_then_recompute → empty fills (idempotence)
    // ─────────────────────────────────────────────────────────────────────────
    #[test]
    fn apply_then_recompute_is_idempotent() {
        let mut manifest = bare_manifest();
        manifest.body_markdown = "Short.".into(); // keep abstract rule inactive

        let identity = identity_with_orcid();
        let plan = compute_autofill(
            &manifest,
            Some(&identity),
            Some("MIT"),
            Some("https://github.com/org/repo"),
        );
        assert!(!plan.fills.is_empty(), "first run must propose fills");

        // Apply the plan.
        let new_meta = apply_autofill(
            manifest.metadata_json.as_deref(),
            &mut manifest.abstract_text,
            &plan,
        )
        .expect("apply must succeed");
        manifest.metadata_json = Some(new_meta);

        // Re-compute — should be empty now.
        let plan2 = compute_autofill(
            &manifest,
            Some(&identity),
            Some("MIT"),
            Some("https://github.com/org/repo"),
        );
        assert!(
            plan2.fills.is_empty(),
            "second run must produce no fills (idempotence); got: {:?}",
            plan2.fills
        );
    }
}
