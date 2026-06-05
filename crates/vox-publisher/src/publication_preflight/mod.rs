//! Readiness checks for [`crate::publication::PublicationManifest`] before journal or repository submission.

mod operator_status;
pub use operator_status::*;
mod models;
pub use models::*;
mod derivation;
use derivation::*;
mod worthiness_extraction;
use std::collections::BTreeSet;
use std::sync::OnceLock;
pub use worthiness_extraction::*;

use regex::Regex;

use crate::publication::PublicationManifest;
use crate::scientific_metadata::{METADATA_KEY_SCIENTIFIC, ScientificPublicationMetadata};

/// Run checks; `ok` is false when any finding has severity [`PreflightSeverity::Error`].
#[must_use]
pub fn run_preflight(manifest: &PublicationManifest, profile: PreflightProfile) -> PreflightReport {
    run_preflight_with_attention(manifest, profile, None)
}

/// Like [`run_preflight`], with optional [`PreflightAttentionInputs`] (for example DB-backed publish gates).
#[must_use]
pub fn run_preflight_with_attention(
    manifest: &PublicationManifest,
    profile: PreflightProfile,
    attention: Option<PreflightAttentionInputs>,
) -> PreflightReport {
    let mut findings: Vec<PreflightFinding> = Vec::new();

    if manifest.title.trim().is_empty() {
        findings.push(PreflightFinding {
            code: "title_empty",
            severity: PreflightSeverity::Error,
            message: "title must not be empty".to_string(),
        });
    }

    if manifest.author.trim().is_empty() {
        findings.push(PreflightFinding {
            code: "author_empty",
            severity: PreflightSeverity::Error,
            message: "author must not be empty".to_string(),
        });
    }

    if profile == PreflightProfile::NewsInbound
        && manifest
            .source_ref
            .as_deref()
            .unwrap_or("")
            .trim()
            .is_empty()
    {
        findings.push(PreflightFinding {
            code: "source_url_missing",
            severity: PreflightSeverity::Error,
            message: "source_ref (original URL) is required for news_inbound preflight".to_string(),
        });
    }

    if let Some(raw) = manifest.metadata_json.as_deref()
        && !raw.trim().is_empty()
    {
        match serde_json::from_str::<serde_json::Value>(raw) {
            Ok(_) => {}
            Err(e) => findings.push(PreflightFinding {
                code: "metadata_json_invalid",
                severity: PreflightSeverity::Error,
                message: format!("metadata_json is not valid JSON: {e}"),
            }),
        }
    }

    match parse_scientific_from_metadata_json(manifest.metadata_json.as_deref()) {
        Ok(Some(sci)) => {
            for (i, a) in sci.authors.iter().enumerate() {
                if a.name.trim().is_empty() {
                    findings.push(PreflightFinding {
                        code: "scientific_author_name_empty",
                        severity: PreflightSeverity::Error,
                        message: format!("scientific_publication.authors[{i}].name is empty"),
                    });
                }
            }
            if !sci.authors.is_empty() {
                let primary = normalize_person_name(&sci.authors[0].name);
                let top = normalize_person_name(&manifest.author);
                if !primary.is_empty() && !top.is_empty() && primary != top {
                    findings.push(PreflightFinding {
                        code: "author_primary_mismatch",
                        severity: PreflightSeverity::Error,
                        message: format!(
                            "manifest.author {:?} does not match scientific_publication.authors[0].name {:?}",
                            manifest.author, sci.authors[0].name
                        ),
                    });
                }
            }
            if sci
                .license_spdx
                .as_ref()
                .is_none_or(|s| s.trim().is_empty())
            {
                if profile == PreflightProfile::MetadataComplete {
                    findings.push(PreflightFinding {
                        code: "license_required",
                        severity: PreflightSeverity::Error,
                        message: "scientific_publication.license_spdx is required for metadata_complete preflight".to_string(),
                    });
                } else {
                    findings.push(PreflightFinding {
                        code: "license_missing",
                        severity: PreflightSeverity::Warning,
                        message: "scientific_publication.license_spdx is unset (recommended for self-archiving and journals)".to_string(),
                    });
                }
            }
            if profile == PreflightProfile::MetadataComplete && sci.authors.is_empty() {
                findings.push(PreflightFinding {
                    code: "scientific_authors_required",
                    severity: PreflightSeverity::Error,
                    message: "metadata_complete requires at least one scientific_publication.authors entry".to_string(),
                });
            }
            let repro_empty = sci.reproducibility.as_ref().is_none_or(|r| {
                r.code_repository_url
                    .as_ref()
                    .is_none_or(|s| s.trim().is_empty())
                    && r.data_repository_url
                        .as_ref()
                        .is_none_or(|s| s.trim().is_empty())
                    && r.artifact_checksum_note
                        .as_ref()
                        .is_none_or(|s| s.trim().is_empty())
            });
            if repro_empty {
                findings.push(PreflightFinding {
                    code: "reproducibility_sparse",
                    severity: PreflightSeverity::Warning,
                    message: "reproducibility block has no code_repository_url, data_repository_url, or artifact_checksum_note".to_string(),
                });
            }
        }
        Ok(None) => {
            if profile == PreflightProfile::MetadataComplete {
                findings.push(PreflightFinding {
                    code: "scientific_metadata_required",
                    severity: PreflightSeverity::Error,
                    message: format!(
                        "metadata_complete requires `{METADATA_KEY_SCIENTIFIC}` in metadata_json"
                    ),
                });
            } else {
                findings.push(PreflightFinding {
                    code: "scientific_metadata_absent",
                    severity: PreflightSeverity::Warning,
                    message: format!(
                        "no `{METADATA_KEY_SCIENTIFIC}` in metadata_json — add structured authors, license, and reproducibility for publication targets"
                    ),
                });
            }
        }
        Err(e) => findings.push(PreflightFinding {
            code: "scientific_metadata_invalid",
            severity: PreflightSeverity::Error,
            message: e,
        }),
    }

    if profile == PreflightProfile::NewsInbound {
        let has_tags = manifest
            .metadata_json
            .as_deref()
            .and_then(|raw| {
                let v: serde_json::Value = serde_json::from_str(raw).ok()?;
                let tags = v.get("tags")?.as_array()?;
                Some(!tags.is_empty())
            })
            .unwrap_or(false);

        if !has_tags {
            findings.push(PreflightFinding {
                code: "initial_classification_missing",
                severity: PreflightSeverity::Error,
                message: "initial classification (at least one tag) is required for news_inbound preflight".to_string(),
            });
        }
    }

    if manifest
        .abstract_text
        .as_deref()
        .is_none_or(|s| s.trim().is_empty())
    {
        if matches!(
            profile,
            PreflightProfile::MetadataComplete
                | PreflightProfile::ArxivAssist
                | PreflightProfile::NewsInbound
        ) {
            findings.push(PreflightFinding {
                code: "abstract_required",
                severity: PreflightSeverity::Error,
                message: match profile {
                    PreflightProfile::MetadataComplete => {
                        "abstract_text is required for metadata_complete preflight".to_string()
                    }
                    PreflightProfile::ArxivAssist => {
                        "abstract_text is required for arxiv_assist preflight (arXiv submission expects an abstract)"
                            .to_string()
                    }
                    PreflightProfile::NewsInbound => {
                        "abstract_text is required for inbound news ingestion preflight".to_string()
                    }
                    _ => "abstract_text is required".to_string(),
                },
            });
        } else {
            findings.push(PreflightFinding {
                code: "abstract_missing",
                severity: PreflightSeverity::Warning,
                message: "abstract_text is empty (journals and arXiv expect an abstract)"
                    .to_string(),
            });
        }
    }

    if let Some(c) = manifest.citations_json.as_deref() {
        let t = c.trim();
        if !t.is_empty() && serde_json::from_str::<serde_json::Value>(t).is_err() {
            findings.push(PreflightFinding {
                code: "citations_json_invalid",
                severity: PreflightSeverity::Error,
                message: "citations_json is not valid JSON".to_string(),
            });
        }
    }

    if profile == PreflightProfile::ArxivAssist {
        if let Some(abs) = manifest.abstract_text.as_deref()
            && abs.chars().count() > 1920
        {
            findings.push(PreflightFinding {
                code: "arxiv_abstract_too_long",
                severity: PreflightSeverity::Error,
                message: format!(
                    "arXiv abstract exceeds 1,920 chars ({} chars). ArXiv moderation boundary.",
                    abs.chars().count()
                ),
            });
        }
        let tc = manifest.title.chars().count();
        if tc > 100 {
            findings.push(PreflightFinding {
                code: "arxiv_title_long",
                severity: PreflightSeverity::Warning,
                message: format!("arXiv title is unusually long ({tc} chars > 100 soft cap)"),
            });
        }
        findings.push(PreflightFinding {
            code: "arxiv_endorsement_required",
            severity: PreflightSeverity::Warning,
            message: "New arXiv categories require endorsement (institutional email is no longer sufficient passing Jan 2026). Ensure submitting author is endorsed.".to_string(),
        });
        findings.push(PreflightFinding {
            code: "arxiv_ai_disclosure",
            severity: PreflightSeverity::Warning,
            message: "arXiv Feb 2026 policy requires explicit formulation disclosure if AI was used for substantive text generation or structuring.".to_string(),
        });
    }

    if profile == PreflightProfile::DoubleBlind {
        let body = &manifest.body_markdown;
        if email_pattern().is_match(body) {
            findings.push(PreflightFinding {
                code: "double_blind_email_in_body",
                severity: PreflightSeverity::Error,
                message: "email-like pattern in body_markdown — remove for double-blind submission"
                    .to_string(),
            });
        }
        if body.to_ascii_lowercase().contains("orcid.org") {
            findings.push(PreflightFinding {
                code: "double_blind_orcid_url_in_body",
                severity: PreflightSeverity::Error,
                message:
                    "`orcid.org` reference in body_markdown — remove for double-blind submission"
                        .to_string(),
            });
        }
        if orcid_id_pattern().is_match(body) {
            findings.push(PreflightFinding {
                code: "double_blind_orcid_id_in_body",
                severity: PreflightSeverity::Error,
                message:
                    "ORCID identifier pattern in body_markdown — remove for double-blind submission"
                        .to_string(),
            });
        }
    }

    let err_n = findings
        .iter()
        .filter(|f| f.severity == PreflightSeverity::Error)
        .count();
    let warn_n = findings
        .iter()
        .filter(|f| f.severity == PreflightSeverity::Warning)
        .count();
    let mut score: i32 = 100 - (err_n as i32) * 25 - (warn_n as i32) * 10;
    score = score.clamp(0, 100);

    let manual_required = collect_manual_required(manifest, attention.as_ref());
    let next_actions = derive_next_actions(manifest, &findings, &manual_required);
    let confidence = derive_confidence(&findings, &manual_required);
    let destination_readiness = collect_destination_readiness(manifest);

    PreflightReport {
        ok: err_n == 0,
        readiness_score: score as u8,
        findings,
        manual_required,
        next_actions,
        confidence,
        destination_readiness,
        worthiness: None,
    }
}

