//! `vox ci command-compliance` — validate [`contracts/cli/command-registry.yaml`](../../../../../../contracts/cli/command-registry.yaml) against docs and implementation sources.
//!
//! Completion policy + schema parity: [`completion_quality::verify_policy_contract`]. Completion
//! telemetry report shapes stay discoverable via [`contracts/index.yaml`](../../../../../../contracts/index.yaml)
//! rows `telemetry-completion-*-v1-schema` (also asserted by `vox ci data-ssot-guards`).

use anyhow::{Context, Result, anyhow};
use std::path::Path;

use vox_bounded_fs::read_utf8_path_capped;

mod capability_registry;
mod docs_sync;
mod mcp_wiring;
pub(crate) mod registry;
#[cfg(test)]
mod tests;
mod validators;

use capability_registry::check_capability_registry;
use docs_sync::{read_cli_reference_for_compliance, read_env_vars_ssot_doc, read_reachability_doc};
use mcp_wiring::check_mcp_tool_wiring;
use registry::{
    MCP_HTTP_READ_ROLE_GOVERNANCE_REL, MCP_TOOL_REGISTRY_REL, REGISTRY_REL, RegistryFile,
    SCHEMA_REL, validate_mcp_http_read_role_governance_against_json_schema,
    validate_mcp_tool_registry_against_json_schema, validate_registry_against_json_schema,
};
use validators::{
    check_catalog_feature_gates_match_registry, check_catalog_generation_smoke,
    check_command_registry_embed_matches_disk, check_compilerd, check_dei,
    check_dockerfiles_cargo_locked_policy, check_env_var_ssot_index,
    check_env_vars_doc_tokens_registered, check_feature_growth_boundaries_projection_gate,
    check_install_policy_surfaces, check_latin_alias_parity_with_catalog,
    check_mcp_http_read_role_governance, check_operator_docs_no_legacy_vox_install_pm_nudge,
    check_packaging_pm_docs_no_resurrected_uv_copies, check_product_lane_schema_parity,
    check_project_pm_commands_no_toolchain_lane, check_reachability, check_ref_cli,
    check_registry_latin_and_handlers, check_root_readme_cli_drift,
    check_rust_ecosystem_policy_gate_docs, check_script_duals, check_tier1_env_vars_documented,
    check_upgrade_toolchain_only, check_vox_cli_lib,
};

use super::command_sync;
use super::completion_quality;

