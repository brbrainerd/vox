//! Output writers: recommendations.jsonl, recommendations.md, staging artifacts.

pub mod jsonl;
pub mod markdown;
pub mod artifacts;

use crate::route::RemediationDecision;
use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: &str = "1.0";

/// One row in recommendations.jsonl.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendationRow {
    pub schema_version: String,
    pub decision: RemediationDecision,
}

impl RecommendationRow {
    pub fn new(decision: RemediationDecision) -> Self {
        Self {
            schema_version: SCHEMA_VERSION.to_string(),
            decision,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::route::ArtifactForm;

    fn decision() -> RemediationDecision {
        RemediationDecision {
            cluster_id: "c1".into(),
            member_commit_shas: vec!["deadbeef".into()],
            member_count: 1,
            total_member_tokens: 42,
            artifact_form: ArtifactForm::CiGate,
            confidence: 0.9,
            synthesized_fix_summary: "summary".into(),
            drafted_artifact: None,
            verified: true,
            refutation_note: "note".into(),
        }
    }

    #[test]
    fn new_stamps_schema_version() {
        let row = RecommendationRow::new(decision());
        assert_eq!(row.schema_version, SCHEMA_VERSION);
        assert_eq!(row.schema_version, "1.0");
        assert_eq!(row.decision.cluster_id, "c1");
    }
}
