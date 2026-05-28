//! Phase 10 governance: negative-result enforcer, cost dashboard, COI declaration.

use serde::{Deserialize, Serialize};

// ─── NegativeResultEnforcer ──────────────────────────────────────────────────

/// Refuses to release a quarterly Atlas when positive findings ≥ threshold and
/// no null-result is present in the same window.
///
/// Default: `positive_threshold = 3` (matches strategic plan §10).
pub struct NegativeResultEnforcer {
    pub positive_threshold: usize,
}

impl Default for NegativeResultEnforcer {
    fn default() -> Self {
        Self {
            positive_threshold: 3,
        }
    }
}

#[derive(Debug)]
pub enum NegativeResultError {
    QuotaNotMet {
        published_positive: usize,
        null_results: usize,
        threshold: usize,
    },
}

impl std::fmt::Display for NegativeResultError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::QuotaNotMet {
                published_positive,
                null_results,
                threshold,
            } => write!(
                f,
                "Atlas release blocked: {published_positive} positive findings published \
                 with {null_results} null-results (need \u{2265}1 when positive count \u{2265} {threshold})"
            ),
        }
    }
}

impl std::error::Error for NegativeResultError {}

impl NegativeResultEnforcer {
    /// Returns `Ok(())` if the Atlas may be released.
    ///
    /// Blocks if `published_positive >= self.positive_threshold` and `null_results == 0`.
    pub fn check(
        &self,
        published_positive: usize,
        null_results: usize,
    ) -> Result<(), NegativeResultError> {
        if published_positive >= self.positive_threshold && null_results == 0 {
            return Err(NegativeResultError::QuotaNotMet {
                published_positive,
                null_results,
                threshold: self.positive_threshold,
            });
        }
        Ok(())
    }
}

// ─── CostDashboard ───────────────────────────────────────────────────────────

/// Cost tracking published in the Atlas itself (per strategic plan §10).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostDashboard {
    pub total_cost_usd: f64,
    pub findings_count: usize,
    pub extractions_count: usize,
    pub atlas_versions_count: usize,
}

impl CostDashboard {
    pub fn cost_per_finding(&self) -> f64 {
        if self.findings_count == 0 {
            return 0.0;
        }
        self.total_cost_usd / self.findings_count as f64
    }

    pub fn cost_per_extraction(&self) -> f64 {
        if self.extractions_count == 0 {
            return 0.0;
        }
        self.total_cost_usd / self.extractions_count as f64
    }

    pub fn cost_per_atlas(&self) -> f64 {
        if self.atlas_versions_count == 0 {
            return 0.0;
        }
        self.total_cost_usd / self.atlas_versions_count as f64
    }

    /// Serialize for embedding in the Atlas manifest JSON.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "total_cost_usd": self.total_cost_usd,
            "findings_count": self.findings_count,
            "extractions_count": self.extractions_count,
            "atlas_versions_count": self.atlas_versions_count,
            "cost_per_finding_usd": self.cost_per_finding(),
            "cost_per_extraction_usd": self.cost_per_extraction(),
            "cost_per_atlas_usd": self.cost_per_atlas(),
        })
    }
}

// ─── CoiDeclaration ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConflictRelationship {
    FinancialInterest,
    ConsultancyOrAdvisory,
    GrantFunding,
    Employment,
    PersonalRelationship,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictOfInterest {
    pub party: String,
    pub relationship: ConflictRelationship,
    pub description: String,
}

/// ICMJE-format COI declaration (<https://www.icmje.org/conflicts-of-interest/>).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoiDeclaration {
    /// Study or Atlas ID this declaration covers.
    pub study_id: String,
    pub conflicts: Vec<ConflictOfInterest>,
    /// Always "2019" for the current ICMJE uniform disclosure form version.
    pub icmje_version: String,
}

impl CoiDeclaration {
    /// Create a declaration with no conflicts.
    pub fn none(study_id: String) -> Self {
        Self {
            study_id,
            conflicts: Vec::new(),
            icmje_version: "2019".to_string(),
        }
    }

