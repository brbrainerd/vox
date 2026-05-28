//! HIR → TypeScript file bundle (production path). **WebIR bridge (OP-S025):** after assembling
//! artifacts, `maybe_web_ir_validate` may validate the module [`crate::web_ir::WebIrModule`] when
//! **`VOX_WEBIR_VALIDATE`** (default **on**): CI and local builds fail codegen when
//! [`validate_web_ir`](crate::web_ir::validate::validate_web_ir) returns diagnostics. Set to `0` / `false` /
//! `no` / `off` to skip the gate (escape hatch).
//!
//! **Escape hatch:** disabling validation skips the structural gate; `routes.manifest.ts` is still emitted
//! only after the validator runs when the gate is enabled (so a failing gate never writes the manifest).
//!
//! **Style + route printer bridge (OP-S059 / S091 / S111 / S137 / S171 / S199):** classic CSS emission and
//! TanStack route files are still assembled here alongside `super::routes`; migrating printers to consume
//! only validated [`crate::web_ir::WebIrModule`] slices is tracked in the internal Web IR blueprint.
//!
//! **Frontend convergence (2026):** OpenAPI / Zod lanes should consume `vox_compiler::contract_ir` types before adding new direct HIR scrape paths — see `docs/src/architecture/frontend-convergence-findings-2026.md`.

use crate::codegen_ts::adt::generate_types;
use crate::projection_bundle::project_bundle_from_hir;

use crate::codegen_ts::reactive::generate_reactive_component;

use crate::codegen_ts::tanstack_query_emit::vox_tanstack_query_tsx;
use crate::codegen_ts::vox_client::{VOX_CLIENT_FILENAME, emit_vox_client};
use vox_compiler::hir::{HirFn, HirModule};

/// Output from the TypeScript code generator.
pub struct CodegenOutput {
    /// List of (filename, content) pairs.
    pub files: Vec<(String, String)>,
    /// Web IR bridge emit statistics
    pub reactive_stats: crate::codegen_ts::reactive::ReactiveViewBridgeStats,
    /// Non-fatal codegen diagnostics (e.g. AI fixtures missing TS lowering).
    pub diagnostics: Vec<crate::web_ir::WebIrDiagnostic>,
}

/// Build mode target for codegen.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum BuildMode {
    /// Emit app code + components (default)
    #[default]
    App,
    /// Emit UI-agnostic models, schemas, and client fetchers
    Library,
}

/// Options for [`generate_with_options`].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CodegenOptions {
    /// Legacy flag (TanStack Start tree emission). **Ignored** — `routes:` now emits `routes.manifest.ts` only.
    pub tanstack_start: bool,
    /// Build Target
    pub target: Option<String>,
    /// Build Mode
    pub mode: BuildMode,
    /// Fail TS codegen when AI fixtures are present but not lowered (`VOX_TS_STRICT_AI` when using [`Self::from_env`]).
    pub strict_ai: bool,
}

impl CodegenOptions {
    /// Reads **`VOX_WEB_TANSTACK_START`** for callers that still thread the flag (**ignored** for TS emit —
    /// route output is always `route_manifest` + components).
    #[must_use]
    pub fn from_env() -> Self {
        let tanstack_start_resolved =
            vox_secrets::resolve_secret(vox_secrets::SecretId::VoxWebTanstackStart);
        Self {
            tanstack_start: tanstack_start_resolved
                .expose()
                .is_some_and(|v: &str| v == "1" || v.eq_ignore_ascii_case("true")),
            target: None,
            mode: BuildMode::App,
            strict_ai: crate::web_migration_env::ts_strict_ai_gate_enabled(),
        }
    }
}

/// Generate TypeScript files from a Vox module (options from [`CodegenOptions::from_env`]).
pub fn generate(hir: &HirModule) -> Result<CodegenOutput, String> {
    generate_with_options(hir, CodegenOptions::from_env())
}

