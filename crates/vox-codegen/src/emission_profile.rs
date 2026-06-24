//! Per-target emission validation SSOT — merges WebIR validate chains into one gate.
//!
//! Call [`EmissionProfile::validate_bundle`] (or [`project_and_validate`](crate::projection_bundle::project_and_validate))
//! once after [`project_bundle_from_hir`](crate::projection_bundle::project_bundle_from_hir).

use std::collections::HashSet;

use vox_compiler::app_contract::APP_CONTRACT_SCHEMA_VERSION;
use vox_compiler::runtime_projection::RUNTIME_PROJECTION_SCHEMA_VERSION;
use vox_compiler::target::Target;
use vox_compiler::tokens::TokenRegistry;
use vox_compiler::typeck::diagnostics::Diagnostic;

use crate::projection_bundle::ProjectionBundle;
use crate::web_ir::validate::{
    is_advisory_diagnostic, validate_web_ir, validate_web_ir_with_registry,
};
use crate::web_ir::{WebIrDiagnostic, WebIrModule};

/// Severity for profile-level diagnostics (mirrors WebIR advisory vs hard split).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileSeverity {
    Error,
    Warning,
}

/// Unified diagnostic emitted by [`EmissionProfile`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileDiagnostic {
    pub code: String,
    pub message: String,
    pub severity: ProfileSeverity,
}

/// Per-target validation rules for a lowered [`ProjectionBundle`].
#[derive(Debug, Clone, Copy)]
pub struct EmissionProfile {
    target: Target,
}

impl EmissionProfile {
    #[must_use]
    pub fn for_target(target: Target) -> Self {
        Self { target }
    }

    /// Validate a full projection bundle for this target.
    #[must_use]
    pub fn validate_bundle(&self, bundle: &ProjectionBundle) -> Vec<ProfileDiagnostic> {
        self.validate_bundle_with_registry(bundle, None)
    }

    /// Validate with optional token registry (enables palette/contrast checks on web path).
    #[must_use]
    pub fn validate_bundle_with_registry(
        &self,
        bundle: &ProjectionBundle,
        registry: Option<&TokenRegistry>,
    ) -> Vec<ProfileDiagnostic> {
        match self.target {
            Target::TypeScript | Target::RustTauri => {
                validate_web_for_bundle(&bundle.web, registry)
            }
            Target::RustAxum | Target::Interpreter => validate_runtime_for_bundle(bundle),
        }
    }

    /// Validate a standalone WebIR module (emitter path that has not yet assembled full bundle).
    #[must_use]
    pub fn validate_web_module(
        web: &WebIrModule,
        registry: Option<&TokenRegistry>,
    ) -> Vec<ProfileDiagnostic> {
        validate_web_for_bundle(web, registry)
    }

    /// Return web-IR diagnostics converted into the unified [`Diagnostic`] envelope
    /// so that `vox check --for-llm` callers see web-IR failures alongside typeck errors.
    ///
    /// Only the TypeScript / RustTauri targets produce WebIR; other targets return an empty vec.
    #[must_use]
    pub fn web_ir_as_diagnostics(
        &self,
        bundle: &ProjectionBundle,
        registry: Option<&TokenRegistry>,
    ) -> Vec<Diagnostic> {
        match self.target {
            Target::TypeScript | Target::RustTauri => {
                let raw = match registry {
                    Some(reg) => validate_web_ir_with_registry(&bundle.web, Some(reg)),
                    None => validate_web_ir(&bundle.web),
                };
                raw.into_iter().map(Into::into).collect()
            }
            Target::RustAxum | Target::Interpreter => vec![],
        }
    }
}

fn map_web_ir(d: WebIrDiagnostic) -> ProfileDiagnostic {
    ProfileDiagnostic {
        severity: if is_advisory_diagnostic(&d) {
            ProfileSeverity::Warning
        } else {
            ProfileSeverity::Error
        },
        code: d.code,
        message: d.message,
    }
}