    pub fn add_conflict(&mut self, conflict: ConflictOfInterest) {
        self.conflicts.push(conflict);
    }

    pub fn has_conflicts(&self) -> bool {
        !self.conflicts.is_empty()
    }

    /// Serialize for embedding in Atlas metadata (disclosures.json).
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "icmje_version": self.icmje_version,
            "study_id": self.study_id,
            "has_conflicts": self.has_conflicts(),
            "conflicts": self.conflicts.iter().map(|c| serde_json::json!({
                "party": c.party,
                "relationship": c.relationship,
                "description": c.description,
            })).collect::<Vec<_>>(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- NegativeResultEnforcer tests ---

    #[test]
    fn enforcer_allows_atlas_with_null_result() {
        let enforcer = NegativeResultEnforcer::default();
        // 3 published positive findings + 1 null result → allowed
        let result = enforcer.check(3, 1);
        assert!(result.is_ok());
    }

    #[test]
    fn enforcer_blocks_when_quota_not_met() {
        let enforcer = NegativeResultEnforcer::default();
        // 3 published positive findings + 0 null results → blocked
        let result = enforcer.check(3, 0);
        assert!(matches!(
            result,
            Err(NegativeResultError::QuotaNotMet {
                published_positive,
                null_results,
                ..
            }) if published_positive == 3 && null_results == 0
        ));
    }

    #[test]
    fn enforcer_allows_fewer_than_threshold() {
        let enforcer = NegativeResultEnforcer::default();
        // Only 2 positive findings → below threshold of 3, no quota required
        let result = enforcer.check(2, 0);
        assert!(result.is_ok());
    }

    // --- CostDashboard tests ---

    #[test]
    fn cost_dashboard_per_finding() {
        let dashboard = CostDashboard {
            total_cost_usd: 150.0,
            findings_count: 5,
            extractions_count: 200,
            atlas_versions_count: 1,
        };
        assert!((dashboard.cost_per_finding() - 30.0).abs() < 1e-9);
        assert!((dashboard.cost_per_extraction() - 0.75).abs() < 1e-9);
        assert!((dashboard.cost_per_atlas() - 150.0).abs() < 1e-9);
    }

    #[test]
    fn cost_dashboard_handles_zero_counts() {
        let dashboard = CostDashboard {
            total_cost_usd: 0.0,
            findings_count: 0,
            extractions_count: 0,
            atlas_versions_count: 0,
        };
        assert_eq!(dashboard.cost_per_finding(), 0.0);
        assert_eq!(dashboard.cost_per_extraction(), 0.0);
        assert_eq!(dashboard.cost_per_atlas(), 0.0);
    }

    #[test]
    fn cost_dashboard_to_json_includes_all_fields() {
        let dashboard = CostDashboard {
            total_cost_usd: 42.5,
            findings_count: 3,
            extractions_count: 100,
            atlas_versions_count: 1,
        };
        let json = dashboard.to_json();
        assert_eq!(json["total_cost_usd"], 42.5);
        assert_eq!(json["findings_count"], 3);
        assert!(json["cost_per_finding_usd"].as_f64().unwrap() > 0.0);
    }

    // --- CoiDeclaration tests ---

    #[test]
    fn coi_declaration_no_conflicts() {
        let decl = CoiDeclaration::none("vox-scientia-2026-q2".into());
        assert!(!decl.has_conflicts());
        let json = decl.to_json();
        assert_eq!(json["has_conflicts"], false);
        assert_eq!(json["icmje_version"], "2019");
    }

    #[test]
    fn coi_declaration_with_conflict() {
        let mut decl = CoiDeclaration::none("study-001".into());
        decl.add_conflict(ConflictOfInterest {
            party: "Provider X Inc.".into(),
            relationship: ConflictRelationship::FinancialInterest,
            description: "Equity holdings in Provider X.".into(),
        });
        assert!(decl.has_conflicts());
        let json = decl.to_json();
        assert_eq!(json["conflicts"].as_array().unwrap().len(), 1);
    }
}