/// Check that every `HirExpr::For` inside a reactive component's view has a `key` expression.
///
/// Returns `Err("validate.list_key.required: …")` on the first violation found.
fn check_for_missing_keys(hir: &HirModule) -> Result<(), String> {
    for rc in &hir.components {
        if let Some(view) = &rc.view {
            if expr_has_keyless_for(view) {
                return Err(format!(
                    "validate.list_key.required: component `{}` has a `for … in …` list render \
                     without a `key` clause. Add `for x in items key=x.id {{ … }}` to give \
                     React stable identity and avoid silent reorder/insert/delete corruption.",
                    rc.name
                ));
            }
        }
    }
    Ok(())
}

fn expr_has_keyless_for(e: &vox_compiler::hir::HirExpr) -> bool {
    use vox_compiler::hir::{HirExpr, HirStmt};
    match e {
        HirExpr::For(_, _, _iter, _body, key, _) => key.is_none(),
        HirExpr::Jsx(el) => el.children.iter().any(expr_has_keyless_for),
        HirExpr::JsxSelfClosing(_) => false,
        HirExpr::JsxFragment(children, _) => children.iter().any(expr_has_keyless_for),
        HirExpr::Block(stmts, _) => stmts.iter().any(|s| {
            if let HirStmt::Expr { expr, .. } = s {
                expr_has_keyless_for(expr)
            } else {
                false
            }
        }),
        HirExpr::If(_, then_b, else_b, _) => {
            then_b.iter().any(|s| {
                if let HirStmt::Expr { expr, .. } = s {
                    expr_has_keyless_for(expr)
                } else {
                    false
                }
            }) || else_b.as_ref().is_some_and(|eb| {
                eb.iter().any(|s| {
                    if let HirStmt::Expr { expr, .. } = s {
                        expr_has_keyless_for(expr)
                    } else {
                        false
                    }
                })
            })
        }
        _ => false,
    }
}

/// Check that every route with a `loader` also has a `pending` and an `error` component.
fn check_route_completeness(hir: &HirModule) -> Result<(), String> {
    for routes_decl in &hir.client_routes {
        check_route_entries_completeness(&routes_decl.entries)?;
    }
    Ok(())
}

fn check_route_entries_completeness(
    entries: &[vox_compiler::ast::decl::RouteEntry],
) -> Result<(), String> {
    for entry in entries {
        if entry.loader_name.is_some() {
            if entry.pending_component_name.is_none() {
                return Err(format!(
                    "validate.route.missing_pending: route `{}` has a loader `{}` but no `pending:` component. \
                     Add `pending: <Component>` to the `with (…)` clause.",
                    entry.path,
                    entry.loader_name.as_deref().unwrap_or("")
                ));
            }
            if entry.error_component_name.is_none() {
                return Err(format!(
                    "validate.route.missing_error: route `{}` has a loader `{}` but no `error:` component. \
                     Add `error: <Component>` to the `with (…)` clause.",
                    entry.path,
                    entry.loader_name.as_deref().unwrap_or("")
                ));
            }
        }
        check_route_entries_completeness(&entry.children)?;
    }
    Ok(())
}

