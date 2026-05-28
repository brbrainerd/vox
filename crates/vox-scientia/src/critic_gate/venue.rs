//! Per-venue policy for whether an LLM critic can substitute for a second
//! human approver.
//!
//! Mirrors the `allows_llm_critic: bool` flag added to each row of
//! `contracts/scientia/venue-catalog.v1.yaml` (Phase E config). The gate
//! refuses the critic path when the venue's policy is `Forbidden`.

use serde::{Deserialize, Serialize};

/// Per-venue critic policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VenueCriticPolicy {
    /// The venue accepts an audited-LLM-critic approval in lieu of the
    /// second human approver.
    Allowed,
    /// The venue does NOT accept critic approvals; the gate requires
    /// ≥2 distinct human approvers.
    Forbidden,
}

impl VenueCriticPolicy {
    pub fn allows_critic(self) -> bool {
        matches!(self, Self::Allowed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowed_means_allows_critic() {
        assert!(VenueCriticPolicy::Allowed.allows_critic());
    }

    #[test]
    fn forbidden_means_does_not_allow_critic() {
        assert!(!VenueCriticPolicy::Forbidden.allows_critic());
    }
}
