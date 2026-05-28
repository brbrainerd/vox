//! The gate decision itself.

use serde::{Deserialize, Serialize};

use super::fingerprint::ModelFingerprint;
use super::role::ApproverRole;
use super::venue::VenueCriticPolicy;

/// Recommendation a critic emits in its signed report. Maps to the
/// `critic_reports.recommendation` column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CriticRecommendation {
    /// Critic clears the artifact without notes.
    Approve,
    /// Critic clears the artifact but raises non-blocking notes that should
    /// be surfaced in `next_actions`.
    ApproveWithNotes,
    /// Critic asks for revisions before approving. Does NOT clear the gate.
    Revise,
    /// Critic rejects the artifact. Does NOT clear the gate.
    Reject,
}

impl CriticRecommendation {
    pub fn clears_gate(self) -> bool {
        matches!(self, Self::Approve | Self::ApproveWithNotes)
    }
}

/// One row from `publication_approvers` projected into gate-relevant fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApproverRecord {
    /// Distinct identity (ORCID for humans, critic-id for LLM critics).
    pub approver_id: String,
    pub role: ApproverRole,
    /// Populated only when `role == AuditedLLMCritic`.
    pub critic_fingerprint: Option<ModelFingerprint>,
    /// Populated only when `role == AuditedLLMCritic`.
    pub critic_recommendation: Option<CriticRecommendation>,
}

/// Inputs to the gate evaluator.
#[derive(Debug, Clone, PartialEq)]
pub struct GateInputs<'a> {
    pub approvers: &'a [ApproverRecord],
    /// Model fingerprints of every artifact-side model whose output
    /// contributed to the publication content (extractor, novelty retrieval,
    /// claim verifier, etc.). The gate excludes any critic whose
    /// fingerprint collides with one of these.
    pub artifact_model_fingerprints: &'a [ModelFingerprint],
    /// Venue's critic policy.
    pub venue_policy: VenueCriticPolicy,
}

/// Reason code for the gate's outcome. Stable strings; persisted in
/// `publication_status_events.code` when the gate fires.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateReason {
    /// ≥2 distinct human approvers — gate cleared via the human path.
    TwoHumans,
    /// 1 human + 1 fingerprint-distinct critic with an approving
    /// recommendation — gate cleared via the critic path.
    HumanPlusAuditedCritic,
    /// Fewer than 1 human approver. The critic path always requires at
    /// least one human; this code fires when none is present.
    NoHumanApprover,
    /// Only one approver overall (human or critic) and not enough to clear.
    InsufficientApprovers,
    /// Venue forbids LLM-critic approvals and only ≤1 human is present.
    VenueForbidsCriticAndOnlyOneHuman,
    /// A critic's fingerprint matched an artifact-side fingerprint —
    /// blocks the GPT-4-grades-GPT-4 hole.
    CriticFingerprintCollidesWithArtifact,
    /// All critic approvals carried `Revise` or `Reject` recommendations.
    CriticRecommendationNotApproving,
}

impl GateReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::TwoHumans => "two_humans",
            Self::HumanPlusAuditedCritic => "human_plus_audited_critic",
            Self::NoHumanApprover => "no_human_approver",
            Self::InsufficientApprovers => "insufficient_approvers",
            Self::VenueForbidsCriticAndOnlyOneHuman => "venue_forbids_critic_and_only_one_human",
            Self::CriticFingerprintCollidesWithArtifact => {
                "critic_fingerprint_collides_with_artifact"
            }
            Self::CriticRecommendationNotApproving => "critic_recommendation_not_approving",
        }
    }
}

/// Structured outcome of the gate evaluator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GateOutcome {
    pub cleared: bool,
    pub reason: GateReason,
    /// Optional list of approver ids that would have to be acted on next
    /// for the gate to clear (e.g., who still needs to sign).
    pub diagnostics: Vec<String>,
}

