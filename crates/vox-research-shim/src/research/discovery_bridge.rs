//! Bridge completed research runs into SCIENTIA finding candidates and manifest metadata.

use anyhow::{Context, Result};
use vox_db::Codex;
use vox_db::store::{FindingCandidateClass, FindingCandidateRow, InsertOutcome};
use vox_research_events::schema_types::FindingCandidateConfidence;
use vox_research_events::{
    DiscoverySignal, DiscoverySignalFamily, DiscoverySignalStrength,
    FindingCandidateClass as EventCandidateClass, FindingCandidateV1, SignalProvenance,
};

use super::orchestrator::helpers::fnv1a_hash;
use super::types::ResearchResult;
use super::verifier::Verdict;

/// Producer name stored on `scientia_finding_candidates` rows from the research pipeline.
pub const RESEARCH_PIPELINE_PRODUCER: &str = "research_pipeline";

/// Root key for linked research sessions inside publication `metadata_json`.
pub const METADATA_KEY_SCIENTIA_EVIDENCE: &str = "scientia_evidence";

/// Field under [`METADATA_KEY_SCIENTIA_EVIDENCE`] listing durable session row ids.
pub const METADATA_KEY_RESEARCH_SESSION_IDS: &str = "research_session_ids";

/// Outcome of attempting to persist a finding candidate from a research run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersistFindingOutcome {
    /// Bar not met — nothing written.
    Skipped,
    /// New row inserted.
    Inserted,
    /// Idempotent re-run — row already present.
    AlreadySeen,
}

/// Supported claim ids with confidence above the promotion threshold used by the pipeline gate.
#[must_use]
pub fn supported_claim_ids(result: &ResearchResult) -> Vec<u64> {
    result
        .research_metadata
        .claim_verdicts
        .iter()
        .filter(|verdict| matches!(verdict.verdict, Verdict::Supported) && verdict.confidence > 0.8)
        .map(|verdict| verdict.claim.claim_id)
        .collect()
}

/// Returns true when the run meets the low discovery bar (quality ≥ 50 and ≥ 1 supported claim).
#[must_use]
pub fn meets_discovery_low_bar(result: &ResearchResult) -> bool {
    result.research_metadata.quality_score >= 50 && !supported_claim_ids(result).is_empty()
}

/// Build a SCIENTIA finding candidate from a completed research result.
#[must_use]
pub fn finding_candidate_from_research_result(result: &ResearchResult) -> FindingCandidateV1 {
    let session_id = result.research_metadata.session_id;
    let quality_score = result.research_metadata.quality_score;
    let supported = supported_claim_ids(result);
    let now_ms = now_ms_i64();

    let signal = DiscoverySignal {
        code: "research_pipeline.supported_claims".to_string(),
        summary: format!(
            "{} supported claims from research run {}",
            supported.len(),
            session_id
        ),
        strength: DiscoverySignalStrength::Supporting,
        family: DiscoverySignalFamily::FindingCandidateSignal,
        source_ref: Some(format!("research-session:{session_id}")),
        provenance: SignalProvenance {
            origin: Some("vox-research-shim.research_pipeline".to_string()),
            repo_path: None,
            metric_type: Some("supported_claims".to_string()),
            run_id: Some(session_id.to_string()),
            recorded_at_ms: Some(now_ms),
            digest: Some(format!("{:016x}", fnv1a_hash(&result.answer))),
        },
    };

    FindingCandidateV1 {
        schema_version: 1,
        candidate_id: format!("finding-{session_id}-{:016x}", fnv1a_hash(&result.answer)),
        candidate_class: EventCandidateClass::Other,
        internal_signals: vec![signal],
        created_at_ms: now_ms,
        publication_id: None,
        title_hint: None,
        novelty_evidence_bundle_id: None,
        worthiness_decision_ref: Some(format!("research-quality-score:{quality_score}")),
        confidence: Some(FindingCandidateConfidence {
            signal_strength: Some((quality_score as f64 / 100.0).clamp(0.0, 1.0)),
            contradiction_risk: None,
            reproducibility_support: Some((supported.len() as f64 / 10.0).min(1.0)),
        }),
        updated_at_ms: None,
    }
}

/// Serialize publication manifest metadata linking one or more research session ids.
#[must_use]
pub fn build_manifest_metadata_json(session_ids: &[i64]) -> String {
    serde_json::json!({
        METADATA_KEY_SCIENTIA_EVIDENCE: {
            METADATA_KEY_RESEARCH_SESSION_IDS: session_ids,
        }
    })
    .to_string()
}

