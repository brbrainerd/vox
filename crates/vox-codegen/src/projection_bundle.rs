//! Single entry point for HIR-derived projections consumed by emitters (WebIR, contracts, shell, capabilities).
//!
//! Call [`project_bundle_from_hir`] once per module; downstream codegen must not re-call individual
//! projectors except through this bundle (enforced by `vox-arch-check`).

use vox_compiler::app_contract::{AppContractModule, project_app_contract};
use vox_compiler::hir::HirModule;
use vox_compiler::required_capabilities::{
    RequiredRuntimeCapabilities, project_required_capabilities,
};
use vox_compiler::runtime_projection::{RuntimeProjectionModule, project_runtime_from_hir};
use vox_compiler::shell_projection::{ShellProjectionModule, project_shell_from_hir};
use vox_compiler::target::Target;
use vox_compiler::tokens::TokenRegistry;

use crate::emission_profile::{EmissionProfile, ProfileDiagnostic, hard_profile_errors};
use crate::web_ir::WebIrModule;
use crate::web_ir::lower::lower_hir_to_web_ir;

/// All machine-readable projections from one `HirModule`.
#[derive(Debug, Clone)]
pub struct ProjectionBundle {
    pub web: WebIrModule,
    pub app: AppContractModule,
    pub runtime: RuntimeProjectionModule,
    pub shell: ShellProjectionModule,
    pub capabilities: RequiredRuntimeCapabilities,
}

/// Lower and project every SSOT surface from `hir` in one pass.
#[must_use]
pub fn project_bundle_from_hir(hir: &HirModule) -> ProjectionBundle {
    ProjectionBundle {
        web: lower_hir_to_web_ir(hir),
        app: project_app_contract(hir),
        runtime: project_runtime_from_hir(hir),
        shell: project_shell_from_hir(hir),
        capabilities: project_required_capabilities(hir),
    }
}

/// Project from HIR and run the unified [`EmissionProfile`] validate gate for `target`.
pub fn project_and_validate(
    hir: &HirModule,
    target: Target,
) -> Result<ProjectionBundle, Vec<ProfileDiagnostic>> {
    project_and_validate_with_registry(hir, target, None)
}

/// Project and validate with optional token registry (palette / contrast on web targets).
pub fn project_and_validate_with_registry(
    hir: &HirModule,
    target: Target,
    registry: Option<&TokenRegistry>,
) -> Result<ProjectionBundle, Vec<ProfileDiagnostic>> {
    let bundle = project_bundle_from_hir(hir);
    let diags =
        EmissionProfile::for_target(target).validate_bundle_with_registry(&bundle, registry);
    let hard = hard_profile_errors(&diags);
    if hard.is_empty() {
        Ok(bundle)
    } else {
        Err(hard)
    }
}
