//! React Native + Expo lowering for the `BuildTarget::Mobile` target.
//!
//! ## Single source of truth
//!
//! This module is **one of two** lowerings for Vox components. The web lowering
//! (under [`super::reactive`] + [`super::component`]) consumes the same `HirModule`
//! and produces React DOM + Tailwind output; this module consumes the same
//! `HirModule` and produces React Native + StyleSheet output. The HIR is the
//! shared source of truth — there is no separate "RN HIR".
//!
//! Future split-brain protection: the `vox-cli-tests` harness asserts that for
//! every Vox source under test, BOTH lowerings produce output that
//! `tsc --noEmit` accepts. Any divergence in supported VUV vocabulary fails CI.
//!
//! ## What's emitted
//!
//! For a Vox module, `generate_rn` produces a self-contained set of files:
//!
//! - `App.tsx`           — Expo Router root layout (when components are present).
//! - `<Component>.tsx`   — one file per `component Name() { ... }`. RN-flavored.
//! - `vox-client.ts`     — reused unchanged from the existing emit; the runtime
//!                         HTTP layer is identical between web and mobile.
//! - `mobile.ts`         — reused unchanged (it already targets the adapter
//!                         contract via `target="rn"`).
//! - `vox-app-contract.json`, `openapi.json`, `schemas.ts`, `types.ts` — reused.
//! - `app.json`, `babel.config.js`, `metro.config.js`, `eas.json` — Expo build
//!                         artifacts (one-shot scaffold, not overwritten).
//!
//! ## What's NOT yet emitted (will produce a clear codegen diagnostic, not silent stub)
//!
//! - `@routes` → expo-router file tree (see follow-on in Phase 1A.4).
//! - Boolean `@form` fields → `<Switch>` (currently emits a TODO marker; users
//!   can hand-edit until the upstream HIR exposes switch metadata).
//! - State machines are emitted unchanged (TypeScript-only; no React DOM coupling).

pub mod component;
pub mod form;
pub mod mobile_utils;
pub mod routes;
pub mod scaffold;

use crate::codegen_ts::emitter::CodegenOptions;
use vox_compiler::hir::HirModule;

/// Output of the RN lowering. Same shape as the web emitter's `CodegenOutput` for symmetry.
pub struct RnCodegenOutput {
    /// `(filename, content)` pairs that the build command writes into `out_dir`.
    pub files: Vec<(String, String)>,
    /// Non-fatal diagnostics emitted during lowering (e.g. unsupported VUV vocabulary).
    pub diagnostics: Vec<crate::web_ir::WebIrDiagnostic>,
}