/// Parse linked research session ids from manifest `metadata_json`.
#[must_use]
pub fn parse_research_session_ids_from_metadata_json(metadata_json: &str) -> Vec<i64> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(metadata_json) else {
        return Vec::new();
    };
    value
        .get(METADATA_KEY_SCIENTIA_EVIDENCE)
        .and_then(|block| block.get(METADATA_KEY_RESEARCH_SESSION_IDS))
        .and_then(|ids| ids.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_i64())
                .filter(|&id| id > 0)
                .collect()
        })
        .unwrap_or_default()
}

fn finding_row_from_candidate(
    candidate: &FindingCandidateV1,
    session_id: i64,
    manifest_metadata_json: &str,
) -> Result<FindingCandidateRow> {
    let internal_signals_json =
        serde_json::to_string(&candidate.internal_signals).context("serialize internal_signals")?;
    let mut confidence_value = candidate
        .confidence
        .clone()
        .map(serde_json::to_value)
        .transpose()
        .context("serialize confidence")?
        .unwrap_or_else(|| serde_json::json!({}));
    if let Some(obj) = confidence_value.as_object_mut() {
        obj.insert(
            "manifest_metadata_json".to_string(),
            serde_json::Value::String(manifest_metadata_json.to_string()),
        );
        if session_id > 0 {
            obj.insert(
                METADATA_KEY_RESEARCH_SESSION_IDS.to_string(),
                serde_json::json!([session_id]),
            );
        }
    }
    let confidence_json = Some(confidence_value.to_string());

    Ok(FindingCandidateRow {
        candidate_id: candidate.candidate_id.clone(),
        candidate_class: FindingCandidateClass::Other,
        publication_id: candidate.publication_id.clone(),
        title_hint: candidate.title_hint.clone(),
        internal_signals_json,
        novelty_evidence_bundle_id: candidate.novelty_evidence_bundle_id.clone(),
        worthiness_decision_ref: candidate.worthiness_decision_ref.clone(),
        confidence_json,
        repository_id: None,
        producer_name: RESEARCH_PIPELINE_PRODUCER.to_string(),
        signal_fingerprint: candidate.candidate_id.clone(),
        created_at_ms: candidate.created_at_ms,
        updated_at_ms: candidate.created_at_ms,
    })
}

/// Persist a finding candidate when the run meets the discovery low bar.
pub async fn persist_finding_candidate_from_research(
    result: &ResearchResult,
    db: &Codex,
) -> Result<PersistFindingOutcome> {
    if !meets_discovery_low_bar(result) {
        return Ok(PersistFindingOutcome::Skipped);
    }

    let session_id = result.research_metadata.session_id;
    let manifest_metadata_json = build_manifest_metadata_json(std::slice::from_ref(&session_id));
    let candidate = finding_candidate_from_research_result(result);
    let row = finding_row_from_candidate(&candidate, session_id, &manifest_metadata_json)?;

    match db.insert_finding_candidate(&row).await {
        Ok(InsertOutcome::Inserted) => {
            surface_research_discovery_inbox(&candidate, db).await;
            Ok(PersistFindingOutcome::Inserted)
        }
        Ok(InsertOutcome::AlreadySeen) => Ok(PersistFindingOutcome::AlreadySeen),
        Err(e) => Err(anyhow::anyhow!("insert_finding_candidate: {e}")),
    }
}

/// Best-effort: mirror a research finding into `scientia_discovery_inbox` so the
/// GUI inbox and WS `scientia.discovery.surfaced` poller can alert operators.
async fn surface_research_discovery_inbox(candidate: &FindingCandidateV1, db: &Codex) {
    let signal_codes: Vec<String> = candidate
        .internal_signals
        .iter()
        .map(|s| s.code.clone())
        .collect();
    let signal_codes_json = match serde_json::to_string(&signal_codes) {
        Ok(json) => json,
        Err(e) => {
            tracing::warn!(error = %e, "discovery_inbox: serialize research signal_codes failed");
            return;
        }
    };
    let intake_tier = if candidate
        .confidence
        .as_ref()
        .and_then(|c| c.signal_strength)
        .unwrap_or(0.0)
        >= 0.75
    {
        "strong_candidate"
    } else {
        "review_suggested"
    };
    if let Err(e) = db
        .insert_discovery_inbox(
            &candidate.candidate_id,
            candidate.created_at_ms,
            intake_tier,
            &signal_codes_json,
        )
        .await
    {
        tracing::warn!(
            candidate_id = %candidate.candidate_id,
            error = %e,
            "discovery_inbox: research surfacing insert failed"
        );
    }
}

