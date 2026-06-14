//! Domain model for skills inside the ARS harness.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::manifest::{ResourceLimits, SkillKind, TrustLevel};

/// Skill payload used by [`crate::runtime::ArsRuntime`] (distinct from OpenClaw list/import DTOs).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArsSkill {
    /// Stable skill id.
    pub id: String,
    /// Logical namespace (e.g. `vox`, `local`, `openclaw`).
    pub namespace: String,
    /// Display name.
    pub name: String,
    /// Semantic version string.
    pub version: String,
    /// Content-addressable hash when known.
    pub content_hash: String,
    /// Short description.
    pub description: Option<String>,
    /// Author label.
    pub author: Option<String>,
    /// Opaque metadata JSON.
    pub metadata: Value,
    /// Skill kind discriminator.
    pub kind: SkillKind,
    /// Optional markdown / instruction body.
    pub body: Option<String>,
    /// Advisory resource limits (used by the container sandbox runner).
    pub resource_limits: ResourceLimits,
    /// Trust classification — drives isolation tier and approval gate.
    ///
    /// Defaults to [`TrustLevel::Community`] for `namespace == "openclaw"` skills;
    /// callers constructing internal builtins should set [`TrustLevel::Trusted`].
    #[serde(default)]
    pub trust: TrustLevel,
}

impl ArsSkill {
    /// Returns `true` if this skill requires an explicit operator approval
    /// before execution is permitted.
    pub fn requires_approval(&self) -> bool {
        self.trust.requires_approval()
    }

    /// Returns `true` if this skill must execute inside the container sandbox.
    pub fn requires_container(&self) -> bool {
        matches!(self.trust, TrustLevel::Community | TrustLevel::Untrusted)
            || matches!(self.kind, SkillKind::Shell)
    }
}

#[cfg(test)]
mod semcov_wave7_tests {
    #![allow(unused_imports, dead_code)]
    use super::*;
    use crate::manifest::{NetworkPolicy, ResourceLimits, SkillKind, TrustLevel};
    use serde_json::json;

    fn make_skill(kind: SkillKind, trust: TrustLevel) -> ArsSkill {
        ArsSkill {
            id: "test-skill".into(),
            namespace: "local".into(),
            name: "Test Skill".into(),
            version: "1.0.0".into(),
            content_hash: "abc123".into(),
            description: None,
            author: None,
            metadata: json!({}),
            kind,
            body: None,
            resource_limits: ResourceLimits::default(),
            trust,
        }
    }

    // Catches: Trusted Shell skill bypassing container requirement
    // (Shell always requires container regardless of trust level)
    #[test]
    fn shell_skill_requires_container_even_when_trusted() {
        let skill = make_skill(SkillKind::Shell, TrustLevel::Trusted);
        assert!(
            skill.requires_container(),
            "Shell skill must require container even when trust level is Trusted"
        );
    }

    // Catches: Trusted Document skill incorrectly requiring container
    #[test]
    fn trusted_document_skill_does_not_require_container() {
        let skill = make_skill(SkillKind::Document, TrustLevel::Trusted);
        assert!(
            !skill.requires_container(),
            "Trusted Document skill must NOT require container isolation"
        );
    }

    // Catches: Community Document skill silently bypassing container gate
    #[test]
    fn community_document_skill_requires_container() {
        let skill = make_skill(SkillKind::Document, TrustLevel::Community);
        assert!(
            skill.requires_container(),
            "Community skill must always require container, even for Document kind"
        );
    }

    // Catches: Untrusted skill not requiring approval (approval gate bypassed)
    #[test]
    fn untrusted_skill_requires_approval() {
        let skill = make_skill(SkillKind::Tool, TrustLevel::Untrusted);
        assert!(
            skill.requires_approval(),
            "Untrusted skill must require explicit operator approval"
        );
    }

    // Catches: Trusted skill incorrectly being gated by approval
    #[test]
    fn trusted_skill_does_not_require_approval() {
        let skill = make_skill(SkillKind::Tool, TrustLevel::Trusted);
        assert!(
            !skill.requires_approval(),
            "Trusted (internal builtin) skill must not require approval"
        );
    }

    // Catches: SkillKind default being Shell (would incorrectly sandbox document skills)
    #[test]
    fn skill_kind_default_is_document_not_shell() {
        assert_eq!(
            SkillKind::default(),
            SkillKind::Document,
            "SkillKind default must be Document, not Shell"
        );
    }
}
