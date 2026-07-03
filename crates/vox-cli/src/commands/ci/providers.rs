//! `VoxCliProviders` — vox-cli's implementation of the vox-cli-contracts seam traits
//! (PR-2 of the CI extraction). The CI dispatcher (today in-crate, later in vox-cli-ci)
//! takes `&dyn` these traits instead of reaching into audit/policy/runtime modules
//! directly, so the subsystem can move out without a back-dependency on vox-cli.

use std::path::Path;

use vox_cli_contracts::{CheckProvider, GateStatusWriter, TerminalPolicyValidator};
use vox_config::{
    PolicyDomain, PolicyEntry, PolicyResult, PolicySeverity, PolicySource, PolicySourceKind,
};

use crate::commands::policy::status_writer;
use crate::commands::runtime::shell::check_terminal;

/// Zero-sized aggregate provider handed to the CI dispatcher.
pub struct VoxCliProviders;

impl CheckProvider for VoxCliProviders {
    /// Enumerate `contracts/ci/check-targets.v1.yaml` into `audit-check` policy entries.
    /// (The canonical home of this logic — `policy_registry::audit_check_entries` delegates here.)
    fn load_check_targets(&self, repo_root: &Path) -> anyhow::Result<Vec<PolicyEntry>> {
        let path = repo_root.join("contracts/ci/check-targets.v1.yaml");
        let text = std::fs::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("read {}: {e}", path.display()))?;
        let manifest: vox_cli_contracts::CheckManifest = serde_yaml::from_str(&text)
            .map_err(|e| anyhow::anyhow!("parse {}: {e}", path.display()))?;
        let mut out: Vec<PolicyEntry> = manifest
            .checks
            .iter()
            .map(|c| {
                let blocking = c.blocking;
                PolicyEntry {
                    id: format!("audit-check/{}", c.id),
                    domain: PolicyDomain::AuditCheck,
                    title: c.id.clone(),
                    group: format!("Audit checks / {}", c.category),
                    description: c.description.clone(),
                    severity: Some(if blocking {
                        PolicySeverity::Error
                    } else {
                        PolicySeverity::Warn
                    }),
                    blocking,
                    runs_on: c.runs_on.clone(),
                    source: PolicySource {
                        kind: PolicySourceKind::Command,
                        reference: format!("contracts/ci/check-targets.v1.yaml#{}", c.id),
                        detail: Some(c.command.join(" ")),
                    },
                    docs: None,
                    default_enabled: true,
                    protected: false,
                    origin: "builtin".to_string(),
                }
            })
            .collect();
        out.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(out)
    }
}

impl GateStatusWriter for VoxCliProviders {
    fn current_branch(&self, repo_root: &Path) -> String {
        status_writer::current_branch(repo_root)
    }
    fn head_commit(&self, repo_root: &Path) -> String {
        status_writer::head_commit(repo_root)
    }
    fn write_results(
        &self,
        repo_root: &Path,
        branch: &str,
        commit: &str,
        ran_at: &str,
        results: Vec<PolicyResult>,
    ) -> anyhow::Result<()> {
        status_writer::write_results(repo_root, branch, commit, ran_at, results)
            .map_err(anyhow::Error::from)
    }
}

impl TerminalPolicyValidator for VoxCliProviders {
    fn default_policy_rel(&self) -> &'static str {
        check_terminal::DEFAULT_POLICY_REL
    }
    fn validate_policy_file(&self, repo_root: &Path, policy: &Path) -> anyhow::Result<()> {
        check_terminal::validate_policy_file(repo_root, policy).map(|_| ())
    }
    fn run_check_for_ci(&self, payload: &str, policy: Option<&Path>) -> anyhow::Result<()> {
        check_terminal::run_check_for_ci(payload, policy)
    }
}

impl vox_cli_ci::HeavyGuardHost for VoxCliProviders {
    /// The Tier-2 guards that reach into vox-cli internals (command catalog/registry,
    /// utils, fs_utils, runtime shell) and so stay here. Mirrors the arms the moved
    /// dispatcher used to run inline. `None` ⇒ not a Tier-2 guard.
    fn dispatch_heavy(
        &self,
        cmd: &vox_cli_ci::cmd_enums::CiCmd,
        root: &Path,
    ) -> Option<anyhow::Result<()>> {
        use anyhow::anyhow;
        use vox_cli_ci::cmd_enums::{CiCmd, OperationsSyncTarget};
        Some(match cmd {
            CiCmd::PolicyRegistry { write } => {
                super::policy_registry::run_generate(root, *write).map_err(|e| anyhow!(e))
            }
            CiCmd::PolicyRegistryParity => {
                super::policy_registry::run_parity(root).map_err(|e| anyhow!(e))
            }
            CiCmd::GuiCatalogParity => super::gui_catalog_parity::run(root),
            CiCmd::GuiSurfaceCoverage { write } => super::gui_surface_coverage::run(root, *write),
            CiCmd::GuiSurfaceRegistry { write } => super::gui_surface_registry::run(root, *write),
            CiCmd::ExecPolicyContract => super::exec_policy_contract::run(root),
            CiCmd::OperationsVerify => super::operations_catalog::verify(root),
            CiCmd::OperationsSync { target, write } => {
                let target = match target {
                    OperationsSyncTarget::Catalog => "catalog",
                    OperationsSyncTarget::Mcp => "mcp",
                    OperationsSyncTarget::Cli => "cli",
                    OperationsSyncTarget::Capability => "capability",
                    OperationsSyncTarget::All => "all",
                };
                super::operations_catalog::sync(root, target, *write)
            }
            CiCmd::CapabilitySync { write } => super::capability_sync::run(root, *write),
            CiCmd::CommandSync { write } => super::command_sync::run(root, *write),
            CiCmd::ReleaseBuild {
                target,
                version,
                out_dir,
                package,
            } => super::release_build::run(root, target, version.as_deref(), out_dir, *package),
            CiCmd::RunnerScale { apply } => super::runner_scale::run_scale(*apply),
            CiCmd::RunnerPreflight => super::runner_scale::run_preflight(),
            CiCmd::RunnerStatus => super::runner_scale::run_status(),
            _ => return None,
        })
    }
}