/// Generate TypeScript with explicit options (callers such as `vox build` should pass config here).
pub fn generate_with_options(
    hir: &HirModule,
    options: CodegenOptions,
) -> Result<CodegenOutput, String> {
    let mut ts_diagnostics = Vec::<crate::web_ir::WebIrDiagnostic>::new();
    let has_ai_fixture = hir
        .functions
        .iter()
        .chain(hir.tests.iter())
        .chain(hir.foralls.iter().map(|p| &p.func))
        .chain(hir.mcp_tools.iter().map(|t| &t.func))
        .chain(hir.mcp_resources.iter().map(|r| &r.func))
        .any(|f| f.ai_fixture.is_some());
    if has_ai_fixture {
        let diag = crate::web_ir::WebIrDiagnostic {
            code: "vox/codegen/missing-ts-ai-lowering".to_string(),
            message: "TypeScript target does not yet lower AI fixtures; see docs/src/architecture/ai-fixtures-ts-lowering-follow-on-2026.md".to_string(),
            span: None,
            category: Some("codegen".to_string()),
        };
        if options.strict_ai {
            return Err(format!("{}: {}", diag.code, diag.message));
        }
        ts_diagnostics.push(diag);
    }
    // Validate: every for-loop in a component view must have a key clause (validate.list_key.required).
    check_for_missing_keys(hir)?;
    // Validate: routes with loaders must have pending + error components.
    check_route_completeness(hir)?;

    let mut files = Vec::new();
    let mut reactive_stats = crate::codegen_ts::reactive::ReactiveViewBridgeStats::default();

    let bundle = project_bundle_from_hir(hir);
    let web_projection = &bundle.web;
    let app_contract = &bundle.app;

    if options.mode != BuildMode::Library && !hir.components.is_empty() {
        for rc in &hir.components {
            let (filename, content) =
                generate_reactive_component(hir, rc, web_projection, &mut reactive_stats);
            files.push((filename, content));
            if !rc.styles.is_empty() {
                let mut css = String::new();
                for block in &rc.styles {
                    css.push_str(&block.selector);
                    css.push_str(" {\n");
                    for (prop, val) in &block.properties {
                        css.push_str(&format!("  {prop}: {val};\n"));
                    }
                    css.push_str("}\n");
                }
                files.push((format!("{}.css", rc.name), css));
            }
        }
    }

    // Generate type definitions
    let types_content = generate_types(hir);
    let has_types = !types_content.is_empty();
    if has_types {
        files.push(("types.ts".to_string(), types_content));
    }

    // Generate typed URL declarations
    let url_content = crate::codegen_ts::url_emit::emit_url_decls(hir);
    if !url_content.is_empty() {
        files.push(("urls.ts".to_string(), url_content));
    }

    // Generate state machine types + reducers
    let sm_content = crate::codegen_ts::state_machine_emit::emit_state_machine_decls(hir);
    if !sm_content.is_empty() {
        files.push(("state_machines.ts".to_string(), sm_content));
    }

    // Phase F: emit `fragments.tsx` for any `fragment Name(args) { … }` decls
    // (per ADR-033). Skipped when the module has no fragments.
    let frag_content = crate::codegen_ts::fragment_emit::emit_fragment_decls(hir);
    if !frag_content.is_empty() {
        files.push((
            crate::codegen_ts::fragment_emit::FRAGMENTS_FILENAME.to_string(),
            frag_content,
        ));
    }

    // Emit @form React components (Task C3).
    let forms_content: String = hir.forms.iter().map(super::form_emit::emit_form).collect();
    if !forms_content.is_empty() {
        let needs_navigate = hir.forms.iter().any(|f| f.success_redirect.is_some());
        let navigate_import = if needs_navigate {
            "import { useNavigate } from '@tanstack/react-router';\n"
        } else {
            ""
        };
        // Collect all `on_submit` endpoint references so they can be imported from vox-client.
        // Without this import, tsc reports "Cannot find name 'submit_item'" etc.
        let mut submit_imports: std::collections::BTreeSet<String> =
            std::collections::BTreeSet::new();
        for form in &hir.forms {
            if let Some(fn_name) = &form.on_submit {
                // Only import if the name actually refers to a known endpoint function.
                let is_endpoint = hir.endpoint_fns.iter().any(|e| &e.name == fn_name);
                if is_endpoint {
                    submit_imports.insert(fn_name.clone());
                }
            }
        }
        let client_import = if !submit_imports.is_empty() {
            let names: Vec<String> = submit_imports.into_iter().collect();
            format!("import {{ {} }} from \"./vox-client\";\n", names.join(", "))
        } else {
            String::new()
        };
        let header = format!(
            "// AUTO-GENERATED by Vox @form emit.\nimport React from 'react';\n{navigate_import}{client_import}"
        );
        files.push(("forms.tsx".into(), format!("{header}\n{forms_content}")));
    }

    // Emit mobile Capacitor setup (Tasks D2-D4: @back_button, `@deep_link`, `@push`).
    if let Some(mobile_content) = super::mobile_emit::emit_mobile_setup(&bundle.shell) {
        files.push(("mobile.ts".into(), mobile_content));
    }

    // Phase D: emit `<Name>Provider.tsx` per `.vox.ui` reactive module
    // (per ADR-032). Skipped when the module has none.
    for (filename, content) in crate::codegen_ts::reactive_module_emit::emit_reactive_modules(hir) {
        files.push((filename, content));
    }
    if let Ok(contract_json) = serde_json::to_string_pretty(&app_contract) {
        files.push(("vox-app-contract.json".to_string(), contract_json));
    }

    if options.mode != BuildMode::Library {
        files.push((
            "vox-tanstack-query.tsx".to_string(),
            vox_tanstack_query_tsx(),
        ));
    }

    // Generate table interfaces + schema from HIR
    if !hir.tables.is_empty() {
        let mut schema = String::new();
        schema.push_str("// Table interfaces generated by Vox compiler\n\n");
        for table in &hir.tables {
            schema.push_str(&format!("export interface {} {{\n", table.name));
            schema.push_str("  _id: number;\n");
            for field in &table.fields {
                let ts_type = crate::codegen_ts::hir_emit::map_hir_type_to_ts(&field.type_ann);
                schema.push_str(&format!("  {}: {};\n", field.name, ts_type));
            }
            schema.push_str("}\n\n");
        }
        files.push(("schema.ts".to_string(), schema));
    }

    let zod_schemas = crate::codegen_ts::zod_emit::generate_zod_schemas(hir);
    let has_schemas = !zod_schemas.is_empty();
    if has_schemas {
        files.push(("schemas.ts".to_string(), zod_schemas));
    }

    // Typed fetch client for `@query` (GET + JSON query values) / `@mutation` / `@server` (POST JSON).
    let has_api_fns = !hir.endpoint_fns.is_empty();
    if has_api_fns {
        files.push((VOX_CLIENT_FILENAME.to_string(), emit_vox_client(hir)));
    }

    // OpenAPI 3.1 spec — emitted when the module declares any user types or
    // endpoints. Reads through Contract IR; conforms to wire-format-v1. See
    // [`crate::codegen_ts::openapi_emit`] and Phase 2 of the external frontend
    // interop plan.
    let has_openapi = has_schemas || has_api_fns;
    if has_openapi {
        let openapi = crate::codegen_ts::openapi_emit::generate_openapi(hir, "vox-app", "0.1.0");
        files.push(("openapi.json".to_string(), openapi));
    }

    if options.mode == BuildMode::Library {
        let pkg_cfg = crate::codegen_ts::library_package_emit::LibraryPackageConfig {
            has_vox_client: has_api_fns,
            has_types,
            has_schemas,
            has_openapi,
            has_schema_ts: !hir.tables.is_empty(),
        };
        if pkg_cfg.any_export() {
            files.push((
                "package.json".to_string(),
                crate::codegen_ts::library_package_emit::emit_library_package_json(pkg_cfg),
            ));
        }
    }

    // Load vox.tokens.json via TokenRegistry: emits typed CSS + TS and validates token refs.
    let token_registry = vox_compiler::tokens::TokenRegistry::load_from_str(
        &std::fs::read_to_string("vox.tokens.json").unwrap_or_default(),
    )
    .ok();
    if let Some(ref reg) = token_registry {
        files.push((
            "vox-tokens.css".to_string(),
            crate::codegen_ts::tokens_emit::emit_tokens_css(reg),
        ));
        files.push((
            "tokens.ts".to_string(),
            crate::codegen_ts::tokens_emit::emit_tokens_ts(reg),
        ));
    }

    maybe_web_ir_validate(web_projection, token_registry.as_ref())?;

    let (manifest_filename, route_manifest) = if options.mode == BuildMode::Library {
        (
            "routes.manifest.json",
            crate::codegen_ts::route_manifest::try_emit_route_manifest_json_from_web_ir(
                web_projection,
                hir,
            )?,
        )
    } else {
        (
            "routes.manifest.ts",
            crate::codegen_ts::route_manifest::try_emit_route_manifest_from_web_ir(
                web_projection,
                hir,
            )?,
        )
    };
    if let Some(manifest) = route_manifest {
        files.push((manifest_filename.to_string(), manifest));
    }

    // GA-09a: typed RouteId module (routes.ts) — emitted whenever routes are declared.
    if !hir.route_ids.is_empty() {
        files.push((
            "routes.ts".to_string(),
            crate::web_ir::href_emit::emit_route_id_module(&hir.route_ids),
        ));
    }

    if options.mode != BuildMode::Library {
        // Generate mobile native bridge
        let mobile_fns: Vec<&HirFn> = hir
            .functions
            .iter()
            .filter(|f| f.is_mobile_native)
            .collect();
        if !mobile_fns.is_empty() {
            let mut mobile_bridge =
                String::from("// Mobile native bridge generated by Vox compiler\n");
            mobile_bridge.push_str("import { invoke } from \"@tauri-apps/api/core\";\n\n");
            for f in mobile_fns {
                mobile_bridge.push_str(&crate::codegen_ts::hir_emit::emit_mobile_bridge_fn(f));
                mobile_bridge.push('\n');
            }
            files.push(("mobile-bridge.ts".to_string(), mobile_bridge));
        }

        let uses_mobile_namespace = hir.imports.iter().any(|imp| {
            imp.item == "mobile" && (imp.module_path.is_empty() || imp.module_path == vec!["std"])
        });
        if uses_mobile_namespace {
            files.push((
                "mobile-utils.ts".to_string(),
                crate::codegen_ts::hir_emit::emit_mobile_web_api_utils(options.target.as_deref()),
            ));
        }
    }

    for env in &hir.environments {
        let spec = vox_deploy_codegen::generate::EnvironmentSpec {
            base_image: env
                .base_image
                .clone()
                .unwrap_or_else(|| "debian:bookworm-slim".to_string()),
            packages: env.packages.clone(),
            env_vars: env.env_vars.clone(),
            exposed_ports: env.exposed_ports.clone(),
            volumes: env.volumes.clone(),
            workdir: env.workdir.clone(),
            copy_instructions: env.copy_instructions.clone(),
            run_commands: env.run_commands.clone(),
            cmd: env.cmd.clone(),
            entrypoint: Vec::new(),
        };
        let dockerfile = vox_deploy_codegen::generate::generate_dockerfile_from_spec(&spec);
        files.push((format!("Dockerfile.{}", env.name), dockerfile));
    }

    if options.mode == BuildMode::Library {
        let package_json = serde_json::json!({
            "name": "vox-generated-api",
            "version": "0.1.0",
            "type": "module",
            "main": "./index.ts",
            "exports": {
                ".": "./index.ts"
            },
            "peerDependencies": {
                "zod": "^3.22.4"
            }
        });
        files.push((
            "package.json".to_string(),
            serde_json::to_string_pretty(&package_json).unwrap(),
        ));

        let mut index_ts = String::new();
        if has_types {
            index_ts.push_str("export * from \"./types\";\n");
        }
        if has_schemas {
            index_ts.push_str("export * from \"./schemas\";\n");
        }
        if has_api_fns {
            index_ts.push_str("export * from \"./vox-client\";\n");
        }
        files.push(("index.ts".to_string(), index_ts));
    }

    Ok(CodegenOutput {
        files,
        reactive_stats,
        diagnostics: ts_diagnostics,
    })
}

