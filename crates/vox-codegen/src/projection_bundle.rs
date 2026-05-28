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
