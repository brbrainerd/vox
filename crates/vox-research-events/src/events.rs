//! `ResearchEvent` — the typed event bus for the SCIENTIA signal ladder.
//!
//! All cross-crate SCIENTIA events flow through this enum. Consumers subscribe via
//! `ResearchEventEmitter`; the orchestrator is the primary producer.
//!
//! Signal ladder mapping:
//!   T0 (observation) → `TelemetryObservation`
//!   T1 (aggregate)   → `AggregateComputed`
//!   T2 (atomic claim)→ `ClaimExtracted`, `ClaimVerified`, `NoveltyAssessed`
//!   T3 (finding)     → `FindingCandidateProposed`, `FindingApproved`
//!   T4 (publication) → `PublicationAttempted`, `PublicationSucceeded`, `PublicationFailed`

use serde::{Deserialize, Serialize};

/// Coarse event category for filtering without deserializing the payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResearchEventKind {
    TelemetryObservation,
    AggregateComputed,
    ClaimExtracted,
    ClaimVerified,
    NoveltyAssessed,
    FindingCandidateProposed,
    FindingApproved,
    PreregistrationSubmitted,
    CampaignStarted,
    CampaignAborted,
    PublicationAttempted,
    PublicationSucceeded,
    PublicationFailed,
    RetractionEmitted,
    RightOfReplyWindowOpened,
    RightOfReplyCleared,
}

/// Typed SCIENTIA pipeline event.
///
/// Consumers match on this enum; serde tags allow JSON transport across
/// process boundaries (e.g. orchestrator → publisher).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event_type", rename_all = "PascalCase")]
pub enum ResearchEvent {
    // ── T0 — Telemetry observation ────────────────────────────────────────────
    TelemetryObservation {
        provider: String,
        metric_type: String,
        value: f64,
        session_id: String,
        recorded_at_ms: i64,
    },

    // ── T1 — Aggregate computed ───────────────────────────────────────────────
    AggregateComputed {
        provider: String,
        metric_type: String,
        window_start_ms: i64,
        window_end_ms: i64,
        value: f64,
        sample_count: u64,
        session_id: String,
    },

    // ── T2 — Atomic claim extracted ───────────────────────────────────────────
    ClaimExtracted {
        claim_id: u64,
        text: String,
        verifiability_score: f64,
        session_id: String,
    },

    // ── T2 — Claim verified ───────────────────────────────────────────────────
    ClaimVerified {
        claim_id: u64,
        verdict: String, // "Supported" | "Contradicted" | "Contested" | "Unverified"
        confidence: f64,
        verifier_model: String,
        session_id: String,
    },

    // ── T2 — Novelty assessed ─────────────────────────────────────────────────
    NoveltyAssessed {
        claim_id: u64,
        is_novel: bool,
        /// Highest cosine similarity to prior corpus (SPECTER2).
        max_prior_similarity: f64,
        evidence_conflict: bool,
        session_id: String,
    },

    // ── T3 — Finding candidate proposed ──────────────────────────────────────
    FindingCandidateProposed {
        finding_id: String,
        claim_ids: Vec<u64>,
        worthiness_score: f64,
        session_id: String,
        /// Full `FindingCandidateV1` JSON when produced by the research pipeline.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        finding_candidate: Option<serde_json::Value>,
    },

    // ── T3 — Finding approved by dual-approver gate ───────────────────────────
    FindingApproved {
        finding_id: String,
        approved_by: Vec<String>,
        session_id: String,
    },

    // ── Pre-registration ──────────────────────────────────────────────────────
    PreregistrationSubmitted {
        prereg_id: String,
        /// SHA-256 of the hypothesis field (tamper evidence without full body).
        hypothesis_digest: String,
    },

    // ── Campaign lifecycle ────────────────────────────────────────────────────
    CampaignStarted {
        campaign_id: String,
        prereg_id: String,
        cost_cap_usd: f64,
    },
    CampaignAborted {
        campaign_id: String,
        reason: String,
        cost_incurred_usd: f64,
    },

    // ── T4 — Publication ──────────────────────────────────────────────────────
    PublicationAttempted {
        manifest_id: String,
        venue: String,
        finding_ids: Vec<String>,
    },
    PublicationSucceeded {
        manifest_id: String,
        doi: Option<String>,
        nanopub_uris: Vec<String>,
    },
    PublicationFailed {
        manifest_id: String,
        error: String,
    },

    // ── Right-of-reply ────────────────────────────────────────────────────────
    RightOfReplyWindowOpened {
        manifest_id: String,
        providers_notified: Vec<String>,
        deadline_ms: i64,
    },
    RightOfReplyCleared {
        manifest_id: String,
        replies_received: u32,
    },

    // ── Retraction ────────────────────────────────────────────────────────────
    RetractionEmitted {
        target_doi: String,
        reason: String,
        nanopub_uri: String,
    },
}

impl ResearchEvent {
    pub fn kind(&self) -> ResearchEventKind {
        match self {
            Self::TelemetryObservation { .. } => ResearchEventKind::TelemetryObservation,
            Self::AggregateComputed { .. } => ResearchEventKind::AggregateComputed,
            Self::ClaimExtracted { .. } => ResearchEventKind::ClaimExtracted,
            Self::ClaimVerified { .. } => ResearchEventKind::ClaimVerified,
            Self::NoveltyAssessed { .. } => ResearchEventKind::NoveltyAssessed,
            Self::FindingCandidateProposed { .. } => ResearchEventKind::FindingCandidateProposed,
            Self::FindingApproved { .. } => ResearchEventKind::FindingApproved,
            Self::PreregistrationSubmitted { .. } => ResearchEventKind::PreregistrationSubmitted,
            Self::CampaignStarted { .. } => ResearchEventKind::CampaignStarted,
            Self::CampaignAborted { .. } => ResearchEventKind::CampaignAborted,
            Self::PublicationAttempted { .. } => ResearchEventKind::PublicationAttempted,
            Self::PublicationSucceeded { .. } => ResearchEventKind::PublicationSucceeded,
            Self::PublicationFailed { .. } => ResearchEventKind::PublicationFailed,
            Self::RightOfReplyWindowOpened { .. } => ResearchEventKind::RightOfReplyWindowOpened,
            Self::RightOfReplyCleared { .. } => ResearchEventKind::RightOfReplyCleared,
            Self::RetractionEmitted { .. } => ResearchEventKind::RetractionEmitted,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn research_event_kind_round_trips() {
        let evt = ResearchEvent::ClaimExtracted {
            claim_id: 42,
            text: "latency increased".to_string(),
            verifiability_score: 0.85,
            session_id: "sess-001".to_string(),
        };
        let json = serde_json::to_string(&evt).unwrap();
        let back: ResearchEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(
            back,
            ResearchEvent::ClaimExtracted { claim_id: 42, .. }
        ));
    }

    #[test]
    fn preregistration_submitted_event_serializes() {
        let evt = ResearchEvent::PreregistrationSubmitted {
            prereg_id: "np:test".to_string(),
            hypothesis_digest: "sha256:abc".to_string(),
        };
        let json = serde_json::to_string(&evt).unwrap();
        assert!(json.contains("PreregistrationSubmitted"));
    }

    #[test]
    fn event_kind_matches_variant() {
        assert_eq!(
            ResearchEvent::ClaimExtracted {
                claim_id: 1,
                text: "t".to_string(),
                verifiability_score: 0.5,
                session_id: "s".to_string(),
            }
            .kind(),
            ResearchEventKind::ClaimExtracted
        );
    }
}