/// Run all command-compliance checks from a repository root (directory containing `AGENTS.md`).
pub fn run(repo_root: &Path) -> Result<()> {
    let reg_path = repo_root.join(REGISTRY_REL);
    let raw =
        read_utf8_path_capped(&reg_path).with_context(|| format!("read {}", reg_path.display()))?;
    let schema_path = repo_root.join(SCHEMA_REL);
    if !schema_path.is_file() {
        return Err(anyhow!("missing {}", schema_path.display()));
    }
    validate_registry_against_json_schema(repo_root, &raw)
        .context("command-registry.yaml JSON Schema validation")?;
    let mcp_registry_raw = read_utf8_path_capped(&repo_root.join(MCP_TOOL_REGISTRY_REL))
        .with_context(|| format!("read {}", MCP_TOOL_REGISTRY_REL))?;
    validate_mcp_tool_registry_against_json_schema(repo_root, &mcp_registry_raw)
        .context("MCP tool-registry.canonical.yaml JSON Schema validation")?;
    let mcp_http_read_role_raw =
        read_utf8_path_capped(&repo_root.join(MCP_HTTP_READ_ROLE_GOVERNANCE_REL))
            .with_context(|| format!("read {}", MCP_HTTP_READ_ROLE_GOVERNANCE_REL))?;
    validate_mcp_http_read_role_governance_against_json_schema(repo_root, &mcp_http_read_role_raw)
        .context("MCP http-read-role-governance.yaml JSON Schema validation")?;
    completion_quality::verify_policy_contract(repo_root).context("completion-policy SSOT")?;
    let reg: RegistryFile = serde_yaml::from_str(&raw).context("parse command-registry.yaml")?;
    if reg.schema_version < 1 {
        return Err(anyhow!(
            "command-registry.yaml: schema_version must be >= 1 (got {})",
            reg.schema_version
        ));
    }

    let env_ssot = read_env_vars_ssot_doc(repo_root)?;
    check_env_var_ssot_index(&reg, &env_ssot)?;
    check_env_vars_doc_tokens_registered(repo_root, &env_ssot)?;
    check_product_lane_schema_parity(repo_root)?;
    check_mcp_http_read_role_governance(repo_root)?;
    check_tier1_env_vars_documented(repo_root, &env_ssot)?;
    check_feature_growth_boundaries_projection_gate(repo_root)?;
    check_rust_ecosystem_policy_gate_docs(repo_root)?;

    let ref_cli = read_cli_reference_for_compliance(repo_root)?;
    let reach = read_reachability_doc(repo_root)?;
    let duals_doc = read_utf8_path_capped(&repo_root.join("docs/src/ci/command-surface-duals.md"))
        .context("read command-surface-duals.md")?;
    let lib_rs = read_utf8_path_capped(&repo_root.join("crates/vox-cli/src/lib.rs"))
        .context("read lib.rs")?;
    let compilerd = read_utf8_path_capped(&repo_root.join("crates/vox-cli/src/compilerd.rs"))
        .context("read compilerd.rs")?;
    let dei = read_utf8_path_capped(&repo_root.join("crates/vox-cli/src/dei_daemon.rs"))
        .context("read dei_daemon.rs")?;
    let mcp_mod = read_utf8_path_capped(&repo_root.join("crates/vox-orchestrator-mcp/src/lib.rs"))
        .context("read vox-orchestrator-mcp lib.rs")?;
    let mcp_dispatch =
        read_utf8_path_capped(&repo_root.join("crates/vox-orchestrator-mcp/src/dispatch.rs"))
            .context("read vox-orchestrator-mcp dispatch.rs")?;
    let mcp_tool_aliases =
        read_utf8_path_capped(&repo_root.join("contracts/mcp/tool-wire-aliases.v1.yaml"))
            .context("read contracts/mcp/tool-wire-aliases.v1.yaml")?;
    let scripts_readme = read_utf8_path_capped(&repo_root.join("scripts/README.md"))
        .context("read scripts/README.md")?;
    let root_readme =
        read_utf8_path_capped(&repo_root.join("README.md")).context("read README.md")?;
    let vox_cli_src = repo_root.join("crates/vox-cli/src");

    check_vox_cli_lib(&reg, &lib_rs)?;
    check_install_policy_surfaces(repo_root)?;
    check_upgrade_toolchain_only(repo_root)?;
    check_project_pm_commands_no_toolchain_lane(repo_root)?;
    check_dockerfiles_cargo_locked_policy(repo_root)?;
    check_operator_docs_no_legacy_vox_install_pm_nudge(repo_root)?;
    check_packaging_pm_docs_no_resurrected_uv_copies(repo_root)?;
    check_registry_latin_and_handlers(&reg, &vox_cli_src)?;
    check_ref_cli(&reg, &ref_cli)?;
    check_reachability(&reg, &reach)?;
    check_compilerd(&reg, &compilerd)?;
    check_dei(&reg, &dei)?;
    check_mcp_tool_wiring(repo_root, &mcp_mod, &mcp_dispatch, &mcp_tool_aliases)?;
    check_capability_registry(repo_root, &reg, &mcp_registry_raw)?;
    crate::commands::ci::operations_catalog::verify(repo_root)?;
    check_script_duals(&reg, &duals_doc, &scripts_readme)?;
    check_catalog_generation_smoke()?;
    check_command_registry_embed_matches_disk(repo_root)?;
    check_catalog_feature_gates_match_registry(&reg)?;
    command_sync::verify(repo_root)?;
    check_latin_alias_parity_with_catalog(repo_root, &lib_rs)?;
    check_root_readme_cli_drift(&root_readme)?;

    println!(
        "command-compliance OK (registry schema v{}, {} operations)",
        reg.schema_version,
        reg.operations.len()
    );
    Ok(())
}