/// Generate React Native + Expo TypeScript files from a Vox module.
///
/// The caller is responsible for writing files. Returns `Err` on hard failures
/// (e.g. unsupported features); use the `diagnostics` field for non-fatal warnings.
pub fn generate_rn(hir: &HirModule, _options: &CodegenOptions) -> Result<RnCodegenOutput, String> {
    let mut files: Vec<(String, String)> = Vec::new();
    let mut diagnostics: Vec<crate::web_ir::WebIrDiagnostic> = Vec::new();

    // Components — one file each. Precompute the reference universes once so
    // each component can emit imports for the sibling components and endpoint
    // fns it actually uses (same sets the web reactive emit derives).
    // `@form` components (emitted into forms.tsx) are referenceable in views too
    // (e.g. a routable page wrapping a form). Include them so the ref resolves;
    // their import path is `./forms` (handled in emit_rn_component).
    let form_names: std::collections::HashSet<String> =
        hir.forms.iter().map(|f| f.name.clone()).collect();
    let known_components: std::collections::HashSet<String> = hir
        .components
        .iter()
        .map(|c| c.name.clone())
        .chain(form_names.iter().cloned())
        .collect();
    // name → ordered param names; keys are the endpoint-import set, values drive
    // the positional→named-object endpoint-call rewrite (see EmitCtx).
    let endpoint_params: std::collections::HashMap<String, Vec<String>> = hir
        .endpoint_fns
        .iter()
        .map(|e| {
            (
                e.name.clone(),
                e.params.iter().map(|p| p.name.clone()).collect(),
            )
        })
        .collect();
    let screen_root_names = crate::codegen_ts::screen_root_component_names(hir);
    for rc in &hir.components {
        let (filename, content) = component::emit_rn_component(
            rc,
            &known_components,
            &form_names,
            &endpoint_params,
            &screen_root_names,
            &hir.imports,
            &mut diagnostics,
        );
        files.push((filename, content));
    }

    // @form declarations — RN-flavored forms (View / Text / TextInput / Pressable).
    // Same validation logic as the web emit; only leaf rendering differs.
    if !hir.forms.is_empty() {
        let forms_body: String = hir.forms.iter().map(form::emit_form).collect();
        // Imports: every form file uses React (with useState) plus the RN primitives
        // we render. The `submit_fn` references are imported from `./vox-client`.
        let mut submit_imports: std::collections::BTreeSet<String> = Default::default();
        for f in &hir.forms {
            if let Some(name) = &f.on_submit {
                if hir.endpoint_fns.iter().any(|e| &e.name == name) {
                    submit_imports.insert(name.clone());
                }
            }
        }
        let client_import = if submit_imports.is_empty() {
            String::new()
        } else {
            format!(
                "import {{ {} }} from \"./vox-client\";\n",
                submit_imports.into_iter().collect::<Vec<_>>().join(", ")
            )
        };
        let header = format!(
            "// AUTO-GENERATED by Vox @form (RN target).\n\
             import React, {{ useState }} from \"react\";\n\
             import {{ View, Text, TextInput, Pressable, StyleSheet }} from \"react-native\";\n\
             {client_import}\n"
        );
        files.push((
            "forms.tsx".into(),
            format!("{header}{forms_body}\n{}", form::RN_FORM_STYLESHEET),
        ));
    }

    // `std.mobile` namespace — when source `import std.mobile`s OR any
    // component / handler body references the `mobile` identifier, emit
    // `mobile-utils.ts` so callers' `mobile.notify(...)` etc. resolve to a
    // real binding that routes through `@vox/runtime-rn::voxRuntime`.
    let uses_mobile_namespace = hir.imports.iter().any(|imp| {
        imp.item == "mobile" && (imp.module_path.is_empty() || imp.module_path == vec!["std"])
    }) || mobile_utils::any_component_uses_mobile(hir);
    if uses_mobile_namespace {
        files.push((
            "mobile-utils.ts".to_string(),
            mobile_utils::emit_mobile_utils_rn(),
        ));
    }

    // Mobile primitives — reuse the existing emit, but with target="rn" so it
    // imports from `@vox/runtime-rn` instead of `@vox/runtime`.
    let bundle = crate::projection_bundle::project_bundle_from_hir(hir);
    let endpoint_param0: std::collections::HashMap<String, String> = hir
        .endpoint_fns
        .iter()
        .filter_map(|e| e.params.first().map(|p| (e.name.clone(), p.name.clone())))
        .collect();
    if let Some(mobile_content) = crate::codegen_ts::mobile_emit::emit_mobile_setup_for_target(
        &bundle.shell,
        Some("rn"),
        &endpoint_param0,
    ) {
        files.push(("mobile.ts".into(), mobile_content));
    }

    // App contract — unchanged (it's pure data, no UI coupling).
    if let Ok(contract_json) = serde_json::to_string_pretty(&bundle.app) {
        files.push(("vox-app-contract.json".to_string(), contract_json));
    }

    // Typed fetch client — same HTTP layer, but the RN flavor resolves the API
    // base via `process.env.EXPO_PUBLIC_*` (Hermes can't parse `import.meta`).
    if !hir.endpoint_fns.is_empty() {
        files.push((
            crate::codegen_ts::vox_client::VOX_CLIENT_FILENAME.to_string(),
            crate::codegen_ts::vox_client::emit_vox_client_for_target(
                hir,
                crate::codegen_ts::vox_client::VoxClientTarget::ReactNative,
            ),
        ));
    }

    // Zod schemas — pure types; reuse.
    let zod_schemas = crate::codegen_ts::zod_emit::generate_zod_schemas(hir);
    let has_schemas = !zod_schemas.is_empty();
    if has_schemas {
        files.push(("schemas.ts".to_string(), zod_schemas));
    }

    // TypeScript types — pure types; reuse.
    let types_content = crate::codegen_ts::adt::generate_types(hir);
    if !types_content.is_empty() {
        files.push(("types.ts".to_string(), types_content));
    }

    // State machines — discriminated-union + reducer, framework-agnostic; reuse.
    let sm_content = crate::codegen_ts::state_machine_emit::emit_state_machine_decls(hir);
    if !sm_content.is_empty() {
        files.push(("state_machines.ts".to_string(), sm_content));
    }

    // OpenAPI — pure schema, useful for client SDK consumers.
    let has_api_fns = !hir.endpoint_fns.is_empty();
    if has_schemas || has_api_fns {
        let openapi = crate::codegen_ts::openapi_emit::generate_openapi(hir, "vox-app", "0.1.0");
        files.push(("openapi.json".to_string(), openapi));
    }

    // @routes — emit Expo Router file-system route tree. When routes are
    // declared, expo-router (not App.tsx) is the app entry; the scaffold
    // adjusts its `main` field accordingly.
    let route_files = routes::emit_expo_router_files(&hir.client_routes);
    let has_routes = !route_files.is_empty();
    for (filename, content) in route_files {
        files.push((filename, content));
    }

    // Expo project scaffolding — emitted only when components exist (no point in
    // an Expo app shell otherwise) and not overwritten on subsequent builds.
    if !hir.components.is_empty() {
        for (filename, content) in scaffold::emit_expo_scaffold(hir, has_routes) {
            files.push((filename, content));
        }
    }

    Ok(RnCodegenOutput { files, diagnostics })
}