fn now_ms_i64() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::research::claims::Claim;
    use crate::research::types::{ResearchMetadata, RetrievalDiagnostics, RoutingTier};
    use crate::research::verifier::{ClaimVerdict, EvidenceSpan, SpanType};

    fn supported_result(session_id: i64, quality: i32) -> ResearchResult {
        ResearchResult {
            answer: "The system supports durable research artifacts.".to_string(),
            sources: vec![],
            citations: vec![],
            research_metadata: ResearchMetadata {
                session_id,
                duration_ms: 1,
                provider: "test".to_string(),
                routing_tier: RoutingTier::Direct,
                confidence: 0.8,
                subquery_count: 1,
                source_count: 0,
                claim_verdicts: vec![ClaimVerdict {
                    claim: Claim {
                        claim_id: 42,
                        text: "supported claim".to_string(),
                        is_numeric: false,
                        is_recent: false,
                        is_named_event: false,
                    },
                    verdict: Verdict::Supported,
                    confidence: 0.91,
                    supporting_count: 1,
                    contradicting_count: 0,
                    evidence_spans: vec![EvidenceSpan {
                        source_id: 0,
                        span_start: 0,
                        span_end: 10,
                        text: "supported".to_string(),
                        span_type: SpanType::Supporting,
                    }],
                }],
                retrieval_diagnostics: RetrievalDiagnostics::default(),
                quality_score: quality,
                planner_degraded: false,
                competence: None,
                self_verification: None,
                citation_audit: None,
            },
        }
    }

    #[test]
    fn manifest_metadata_json_round_trips_session_ids() {
        let json = build_manifest_metadata_json(&[7, 9]);
        let ids = parse_research_session_ids_from_metadata_json(&json);
        assert_eq!(ids, vec![7, 9]);
    }

    #[test]
    fn finding_candidate_validates_against_schema() {
        let result = supported_result(3, 72);
        let candidate = finding_candidate_from_research_result(&result);
        let schema: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../contracts/scientia/finding-candidate.v1.schema.json"
        )))
        .expect("schema parses");
        let value = serde_json::to_value(&candidate).expect("candidate serializes");
        let validator = jsonschema::validator_for(&schema).expect("schema compiles");
        validator.validate(&value).expect("candidate validates");
    }

    #[test]
    fn low_bar_requires_supported_claim_and_quality() {
        let mut result = supported_result(1, 49);
        assert!(!meets_discovery_low_bar(&result));
        result.research_metadata.quality_score = 50;
        assert!(meets_discovery_low_bar(&result));
        result.research_metadata.claim_verdicts.clear();
        assert!(!meets_discovery_low_bar(&result));
    }

    #[tokio::test]
    async fn persist_skips_below_bar_and_inserts_above() {
        let db = vox_db::VoxDb::connect(vox_db::DbConfig::Memory)
            .await
            .expect("memory db");
        let sid = 42_i64;

        let low = supported_result(sid, 40);
        assert_eq!(
            persist_finding_candidate_from_research(&low, &db)
                .await
                .expect("skip"),
            PersistFindingOutcome::Skipped
        );

        let high = supported_result(sid, 55);
        assert_eq!(
            persist_finding_candidate_from_research(&high, &db)
                .await
                .expect("insert"),
            PersistFindingOutcome::Inserted
        );

        let row = db
            .get_finding_candidate(&finding_candidate_from_research_result(&high).candidate_id)
            .await
            .expect("get")
            .expect("row");
        let manifest_json = build_manifest_metadata_json(&[sid]);
        assert_eq!(
            parse_research_session_ids_from_metadata_json(&manifest_json),
            vec![sid]
        );
        let confidence: serde_json::Value =
            serde_json::from_str(row.confidence_json.as_deref().unwrap_or("{}"))
                .expect("confidence");
        assert_eq!(
            confidence[METADATA_KEY_RESEARCH_SESSION_IDS],
            serde_json::json!([sid])
        );
    }
}
