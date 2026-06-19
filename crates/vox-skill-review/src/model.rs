//! Severity-graded review findings + the overall verdict.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Severity {
    Info,
    Warn,
    Error,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReviewItem {
    pub severity: Severity,
    pub rule: String, // e.g. "frontmatter/missing-description"
    pub message: String,
}

/// Gate-before-listing verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Verdict {
    /// No Error/Critical findings — safe to auto-list at the community tier.
    Pass,
    /// At least one Error/Critical — must escalate to a human reviewer.
    NeedsHuman,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReviewReport {
    pub skill_id: String,
    pub items: Vec<ReviewItem>,
    pub suggested_tags: Vec<String>,
    pub verdict: Verdict,
}

impl ReviewReport {
    /// Verdict from the highest-severity item (gate-before-listing).
    pub fn verdict_for(items: &[ReviewItem]) -> Verdict {
        if items.iter().any(|i| i.severity >= Severity::Error) {
            Verdict::NeedsHuman
        } else {
            Verdict::Pass
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_forces_needs_human() {
        let items = vec![ReviewItem {
            severity: Severity::Error,
            rule: "x".into(),
            message: "m".into(),
        }];
        assert_eq!(ReviewReport::verdict_for(&items), Verdict::NeedsHuman);
    }

    #[test]
    fn warnings_only_pass() {
        let items = vec![ReviewItem {
            severity: Severity::Warn,
            rule: "x".into(),
            message: "m".into(),
        }];
        assert_eq!(ReviewReport::verdict_for(&items), Verdict::Pass);
    }
}
