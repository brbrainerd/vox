//! Approver roles.

use serde::{Deserialize, Serialize};

/// Discriminates the *kind* of approver. Persisted as the
/// `publication_approvers.approver_role` TEXT column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ApproverRole {
    /// Identified human reviewer; their ORCID and signature are recorded
    /// upstream.
    #[serde(rename = "human")]
    Human,
    /// Audited LLM critic. Per the worthiness rubric, must:
    /// - sign approvals under an ORCID-distinct critic identity,
    /// - be auto-disclosed in `AiDisclosureBlock` with the CRediT role
    ///   `Validation` only (never `Investigation` or
    ///   `Writing — original draft`),
    /// - use a model architecturally distinct from any used in the
    ///   artifact pipeline (enforced by [`crate::ModelFingerprint`]).
    ///
    /// Explicit `rename` keeps the serialized form aligned with
    /// [`ApproverRole::as_sql`] — serde's default snake_case derivation
    /// would emit `audited_l_l_m_critic`, which diverges from the DB
    /// column value.
    #[serde(rename = "audited_llm_critic")]
    AuditedLLMCritic,
}

impl ApproverRole {
    pub fn as_sql(&self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::AuditedLLMCritic => "audited_llm_critic",
        }
    }

    pub fn from_sql(s: &str) -> Option<Self> {
        match s {
            "human" => Some(Self::Human),
            "audited_llm_critic" => Some(Self::AuditedLLMCritic),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_round_trips_through_sql_string() {
        for r in [ApproverRole::Human, ApproverRole::AuditedLLMCritic] {
            assert_eq!(ApproverRole::from_sql(r.as_sql()), Some(r));
        }
        assert_eq!(ApproverRole::from_sql("nope"), None);
    }

    #[test]
    fn role_serializes_as_snake_case_json() {
        assert_eq!(
            serde_json::to_string(&ApproverRole::AuditedLLMCritic).unwrap(),
            "\"audited_llm_critic\""
        );
    }
}
