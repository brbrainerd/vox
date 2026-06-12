use crate::codegen_ts::hir_emit::{
    EmitCtx, emit_block_stmts, emit_hir_expr, emit_hir_stmt, extract_state_deps_with_diagnostics,
    map_hir_type_to_ts, wrap_effect_body_if_async,
};
use crate::web_ir::WebIrModule;
use std::collections::HashSet;
use vox_compiler::hir::*;

use super::bindings::collect_component_import_refs;
use super::hooks::{collect_reactive_binding_names, react_import_line};
use super::imports::{emit_external_lib_support, emit_react_es_import_lines};
use super::view::{ReactiveViewBridgeStats, emit_reactive_view_body};
pub(crate) fn emit_dep_inference_hints(out: &mut String, owner: &str, unannotated: &[String]) {
    if unannotated.is_empty() {
        return;
    }
    out.push_str(&format!(
        "  // dep_inference.over_track: `{}` calls [{}] which lack `@reactive` — \
         reactive reads inside those bodies will not trigger re-runs. Add `@reactive` \
         to the callee(s) to opt in to cross-call dep tracking.\n",
        owner,
        unannotated
            .iter()
            .map(|n| format!("`{n}`"))
            .collect::<Vec<_>>()
            .join(", ")
    ));
}

/// `hir` must be the full module (imports, endpoints, `@reactive` callees, etc.).
///
/// `web_projection` must be the same Web IR graph as [`crate::projection_bundle::project_bundle_from_hir`]`(hir).web`
/// so reactive emit does not re-lower the module per component.
pub fn generate_reactive_component(
    hir: &HirModule,
    rc: &HirReactiveComponent,
    web_projection: &WebIrModule,
    stats: &mut ReactiveViewBridgeStats,
) -> (String, String) {
    let name = &rc.name;
    let filename = format!("{name}.tsx");
    let mut out = String::new();

    let state_names = collect_reactive_binding_names(&rc.members);

    // Phase E: collect `@reactive`-annotated free functions visible from this module so the
    // dep walker can recurse one level into their bodies when called from `derived` / `effect`.
    // Functions without `@reactive` are not indexed; their call sites contribute no deps from
    // inside the callee (conservative under-tracking, opt-in extension).
    let reactive_callees: std::collections::HashMap<String, Vec<HirStmt>> = hir
        .functions
        .iter()
        .filter(|f| f.is_reactive)
        .map(|f| (f.name.clone(), f.body.clone()))
        .collect();
    // Phase E tier-2: full set of in-module fn names — used by the dep walker to
    // distinguish "in-module fn missing @reactive" (worth a hint) from "method call /
    // stdlib call / unknown identifier" (silent).
    let visible_fn_names: HashSet<String> = hir.functions.iter().map(|f| f.name.clone()).collect();

    out.push_str(&react_import_line(&rc.members));

    // Phase 5: external React components/hooks. Supports default, named, and
    // namespace `import react …` forms, grouped per module specifier.
    let react_es = emit_react_es_import_lines(&hir.imports);
    if !react_es.is_empty() {
        out.push_str(&react_es);
        out.push('\n');
    }
    // Phase 5 SSOT: required CSS imports + mandatory-provider guidance for known libs.
    let lib_support = emit_external_lib_support(&hir.imports, false);
    if !lib_support.is_empty() {
        out.push_str(&lib_support);
        out.push('\n');
    }

    // Cross-file imports — sibling components and endpoint fns this component
    // references, anywhere in its view or member bodies. Shared with the RN
    // emit via `collect_component_import_refs` so both targets agree.
    // `@form` components (emitted into `forms.tsx`) are referenceable in a view
    // too — e.g. a routable page wrapping a form: `component MoodPage { view: …
    // Mood() }`. Include their names so the ref is recognized, and import them
    // from `./forms` (not the per-component `./Name` convention).
    let form_names: HashSet<String> = hir.forms.iter().map(|f| f.name.clone()).collect();
    let known_components: HashSet<String> = hir
        .components
        .iter()
        .map(|c| c.name.clone())
        .chain(form_names.iter().cloned())
        .collect();
    let endpoint_names: HashSet<String> = hir.endpoint_fns.iter().map(|e| e.name.clone()).collect();
    let (sorted_refs, endpoint_refs) =
        collect_component_import_refs(rc, &known_components, &endpoint_names);
    for comp in &sorted_refs {
        if form_names.contains(comp) {
            out.push_str(&format!("import {{ {comp} }} from \"./forms\";\n"));
        } else {
            out.push_str(&format!("import {{ {comp} }} from \"./{comp}\";\n"));
        }
    }
    if !sorted_refs.is_empty() {
        out.push('\n');
    }
    // Bug D: endpoint fns are exported from `vox-client.ts`
    // (see [`crate::codegen_ts::vox_client`]).
    if !endpoint_refs.is_empty() {
        out.push_str(&format!(
            "import {{ {} }} from \"./vox-client\";\n\n",
            endpoint_refs.join(", ")
        ));
    }

    if !rc.styles.is_empty() {
        out.push_str(&format!("import \"./{name}.css\";\n\n"));
    }

    if !rc.params.is_empty() {
        out.push_str(&format!("export interface {name}Props {{\n"));
        for param in &rc.params {
            let ts_type = param
                .type_ann
                .as_ref()
                .map_or("any".to_string(), map_hir_type_to_ts);
            out.push_str(&format!("  {}: {};\n", param.name, ts_type));
        }
        out.push_str("}\n\n");
    }

    if rc.params.is_empty() {
        out.push_str(&format!(
            "export function {name}(): React.ReactElement {{\n"
        ));
    } else {
        let param_names: Vec<String> = rc.params.iter().map(|p| p.name.clone()).collect();
        out.push_str(&format!(
            "export function {name}({{ {} }}: {name}Props): React.ReactElement {{\n",
            param_names.join(", ")
        ));
    }

    // §1.A.2: build an emit context that threads endpoint (async) fn names into handler emission,
    // so calls to @endpoint fns inside onClick/onChange etc. receive `await`.
    // `endpoint_params` additionally drives the positional→named-object call
    // rewrite (vox-client endpoint fns take a single args object); shared with
    // the RN emit via `EmitCtx`.
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
    let plain_ctx = EmitCtx::with_endpoints(&state_names, &endpoint_params);
    let view_ctx =
        EmitCtx::with_async_and_endpoints(&state_names, &endpoint_names, &endpoint_params);

    for member in &rc.members {
        match member {
            HirReactiveMember::State(s) => {
                let init = emit_hir_expr(&s.init, &plain_ctx);
                out.push_str(&format!(
                    "  const [{}, set_{}] = useState({});\n",
                    s.name, s.name, init
                ));
            }
            HirReactiveMember::Derived(d) => {
                let expr = emit_hir_expr(&d.expr, &plain_ctx);
                let analysis = extract_state_deps_with_diagnostics(
                    &d.expr,
                    &state_names,
                    &reactive_callees,
                    &visible_fn_names,
                );
                emit_dep_inference_hints(&mut out, &d.name, &analysis.unannotated_calls);
                let dep_str = analysis.deps.join(", ");
                out.push_str(&format!(
                    "  const {} = useMemo(() => {}, [{}]);\n",
                    d.name, expr, dep_str
                ));
            }
            HirReactiveMember::Effect(e) => {
                // Async-aware ctx (`view_ctx`) so calls to async @endpoint fns
                // get `await`; the body is then wrapped in a fire-and-forget
                // async IIFE since the useEffect callback must stay sync.
                let stmts_str = emit_block_stmts(&e.body, &view_ctx, 2);
                let body = wrap_effect_body_if_async(&stmts_str, 2);
                let analysis = extract_state_deps_with_diagnostics(
                    &e.body,
                    &state_names,
                    &reactive_callees,
                    &visible_fn_names,
                );
                emit_dep_inference_hints(&mut out, "effect", &analysis.unannotated_calls);
                let dep_str = analysis.deps.join(", ");
                out.push_str(&format!(
                    "  useEffect(() => {{\n{}  }}, [{}]);\n",
                    body, dep_str
                ));
            }
            HirReactiveMember::OnMount(m) => {
                let stmts_str = emit_block_stmts(&m.body, &view_ctx, 2);
                let body = wrap_effect_body_if_async(&stmts_str, 2);
                out.push_str(&format!("  useEffect(() => {{\n{}  }}, []);\n", body));
            }
            HirReactiveMember::OnCleanup(c) => {
                let stmts_str = emit_block_stmts(&c.body, &view_ctx, 2);
                let body = wrap_effect_body_if_async(&stmts_str, 2);
                out.push_str(&format!("  useEffect(() => () => {{\n{}  }}, []);\n", body));
            }
            HirReactiveMember::Stmt(s) => {
                out.push_str(&emit_hir_stmt(s, &plain_ctx, 2));
            }
        }
    }

    if let Some(view_expr) = &rc.view {
        let view_js = emit_reactive_view_body(name, rc, &view_ctx, web_projection, stats);
        // Screen-root components get default horizontal edge padding (`px-4` =
        // 16px, matching the RN target's paddingHorizontal:16) unless the root
        // view opts out with `bleed`. Same screen-root set + opt-out rule as RN.
        let pad_screen = crate::codegen_ts::screen_root_component_names(hir).contains(name)
            && !crate::codegen_ts::rn::component::root_view_bleeds(view_expr);
        if pad_screen {
            out.push_str(&format!(
                "  return (\n    <div className=\"px-4\">\n{}\n    </div>\n  );\n",
                view_js
            ));
        } else {
            out.push_str(&format!("  return (\n{}\n  );\n", view_js));
        }
    }

    out.push_str("}\n");
    (filename, out)
}