fn validate_web_for_bundle(
    web: &WebIrModule,
    registry: Option<&TokenRegistry>,
) -> Vec<ProfileDiagnostic> {
    let diags = match registry {
        Some(reg) => validate_web_ir_with_registry(web, Some(reg)),
        None => validate_web_ir(web),
    };
    diags.into_iter().map(map_web_ir).collect()
}

fn validate_runtime_for_bundle(bundle: &ProjectionBundle) -> Vec<ProfileDiagnostic> {
    let mut out = Vec::new();

    if bundle.app.schema_version != APP_CONTRACT_SCHEMA_VERSION {
        out.push(ProfileDiagnostic {
            code: "vox/emission/app_contract_schema_drift".to_string(),
            message: format!(
                "AppContract schema_version {} != expected {APP_CONTRACT_SCHEMA_VERSION}",
                bundle.app.schema_version
            ),
            severity: ProfileSeverity::Error,
        });
    }

    if bundle.runtime.schema_version != RUNTIME_PROJECTION_SCHEMA_VERSION {
        out.push(ProfileDiagnostic {
            code: "vox/emission/runtime_projection_schema_drift".to_string(),
            message: format!(
                "RuntimeProjection schema_version {} != expected {RUNTIME_PROJECTION_SCHEMA_VERSION}",
                bundle.runtime.schema_version
            ),
            severity: ProfileSeverity::Error,
        });
    }

    let mut route_keys = HashSet::new();
    for route in &bundle.app.http_routes {
        let key = format!("{}:{}", route.method, route.path);
        if !route_keys.insert(key.clone()) {
            out.push(ProfileDiagnostic {
                code: "vox/emission/duplicate_http_route".to_string(),
                message: format!("duplicate HTTP route {key}"),
                severity: ProfileSeverity::Error,
            });
        }
    }

    out
}

