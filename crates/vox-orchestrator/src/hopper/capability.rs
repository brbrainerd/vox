//! `DeveloperOverride` capability token (Hp-T4).
//!
//! Only code with access to `DeveloperOverrideMint` (the sealed minter, held
//! by the dashboard and the CLI) can produce this token. The hopper's
//! `reprioritize` method refuses to accept a plain `TaskPriority` argument
//! without one — it requires `(TaskPriority, DeveloperOverride)`.
//!
//! The design mirrors `vox-orchestrator-cap-mint` for VCS capabilities.

/// A token proving the holder is authorized to override the orchestrator's
/// classified priority. Constructed only via `DeveloperOverrideMint::mint`.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct DeveloperOverride {
    pub actor: String,
    pub reason: String,
    pub audit_id: String,
}

/// The minter. Hold this to be able to produce `DeveloperOverride` tokens.
/// Mint one via `DeveloperOverrideMint::new()` in the dashboard or CLI.
pub struct DeveloperOverrideMint(());

impl DeveloperOverrideMint {
    /// Construct the minter. This is unrestricted at the type level because
    /// we don't have the full P3-T6 sealed-trait facade yet; callers are
    /// expected to be the dashboard action handlers and the CLI only.
    pub fn new() -> Self {
        Self(())
    }

    /// Produce a `DeveloperOverride` token for the given actor/reason pair.
    /// The `audit_id` should be the ID returned by `AuditWriter::record`.
    pub fn mint(
        &self,
        actor: impl Into<String>,
        reason: impl Into<String>,
        audit_id: impl Into<String>,
    ) -> DeveloperOverride {
        DeveloperOverride {
            actor: actor.into(),
            reason: reason.into(),
            audit_id: audit_id.into(),
        }
    }
}

impl Default for DeveloperOverrideMint {
    fn default() -> Self {
        Self::new()
    }
}
