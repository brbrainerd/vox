//! Unified policy catalog model (CI gates, language rules, audits).
//!
//! Loaded from `contracts/policy/policy-registry.v1.yaml`. See
//! `docs/superpowers/specs/2026-06-06-unified-policy-registry-and-governance-surface-design.md`.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Top-level catalog document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolicyRegistry {
    pub schema_version: u32,
    #[serde(default)]
    pub policies: Vec<PolicyEntry>,
}

/// One governable policy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolicyEntry {
    pub id: String,
    pub domain: PolicyDomain,
    pub title: String,
    pub group: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub severity: Option<PolicySeverity>,
    #[serde(default)]
    pub blocking: bool,
    #[serde(default)]
    pub runs_on: Vec<String>,
    pub source: PolicySource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub docs: Option<String>,
    #[serde(default = "default_true")]
    pub default_enabled: bool,
    #[serde(default)]
    pub protected: bool,
    #[serde(default = "default_origin")]
    pub origin: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PolicyDomain {
    CiGate,
    AuditCheck,
    CrlGate,
    CodeAuditRule,
    ArchRule,
    WorkflowJob,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PolicySeverity {
    Info,
    Warn,
    Error,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolicySource {
    pub kind: PolicySourceKind,
    #[serde(rename = "ref")]
    pub reference: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PolicySourceKind {
    Pattern,
    Command,
    Guard,
    Subcommand,
    Workflow,
}

fn default_true() -> bool {
    true
}
fn default_origin() -> String {
    "builtin".to_string()
}

/// Error returned when the registry cannot be loaded.
#[derive(Debug)]
pub enum PolicyRegistryError {
    Io(std::io::Error),
    Parse(serde_yaml::Error),
}

impl std::fmt::Display for PolicyRegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PolicyRegistryError::Io(e) => write!(f, "reading policy registry: {e}"),
            PolicyRegistryError::Parse(e) => write!(f, "parsing policy registry: {e}"),
        }
    }
}
impl std::error::Error for PolicyRegistryError {}

/// Canonical contract path, relative to the repo root.
pub const REGISTRY_REL_PATH: &str = "contracts/policy/policy-registry.v1.yaml";

/// Load and parse the policy registry from a repo root.
///
/// `vox-config` does not self-discover the workspace root (see
/// `VoxConfig::load_from_repo_root`); the caller passes it.
pub fn load_policy_registry(repo_root: &Path) -> Result<PolicyRegistry, PolicyRegistryError> {
    let path = repo_root.join(REGISTRY_REL_PATH);
    let text = std::fs::read_to_string(&path).map_err(PolicyRegistryError::Io)?;
    serde_yaml::from_str(&text).map_err(PolicyRegistryError::Parse)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_minimal_entry() {
        let yaml = r#"
schema_version: 1
policies:
  - id: code-audit/stub/todo
    domain: code-audit-rule
    title: TODO stub detector
    group: "Language rules / Stubs (TOESTUB)"
    description: Flags stub placeholders left in shipped code.
    severity: error
    blocking: true
    runs_on: [pre-commit, ci]
    source:
      kind: pattern
      ref: "contracts/code-audit/rules.v1.yaml#stub/todo"
      detail: "todo!()|unimplemented!()"
"#;
        let reg: PolicyRegistry = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(reg.schema_version, 1);
        assert_eq!(reg.policies.len(), 1);
        let e = &reg.policies[0];
        assert_eq!(e.id, "code-audit/stub/todo");
        assert_eq!(e.domain, PolicyDomain::CodeAuditRule);
        assert_eq!(e.severity, Some(PolicySeverity::Error));
        assert!(e.blocking);
        assert!(e.default_enabled, "default_enabled defaults to true");
        assert_eq!(e.origin, "builtin");
        assert_eq!(e.source.kind, PolicySourceKind::Pattern);
        assert_eq!(
            e.source.reference,
            "contracts/code-audit/rules.v1.yaml#stub/todo"
        );
    }

    #[test]
    fn load_roundtrip_from_tempdir() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let contracts = dir.path().join("contracts/policy");
        std::fs::create_dir_all(&contracts).unwrap();
        let mut f = std::fs::File::create(contracts.join("policy-registry.v1.yaml")).unwrap();
        write!(
            f,
            "schema_version: 1\npolicies:\n  - id: code-audit/stub/todo\n    domain: code-audit-rule\n    title: T\n    group: G\n    description: D\n    source:\n      kind: pattern\n      ref: r\n"
        )
        .unwrap();
        let reg = load_policy_registry(dir.path()).unwrap();
        assert_eq!(reg.policies.len(), 1);
        assert_eq!(reg.policies[0].origin, "builtin");
    }
}