/// Returns hard (non-advisory) profile diagnostics only.
#[must_use]
pub fn hard_profile_errors(diags: &[ProfileDiagnostic]) -> Vec<ProfileDiagnostic> {
    diags
        .iter()
        .filter(|d| d.severity == ProfileSeverity::Error)
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use vox_compiler::ast::span::Span;
    use vox_compiler::hir::nodes::DefId;
    use vox_compiler::hir::{HirExpr, HirModule, HirReactiveComponent};

    use super::*;
    use crate::projection_bundle::project_bundle_from_hir;
    use crate::web_ir::{CssColor, StyleDeclarationValue, StyleNode, StyleSelector, WebIrModule};

    fn minimal_hir_with_view() -> HirModule {
        let mut hir = HirModule::default();
        hir.components.push(HirReactiveComponent {
            id: DefId(1),
            name: "Hello".to_string(),
            params: vec![],
            members: vec![],
            view: Some(HirExpr::StringLit("hi".to_string(), Span::new(0, 0))),
            styles: vec![],
            layer: None,
            span: Span::new(0, 0),
        });
        hir
    }

    #[test]
    fn profile_validate_runs_webir_for_typescript_target() {
        let hir = minimal_hir_with_view();
        let bundle = project_bundle_from_hir(&hir);
        let diags = EmissionProfile::for_target(Target::TypeScript).validate_bundle(&bundle);
        assert!(
            diags
                .iter()
                .all(|d| d.code.starts_with("web_ir_validate.") || d.code.starts_with("vox/")),
            "unexpected diagnostic codes: {diags:?}"
        );
    }

    #[test]
    fn profile_rejects_literal_color_in_style_block() {
        let mut web = WebIrModule::default();
        web.style_nodes.push(StyleNode::Rule {
            specificity: (0, 1, 0),
            selector: StyleSelector::Class("bad".into()),
            declarations: vec![(
                "color".into(),
                StyleDeclarationValue::Color(CssColor::Hex("#ff0000".into())),
            )],
            is_raw_css: false,
            span: None,
        });
        let diags = EmissionProfile::validate_web_module(&web, None);
        assert!(
            diags.iter().any(|d| {
                d.code == "web_ir_validate.style.literal_color_value"
                    && d.severity == ProfileSeverity::Error
            }),
            "expected hard literal_color_value: {diags:?}"
        );
    }

    #[test]
    fn runtime_profile_flags_duplicate_http_routes() {
        use vox_compiler::app_contract::{
            AppContractModule, AppHttpRouteContract, AppServerConfigContract,
        };
        use vox_compiler::required_capabilities::{
            REQUIRED_CAPABILITIES_SCHEMA_VERSION, RequiredRuntimeCapabilities,
        };

        let bundle = ProjectionBundle {
            web: WebIrModule::default(),
            app: AppContractModule {
                schema_version: APP_CONTRACT_SCHEMA_VERSION,
                http_routes: vec![
                    AppHttpRouteContract {
                        method: "GET".to_string(),
                        path: "/api/ping".to_string(),
                        route_contract: "ping".to_string(),
                        return_type: None,
                    },
                    AppHttpRouteContract {
                        method: "GET".to_string(),
                        path: "/api/ping".to_string(),
                        route_contract: "ping_dup".to_string(),
                        return_type: None,
                    },
                ],
                server_fns: vec![],
                query_fns: vec![],
                mutation_fns: vec![],
                mcp_tools: vec![],
                mcp_resources: vec![],
                server_config: AppServerConfigContract {
                    bind_host: "127.0.0.1".to_string(),
                    default_port: 3000,
                    port_env_var: "VOX_PORT".to_string(),
                    dev_proxy_env_var: "VOX_SSR_DEV_URL".to_string(),
                    static_assets_embed_dir: "public/".to_string(),
                },
            },
            runtime: vox_compiler::runtime_projection::RuntimeProjectionModule {
                schema_version: RUNTIME_PROJECTION_SCHEMA_VERSION,
                module_task_capability_hints: None,
                host_capability_probe: None,
                db_planning_policies: vec![],
            },
            shell: vox_compiler::shell_projection::ShellProjectionModule::default(),
            capabilities: RequiredRuntimeCapabilities {
                schema_version: REQUIRED_CAPABILITIES_SCHEMA_VERSION,
                capability_ids: vec![],
            },
        };
        let diags = EmissionProfile::for_target(Target::RustAxum).validate_bundle(&bundle);
        assert!(
            diags
                .iter()
                .any(|d| d.code == "vox/emission/duplicate_http_route"),
            "{diags:?}"
        );
    }

    /// Verify that a known web-IR error appears in the converted [`Diagnostic`] vec
    /// produced by [`EmissionProfile::web_ir_as_diagnostics`].
    #[test]
    fn web_ir_as_diagnostics_surfaces_literal_color_error() {
        use vox_compiler::typeck::diagnostics::TypeckSeverity;

        let mut web = WebIrModule::default();
        web.style_nodes.push(StyleNode::Rule {
            specificity: (0, 1, 0),
            selector: StyleSelector::Class("bad".into()),
            declarations: vec![(
                "color".into(),
                StyleDeclarationValue::Color(CssColor::Hex("#ff0000".into())),
            )],
            is_raw_css: false,
            span: None,
        });

        // Wrap in a minimal ProjectionBundle using project_bundle_from_hir so
        // we don't need to keep the struct literal in sync with upstream changes.
        let base_bundle = project_bundle_from_hir(&HirModule::default());
        let bundle = ProjectionBundle { web, ..base_bundle };

        let diags =
            EmissionProfile::for_target(Target::TypeScript).web_ir_as_diagnostics(&bundle, None);

        assert!(
            diags.iter().any(|d| {
                d.code.as_deref() == Some("web_ir_validate.style.literal_color_value")
                    && d.severity == TypeckSeverity::Error
            }),
            "expected literal_color_value as Diagnostic::Error; got: {diags:?}"
        );
    }
}
