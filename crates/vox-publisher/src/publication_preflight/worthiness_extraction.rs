use super::*;
/// Parse `metadata_json.scientific_publication` if present.
pub fn parse_scientific_from_metadata_json(
    metadata_json: Option<&str>,
) -> Result<Option<ScientificPublicationMetadata>, String> {
    let Some(raw) = metadata_json else {
        return Ok(None);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let root: serde_json::Value =
        serde_json::from_str(trimmed).map_err(|e| format!("metadata_json: {e}"))?;
    let Some(block) = root.get(METADATA_KEY_SCIENTIFIC) else {
        return Ok(None);
    };
    serde_json::from_value(block.clone()).map_err(|e| format!("scientific_publication: {e}"))
}
pub(super) fn clamp01(x: f64) -> f64 {
    x.clamp(0.0, 1.0)
}
/// Build [`crate::publication_worthiness::WorthinessInputs`] from manifest + preflight (automated proxy only).
///
/// This is intentionally conservative: benchmark-style fields are weakly informed, and
/// [`crate::publication_worthiness::WorthinessInputs::meaningful_advance`] is always `false`.
#[must_use]
pub fn worthiness_inputs_from_manifest_and_preflight(
    manifest: &PublicationManifest,
    report: &PreflightReport,
    heuristics: Option<&crate::scientia_heuristics::ScientiaHeuristics>,
) -> crate::publication_worthiness::WorthinessInputs {
    let h_fallback = crate::scientia_heuristics::ScientiaHeuristics::default();
    let h = heuristics.unwrap_or(&h_fallback);
    let r = (report.readiness_score as f64 / 100.0).clamp(0.0, 1.0);

    let mut red_line_violation_ids: Vec<String> = Vec::new();
    for f in &report.findings {
        if f.severity != PreflightSeverity::Error {
            continue;
        }
        match f.code {
            "citations_json_invalid"
            | "metadata_json_invalid"
            | "scientific_metadata_invalid"
            | "author_primary_mismatch"
                if !red_line_violation_ids
                    .iter()
                    .any(|x| x == "claim_evidence_mismatch") =>
            {
                red_line_violation_ids.push("claim_evidence_mismatch".to_string());
            }
            _ => {}
        }
    }

    // Phase D wiring — prefer the *measured* claim support ratio over the
    // citation-presence heuristic when the most recent
    // `publication-extract-claims` run wrote a summary into
    // `metadata_json.scientia_evidence.extracted_claims`. A measured
    // value of `support_ratio = supported / total_atomic` is a direct
    // (not derived) signal for `claim_evidence_coverage`.
    let measured_claim_coverage: Option<f64> =
        crate::scientia_evidence::parse_scientia_evidence(manifest.metadata_json.as_deref())
            .and_then(|e| e.extracted_claims)
            .filter(|s| s.total_atomic > 0)
            .map(|s| s.support_ratio());

    let citation_score = if let Some(measured) = measured_claim_coverage {
        clamp01(measured)
    } else {
        match manifest.citations_json.as_deref() {
            Some(raw) if !raw.trim().is_empty() => {
                match serde_json::from_str::<serde_json::Value>(raw) {
                    Ok(v) if v.as_array().is_some_and(|a| !a.is_empty()) => {
                        clamp01(0.55 + 0.45 * r)
                    }
                    Ok(_) => clamp01(0.35 + 0.35 * r),
                    Err(_) => clamp01(0.3 * r),
                }
            }
            _ => clamp01(0.35 * r),
        }
    };

    let sci = parse_scientific_from_metadata_json(manifest.metadata_json.as_deref())
        .ok()
        .flatten();

    let repro_score = match sci.as_ref().and_then(|s| s.reproducibility.as_ref()) {
        Some(rep)
            if rep
                .code_repository_url
                .as_ref()
                .is_some_and(|s| !s.trim().is_empty())
                || rep
                    .data_repository_url
                    .as_ref()
                    .is_some_and(|s| !s.trim().is_empty())
                || rep
                    .artifact_checksum_note
                    .as_ref()
                    .is_some_and(|s| !s.trim().is_empty()) =>
        {
            clamp01(0.62 + 0.33 * r)
        }
        _ => clamp01(0.42 * r),
    };

    let meta_score = match &sci {
        Some(s) => {
            let mut pts = 0u32;
            const MAX: u32 = 5;
            if !s.authors.is_empty() {
                pts += 1;
            }
            if s.license_spdx
                .as_ref()
                .is_some_and(|x| !x.trim().is_empty())
            {
                pts += 1;
            }
            if s.funding_statement
                .as_ref()
                .is_some_and(|x| !x.trim().is_empty())
            {
                pts += 1;
            }
            if s.competing_interests_statement
                .as_ref()
                .is_some_and(|x| !x.trim().is_empty())
            {
                pts += 1;
            }
            if s.reproducibility.as_ref().is_some_and(|rep| {
                rep.code_repository_url
                    .as_ref()
                    .is_some_and(|x| !x.trim().is_empty())
                    || rep
                        .data_repository_url
                        .as_ref()
                        .is_some_and(|x| !x.trim().is_empty())
                    || rep
                        .artifact_checksum_note
                        .as_ref()
                        .is_some_and(|x| !x.trim().is_empty())
            }) {
                pts += 1;
            }
            clamp01(0.15 + (f64::from(pts) / f64::from(MAX)) * 0.75 * (0.5 + 0.5 * r))
        }
        None => clamp01(0.2 * r),
    };

    let ai_disclosure = match sci.as_ref().and_then(|s| s.ethics_and_impact.as_ref()) {
        Some(e)
            if e.broader_impact_statement
                .as_ref()
                .is_some_and(|x| !x.trim().is_empty())
                || e.irb_or_human_subjects_note
                    .as_ref()
                    .is_some_and(|x| !x.trim().is_empty()) =>
        {
            1.0
        }
        _ => 0.85,
    };

    let before_after = clamp01(0.35 * r);

    let abstract_boost = manifest.abstract_text.as_deref().map_or(0.0, |s| {
        if s.trim().is_empty() {
            0.0
        } else {
            h.worthiness_epistemic_abstract_boost
        }
    });

    let epistemic =
        clamp01(h.worthiness_epistemic_base + h.worthiness_epistemic_r_coef * r + abstract_boost);
    let novelty = clamp01(h.worthiness_novelty_base + h.worthiness_novelty_r_coef * r);
    let reliability = clamp01(0.48 + 0.47 * r);

    let mut inputs = crate::publication_worthiness::WorthinessInputs {
        red_line_violation_ids,
        repeated_unresolved_contradiction: false,
        claim_evidence_coverage: citation_score,
        artifact_replayability: repro_score,
        // Phase B: measured value is written back by `vox-replay-runner` after
        // a sandboxed re-execution of the manifest's RO-Crate `mainEntity`.
        // Absent here means downstream gates fall back to the declared score.
        artifact_replayability_measured: None,
        before_after_pair_integrity: before_after,
        metadata_completeness: meta_score,
        ai_disclosure_compliance: ai_disclosure,
        epistemic,
        reproducibility: repro_score,
        novelty,
        reliability,
        metadata_policy: meta_score,
        meaningful_advance: false,
    };
    if let Some(evidence) =
        crate::scientia_evidence::parse_scientia_evidence(manifest.metadata_json.as_deref())
    {
        inputs = crate::scientia_evidence::apply_scientia_evidence(inputs, &evidence, h);
    }
    if let Some(bundle) = crate::scientia_prior_art::parse_novelty_bundle_from_metadata_json(
        manifest.metadata_json.as_deref(),
    ) {
        let _prior_notes = crate::publication_worthiness::apply_prior_art_to_worthiness_inputs(
            &mut inputs,
            Some(&bundle),
            Some(h),
        );
    }
    inputs
}