/// Evaluate the gate against the supplied inputs.
pub fn evaluate_gate(inputs: &GateInputs<'_>) -> GateOutcome {
    let mut humans: Vec<&ApproverRecord> = Vec::new();
    let mut critics: Vec<&ApproverRecord> = Vec::new();
    for a in inputs.approvers {
        match a.role {
            ApproverRole::Human => humans.push(a),
            ApproverRole::AuditedLLMCritic => critics.push(a),
        }
    }

    // Distinct humans by approver_id.
    let mut seen = std::collections::HashSet::new();
    let distinct_humans: Vec<&&ApproverRecord> = humans
        .iter()
        .filter(|a| seen.insert(a.approver_id.as_str()))
        .collect();

    // Path 1: ≥2 distinct humans.
    if distinct_humans.len() >= 2 {
        return GateOutcome {
            cleared: true,
            reason: GateReason::TwoHumans,
            diagnostics: vec![],
        };
    }

    // Path 2: human + audited critic with a non-colliding fingerprint and
    // an approving recommendation, and venue allows critic.
    let has_one_human = !distinct_humans.is_empty();
    if !has_one_human {
        // Neither path 1 nor 2 can clear: there's at most 0 humans.
        // Disambiguate: critics alone never clear.
        return GateOutcome {
            cleared: false,
            reason: if inputs.approvers.is_empty() {
                GateReason::InsufficientApprovers
            } else {
                GateReason::NoHumanApprover
            },
            diagnostics: vec!["add at least one distinct human approver".into()],
        };
    }
    if !inputs.venue_policy.allows_critic() {
        // Only one human, and venue forbids critic substitution.
        return GateOutcome {
            cleared: false,
            reason: GateReason::VenueForbidsCriticAndOnlyOneHuman,
            diagnostics: vec!["add a second human approver".into()],
        };
    }

    // Scan critics for a viable one.
    let mut had_critic = false;
    let mut had_collision = false;
    let mut had_non_approving = false;
    for c in &critics {
        had_critic = true;
        let Some(fp) = &c.critic_fingerprint else {
            continue;
        };
        if inputs
            .artifact_model_fingerprints
            .iter()
            .any(|af| af.collides_with(fp))
        {
            had_collision = true;
            continue;
        }
        let Some(rec) = c.critic_recommendation else {
            had_non_approving = true;
            continue;
        };
        if !rec.clears_gate() {
            had_non_approving = true;
            continue;
        }
        // Found a viable critic. Gate cleared.
        return GateOutcome {
            cleared: true,
            reason: GateReason::HumanPlusAuditedCritic,
            diagnostics: vec![],
        };
    }

    // No viable critic — return the most specific failure reason.
    let reason = if had_collision {
        GateReason::CriticFingerprintCollidesWithArtifact
    } else if had_non_approving {
        GateReason::CriticRecommendationNotApproving
    } else if had_critic {
        GateReason::InsufficientApprovers
    } else {
        GateReason::InsufficientApprovers
    };
    let mut diagnostics = Vec::new();
    if had_collision {
        diagnostics.push(
            "use a critic whose model fingerprint does not collide with any artifact-side model"
                .into(),
        );
    }
    if had_non_approving {
        diagnostics.push("critic must recommend Approve or ApproveWithNotes".into());
    }
    if !had_critic {
        diagnostics.push("add an audited LLM critic approval, or a second human approver".into());
    }
    GateOutcome {
        cleared: false,
        reason,
        diagnostics,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::fingerprint::ModelFingerprint;

    fn human(id: &str) -> ApproverRecord {
        ApproverRecord {
            approver_id: id.into(),
            role: ApproverRole::Human,
            critic_fingerprint: None,
            critic_recommendation: None,
        }
    }

    fn critic(id: &str, fp: ModelFingerprint, rec: CriticRecommendation) -> ApproverRecord {
        ApproverRecord {
            approver_id: id.into(),
            role: ApproverRole::AuditedLLMCritic,
            critic_fingerprint: Some(fp),
            critic_recommendation: Some(rec),
        }
    }

    fn fp(provider: &str, model: &str) -> ModelFingerprint {
        ModelFingerprint {
            provider: provider.into(),
            model_id: model.into(),
            parameter_count_hint: None,
            training_cutoff: None,
        }
    }

    #[test]
    fn two_distinct_humans_clear_via_human_path() {
        let approvers = vec![human("alice"), human("bob")];
        let inputs = GateInputs {
            approvers: &approvers,
            artifact_model_fingerprints: &[],
            venue_policy: VenueCriticPolicy::Forbidden,
        };
        let out = evaluate_gate(&inputs);
        assert!(out.cleared);
        assert_eq!(out.reason, GateReason::TwoHumans);
    }

    #[test]
    fn duplicate_human_id_does_not_count_twice() {
        let approvers = vec![human("alice"), human("alice")];
        let inputs = GateInputs {
            approvers: &approvers,
            artifact_model_fingerprints: &[],
            venue_policy: VenueCriticPolicy::Allowed,
        };
        let out = evaluate_gate(&inputs);
        assert!(!out.cleared);
    }

    #[test]
    fn human_plus_critic_clears_when_venue_allows_and_fingerprint_distinct() {
        let approvers = vec![
            human("alice"),
            critic(
                "critic-1",
                fp("anthropic", "claude-3-5-sonnet"),
                CriticRecommendation::Approve,
            ),
        ];
        let artifact_fps = vec![fp("openai", "gpt-4o")];
        let inputs = GateInputs {
            approvers: &approvers,
            artifact_model_fingerprints: &artifact_fps,
            venue_policy: VenueCriticPolicy::Allowed,
        };
        let out = evaluate_gate(&inputs);
        assert!(out.cleared);
        assert_eq!(out.reason, GateReason::HumanPlusAuditedCritic);
    }

    #[test]
    fn human_plus_critic_blocked_when_venue_forbids() {
        let approvers = vec![
            human("alice"),
            critic(
                "critic-1",
                fp("anthropic", "claude-3-5-sonnet"),
                CriticRecommendation::Approve,
            ),
        ];
        let inputs = GateInputs {
            approvers: &approvers,
            artifact_model_fingerprints: &[],
            venue_policy: VenueCriticPolicy::Forbidden,
        };
        let out = evaluate_gate(&inputs);
        assert!(!out.cleared);
        assert_eq!(out.reason, GateReason::VenueForbidsCriticAndOnlyOneHuman);
    }

    #[test]
    fn critic_blocked_when_fingerprint_collides_with_artifact_side() {
        let model = fp("anthropic", "claude-3-5-sonnet");
        let approvers = vec![
            human("alice"),
            critic("critic-1", model.clone(), CriticRecommendation::Approve),
        ];
        let artifact_fps = vec![model];
        let inputs = GateInputs {
            approvers: &approvers,
            artifact_model_fingerprints: &artifact_fps,
            venue_policy: VenueCriticPolicy::Allowed,
        };
        let out = evaluate_gate(&inputs);
        assert!(!out.cleared);
        assert_eq!(out.reason, GateReason::CriticFingerprintCollidesWithArtifact);
    }

    #[test]
    fn critic_blocked_when_recommendation_is_revise_or_reject() {
        let approvers = vec![
            human("alice"),
            critic(
                "critic-1",
                fp("acme", "model-z"),
                CriticRecommendation::Revise,
            ),
        ];
        let inputs = GateInputs {
            approvers: &approvers,
            artifact_model_fingerprints: &[],
            venue_policy: VenueCriticPolicy::Allowed,
        };
        let out = evaluate_gate(&inputs);
        assert!(!out.cleared);
        assert_eq!(out.reason, GateReason::CriticRecommendationNotApproving);
    }

    #[test]
    fn critic_alone_never_clears_the_gate() {
        let approvers = vec![critic(
            "critic-1",
            fp("acme", "model-z"),
            CriticRecommendation::Approve,
        )];
        let inputs = GateInputs {
            approvers: &approvers,
            artifact_model_fingerprints: &[],
            venue_policy: VenueCriticPolicy::Allowed,
        };
        let out = evaluate_gate(&inputs);
        assert!(!out.cleared);
        assert_eq!(out.reason, GateReason::NoHumanApprover);
    }

    #[test]
    fn approve_with_notes_clears_gate() {
        let approvers = vec![
            human("alice"),
            critic(
                "critic-1",
                fp("acme", "model-z"),
                CriticRecommendation::ApproveWithNotes,
            ),
        ];
        let inputs = GateInputs {
            approvers: &approvers,
            artifact_model_fingerprints: &[],
            venue_policy: VenueCriticPolicy::Allowed,
        };
        let out = evaluate_gate(&inputs);
        assert!(out.cleared);
    }

    #[test]
    fn empty_approver_list_yields_insufficient_approvers() {
        let inputs = GateInputs {
            approvers: &[],
            artifact_model_fingerprints: &[],
            venue_policy: VenueCriticPolicy::Allowed,
        };
        let out = evaluate_gate(&inputs);
        assert!(!out.cleared);
        assert_eq!(out.reason, GateReason::InsufficientApprovers);
    }

    #[test]
    fn gate_reason_strings_are_stable() {
        // Snapshot test: callers persist these strings in
        // publication_status_events, so they must not drift silently.
        assert_eq!(GateReason::TwoHumans.as_str(), "two_humans");
        assert_eq!(
            GateReason::HumanPlusAuditedCritic.as_str(),
            "human_plus_audited_critic"
        );
        assert_eq!(
            GateReason::CriticFingerprintCollidesWithArtifact.as_str(),
            "critic_fingerprint_collides_with_artifact"
        );
    }
}