/// WebIR lower + validate gate (OP-0113, OP-0124). **On by default;** set `VOX_WEBIR_VALIDATE=0` / `false` /
/// `no` / `off` to skip.
///
/// When a [`vox_compiler::tokens::TokenRegistry`] is supplied, token reference resolution and
/// WCAG contrast validation run as part of the style stage (TASK-4.4).
fn maybe_web_ir_validate(
    web: &crate::web_ir::WebIrModule,
    registry: Option<&vox_compiler::tokens::TokenRegistry>,
) -> Result<(), String> {
    if !crate::web_migration_env::web_ir_validate_gate_enabled() {
        return Ok(());
    }
    let diags = crate::web_ir::validate::validate_web_ir_with_registry(web, registry);
    // Advisory diagnostics must not block codegen — only hard errors gate the build.
    let error_diags: Vec<crate::web_ir::WebIrDiagnostic> =
        diags.into_iter().filter(|d| !is_advisory_diag(d)).collect();
    if error_diags.is_empty() {
        return Ok(());
    }
    Err(format!(
        "VOX_WEBIR_VALIDATE: {}",
        crate::web_ir::validate::format_web_ir_validate_failure(&error_diags)
    ))
}

fn is_advisory_diag(d: &crate::web_ir::WebIrDiagnostic) -> bool {
    crate::web_ir::validate::is_advisory_diagnostic(d)
}