/// Same as [`run_preflight`], then attaches [`PreflightReport::worthiness`] using `contract`.
#[must_use]
pub fn run_preflight_with_worthiness(
    manifest: &PublicationManifest,
    profile: PreflightProfile,
    contract: &crate::publication_worthiness::PublicationWorthinessContract,
) -> PreflightReport {
    run_preflight_with_worthiness_attention(manifest, profile, contract, None)
}

/// Same as [`run_preflight_with_worthiness`] with contract-driven heuristics from the dynamics seed.
#[must_use]
pub fn run_preflight_with_worthiness_heuristics(
    manifest: &PublicationManifest,
    profile: PreflightProfile,
    contract: &crate::publication_worthiness::PublicationWorthinessContract,
    heuristics: &crate::scientia_heuristics::ScientiaHeuristics,
) -> PreflightReport {
    run_preflight_with_worthiness_attention_heuristics(
        manifest, profile, contract, None, heuristics,
    )
}

/// Like [`run_preflight_with_worthiness`], with optional attention inputs.
#[must_use]
pub fn run_preflight_with_worthiness_attention(
    manifest: &PublicationManifest,
    profile: PreflightProfile,
    contract: &crate::publication_worthiness::PublicationWorthinessContract,
    attention: Option<PreflightAttentionInputs>,
) -> PreflightReport {
    let default = crate::scientia_heuristics::ScientiaHeuristics::default();
    run_preflight_with_worthiness_attention_heuristics(
        manifest, profile, contract, attention, &default,
    )
}

