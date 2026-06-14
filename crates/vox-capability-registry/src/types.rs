//! Core capability registry types.

/// Whether a capability is exposed to Mens chat tool lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopuliExposure {
    /// Advertise when an in-process executor also implements the MCP tool name.
    Auto,
    /// Never advertise to chat.
    Off,
}

/// How to invoke a capability (MCP name, etc.).
#[derive(Debug, Clone)]
pub struct InvocationForms {
    /// MCP tool id (e.g. `vox_oratio_transcribe`).
    pub mcp_tool: Option<String>,
}

/// One logical capability (may map to an MCP tool name).
#[derive(Debug, Clone)]
pub struct CapabilityDescriptor {
    /// Stable id for parameters lookup (e.g. `oratio.transcribe`).
    pub capability_id: String,
    /// Human-readable description for LLM tool lists.
    pub description: String,
    /// Chat exposure policy.
    pub populi_exposure: PopuliExposure,
    /// Invocation mapping (MCP name, …).
    pub invocation_forms: InvocationForms,
}

/// Full registry (extend with new capabilities as executors gain parity).
#[derive(Debug, Clone)]
pub struct CapabilityRegistry {
    caps: Vec<CapabilityDescriptor>,
}

impl CapabilityRegistry {
    /// Build a registry from descriptors (used by [`crate::default_registry`]).
    #[must_use]
    pub fn from_descriptors(caps: Vec<CapabilityDescriptor>) -> Self {
        Self { caps }
    }

    /// Capabilities eligible for Mens chat (subject to executor intersection).
    pub fn mens_chat_capabilities(&self) -> impl Iterator<Item = &CapabilityDescriptor> + '_ {
        self.caps
            .iter()
            .filter(|c| c.populi_exposure == PopuliExposure::Auto)
    }
}

#[cfg(test)]
mod semcov_wave9_tests {
    #![allow(unused_imports, dead_code)]
    use super::*;

    fn make_desc(id: &str, exposure: PopuliExposure) -> CapabilityDescriptor {
        CapabilityDescriptor {
            capability_id: id.to_string(),
            description: "test".to_string(),
            populi_exposure: exposure,
            invocation_forms: InvocationForms {
                mcp_tool: Some(id.to_string()),
            },
        }
    }

    // Catches: mens_chat_capabilities including Off-exposure capabilities, leaking
    // internal or admin tools into the Mens chat tool list.
    #[test]
    fn mens_chat_capabilities_excludes_off_exposure() {
        let reg = CapabilityRegistry::from_descriptors(vec![
            make_desc("public-tool", PopuliExposure::Auto),
            make_desc("internal-tool", PopuliExposure::Off),
        ]);
        let visible: Vec<_> = reg.mens_chat_capabilities().collect();
        assert_eq!(visible.len(), 1, "only Auto-exposed caps should be visible");
        assert_eq!(visible[0].capability_id, "public-tool");
    }

    // Catches: from_descriptors with an empty vec panicking or returning a corrupt
    // state instead of a valid empty registry.
    #[test]
    fn empty_registry_has_zero_chat_capabilities() {
        let reg = CapabilityRegistry::from_descriptors(vec![]);
        let count = reg.mens_chat_capabilities().count();
        assert_eq!(count, 0, "empty registry must yield zero chat capabilities");
    }

    // Catches: from_descriptors accidentally deduplicating or dropping duplicate
    // capability_ids rather than preserving all entries as provided.
    #[test]
    fn duplicate_ids_in_registry_both_preserved() {
        let reg = CapabilityRegistry::from_descriptors(vec![
            make_desc("dup-id", PopuliExposure::Auto),
            make_desc("dup-id", PopuliExposure::Auto),
        ]);
        let count = reg.mens_chat_capabilities().count();
        assert_eq!(
            count, 2,
            "duplicate ids must both be preserved (dedup is caller's job)"
        );
    }

    // Catches: mens_chat_capabilities returning Off entries when all caps are Auto,
    // verifying the filter logic is correct direction.
    #[test]
    fn all_auto_registry_all_visible() {
        let descs: Vec<_> = (0..5)
            .map(|i| make_desc(&format!("cap-{i}"), PopuliExposure::Auto))
            .collect();
        let reg = CapabilityRegistry::from_descriptors(descs);
        assert_eq!(reg.mens_chat_capabilities().count(), 5);
    }
}
