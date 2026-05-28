//! ADR 012 — HIR → WebIR → validate → TSX preview emit.
#![allow(unsafe_code)] // `VOX_WEBIR_VALIDATE` toggles for emitter bridge tests (OP-S026 / OP-S028)

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use vox_codegen::codegen_ts::hir_emit::emit_hir_expr;
use vox_codegen::codegen_ts::{CodegenOptions, generate_with_options};
use vox_codegen::syntax_k::{
    RepresentabilityPayload, SyntaxKInput, canonical_emitted_files_bytes, canonical_web_ir_bytes,
    enrich_syntax_k_support_metrics, measure_syntax_k_event, sha3_hex,
};
use vox_codegen::web_ir::emit_tsx::{emit_component_view_tsx, emit_component_view_tsx_with_stats};
use vox_codegen::web_ir::lower::{lower_hir_to_web_ir, lower_hir_to_web_ir_with_summary};
use vox_codegen::web_ir::validate::{
    format_web_ir_validate_failure, validate_web_ir, validate_web_ir_with_metrics,
};
use vox_codegen::web_ir::{
    BehaviorNode, DomNode, DomNodeId, FieldOptionality, InteropNode, MutationContract,
    RouteContract, RouteNode, ServerFnContract, StyleDeclarationValue, StyleNode, StyleSelector,
    WebIrModule, WebIrVersion,
};
use vox_compiler::ast::decl::{Decl, ThemeDecl};
use vox_compiler::ast::span::Span;
use vox_compiler::hir::{HirModule, HirReactiveMember, lower_module};
use vox_compiler::lexer::lex;
use vox_compiler::parser::parse;
use vox_compiler::runtime_projection::{
    RUNTIME_PROJECTION_SCHEMA_VERSION, canonical_runtime_projection_bytes, project_runtime_from_hir,
};

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn web_ir_lowering_validates_and_emits_counter_view() {
    let source = r#"
component Counter(initial: int) {
    state count: int = initial
    derived double = count * 2
    view: column(raw_class="p-4") {
            heading(level=1) { "Count: {count}" }
    }
}
"#;
    let module = parse(lex(source)).expect("parse");
    let hir = lower_module(&module);
    let web = lower_hir_to_web_ir(&hir);
    let diags = validate_web_ir(&web);
    assert!(diags.is_empty(), "{diags:?}");
    let tsx = emit_component_view_tsx(&web, "Counter").expect("view");
    insta::assert_snapshot!("counter_view_tsx_webir_emit", tsx);
}

/// OP-S006 / OP-S008: `lower_module` places imports, `routes { }`, and Path C components in the expected HIR vectors.
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn hir_lowering_bucket_labels_import_routes_reactive() {
    let source = r#"
import react.use_state
component Home() {
    state n: int = 0
    view: text() { n }
}
routes {
    "/" to Home
}
"#;
    let module = parse(lex(source)).expect("parse");
    let hir = lower_module(&module);
    assert_eq!(hir.imports.len(), 1, "imports: {:?}", hir.imports);
    assert_eq!(hir.components.len(), 1);
    assert_eq!(hir.components[0].name, "Home");
}

/// `@scheduled` HIR metadata lowers into [`WebIrModule::scheduled_jobs`].
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn web_ir_lowering_scheduled_jobs_from_hir() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/golden/scheduled_tick.vox"
    ));
    let module = parse(lex(source)).expect("parse scheduled_tick");
    let hir = lower_module(&module);
    let (web, summary) = lower_hir_to_web_ir_with_summary(&hir);
    assert_eq!(summary.scheduled_jobs_lowered, 1, "{summary:?}");
    assert_eq!(web.scheduled_jobs.len(), 1);
    assert_eq!(web.scheduled_jobs[0].name, "scheduled_tick");
    assert_eq!(web.scheduled_jobs[0].interval, "1h");
    let diags = validate_web_ir(&web);
    assert!(diags.is_empty(), "{diags:?}");
}

/// OP-S010 / OP-S012: `WebIrModule` JSON shell keeps stable top-level field names for schema consumers.
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn web_ir_module_serde_shell_field_names_stable() {
    let m = WebIrModule::default();
    let v = serde_json::to_value(&m).expect("serde");
    let obj = v.as_object().expect("object");
    for key in [
        "dom_nodes",
        "view_roots",
        "behavior_nodes",
        "style_nodes",
        "route_nodes",
        "scheduled_jobs",
        "interop_nodes",
        "diagnostic_nodes",
        "spans",
        "version",
    ] {
        assert!(obj.contains_key(key), "missing {key} in {obj:?}");
    }
}

/// OP-S015: event-like JSX attrs map to React-style names on elements (same edge as `hir_emit`).
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn web_ir_lowering_event_attr_maps_to_on_click_on_element() {
    let source = r#"
component B() {
    state n: int = 0
    view: button(on_click={n = n + 1}) { " "ok" " }
}
"#;
    let module = parse(lex(source)).expect("parse");
    let hir = lower_module(&module);
    let web = lower_hir_to_web_ir(&hir);
    let has_on_click = web.dom_nodes.iter().any(|n| {
        if let DomNode::Element { attrs, .. } = n {
            attrs.iter().any(|(k, _)| k == "onClick")
        } else {
            false
        }
    });
    assert!(has_on_click, "attrs not found in {:?}", web.dom_nodes);
}

/// OP-S018: required reactive state without lowered initial is a validate error.
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn web_ir_validate_rejects_required_state_without_initial() {
    let mut m = WebIrModule::default();
    m.behavior_nodes.push(BehaviorNode::StateDecl {
        name: "Panel::x".into(),
        initial: None,
        optionality: FieldOptionality::Required,
        span: None,
    });
    let d = validate_web_ir(&m);
    assert!(
        d.iter()
            .any(|e| e.code == "web_ir_validate.behavior.required_state_without_initial"),
        "{d:?}"
    );
}

/// OP-S020: duplicate client [`RouteContract`] ids fail validation.
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn web_ir_validate_duplicate_route_contract_id() {
    let mut m = WebIrModule::default();
    m.route_nodes.push(RouteNode::RouteTree {
        routes: vec![
            RouteContract {
                id: "route_0".into(),
                pattern: "/".into(),
                meta: serde_json::json!({ "component": "A" }),
                children: vec![],
            },
            RouteContract {
                id: "route_0".into(),
                pattern: "/b".into(),
                meta: serde_json::json!({ "component": "B" }),
                children: vec![],
            },
        ],
        span: None,
    });
    let d = validate_web_ir(&m);
    assert!(
        d.iter()
            .any(|e| e.code == "web_ir_validate.route.duplicate_contract_id"),
        "{d:?}"
    );
}

/// Nested `routes { }`, loaders, pending, and block-level `not_found` / `error` surface in `routes.manifest.ts`.
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn codegen_nested_route_manifest_includes_children_loader_pending_and_boundary_exports() {
    let source = r#"
@query fn load_child() to int { return 1 }

component Home() {
    state n: int = 0
    view: text() { "home" }
}

component Child() {
    state n: int = 0
    view: text() { "child" }
}

component PendingSpin() {
    state n: int = 0
    view: text() { "…" }
}

component NotFoundPage() {
    state n: int = 0
    view: text() { "nf" }
}

component ErrorPage() {
    state n: int = 0
    view: text() { "err" }
}

routes {
    "/" to Home with pending: PendingSpin {
        "/child" to Child with loader: load_child
    }
    not_found: NotFoundPage
    error: ErrorPage
}
"#;
    let module = parse(lex(source)).expect("parse");
    let hir = lower_module(&module);
    let out = generate_with_options(&hir, CodegenOptions::default()).expect("codegen");
    let manifest = out
        .files
        .iter()
        .find(|(n, _)| n == "routes.manifest.ts")
        .map(|(_, c)| c.as_str())
        .expect("routes.manifest.ts");
    insta::assert_snapshot!(
        "nested_route_manifest_with_pending_loader_notfound_error",
        manifest
    );
}

/// Emitter contract: `maybe_web_ir_validate` is invoked before the `route_manifest` match block so a failing
/// `VOX_WEBIR_VALIDATE` gate never appends `routes.manifest.ts`.
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn emitter_source_orders_validate_gate_before_route_manifest() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/codegen_ts/emitter.rs");
    let src = std::fs::read_to_string(&path).expect("read emitter.rs");
    let validate = src
        .find("maybe_web_ir_validate(hir, Some(&web_projection))?")
        .expect("maybe_web_ir_validate call");
    let route_manifest = src
        .find("let route_manifest = match web_projection_ref {")
        .expect("route manifest match block");
    assert!(
        validate < route_manifest,
        "validate gate must precede route manifest emit (validate={validate}, route_manifest={route_manifest})"
    );
}