/// Attention + explicit heuristics (SSOT dynamics seed).
#[must_use]
pub fn run_preflight_with_worthiness_attention_heuristics(
    manifest: &PublicationManifest,
    profile: PreflightProfile,
    contract: &crate::publication_worthiness::PublicationWorthinessContract,
    attention: Option<PreflightAttentionInputs>,
    heuristics: &crate::scientia_heuristics::ScientiaHeuristics,
) -> PreflightReport {
    let mut report = run_preflight_with_attention(manifest, profile, attention);
    if let Some(bundle) = crate::scientia_prior_art::parse_novelty_bundle_from_metadata_json(
        manifest.metadata_json.as_deref(),
    ) {
        let max_lex = bundle
            .overlap_summary
            .as_ref()
            .and_then(|s| s.max_lexical_score)
            .unwrap_or(0.0);
        if max_lex >= heuristics.preflight_novelty_high_lex_warn {
            report.findings.push(PreflightFinding {
                code: "novelty_prior_art_high_lexical_overlap",
                severity: PreflightSeverity::Warning,
                message: format!(
                    "Prior-art lexical overlap {max_lex:.2} — review top hits in scientia_novelty_bundle; novelty score may be capped in worthiness."
                ),
            });
        }
    }
    if matches!(profile, PreflightProfile::DoubleBlind) {
        let venue_notes = crate::publication_worthiness::machine_venue_profile_violations(
            contract,
            "tmlr_double_blind",
            &report,
        );
        for note in venue_notes {
            report.findings.push(PreflightFinding {
                code: "venue_required_check_failed",
                severity: PreflightSeverity::Error,
                message: note,
            });
        }
    }
    let inputs = worthiness_inputs_from_manifest_and_preflight(manifest, &report, Some(heuristics));
    let eval = crate::publication_worthiness::evaluate_worthiness(contract, &inputs);
    report.worthiness = Some(eval);
    report
}

#[cfg(test)]
mod tests;
