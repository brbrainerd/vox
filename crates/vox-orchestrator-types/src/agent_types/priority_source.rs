//! `PrioritySource` typed partial order for the unified task hopper (Hp-T3).
//!
//! Encodes *who* set a priority so the orchestrator can enforce the invariant:
//!
//! > **`Developer` dominates `Orchestrator` dominates `LearningPolicy`.**
//!
//! A priority decorated with `PrioritySource::Developer` may only be mutated by
//! a caller that presents a `DeveloperOverride` capability token (Hp-T4). Automated
//! scheduling policies may only set or mutate priorities at or below their own
//! source tier — an `Orchestrator`-sourced policy cannot overwrite a
//! `Developer`-sourced priority.
//!
//! The `Ord` derivation encodes the dominance order by integer discriminant:
//! `LearningPolicy(0) < Orchestrator(1) < Developer(2)`.

use serde::{Deserialize, Serialize};

/// Source of authority that established or last mutated a task priority.
///
/// `Developer` dominates `Orchestrator` dominates `LearningPolicy`. Higher
/// `PrioritySource` values may not be overwritten without explicit authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PrioritySource {
    /// Priority was emitted as an advisory suggestion by an automated
    /// learning / prediction policy. May be overridden by any higher authority.
    LearningPolicy = 0,
    /// Priority was assigned by the orchestrator's scheduling or routing policy.
    /// May be overridden by a developer action but not by a learning policy.
    Orchestrator = 1,
    /// Priority was explicitly set by a human developer via the CLI or dashboard.
    /// May only be mutated by another developer action (requires `DeveloperOverride`
    /// capability token, see Hp-T4).
    Developer = 2,
}

impl PrioritySource {
    /// Returns `true` if `self` is authoritative over `other` — i.e. `self`
    /// may overwrite a priority that was set by `other`.
    ///
    /// A source may always overwrite itself (e.g. two consecutive developer
    /// overrides are both legal).
    #[must_use]
    pub fn dominates(self, other: Self) -> bool {
        self >= other
    }

    /// Human-readable name for telemetry / logging.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LearningPolicy => "learning_policy",
            Self::Orchestrator => "orchestrator",
            Self::Developer => "developer",
        }
    }
}

impl std::fmt::Display for PrioritySource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn developer_dominates_orchestrator_and_learning_policy() {
        assert!(PrioritySource::Developer.dominates(PrioritySource::Orchestrator));
        assert!(PrioritySource::Developer.dominates(PrioritySource::LearningPolicy));
    }

    #[test]
    fn orchestrator_dominates_learning_policy_not_developer() {
        assert!(PrioritySource::Orchestrator.dominates(PrioritySource::LearningPolicy));
        assert!(!PrioritySource::Orchestrator.dominates(PrioritySource::Developer));
    }

    #[test]
    fn learning_policy_dominates_only_itself() {
        assert!(PrioritySource::LearningPolicy.dominates(PrioritySource::LearningPolicy));
        assert!(!PrioritySource::LearningPolicy.dominates(PrioritySource::Orchestrator));
        assert!(!PrioritySource::LearningPolicy.dominates(PrioritySource::Developer));
    }

    #[test]
    fn self_dominates_self() {
        assert!(PrioritySource::Developer.dominates(PrioritySource::Developer));
        assert!(PrioritySource::Orchestrator.dominates(PrioritySource::Orchestrator));
    }

    #[test]
    fn ord_encodes_dominance() {
        assert!(PrioritySource::Developer > PrioritySource::Orchestrator);
        assert!(PrioritySource::Developer > PrioritySource::LearningPolicy);
        assert!(PrioritySource::Orchestrator > PrioritySource::LearningPolicy);
    }

    #[test]
    fn serde_roundtrip() {
        let src = PrioritySource::Developer;
        let json = serde_json::to_string(&src).unwrap();
        assert_eq!(json, r#""developer""#);
        let back: PrioritySource = serde_json::from_str(&json).unwrap();
        assert_eq!(back, src);

        let json2 = serde_json::to_string(&PrioritySource::LearningPolicy).unwrap();
        assert_eq!(json2, r#""learning_policy""#);
    }
}