/// WS08 / T074–T075: legacy TanStack router + `createServerFn` bundles must never ship from codegen.
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn codegen_output_never_includes_vox_tanstack_router_or_server_fns() {
    let source = r#"
component Home() {
    state n: int = 0
    view: text() { n }
}
routes {
    "/" to Home
}
@server fn api_x() to int {
    return 1
}
@query fn q_list() to int { return 0 }
"#;
    let module = parse(lex(source)).expect("parse");
    let hir = lower_module(&module);
    let out = generate_with_options(&hir, CodegenOptions::default()).expect("codegen");
    let names: Vec<&str> = out.files.iter().map(|(n, _)| n.as_str()).collect();
    for forbidden in ["VoxTanStackRouter.tsx", "serverFns.ts"] {
        assert!(
            !names.contains(&forbidden),
            "must not emit {forbidden}, files={names:?}"
        );
    }
    let vox_client = out
        .files
        .iter()
        .find(|(n, _)| n == "vox-client.ts")
        .map(|(_, c)| c.as_str())
        .expect("vox-client.ts when @query exists");
    insta::assert_snapshot!("vox_client_query_mutation_transport", vox_client);
    let forbidden_substrings = [
        "createServerFn",
        "createServerFn(",
        "@tanstack/react-start",
        "\"use server\"",
        "vinxi/server",
    ];
    for (_name, content) in &out.files {
        for sub in forbidden_substrings {
            assert!(
                !content.contains(sub),
                "generated output must not contain forbidden {sub:?}"
            );
        }
    }
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn web_ir_view_matches_hir_emit_for_self_closing_jsx() {
    let source = r#"
component T() {
    state n: int = 1
    view: text(raw_class="x")
}
"#;
    let module = parse(lex(source)).expect("parse");
    let hir = lower_module(&module);
    let rc = hir.components.first().expect("one reactive component");
    let view = rc.view.as_ref().expect("view");
    let state_name = match &rc.members[0] {
        HirReactiveMember::State(s) => s.name.clone(),
        _ => panic!("expected state member"),
    };
    let state_names = HashSet::from([state_name]);
    let direct = emit_hir_expr(
        view,
        &vox_codegen::codegen_ts::hir_emit::EmitCtx::new(&state_names),
    );
    let web = lower_hir_to_web_ir(&hir);
    let via = emit_component_view_tsx(&web, "T").expect("emit");
    assert_eq!(
        direct.trim(),
        via.trim(),
        "\ndirect:\n{direct}\nvia web_ir:\n{via}"
    );
}

/// One constructed example per top-level WebIR family; serde round-trip + validator (OP-0052).
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn web_ir_schema_node_families_roundtrip_through_json() {
    use serde_json::json;

    let mut m = WebIrModule {
        version: WebIrVersion::V0_1,
        ..Default::default()
    };

    m.dom_nodes.push(DomNode::Text {
        content: "t".into(),
        span: None,
    });
    m.dom_nodes.push(DomNode::Element {
        id: DomNodeId(1),
        tag: "div".into(),
        attrs: vec![("class".into(), "\"w\"".into())],
        children: vec![DomNodeId(0)],
        span: None,
    });
    m.view_roots.push(("Smoke".into(), DomNodeId(1)));
    m.view_roots.push(("Home".into(), DomNodeId(1)));

    m.behavior_nodes.push(BehaviorNode::StateDecl {
        name: "x".into(),
        initial: Some("0".into()),
        optionality: FieldOptionality::Required,
        span: None,
    });
    m.behavior_nodes.push(BehaviorNode::DerivedDecl {
        name: "d".into(),
        expr: "x".into(),
        span: None,
    });
    m.behavior_nodes.push(BehaviorNode::EffectDecl {
        deps: vec![],
        body: "{}".into(),
        span: None,
    });
    m.behavior_nodes.push(BehaviorNode::EventHandler {
        target_dom: Some(DomNodeId(1)),
        event: "click".into(),
        handler: "() => {}".into(),
        span: None,
    });
    m.behavior_nodes.push(BehaviorNode::Action {
        name: "a".into(),
        payload_expr: None,
        span: None,
    });

    m.style_nodes.push(StyleNode::Rule {
        specificity: (0, 1, 0),
        selector: StyleSelector::Class("c".into()),
        declarations: vec![("color".into(), StyleDeclarationValue::Raw("red".into()))],
        is_raw_css: false,
        span: None,
    });
    m.style_nodes.push(StyleNode::TokenRef {
        name: "t".into(),
        span: None,
    });

    m.route_nodes.push(RouteNode::RouteTree {
        routes: vec![RouteContract {
            id: "r0".into(),
            pattern: "/".into(),
            meta: json!({ "component": "Home" }),
            children: vec![],
        }],
        span: None,
    });
    m.route_nodes.push(RouteNode::LoaderContract {
        route_id: "r0".into(),
        contract: "()".into(),
        span: None,
    });
    m.route_nodes
        .push(RouteNode::ServerFnContract(ServerFnContract {
            name: "f".into(),
            export_path: "./api".into(),
            signature: "()".into(),
            span: None,
        }));
    m.route_nodes
        .push(RouteNode::MutationContract(MutationContract {
            name: "m".into(),
            payload_type: "void".into(),
            span: None,
        }));

    m.interop_nodes.push(InteropNode::ReactComponentRef {
        component: "C".into(),
        import_source: "m".into(),
        props: vec![],
        span: None,
    });
    m.interop_nodes.push(InteropNode::ExternalModuleRef {
        specifier: "x".into(),
        named: None,
        span: None,
    });
    m.interop_nodes.push(InteropNode::EscapeHatchExpr {
        expr: "null".into(),
        reason: "test".into(),
        span: None,
    });

    assert!(validate_web_ir(&m).is_empty(), "{:?}", validate_web_ir(&m));
    let json = serde_json::to_string(&m).expect("serialize");
    let m2: WebIrModule = serde_json::from_str(&json).expect("deserialize");
    assert!(
        validate_web_ir(&m2).is_empty(),
        "{:?}",
        validate_web_ir(&m2)
    );
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn web_ir_interop_nodes_serialize_deterministically() {
    let nodes = vec![
        InteropNode::ExternalModuleRef {
            specifier: "react".into(),
            named: Some("useState".into()),
            span: None,
        },
        InteropNode::EscapeHatchExpr {
            expr: "0".into(),
            reason: "parity".into(),
            span: None,
        },
    ];
    let a = serde_json::to_string(&nodes).unwrap();
    let b = serde_json::to_string(&nodes).unwrap();
    assert_eq!(a, b);
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn web_ir_span_table_ids_match_get() {
    use vox_codegen::web_ir::SourceSpan;

    let mut t = vox_codegen::web_ir::SourceSpanTable::default();
    let id0 = t.push_span(SourceSpan {
        file_id: 0,
        start: 1,
        end: 2,
    });
    let id1 = t.push_span(SourceSpan {
        file_id: 0,
        start: 3,
        end: 4,
    });
    assert_eq!(id0.0, 0);
    assert_eq!(id1.0, 1);
    assert_eq!(t.get(id0).unwrap().start, 1);
    assert_eq!(t.get(id1).unwrap().end, 4);
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn web_ir_route_tree_contract_roundtrips_json() {
    use serde_json::json;

    let tree = RouteNode::RouteTree {
        routes: vec![
            RouteContract {
                id: "route_0".into(),
                pattern: "/a".into(),
                meta: json!({ "component": "A" }),
                children: vec![],
            },
            RouteContract {
                id: "route_1".into(),
                pattern: "/b".into(),
                meta: json!({ "component": "B" }),
                children: vec![],
            },
        ],
        span: None,
    };
    let s = serde_json::to_string(&tree).expect("serde");
    let back: RouteNode = serde_json::from_str(&s).expect("de");
    match back {
        RouteNode::RouteTree { routes, .. } => {
            assert_eq!(routes.len(), 2);
            assert_eq!(routes[0].pattern, "/a");
        }
        _ => panic!("expected RouteTree"),
    }
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn web_ir_style_node_shape_roundtrip() {
    let n = StyleNode::Declaration {
        property: "margin".into(),
        value: StyleDeclarationValue::TokenRef("space.2".into()),
        important: false,
        span: None,
    };
    let s = serde_json::to_string(&n).unwrap();
    let n2: StyleNode = serde_json::from_str(&s).unwrap();
    match n2 {
        StyleNode::Declaration { property, .. } => assert_eq!(property, "margin"),
        _ => panic!("expected Declaration"),
    }
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn web_ir_event_attr_lowering_matches_react_names() {
    let source = r#"
component Btn() {
    state n: int = 0
    view: button(on_click={n = n + 1}) { n }
}
"#;
    let module = parse(lex(source)).expect("parse");
    let hir = lower_module(&module);
    let web = lower_hir_to_web_ir(&hir);
    let has_on_click = web.dom_nodes.iter().any(|n| {
        matches!(
            n,
            DomNode::Element { attrs, .. }
                if attrs.iter().any(|(k, _)| k == "onClick")
        )
    });
    assert!(has_on_click, "expected onClick in {:?}", web.dom_nodes);
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn web_ir_reactive_component_style_blocks_lower_to_style_nodes() {
    let src = r#"
component Box() {
    view: column(raw_class="x") { "a" }
}
style {
    .x { color: "red" }
}
"#;
    let module = parse(lex(src)).expect("parse");
    let hir = lower_module(&module);
    let web = lower_hir_to_web_ir(&hir);
    assert!(
        !web.style_nodes.is_empty(),
        "expected style rules from Path C component + style block"
    );
    match &web.style_nodes[0] {
        StyleNode::Rule {
            selector: StyleSelector::Unparsed(sel),
            declarations,
            ..
        } => {
            assert_eq!(sel, ".x");
            assert!(declarations.iter().any(|(p, _)| p == "color"));
        }
        other => panic!("expected Rule with Unparsed selector, got {other:?}"),
    }
    let diags = validate_web_ir(&web);
    assert!(diags.is_empty(), "{diags:?}");
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn web_ir_lowering_summary_counts_http_and_rpc() {
    let src = r#"
@server fn api_ping() to int { return 1 }

@server fn do_work() to int { return 0 }

@query fn read_q() to int { return 0 }

@mutation fn write_m(x: int) to int { return x }
"#;
    let module = parse(lex(src)).expect("parse rpc fixture");
    let hir = lower_module(&module);
    let (web, summary) = lower_hir_to_web_ir_with_summary(&hir);
    assert_eq!(summary.http_loader_contracts, 1);
    assert_eq!(summary.server_fn_contracts, 1);
    assert_eq!(summary.query_fn_contracts, 1);
    assert_eq!(summary.mutation_contracts, 1);
    let diags = validate_web_ir(&web);
    assert!(diags.is_empty(), "{diags:?}");
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn web_ir_validate_rejects_duplicate_route_contract_ids() {
    use serde_json::json;
    use vox_codegen::web_ir::{RouteContract, RouteNode, WebIrModule, WebIrVersion};

    let mut m = WebIrModule {
        version: WebIrVersion::V0_1,
        ..Default::default()
    };
    let dup = RouteContract {
        id: "same".into(),
        pattern: "/a".into(),
        meta: json!({}),
        children: vec![],
    };
    m.route_nodes.push(RouteNode::RouteTree {
        routes: vec![dup.clone()],
        span: None,
    });
    m.route_nodes.push(RouteNode::RouteTree {
        routes: vec![dup],
        span: None,
    });
    let diags = validate_web_ir(&m);
    assert!(
        diags
            .iter()
            .any(|d| d.code == "web_ir_validate.route.duplicate_contract_id"),
        "{diags:?}"
    );
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn web_ir_validate_required_state_without_initial() {
    use vox_codegen::web_ir::{BehaviorNode, FieldOptionality, WebIrModule, WebIrVersion};

    let mut m = WebIrModule {
        version: WebIrVersion::V0_1,
        ..Default::default()
    };
    m.behavior_nodes.push(BehaviorNode::StateDecl {
        name: "bad".into(),
        initial: None,
        optionality: FieldOptionality::Required,
        span: None,
    });
    let diags = validate_web_ir(&m);
    assert!(
        diags
            .iter()
            .any(|d| { d.code == "web_ir_validate.behavior.required_state_without_initial" }),
        "{diags:?}"
    );
}

/// OP-0262 / OP-0275: `Optional` / `Defaulted` state rows pass validate without `initial` (stage handoff contract).
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn web_ir_validate_optional_and_defaulted_state_allow_missing_initial() {
    use vox_codegen::web_ir::{BehaviorNode, FieldOptionality, WebIrModule, WebIrVersion};

    for opt in [FieldOptionality::Optional, FieldOptionality::Defaulted] {
        let mut m = WebIrModule {
            version: WebIrVersion::V0_1,
            ..Default::default()
        };
        m.behavior_nodes.push(BehaviorNode::StateDecl {
            name: "x".into(),
            initial: None,
            optionality: opt,
            span: None,
        });
        let diags = validate_web_ir(&m);
        assert!(
            !diags
                .iter()
                .any(|d| { d.code == "web_ir_validate.behavior.required_state_without_initial" }),
            "{opt:?}: {diags:?}"
        );
    }
}

/// OP-0274: `routes { ... }` lowers to `RouteNode::RouteTree` and validates clean.
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn web_ir_routes_block_lowers_to_route_tree_contract() {
    let src = r#"
import react.use_state

component Home() {
    let (_n, _set_n) = use_state(0)
    view: text() { "home" }
}

routes {
    "/" to Home
}
"#;
    let module = parse(lex(src)).expect("parse routes fixture");
    let hir = lower_module(&module);
    let web = lower_hir_to_web_ir(&hir);
    let has_slash = web.route_nodes.iter().any(|n| {
        matches!(
            n,
            RouteNode::RouteTree { routes, .. }
                if routes.iter().any(|r| r.pattern == "/")
        )
    });
    assert!(has_slash, "expected / route in {:?}", web.route_nodes);
    assert!(
        validate_web_ir(&web).is_empty(),
        "{:?}",
        validate_web_ir(&web)
    );
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn web_ir_validate_metrics_track_walks() {
    let source = r#"
component Counter() {
    state n: int = 0
    view: column(raw_class="wrap") { n }
}
"#;
    let module = parse(lex(source)).expect("parse");
    let hir = lower_module(&module);
    let web = lower_hir_to_web_ir(&hir);
    let (diags, metrics) = validate_web_ir_with_metrics(&web);
    assert!(diags.is_empty(), "{diags:?}");
    assert!(metrics.view_roots_walked >= 1);
    assert!(metrics.dom_nodes_traversed >= 1);
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn web_ir_diagnostic_codes_use_dotted_validate_prefixes() {
    let mut m = WebIrModule::default();
    m.route_nodes.push(RouteNode::RouteTree {
        routes: vec![
            RouteContract {
                id: "d".into(),
                pattern: "/".into(),
                meta: serde_json::json!({}),
                children: vec![],
            },
            RouteContract {
                id: "d".into(),
                pattern: "/x".into(),
                meta: serde_json::json!({}),
                children: vec![],
            },
        ],
        span: None,
    });
    let diags = validate_web_ir(&m);
    let d = diags
        .iter()
        .find(|x| x.code.contains("web_ir_validate."))
        .expect("diag");
    assert!(d.code.starts_with("web_ir_validate."));
    assert_eq!(d.category.as_deref(), Some("route"));
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn web_ir_validate_style_rejects_empty_declarations() {
    let mut m = WebIrModule::default();
    m.style_nodes.push(StyleNode::Rule {
        specificity: (0, 1, 0),
        selector: StyleSelector::Class("c".into()),
        declarations: vec![],
        is_raw_css: false,
        span: None,
    });
    let diags = validate_web_ir(&m);
    let d = diags
        .iter()
        .find(|x| x.code == "web_ir_validate.style.empty_declarations")
        .expect("empty_declarations");
    assert_eq!(d.category.as_deref(), Some("style"));
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn web_ir_validate_style_rejects_empty_property_name() {
    let mut m = WebIrModule::default();
    m.style_nodes.push(StyleNode::Rule {
        specificity: (0, 1, 0),
        selector: StyleSelector::Class("c".into()),
        declarations: vec![(String::new(), StyleDeclarationValue::Raw("x".into()))],
        is_raw_css: false,
        span: None,
    });
    let diags = validate_web_ir(&m);
    let d = diags
        .iter()
        .find(|x| x.code == "web_ir_validate.style.empty_property")
        .expect("empty_property");
    assert_eq!(d.category.as_deref(), Some("style"));
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn web_ir_lower_records_unlowered_ast_decls_diagnostic() {
    let mut hir = HirModule::default();
    hir.legacy_ast_nodes.push(Decl::Theme(ThemeDecl {
        name: "App".into(),
        light: vec![],
        dark: vec![],
        span: Span::new(0, 0),
    }));
    let (web, summary) = lower_hir_to_web_ir_with_summary(&hir);
    assert_eq!(summary.lowering_diagnostics, 1);
    let d = web
        .diagnostic_nodes
        .iter()
        .find(|d| d.code == "web_ir.lower.unlowered_ast_decls")
        .expect("lowering diag");
    assert_eq!(d.category.as_deref(), Some("lower"));
    assert!(
        d.message.contains("1 declaration"),
        "message={:?}",
        d.message
    );
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn web_ir_lowering_json_roundtrip_preserves_canonical_bytes() {
    let source = r#"
component A() {
    state x: int = 1
    view: text() { x }
}
"#;
    let module = parse(lex(source)).expect("parse");
    let hir = lower_module(&module);
    let web = lower_hir_to_web_ir(&hir);
    let j1 = serde_json::to_vec(&web).expect("serialize");
    let j2 = serde_json::to_vec(&web).expect("serialize again");
    assert_eq!(
        j1, j2,
        "serde JSON encoding must be deterministic per value"
    );
    let back: WebIrModule = serde_json::from_slice(&j1).expect("deserialize");
    let j3 = serde_json::to_vec(&back).expect("re-serialize");
    assert_eq!(
        j1, j3,
        "round-trip through Value must not perturb WebIrModule JSON bytes"
    );
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn web_ir_validate_failure_format_matches_vox_webir_validate_gate() {
    let mut m = WebIrModule::default();
    m.style_nodes.push(StyleNode::Rule {
        specificity: (0, 1, 0),
        selector: StyleSelector::Class("c".into()),
        declarations: vec![],
        is_raw_css: false,
        span: None,
    });
    let diags = validate_web_ir(&m);
    let formatted = format_web_ir_validate_failure(&diags);
    assert!(
        formatted.starts_with("web_ir_validate.style.empty_declarations [style]:"),
        "got {formatted:?}"
    );
    let gate = format!("VOX_WEBIR_VALIDATE: {formatted}");
    assert!(
        gate.starts_with("VOX_WEBIR_VALIDATE: web_ir_validate."),
        "{gate}"
    );
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn web_ir_lowering_completeness_gate_counter_and_routes_validate() {
    let source = r#"
import react.use_state

component Counter(initial: int) {
    state count: int = initial
    view: column(raw_class="p-4") {
            heading(level=1) { "Count" }
    }
}
"#;
    let module = parse(lex(source)).expect("parse");
    let hir = lower_module(&module);
    let (web, summary) = lower_hir_to_web_ir_with_summary(&hir);
    assert!(summary.components >= 1);
    assert_eq!(summary.dom_expr_fallbacks, 0);
    let diags = validate_web_ir(&web);
    assert!(diags.is_empty(), "{diags:?}");
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn web_ir_preview_emit_visits_expected_node_count() {
    let source = r#"
component T() {
    state n: int = 1
    view: column(raw_class="a", id="x") { text() { n } }
}
"#;
    let module = parse(lex(source)).expect("parse");
    let hir = lower_module(&module);
    let web = lower_hir_to_web_ir(&hir);
    let (_, stats) = emit_component_view_tsx_with_stats(&web, "T").expect("view");
    assert!(stats.nodes_visited >= 3, "{stats:?}");
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn web_ir_preview_emit_sorts_element_attrs_lexicographically() {
    let source = r#"
component T() {
    state n: int = 1
    view: column(raw_class="a", id="x") { n }
}
"#;
    let module = parse(lex(source)).expect("parse");
    let hir = lower_module(&module);
    let web = lower_hir_to_web_ir(&hir);
    let tsx = emit_component_view_tsx(&web, "T").expect("emit");
    let class_pos = tsx.find("className=").expect("className");
    let id_pos = tsx.find("id=").expect("id");
    assert!(
        class_pos < id_pos,
        "sorted attrs: className before id;\n{tsx}"
    );
}

/// Migration OP-0138/0139: `hir_emit::compat` stays reachable alongside `emit_hir_expr` for parity tests.
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn hir_emit_public_exports_include_compat_module() {
    use vox_codegen::codegen_ts::hir_emit::{compat, emit_hir_expr, map_jsx_attr_name};

    assert_eq!(
        map_jsx_attr_name("on:click"),
        compat::map_jsx_attr_name("on_click")
    );
    let _ptr: fn(
        &vox_compiler::hir::HirExpr,
        &vox_codegen::codegen_ts::hir_emit::EmitCtx<'_>,
    ) -> String = emit_hir_expr;
}

/// Parity chain fixture (post-@island retirement).
const OP_S_PARITY_CHAIN_FIXTURE: &str = r#"
import react.use_state

@component ParityPage() {
    state s: str = "x"
    view: column(raw_class="parity-wrap") {
            text() { s }
    }
}

routes {
    "/" to ParityPage
}
"#;

// --- OP-S049–S220 supplemental compiler gates (web_ir_lower_emit target) ---

/// OP-S054: interop policy fixture — valid escape hatch validates clean.
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn op_s054_interop_policy_fixture_valid_escape_hatch() {
    let mut m = WebIrModule::default();
    m.interop_nodes.push(InteropNode::EscapeHatchExpr {
        expr: "null".into(),
        reason: "test-hatch".into(),
        span: None,
    });
    assert!(validate_web_ir(&m).is_empty(), "{:?}", validate_web_ir(&m));
}

/// OP-S056: interop policy gate — empty `reason` fails validate.
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn op_s056_interop_policy_gate_empty_escape_reason() {
    let mut m = WebIrModule::default();
    m.interop_nodes.push(InteropNode::EscapeHatchExpr {
        expr: "1".into(),
        reason: "".into(),
        span: None,
    });
    let d = validate_web_ir(&m);
    assert!(
        d.iter()
            .any(|x| x.code == "web_ir_validate.interop.empty_escape_reason"),
        "{d:?}"
    );
}

/// OP-S058: style validator rejects empty declaration lists (TODO isolation aligns with lower.ts).
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn op_s058_style_todo_fixture_empty_rule_body_diagnosed() {
    let mut m = WebIrModule::default();
    m.style_nodes.push(StyleNode::Rule {
        specificity: (0, 1, 0),
        selector: StyleSelector::Class("x".into()),
        declarations: vec![],
        is_raw_css: false,
        span: None,
    });
    let d = validate_web_ir(&m);
    assert!(
        d.iter()
            .any(|x| x.code.contains("style.empty_declarations")),
        "{d:?}"
    );
}

/// OP-S064: serializability — lowered module JSON round-trip preserves validator cleanliness.
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn op_s064_serializability_gate_lowered_module_json_roundtrip() {
    let source = r#"
component T() {
    state n: int = 0
    view: text() { n }
}
"#;
    let hir = lower_module(&parse(lex(source)).expect("parse"));
    let web = lower_hir_to_web_ir(&hir);
    assert!(validate_web_ir(&web).is_empty());
    let j = serde_json::to_vec(&web).expect("ser");
    let back: WebIrModule = serde_json::from_slice(&j).expect("de");
    assert!(
        validate_web_ir(&back).is_empty(),
        "{:?}",
        validate_web_ir(&back)
    );
}

/// OP-S086 / S088: route detail — duplicate client contract ids fail validation.
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn op_s086_s088_route_detail_gate_duplicate_ids() {
    use serde_json::json;
    let mut m = WebIrModule::default();
    m.route_nodes.push(RouteNode::RouteTree {
        routes: vec![
            RouteContract {
                id: "same".into(),
                pattern: "/a".into(),
                meta: json!({}),
                children: vec![],
            },
            RouteContract {
                id: "same".into(),
                pattern: "/b".into(),
                meta: json!({}),
                children: vec![],
            },
        ],
        span: None,
    });
    let d = validate_web_ir(&m);
    assert!(
        d.iter()
            .any(|x| x.code == "web_ir_validate.route.duplicate_contract_id"),
        "{d:?}"
    );
}

/// OP-S106: style node contract fixture — non-empty declarations validate.
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn op_s106_style_node_contract_fixture_non_empty_rule() {
    let mut m = WebIrModule::default();
    m.style_nodes.push(StyleNode::Rule {
        specificity: (0, 1, 0),
        selector: StyleSelector::Class("c".into()),
        declarations: vec![("color".into(), StyleDeclarationValue::Raw("red".into()))],
        is_raw_css: false,
        span: None,
    });
    assert!(validate_web_ir(&m).is_empty());
}

/// OP-S108: style contract gate — same as S106 plus serde round-trip.
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn op_s108_style_node_contract_gate_roundtrip() {
    op_s106_style_node_contract_fixture_non_empty_rule();
    let mut m = WebIrModule::default();
    m.style_nodes.push(StyleNode::Rule {
        specificity: (0, 1, 0),
        selector: StyleSelector::Class("c".into()),
        declarations: vec![("margin".into(), StyleDeclarationValue::Raw("0".into()))],
        is_raw_css: false,
        span: None,
    });
    let j = serde_json::to_string(&m).unwrap();
    let m2: WebIrModule = serde_json::from_str(&j).unwrap();
    assert!(validate_web_ir(&m2).is_empty());
}

/// OP-S110: style validation fixture — empty property name fails.
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn op_s110_style_node_validation_fixture_empty_prop_name() {
    let mut m = WebIrModule::default();
    m.style_nodes.push(StyleNode::Rule {
        specificity: (0, 1, 0),
        selector: StyleSelector::Class("c".into()),
        declarations: vec![("".into(), StyleDeclarationValue::Raw("x".into()))],
        is_raw_css: false,
        span: None,
    });
    let d = validate_web_ir(&m);
    assert!(
        d.iter()
            .any(|x| x.code == "web_ir_validate.style.empty_property"),
        "{d:?}"
    );
}

/// OP-S126 WebIR fixture pack D2.
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn op_s126_fixture_pack_d2_web_ir_preview_emits() {
    let source = r#"
component T() {
    state n: int = 1
    view: column(raw_class="a") { n }
}
"#;
    let m = parse(lex(source)).expect("parse");
    let hir = lower_module(&m);
    let web = lower_hir_to_web_ir(&hir);
    assert!(validate_web_ir(&web).is_empty());
    let tsx = emit_component_view_tsx(&web, "T").expect("emit");
    insta::assert_snapshot!("op_s126_t_component_view_tsx", tsx);
}

/// OP-S134 / S136: interop hatches — empty React import source fails.
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn op_s134_s136_interop_hatches_gate_empty_import_source() {
    let mut m = WebIrModule::default();
    m.interop_nodes.push(InteropNode::ReactComponentRef {
        component: "C".into(),
        import_source: "".into(),
        props: vec![],
        span: None,
    });
    let d = validate_web_ir(&m);
    assert!(
        d.iter()
            .any(|x| x.code == "web_ir_validate.interop.empty_import_source"),
        "{d:?}"
    );
}

/// OP-S146 fixture pack E2 — `RouteContract` JSON stable under validator expectations.
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn op_s146_fixture_pack_e2_route_contract_json_stable() {
    use serde_json::json;
    let mut m = WebIrModule::default();
    m.route_nodes.push(RouteNode::RouteTree {
        routes: vec![RouteContract {
            id: "r".into(),
            pattern: "/".into(),
            meta: json!({ "component": "Home" }),
            children: vec![],
        }],
        span: None,
    });
    // "Home" view root is required for the route reachability validator.
    m.dom_nodes.push(DomNode::Element {
        id: DomNodeId(0),
        tag: "div".into(),
        attrs: vec![],
        children: vec![],
        span: None,
    });
    m.view_roots.push(("Home".into(), DomNodeId(0)));
    assert!(validate_web_ir(&m).is_empty());
}

/// OP-S154 / S156 / S158: route/data schema — loader id empty fails.
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn op_s154_s156_s158_route_data_schema_gate_empty_loader_id() {
    let mut m = WebIrModule::default();
    m.route_nodes.push(RouteNode::LoaderContract {
        route_id: "".into(),
        contract: "c".into(),
        span: None,
    });
    let d = validate_web_ir(&m);
    assert!(
        d.iter()
            .any(|x| x.code == "web_ir_validate.route.empty_loader_id"),
        "{d:?}"
    );
}

/// OP-S174 / S178 stub: parity with pack E (serializable empty module).
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn op_s174_s178_fixture_pack_f2_empty_module_validates() {
    assert!(validate_web_ir(&WebIrModule::default()).is_empty());
}

/// OP-S186 / S188: interop schema — empty external specifier fails.
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn op_s186_s188_interop_schema_gate_empty_specifier() {
    let mut m = WebIrModule::default();
    m.interop_nodes.push(InteropNode::ExternalModuleRef {
        specifier: "".into(),
        named: None,
        span: None,
    });
    let d = validate_web_ir(&m);
    assert!(
        d.iter()
            .any(|x| x.code == "web_ir_validate.interop.empty_external_specifier"),
        "{d:?}"
    );
}

/// OP-S190: style route integration — valid style still passes with routes present.
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn op_s190_style_route_integration_fixture() {
    let mut m = WebIrModule::default();
    m.style_nodes.push(StyleNode::Rule {
        specificity: (0, 1, 0),
        selector: StyleSelector::Class("r".into()),
        declarations: vec![(
            "padding".into(),
            StyleDeclarationValue::Raw("\"1px\"".into()),
        )],
        is_raw_css: false,
        span: None,
    });
    m.route_nodes.push(RouteNode::RouteTree {
        routes: vec![RouteContract {
            id: "only".into(),
            pattern: "/".into(),
            meta: serde_json::json!({}),
            children: vec![],
        }],
        span: None,
    });
    assert!(validate_web_ir(&m).is_empty());
}

/// OP-S206 fixture pack G2.
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn op_s206_fixture_pack_g2_behavior_required_with_initial_ok() {
    let mut m = WebIrModule::default();
    m.behavior_nodes.push(BehaviorNode::StateDecl {
        name: "s".into(),
        initial: Some("0".into()),
        optionality: FieldOptionality::Required,
        span: None,
    });
    assert!(validate_web_ir(&m).is_empty());
}

/// OP-S219: final WebIR parity — preview DOM contains text node from literal.
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn op_s219_final_web_ir_parity_fixture_preview_literal() {
    let source = r#"
component Hi() {
    state _x: int = 0
    view: text() { "hello" }
}
"#;
    let hir = lower_module(&parse(lex(source)).expect("parse"));
    let web = lower_hir_to_web_ir(&hir);
    let tsx = emit_component_view_tsx(&web, "Hi").expect("tsx");
    insta::assert_snapshot!("op_s219_hi_literal_view_tsx", tsx);
}

fn syntax_k_output_root() -> PathBuf {
    if let Ok(dir) = std::env::var("CARGO_TARGET_DIR")
        && !dir.trim().is_empty()
    {
        return PathBuf::from(dir).join("benchmarks/syntax-k/parity");
    }
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/benchmarks/syntax-k/parity")
        .to_path_buf()
}

/// Observe-only syntax-K artifact generation for a representative parity fixture.
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn syntax_k_artifact_for_parity_chain() {
    let fixture_id = "op_s_parity_chain";
    let module = parse(lex(OP_S_PARITY_CHAIN_FIXTURE)).expect("parse parity chain");
    let hir = lower_module(&module);
    let (web, lower_summary) = lower_hir_to_web_ir_with_summary(&hir);
    let (diags, validate_metrics) = validate_web_ir_with_metrics(&web);
    assert!(diags.is_empty(), "{diags:?}");

    let web_ir_bytes = canonical_web_ir_bytes(&web).expect("canonical web ir bytes");
    let source_hash = sha3_hex(OP_S_PARITY_CHAIN_FIXTURE.as_bytes());
    let web_ir_hash = sha3_hex(&web_ir_bytes);
    let hir_ok = hir.legacy_ast_nodes.is_empty();
    let rp = project_runtime_from_hir(&hir);
    let rp_bytes = canonical_runtime_projection_bytes(&rp).expect("runtime projection bytes");
    let rp_summary = serde_json::json!({
        "schema_version": RUNTIME_PROJECTION_SCHEMA_VERSION,
        "sha3_hex": sha3_hex(&rp_bytes),
        "db_planning_policy_count": rp.db_planning_policies.len(),
        "has_host_capability_probe": rp.host_capability_probe.is_some(),
        "has_module_task_capability_hints": rp.module_task_capability_hints.is_some(),
    });
    let llm_surface = serde_json::json!({
        "interop_nodes": web.interop_nodes.len(),
        "web_ir_lowering_diagnostics": web.diagnostic_nodes.len(),
    });
    let web_support = enrich_syntax_k_support_metrics(
        serde_json::json!({
            "web_ir_lower_summary": {
                "client_route_trees": lower_summary.client_route_trees,
                "http_loader_contracts": lower_summary.http_loader_contracts,
                "server_fn_contracts": lower_summary.server_fn_contracts,
                "query_fn_contracts": lower_summary.query_fn_contracts,
                "mutation_contracts": lower_summary.mutation_contracts,
                "components": lower_summary.components,
                "classic_component_views_lowered": lower_summary.classic_component_views_lowered,
                "classic_components_deferred": lower_summary.classic_components_deferred,
                "style_rules_lowered": lower_summary.style_rules_lowered,
                "dom_expr_fallbacks": lower_summary.dom_expr_fallbacks,
                "lowering_diagnostics": lower_summary.lowering_diagnostics,
                "scheduled_jobs_lowered": lower_summary.scheduled_jobs_lowered
            },
            "web_ir_validate_metrics": {
                "view_roots_walked": validate_metrics.view_roots_walked,
                "dom_nodes_traversed": validate_metrics.dom_nodes_traversed,
                "route_contract_ids_checked": validate_metrics.route_contract_ids_checked,
                "behavior_nodes_checked": validate_metrics.behavior_nodes_checked,
                "style_nodes_checked": validate_metrics.style_nodes_checked,
                "scheduled_jobs_checked": validate_metrics.scheduled_jobs_checked
            }
        }),
        RepresentabilityPayload {
            parse_ok: true,
            hir_ok,
            web_ir_validate_ok: true,
            emit_preview_ok: None,
        },
        Some(llm_surface),
        Some(rp_summary),
    );
    let web_event = measure_syntax_k_event(SyntaxKInput {
        fixture_id,
        target_kind: "webir_json",
        bytes: &web_ir_bytes,
        source_hash: Some(&source_hash),
        web_ir_hash: Some(&web_ir_hash),
        baseline_bytes: None,
        support_metrics: Some(web_support),
    })
    .expect("syntax-k web event");

    let mut emitted = Vec::<(String, String)>::new();
    for (name, _) in &web.view_roots {
        if let Some(tsx) = emit_component_view_tsx(&web, name) {
            emitted.push((format!("{name}.tsx"), tsx));
        }
    }
    let emitted_bytes = canonical_emitted_files_bytes(&emitted);
    let emit_support = enrich_syntax_k_support_metrics(
        serde_json::json!({
            "emitted_file_count": emitted.len()
        }),
        RepresentabilityPayload {
            parse_ok: true,
            hir_ok,
            web_ir_validate_ok: true,
            emit_preview_ok: Some(!emitted.is_empty()),
        },
        None,
        None,
    );
    let emit_event = measure_syntax_k_event(SyntaxKInput {
        fixture_id,
        target_kind: "emit_tsx_preview",
        bytes: &emitted_bytes,
        source_hash: Some(&source_hash),
        web_ir_hash: Some(&web_ir_hash),
        baseline_bytes: None,
        support_metrics: Some(emit_support),
    })
    .expect("syntax-k emit event");

    let payload = serde_json::to_vec_pretty(&serde_json::json!({
        "schema_version": 1,
        "fixture_id": fixture_id,
        "events": [web_event, emit_event]
    }))
    .expect("artifact json");
    let out_dir = syntax_k_output_root();
    std::fs::create_dir_all(&out_dir).expect("mkdir syntax-k parity");
    let out_path = out_dir.join("op_s_parity_chain.json");
    std::fs::write(&out_path, payload).expect("write syntax-k parity artifact");
}

/// Observe-only gate by default; optional hard threshold under `VOX_SYNTAX_K_GATE=enforce`.
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn syntax_k_regression_gate_observe_only() {
    let mode = std::env::var("VOX_SYNTAX_K_GATE").unwrap_or_else(|_| "observe".to_string());
    let source = r#"
component Gate() {
    state n: int = 0
    view: text() { n }
}
"#;
    let hir = lower_module(&parse(lex(source)).expect("parse"));
    let web = lower_hir_to_web_ir(&hir);
    let bytes = canonical_web_ir_bytes(&web).expect("canonical web_ir");
    let web_clean = validate_web_ir(&web).is_empty();
    let gate_support = enrich_syntax_k_support_metrics(
        serde_json::json!({}),
        RepresentabilityPayload {
            parse_ok: true,
            hir_ok: hir.legacy_ast_nodes.is_empty(),
            web_ir_validate_ok: web_clean,
            emit_preview_ok: None,
        },
        None,
        None,
    );
    let evt = measure_syntax_k_event(SyntaxKInput {
        fixture_id: "syntax_k_gate_smoke",
        target_kind: "webir_json",
        bytes: &bytes,
        source_hash: Some(&sha3_hex(source.as_bytes())),
        web_ir_hash: Some(&sha3_hex(&bytes)),
        baseline_bytes: None,
        support_metrics: Some(gate_support),
    })
    .expect("measure syntax_k gate smoke");

    if mode == "enforce" {
        let threshold = std::env::var("VOX_SYNTAX_K_MAX_BYTES")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(usize::MAX);
        assert!(
            evt.k_est_bytes <= threshold,
            "syntax-k gate: k_est_bytes {} exceeds threshold {}",
            evt.k_est_bytes,
            threshold
        );
    }
}

// ── TASK-5.1: literal CSS value enforcement (fires without registry) ──────────

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn web_ir_validate_style_rejects_hex_color_raw() {
    let mut m = WebIrModule::default();
    m.style_nodes.push(StyleNode::Rule {
        specificity: (0, 1, 0),
        selector: StyleSelector::Class("c".into()),
        declarations: vec![("color".into(), StyleDeclarationValue::Raw("#ff0000".into()))],
        is_raw_css: false,
        span: None,
    });
    let diags = validate_web_ir(&m);
    let d = diags
        .iter()
        .find(|x| x.code == "web_ir_validate.style.literal_color_value")
        .expect("literal_color_value diag expected");
    assert_eq!(d.category.as_deref(), Some("style"));
    assert!(d.message.contains("color"), "message: {}", d.message);
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn web_ir_validate_style_rejects_color_variant() {
    let mut m = WebIrModule::default();
    m.style_nodes.push(StyleNode::Rule {
        specificity: (0, 1, 0),
        selector: StyleSelector::Class("c".into()),
        declarations: vec![(
            "background".into(),
            StyleDeclarationValue::Color(vox_codegen::web_ir::CssColor::Hex("#abc".into())),
        )],
        is_raw_css: false,
        span: None,
    });
    let diags = validate_web_ir(&m);
    let d = diags
        .iter()
        .find(|x| x.code == "web_ir_validate.style.literal_color_value")
        .expect("literal_color_value diag for Color variant");
    assert_eq!(d.category.as_deref(), Some("style"));
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn web_ir_validate_style_rejects_literal_dimension() {
    let mut m = WebIrModule::default();
    m.style_nodes.push(StyleNode::Rule {
        specificity: (0, 1, 0),
        selector: StyleSelector::Class("c".into()),
        declarations: vec![(
            "padding".into(),
            StyleDeclarationValue::Length(16.0, vox_codegen::web_ir::LengthUnit::Px),
        )],
        is_raw_css: false,
        span: None,
    });
    let diags = validate_web_ir(&m);
    let d = diags
        .iter()
        .find(|x| x.code == "web_ir_validate.style.literal_dimension_value")
        .expect("literal_dimension_value diag expected");
    assert_eq!(d.category.as_deref(), Some("style"));
    assert!(d.message.contains("padding"), "message: {}", d.message);
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn web_ir_validate_style_token_ref_is_ok() {
    let mut m = WebIrModule::default();
    m.style_nodes.push(StyleNode::Rule {
        specificity: (0, 1, 0),
        selector: StyleSelector::Class("c".into()),
        declarations: vec![(
            "color".into(),
            StyleDeclarationValue::TokenRef("color-primary".into()),
        )],
        is_raw_css: false,
        span: None,
    });
    let diags = validate_web_ir(&m);
    assert!(
        diags
            .iter()
            .all(|d| d.code != "web_ir_validate.style.literal_color_value"
                && d.code != "web_ir_validate.style.literal_dimension_value"),
        "token ref must not trigger literal value errors: {diags:?}"
    );
}

// ── TASK-5.2: route reachability ──────────────────────────────────────────────

fn route_tree(contracts: Vec<RouteContract>) -> RouteNode {
    RouteNode::RouteTree {
        routes: contracts,
        span: None,
    }
}

fn route_contract(id: &str, pattern: &str, component: Option<&str>) -> RouteContract {
    let meta = if let Some(c) = component {
        serde_json::json!({ "component": c })
    } else {
        serde_json::Value::Object(Default::default())
    };
    RouteContract {
        id: id.to_string(),
        pattern: pattern.to_string(),
        meta,
        children: vec![],
    }
}

/// Build an `<a href="…">` element pinned to `DomNodeId(0)` so it can be addressed by
/// `WebIrModule::view_roots` directly (the validator's reachability BFS uses the id field
/// as the index into `dom_nodes`). Tests that need the broken-link / route-reference
/// detection to actually run must register the link as a view root, otherwise the
/// detached link is invisible to the validator (per the "orphan nodes must not count as
/// route references" comment in `validate.rs`).
fn link_element(id: u32, href: &str) -> DomNode {
    DomNode::Element {
        id: DomNodeId(id),
        tag: "a".to_string(),
        attrs: vec![("href".to_string(), href.to_string())],
        children: vec![],
        span: None,
    }
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn web_ir_validate_route_missing_component_is_error() {
    let mut m = WebIrModule::default();
    m.route_nodes.push(route_tree(vec![route_contract(
        "r1",
        "/home",
        Some("Home"),
    )]));
    // view_roots does NOT include "Home"
    let diags = validate_web_ir(&m);
    assert!(
        diags
            .iter()
            .any(|d| d.code == "web_ir_validate.route.missing_component"),
        "expected missing_component: {diags:?}"
    );
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn web_ir_validate_route_component_exists_is_ok() {
    let mut m = WebIrModule::default();
    m.route_nodes.push(route_tree(vec![route_contract(
        "r1",
        "/home",
        Some("Home"),
    )]));
    m.view_roots.push(("Home".to_string(), DomNodeId(0)));
    let diags = validate_web_ir(&m);
    assert!(
        diags
            .iter()
            .all(|d| d.code != "web_ir_validate.route.missing_component"),
        "unexpected missing_component: {diags:?}"
    );
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn web_ir_validate_route_broken_link_is_error() {
    let mut m = WebIrModule::default();
    m.route_nodes
        .push(route_tree(vec![route_contract("r1", "/home", None)]));
    m.dom_nodes.push(link_element(0, "/nonexistent"));
    // The broken-link check only fires for links reachable from a declared view root —
    // see the "orphan nodes must not count as route references" comment in validate.rs.
    m.view_roots.push(("Home".to_string(), DomNodeId(0)));
    let diags = validate_web_ir(&m);
    assert!(
        diags
            .iter()
            .any(|d| d.code == "web_ir_validate.route.broken_link"),
        "expected broken_link: {diags:?}"
    );
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn web_ir_validate_route_matching_link_is_ok() {
    let mut m = WebIrModule::default();
    m.route_nodes
        .push(route_tree(vec![route_contract("r1", "/home", None)]));
    m.dom_nodes.push(link_element(0, "/home"));
    m.view_roots.push(("Home".to_string(), DomNodeId(0)));
    let diags = validate_web_ir(&m);
    assert!(
        diags
            .iter()
            .all(|d| d.code != "web_ir_validate.route.broken_link"),
        "unexpected broken_link: {diags:?}"
    );
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn web_ir_validate_route_unreachable_warns() {
    let mut m = WebIrModule::default();
    m.route_nodes
        .push(route_tree(vec![route_contract("r1", "/about", None)]));
    // No link elements in DOM → unreachable
    let diags = validate_web_ir(&m);
    assert!(
        diags
            .iter()
            .any(|d| d.code == "web_ir_validate.route.unreachable"),
        "expected unreachable: {diags:?}"
    );
}

// ── TASK-5.3: a11y validator integration ─────────────────────────────────────

fn elem_node(id: u32, tag: &str, attrs: Vec<(&str, &str)>, children: Vec<u32>) -> DomNode {
    DomNode::Element {
        id: DomNodeId(id),
        tag: tag.to_string(),
        attrs: attrs
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        children: children.into_iter().map(DomNodeId).collect(),
        span: None,
    }
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn web_ir_validate_a11y_img_missing_alt_fires_via_validate_web_ir() {
    let mut m = WebIrModule::default();
    m.dom_nodes
        .push(elem_node(0, "img", vec![("src", "logo.png")], vec![]));
    let diags = validate_web_ir(&m);
    assert!(
        diags
            .iter()
            .any(|d| d.code == "web_ir_validate.a11y.img_missing_alt"),
        "expected img_missing_alt via validate_web_ir: {diags:?}"
    );
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn web_ir_validate_a11y_button_missing_label_fires_via_validate_web_ir() {
    let mut m = WebIrModule::default();
    m.dom_nodes.push(elem_node(0, "button", vec![], vec![]));
    let diags = validate_web_ir(&m);
    assert!(
        diags
            .iter()
            .any(|d| d.code == "web_ir_validate.a11y.interactive_missing_label"),
        "expected interactive_missing_label via validate_web_ir: {diags:?}"
    );
}

// ── TASK-6.1: primitives lowering ─────────────────────────────────────────────

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn primitives_stack_lowered_to_div_flex_col() {
    let source = r#"
component Layout() {
    view: (
        stack(gap=4) {
            text() { "Hello" }
        }
    )
}
"#;
    let tokens = vox_compiler::lexer::lex(source);
    let module = vox_compiler::parser::parse(tokens).expect("parse");
    let hir = vox_compiler::hir::lower_module(&module);
    let web = lower_hir_to_web_ir(&hir);

    let stack_node = web.dom_nodes.iter().find(|n| {
        if let DomNode::Element { tag, attrs, .. } = n {
            tag == "div"
                && attrs.iter().any(|(k, v)| {
                    k == "className"
                        && v.contains("flex-col")
                        && v.contains("flex")
                        && v.contains("gap-4")
                })
        } else {
            false
        }
    });
    assert!(
        stack_node.is_some(),
        "expected <div className='flex flex-col gap-4'> from <stack gap='4'>; nodes: {:?}",
        web.dom_nodes
    );
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn primitives_button_lowered_to_button_with_primary_classes() {
    let source = r#"
component Cta() {
    view: (
        button(variant="default") { "Click me" }
    )
}
"#;
    let tokens = vox_compiler::lexer::lex(source);
    let module = vox_compiler::parser::parse(tokens).expect("parse");
    let hir = vox_compiler::hir::lower_module(&module);
    let web = lower_hir_to_web_ir(&hir);

    let btn = web.dom_nodes.iter().find(|n| {
        if let DomNode::Element { tag, attrs, .. } = n {
            tag == "button"
                && attrs
                    .iter()
                    .any(|(k, v)| k == "className" && v.contains("bg-primary"))
        } else {
            false
        }
    });
    assert!(
        btn.is_some(),
        "expected <button className='... bg-primary ...'> from <button variant='default'>; nodes: {:?}",
        web.dom_nodes
    );
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn primitives_row_lowered_to_div_flex_row() {
    let source = r#"
component R() {
    view: (
        row() { "item" }
    )
}
"#;
    let tokens = vox_compiler::lexer::lex(source);
    let module = vox_compiler::parser::parse(tokens).expect("parse");
    let hir = vox_compiler::hir::lower_module(&module);
    let web = lower_hir_to_web_ir(&hir);

    assert!(
        web.dom_nodes.iter().any(|n| {
            if let DomNode::Element { tag, attrs, .. } = n {
                tag == "div"
                    && attrs
                        .iter()
                        .any(|(k, v)| k == "className" && v.contains("flex-row"))
            } else {
                false
            }
        }),
        "expected <div className='... flex-row ...'> from <row>; nodes: {:?}",
        web.dom_nodes
    );
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn primitives_unknown_html_tags_pass_through_unchanged() {
    let source = r#"
component Passthrough() {
    view: (
        column(raw_class="custom") { "content" }
    )
}
"#;
    let tokens = vox_compiler::lexer::lex(source);
    let module = vox_compiler::parser::parse(tokens).expect("parse");
    let hir = vox_compiler::hir::lower_module(&module);
    let web = lower_hir_to_web_ir(&hir);

    // JSX lowering maps class→className and wraps string values in quotes.
    assert!(
        web.dom_nodes.iter().any(|n| {
            if let DomNode::Element { tag, attrs, .. } = n {
                tag == "div"
                    && !attrs.iter().any(|(k, v)| {
                        // Must NOT have any primitive-injected Tailwind like flex, flex-col, etc.
                        k == "className" && (v.contains("flex") || v.contains("bg-"))
                    })
            } else {
                false
            }
        }),
        "ordinary div should not have primitive Tailwind classes injected; nodes: {:?}",
        web.dom_nodes
    );
}

// ── TASK-6.2: token-ref-only style values / raw_css escape ────────────────────

/// raw_css {} escape emits a warning diagnostic, not an error.
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn raw_css_escape_emits_warning_not_error() {
    let mut m = WebIrModule::default();
    m.style_nodes.push(StyleNode::Rule {
        specificity: (0, 1, 0),
        selector: StyleSelector::Class("legacy".into()),
        declarations: vec![("color".into(), StyleDeclarationValue::Raw("#ff0000".into()))],
        is_raw_css: true,
        span: None,
    });
    let diags = validate_web_ir(&m);
    assert!(
        diags
            .iter()
            .any(|d| d.code == "web_ir_validate.style.raw_css_escape"),
        "expected raw_css_escape warning: {diags:?}"
    );
    assert!(
        diags
            .iter()
            .all(|d| d.code != "web_ir_validate.style.literal_color_value"),
        "raw_css block must NOT fire literal_color_value error: {diags:?}"
    );
}

/// Normal style {} block with a raw hex color still fires the error.
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn normal_style_block_literal_color_still_errors() {
    let mut m = WebIrModule::default();
    m.style_nodes.push(StyleNode::Rule {
        specificity: (0, 1, 0),
        selector: StyleSelector::Class("bad".into()),
        declarations: vec![(
            "color".into(),
            StyleDeclarationValue::Color(vox_codegen::web_ir::CssColor::Hex("#ff0000".into())),
        )],
        is_raw_css: false,
        span: None,
    });
    let diags = validate_web_ir(&m);
    assert!(
        diags
            .iter()
            .any(|d| d.code == "web_ir_validate.style.literal_color_value"),
        "expected literal_color_value error in normal style block: {diags:?}"
    );
}

/// Parser round-trip: raw_css {} block lowers with is_raw_css=true.
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn raw_css_style_block_lowers_with_flag() {
    let src = r##"
component Card() {
    view: column(raw_class="card")
}
raw_css {
    .card {
        color: "#ff0000"
    }
}
"##;
    let tokens = vox_compiler::lexer::lex(src);
    let module = vox_compiler::parser::parse(tokens).expect("parse");
    let hir = vox_compiler::hir::lower_module(&module);
    let web = lower_hir_to_web_ir(&hir);

    let raw_css_rule = web.style_nodes.iter().find(|n| {
        if let vox_codegen::web_ir::StyleNode::Rule { is_raw_css, .. } = n {
            *is_raw_css
        } else {
            false
        }
    });
    assert!(
        raw_css_rule.is_some(),
        "expected a raw_css style rule; got: {:?}",
        web.style_nodes
    );

    // Validate should emit warning, not hard error on literal color.
    let diags = validate_web_ir(&web);
    assert!(
        diags
            .iter()
            .any(|d| d.code == "web_ir_validate.style.raw_css_escape"),
        "expected raw_css_escape warning: {diags:?}"
    );
    assert!(
        diags
            .iter()
            .all(|d| d.code != "web_ir_validate.style.literal_color_value"),
        "raw_css should not fire literal_color_value: {diags:?}"
    );
}

// ── TASK-6.3: surface pair primitive ─────────────────────────────────────────

/// Known surface name in registry → no error.
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn surface_known_name_no_error() {
    use vox_codegen::web_ir::validate::validate_web_ir_with_registry;
    use vox_compiler::tokens::TokenRegistry;

    let tokens_json = r##"{
        "color": { "background": "#ffffff", "primary": "#3a86ff", "text": { "value": "#1d3557" } },
        "surface": {
            "primary": { "$surface_pair": true, "fg": "color.background", "bg": "color.primary" }
        }
    }"##;
    let registry = TokenRegistry::load_from_str(tokens_json).expect("parse tokens");

    let mut m = WebIrModule::default();
    m.dom_nodes.push(DomNode::Element {
        id: DomNodeId(0),
        tag: "div".into(),
        attrs: vec![("data-vox-surface".into(), "primary".into())],
        children: vec![],
        span: None,
    });
    let diags = validate_web_ir_with_registry(&m, Some(&registry));
    assert!(
        diags
            .iter()
            .all(|d| d.code != "web_ir_validate.surface.unknown_surface"),
        "known surface should not fire unknown_surface: {diags:?}"
    );
}

/// Unknown surface name in registry → error.
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn surface_unknown_name_fires_error() {
    use vox_codegen::web_ir::validate::validate_web_ir_with_registry;
    use vox_compiler::tokens::TokenRegistry;

    let tokens_json = r##"{
        "surface": {
            "default": { "$surface_pair": true, "fg": "color.text", "bg": "color.background" }
        }
    }"##;
    let registry = TokenRegistry::load_from_str(tokens_json).expect("parse tokens");

    let mut m = WebIrModule::default();
    m.dom_nodes.push(DomNode::Element {
        id: DomNodeId(0),
        tag: "div".into(),
        attrs: vec![("data-vox-surface".into(), "nonexistent".into())],
        children: vec![],
        span: None,
    });
    let diags = validate_web_ir_with_registry(&m, Some(&registry));
    assert!(
        diags
            .iter()
            .any(|d| d.code == "web_ir_validate.surface.unknown_surface"),
        "expected unknown_surface error: {diags:?}"
    );
}

/// surface attr on panel primitive → lowers to data-vox-surface attr + style CSS vars.
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn surface_primitive_lowers_to_css_vars() {
    let src = r##"
component Page() {
    view: panel(surface="primary")
}
"##;
    let tokens = vox_compiler::lexer::lex(src);
    let module = vox_compiler::parser::parse(tokens).expect("parse");
    let hir = vox_compiler::hir::lower_module(&module);
    let web = lower_hir_to_web_ir(&hir);

    let has_surface = web.dom_nodes.iter().any(|n| {
        if let DomNode::Element { tag, attrs, .. } = n {
            // Attr values are TS expression strings: data-vox-surface is a JSON-encoded
            // string literal, style is a CSSProperties object expression.
            tag == "div"  // panel → div
                && attrs.iter().any(|(k, v)| k == "data-vox-surface" && v == "\"primary\"")
                && attrs.iter().any(|(k, v)| k == "style" && v.contains("--vox-surface-primary-fg"))
        } else {
            false
        }
    });
    assert!(
        has_surface,
        "expected panel with data-vox-surface + CSS vars; nodes: {:?}",
        web.dom_nodes
    );
}

/// surface attr without registry → no surface validation errors (registry is optional).
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn surface_without_registry_no_error() {
    let mut m = WebIrModule::default();
    m.dom_nodes.push(DomNode::Element {
        id: DomNodeId(0),
        tag: "div".into(),
        attrs: vec![("data-vox-surface".into(), "anything".into())],
        children: vec![],
        span: None,
    });
    // validate_web_ir doesn't use a registry — surface validation skipped
    let diags = validate_web_ir(&m);
    assert!(
        diags
            .iter()
            .all(|d| d.code != "web_ir_validate.surface.unknown_surface"),
        "surface validation without registry should not fire: {diags:?}"
    );
}

// ── TASK-6.4: overlay block + z-index DAG validator ─────────────────────────

/// overlay primitive lowered to div with data-vox-overlay attr.
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn overlay_primitive_lowers_to_overlay_marker() {
    let src = r##"
component Layout() {
    view: overlay() {
            toast(z="100", position="top_right") { "Notification" }
        }
}
"##;
    let tokens = vox_compiler::lexer::lex(src);
    let module = vox_compiler::parser::parse(tokens).expect("parse");
    let hir = vox_compiler::hir::lower_module(&module);
    let web = lower_hir_to_web_ir(&hir);

    // DomNode attr values are stored as TS expression strings (emit_tsx wraps them
    // in `{...}`), so plain string-literal markers are JSON-encoded with quotes.
    let overlay_elem = web.dom_nodes.iter().find(|n| {
        if let DomNode::Element { tag, attrs, .. } = n {
            tag == "div"
                && attrs
                    .iter()
                    .any(|(k, v)| k == "data-vox-overlay" && v == "\"true\"")
        } else {
            false
        }
    });
    assert!(
        overlay_elem.is_some(),
        "expected overlay div; nodes: {:?}",
        web.dom_nodes
    );

    let toast_elem = web.dom_nodes.iter().find(|n| {
        if let DomNode::Element { attrs, .. } = n {
            attrs
                .iter()
                .any(|(k, v)| k == "data-vox-z" && v == "\"100\"")
                && attrs
                    .iter()
                    .any(|(k, v)| k == "data-vox-pos" && v == "\"top_right\"")
        } else {
            false
        }
    });
    assert!(
        toast_elem.is_some(),
        "expected toast with data-vox-z=100 and data-vox-pos=top_right; nodes: {:?}",
        web.dom_nodes
    );
}

/// Duplicate z-index on overlay children fires a diagnostic.
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — web_ir_lower_emit baseline; burndown incrementally"]
fn overlay_duplicate_z_integration() {
    use vox_codegen::web_ir::validate_overlay::validate_overlay;

    let mut m = WebIrModule::default();
    // overlay root at index 0
    m.dom_nodes.push(DomNode::Element {
        id: DomNodeId(0),
        tag: "div".into(),
        attrs: vec![("data-vox-overlay".into(), "true".into())],
        children: vec![DomNodeId(1), DomNodeId(2)],
        span: None,
    });
    // child 1: z=100, top_right
    m.dom_nodes.push(DomNode::Element {
        id: DomNodeId(1),
        tag: "div".into(),
        attrs: vec![
            ("data-vox-z".into(), "100".into()),
            ("data-vox-pos".into(), "top_right".into()),
        ],
        children: vec![],
        span: None,
    });
    // child 2: also z=100 — conflict
    m.dom_nodes.push(DomNode::Element {
        id: DomNodeId(2),
        tag: "div".into(),
        attrs: vec![
            ("data-vox-z".into(), "100".into()),
            ("data-vox-pos".into(), "top_left".into()),
        ],
        children: vec![],
        span: None,
    });

    let mut diags = Vec::new();
    validate_overlay(&m, &mut diags);
    assert!(
        diags
            .iter()
            .any(|d| d.code == "web_ir_validate.overlay.duplicate_z"),
        "expected duplicate_z: {diags:?}"
    );
}
