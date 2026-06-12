//! WebIR validation pass (ADR 012) — structural checks before target emitters.
//!
//! ## Stages (OP-0081)
//! - **DOM** — view root ids, arena bounds, edge walks (existing).
//! - **Routes** — duplicate contract ids, loader / RPC shape (OP-0084, OP-0086).
//! - **Behavior** — [`crate::web_ir::FieldOptionality`] vs initial value (OP-0082).
//! - **Style** — empty rules and declarations (OP-0088).
//! - **Interop** — non-empty fields on [`crate::web_ir::InteropNode`] (ADR 012).
//!
//! Diagnostic **codes** use dotted prefixes (`web_ir_validate.dom.*`, `web_ir_validate.route.*`, …)
//! for dashboards (OP-0092).
//!
//! **Serializability + interop enforcement (OP-S063 / S109 / S135 / S157 / S187):** every
//! `web_ir_validate.*` code should be stable for CI dashboards; [`super::WebIrModule`] JSON round-trips
//! are tested in `web_ir_lower_emit.rs`. Interop rows use `web_ir_validate.interop.*` for escape hatch
//! and import policy.

use std::collections::HashSet;

use super::{
    BehaviorNode, CssColor, DomNode, DomNodeId, FieldOptionality, InteropNode, RouteContract,
    RouteNode, StyleDeclarationValue, StyleNode, StyleSelector, WebIrDiagnostic, WebIrModule,
    WebIrValidateMetrics,
};

fn walk_route_contract_ids(
    nodes: &[RouteContract],
    seen: &mut HashSet<String>,
    out: &mut Vec<WebIrDiagnostic>,
    metrics: &mut WebIrValidateMetrics,
) {
    for r in nodes {
        metrics.route_contract_ids_checked += 1;
        if !seen.insert(r.id.clone()) {
            out.push(WebIrDiagnostic {
                code: "web_ir_validate.route.duplicate_contract_id".to_string(),
                message: format!("duplicate RouteContract.id {:?}", r.id),
                span: None,
                category: Some("route".to_string()),
            });
        }
        if !r.children.is_empty() {
            walk_route_contract_ids(&r.children, seen, out, metrics);
        }
    }
}

fn check_dom_id(out: &mut Vec<WebIrDiagnostic>, len: usize, id: DomNodeId, ctx: &str) -> bool {
    if (id.0 as usize) >= len {
        out.push(WebIrDiagnostic {
            code: "web_ir_validate.dom.id_oob".to_string(),
            message: format!("{ctx}: DomNodeId({}) out of range (len {len})", id.0),
            span: None,
            category: Some("dom".to_string()),
        });
        return false;
    }
    true
}

fn walk_dom_edges(
    out: &mut Vec<WebIrDiagnostic>,
    module: &WebIrModule,
    id: DomNodeId,
    metrics: &mut WebIrValidateMetrics,
) {
    let len = module.dom_nodes.len();
    metrics.dom_nodes_traversed += 1;
    if !check_dom_id(out, len, id, "walk") {
        return;
    }
    let Some(node) = module.dom_nodes.get(id.0 as usize) else {
        return;
    };
    let child_ids: Vec<DomNodeId> = match node {
        DomNode::Element { children, .. } | DomNode::Fragment { children, .. } => children.clone(),
        DomNode::Conditional {
            then_children,
            else_children,
            ..
        } => {
            let mut v = then_children.clone();
            v.extend(else_children.iter().copied());
            v
        }
        DomNode::Loop { body, .. } => body.clone(),
        DomNode::Text { .. } | DomNode::Slot { .. } | DomNode::Expr { .. } => vec![],
    };
    for c in child_ids {
        walk_dom_edges(out, module, c, metrics);
    }
}

fn validate_dom_roots(
    module: &WebIrModule,
    out: &mut Vec<WebIrDiagnostic>,
    metrics: &mut WebIrValidateMetrics,
) {
    if module.dom_nodes.len() > 1_000_000 {
        out.push(WebIrDiagnostic {
            code: "web_ir_validate.dom.arena_too_large".to_string(),
            message: "dom node arena exceeds implementation limit".to_string(),
            span: None,
            category: Some("dom".to_string()),
        });
    }

    for (name, root) in &module.view_roots {
        metrics.view_roots_walked += 1;
        if !check_dom_id(
            out,
            module.dom_nodes.len(),
            *root,
            &format!("view root '{name}'"),
        ) {
            continue;
        }
        walk_dom_edges(out, module, *root, metrics);
    }
}

/// **Stage Routes (OP-S019):** enforces unique [`RouteContract::id`] per module, loader id/contract non-empty,
/// and non-empty server/mutation contract fields — router-facing summaries only; HTTP semantics live on HIR.
fn validate_route_families(
    module: &WebIrModule,
    out: &mut Vec<WebIrDiagnostic>,
    metrics: &mut WebIrValidateMetrics,
) {
    let mut seen_route_contract_ids = HashSet::<String>::new();
    let mut seen_loader_ids = HashSet::<String>::new();

    for node in &module.route_nodes {
        match node {
            RouteNode::RouteTree { routes, .. } => {
                walk_route_contract_ids(routes, &mut seen_route_contract_ids, out, metrics);
            }
            RouteNode::LoaderContract {
                route_id, contract, ..
            } => {
                if !seen_loader_ids.insert(route_id.clone()) {
                    out.push(WebIrDiagnostic {
                        code: "web_ir_validate.route.duplicate_loader_id".to_string(),
                        message: format!("duplicate LoaderContract.route_id {:?}", route_id),
                        span: None,
                        category: Some("route".to_string()),
                    });
                }
                if route_id.is_empty() {
                    out.push(WebIrDiagnostic {
                        code: "web_ir_validate.route.empty_loader_id".to_string(),
                        message: "LoaderContract.route_id must not be empty".to_string(),
                        span: None,
                        category: Some("route".to_string()),
                    });
                }
                if contract.is_empty() {
                    out.push(WebIrDiagnostic {
                        code: "web_ir_validate.route.empty_loader_contract".to_string(),
                        message: "LoaderContract.contract must not be empty".to_string(),
                        span: None,
                        category: Some("route".to_string()),
                    });
                }
            }
            RouteNode::ServerFnContract(s) => {
                if s.name.is_empty() {
                    out.push(WebIrDiagnostic {
                        code: "web_ir_validate.route.empty_server_fn_name".to_string(),
                        message: "ServerFnContract.name must not be empty".to_string(),
                        span: None,
                        category: Some("route".to_string()),
                    });
                }
                if s.export_path.is_empty() {
                    out.push(WebIrDiagnostic {
                        code: "web_ir_validate.route.empty_server_export_path".to_string(),
                        message: "ServerFnContract.export_path must not be empty".to_string(),
                        span: None,
                        category: Some("route".to_string()),
                    });
                }
                if s.signature.is_empty() {
                    out.push(WebIrDiagnostic {
                        code: "web_ir_validate.route.empty_server_signature".to_string(),
                        message: "ServerFnContract.signature must not be empty".to_string(),
                        span: None,
                        category: Some("route".to_string()),
                    });
                }
            }
            RouteNode::MutationContract(m) => {
                if m.name.is_empty() {
                    out.push(WebIrDiagnostic {
                        code: "web_ir_validate.route.empty_mutation_name".to_string(),
                        message: "MutationContract.name must not be empty".to_string(),
                        span: None,
                        category: Some("route".to_string()),
                    });
                }
                if m.payload_type.is_empty() {
                    out.push(WebIrDiagnostic {
                        code: "web_ir_validate.route.empty_mutation_payload_type".to_string(),
                        message: "MutationContract.payload_type must not be empty".to_string(),
                        span: None,
                        category: Some("route".to_string()),
                    });
                }
            }
        }
    }
}

/// **Behavior / optionality (OP-S017):** currently guards `StateDecl` — [`FieldOptionality::Required`] must
/// ship a lowered `initial`; extend here for `Optional`/`Defaulted` invariants on props.
fn validate_behaviors(
    module: &WebIrModule,
    out: &mut Vec<WebIrDiagnostic>,
    metrics: &mut WebIrValidateMetrics,
) {
    for b in &module.behavior_nodes {
        metrics.behavior_nodes_checked += 1;
        if let BehaviorNode::StateDecl {
            optionality,
            initial,
            name,
            ..
        } = b
            && matches!(optionality, FieldOptionality::Required)
            && initial.is_none()
        {
            out.push(WebIrDiagnostic {
                code: "web_ir_validate.behavior.required_state_without_initial".to_string(),
                message: format!(
                    "StateDecl '{name}' is Required but has no lowered initial expression"
                ),
                span: None,
                category: Some("behavior".to_string()),
            });
        }
    }
}

fn validate_styles_with_registry(
    module: &WebIrModule,
    out: &mut Vec<WebIrDiagnostic>,
    metrics: &mut WebIrValidateMetrics,
    registry: Option<&vox_compiler::tokens::TokenRegistry>,
) {
    let mut seen_selectors: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();

    for node in &module.style_nodes {
        metrics.style_nodes_checked += 1;
        if let StyleNode::Rule {
            selector,
            declarations,
            is_raw_css,
            ..
        } = node
        {
            // raw_css {} escape hatch: emit a single warning per rule instead of errors.
            if *is_raw_css {
                out.push(WebIrDiagnostic {
                    code: "web_ir_validate.style.raw_css_escape".to_string(),
                    message: "raw_css {{ }} escape hatch used — prefer design tokens for all style values".to_string(),
                    span: None,
                    category: Some("style".to_string()),
                });
                continue;
            }
            let sel_key = match selector {
                StyleSelector::Class(c) => format!(".{}", c),
                StyleSelector::Id(i) => format!("#{}", i),
                StyleSelector::Element(e) => e.clone(),
                StyleSelector::Unparsed(u) => u.clone(),
                StyleSelector::Compound(_) => "compound".to_string(), // Simplified for now
                StyleSelector::Pseudo { pseudo, .. } => format!("pseudo:{}", pseudo),
            };

            if declarations.is_empty() {
                out.push(WebIrDiagnostic {
                    code: "web_ir_validate.style.empty_declarations".to_string(),
                    message: "StyleNode::Rule has no declarations".to_string(),
                    span: None,
                    category: Some("style".to_string()),
                });
            }

            let mut props_in_rule = std::collections::HashSet::new();
            for (prop, decl_val) in declarations {
                if prop.is_empty() {
                    out.push(WebIrDiagnostic {
                        code: "web_ir_validate.style.empty_property".to_string(),
                        message: "style declaration property name must not be empty".to_string(),
                        span: None,
                        category: Some("style".to_string()),
                    });
                } else {
                    let css_prop = prop.chars().fold(String::new(), |mut acc, c| {
                        if c.is_uppercase() {
                            acc.push('-');
                            acc.push(c.to_ascii_lowercase());
                        } else {
                            acc.push(c);
                        }
                        acc
                    });

                    if !props_in_rule.insert(css_prop.clone()) {
                        out.push(WebIrDiagnostic {
                            code: "web_ir_validate.style.duplicate_property_in_rule".to_string(),
                            message: format!("Duplicate property '{}' in the same rule", prop),
                            span: None,
                            category: Some("style".to_string()),
                        });
                    }

                    if !vox_compiler::lowering_shared::css_props::is_allowed_css_property(&css_prop)
                    {
                        out.push(WebIrDiagnostic {
                            code: "web_ir_validate.style.unknown_property".to_string(),
                            message: format!(
                                "Unknown CSS property '{}' (normalized to '{}')",
                                prop, css_prop
                            ),
                            span: None,
                            category: Some("style".to_string()),
                        });
                    }

                    // TASK-5.1: literal CSS value enforcement (fires regardless of registry).
                    check_literal_value(prop, decl_val, out);

                    if let Some(existing_props) = seen_selectors.get(&sel_key)
                        && existing_props.contains(&css_prop)
                    {
                        out.push(WebIrDiagnostic {
                                code: "web_ir_validate.style.specificity_conflict".to_string(),
                                message: format!("Property '{}' redefined for selector '{}' at same specificity level", prop, sel_key),
                                span: None,
                                category: Some("style".to_string()),
                            });
                    }

                    // Token registry checks (only when a registry is loaded)
                    if let Some(reg) = registry {
                        check_declaration_against_registry(prop, decl_val, reg, out);
                    }
                }
            }

            let normalized_props: Vec<String> = declarations
                .iter()
                .map(|(p, _)| {
                    p.chars().fold(String::new(), |mut acc, c| {
                        if c.is_uppercase() {
                            acc.push('-');
                            acc.push(c.to_ascii_lowercase());
                        } else {
                            acc.push(c);
                        }
                        acc
                    })
                })
                .collect();

            seen_selectors
                .entry(sel_key)
                .or_default()
                .extend(normalized_props);
        }
    }

    // TASK-6.3: validate data-vox-surface attrs against registered surface pairs.
    if let Some(reg) = registry {
        validate_surface_refs(module, reg, out);
    }

    // Validate WCAG contrast pairs declared in the token file
    if let Some(reg) = registry {
        for diag in reg.validate_contrast() {
            let (code, severity_label) = match diag.severity {
                vox_compiler::tokens::ContrastSeverity::Warning => {
                    ("web_ir_validate.style.token_contrast_warning", "warning")
                }
                vox_compiler::tokens::ContrastSeverity::Error => {
                    ("web_ir_validate.style.token_contrast_error", "error")
                }
            };
            out.push(WebIrDiagnostic {
                code: code.to_string(),
                message: format!(
                    "Token contrast {}: '{}' on '{}' is {:.2}:1 (requires ≥{:.1}:1 per WCAG 2.1)",
                    severity_label,
                    diag.foreground_key,
                    diag.background_key,
                    diag.ratio,
                    diag.threshold
                ),
                span: None,
                category: Some("style".to_string()),
            });
        }
    }
}

/// TASK-5.2: route reachability — component existence + broken link detection + unreachable routes.
fn validate_route_reachability(
    module: &WebIrModule,
    out: &mut Vec<WebIrDiagnostic>,
    _metrics: &mut WebIrValidateMetrics,
) {
    // Collect (pattern, Option<component_name>) from all route trees.
    let mut route_entries: Vec<(String, Option<String>)> = Vec::new();
    for node in &module.route_nodes {
        if let RouteNode::RouteTree { routes, .. } = node {
            collect_route_entries(routes, &mut route_entries);
        }
    }
    if route_entries.is_empty() {
        return;
    }

    // Check that component names declared in route meta exist as view roots.
    let view_root_names: HashSet<&str> =
        module.view_roots.iter().map(|(n, _)| n.as_str()).collect();
    for (pattern, component_opt) in &route_entries {
        if let Some(component) = component_opt
            && !component.is_empty()
            && !view_root_names.contains(component.as_str())
        {
            out.push(WebIrDiagnostic {
                code: "web_ir_validate.route.missing_component".to_string(),
                message: format!(
                    "Route '{}' declares component '{}' but no matching view root exists",
                    pattern, component
                ),
                span: None,
                category: Some("route".to_string()),
            });
        }
    }

    // Collect all literal href/to values from link elements in the DOM arena,
    // but only from nodes that are reachable from a declared view root.
    // Orphan / detached nodes must not count as route references — they could
    // hide `web_ir_validate.route.unreachable` diagnostics.
    let known_patterns: HashSet<&str> = route_entries.iter().map(|(p, _)| p.as_str()).collect();
    let mut referenced: HashSet<String> = HashSet::new();

    // BFS/DFS from every view root to collect the reachable node set.
    let mut reachable: HashSet<DomNodeId> = HashSet::new();
    let mut work: Vec<DomNodeId> = module.view_roots.iter().map(|(_, id)| *id).collect();
    while let Some(id) = work.pop() {
        if !reachable.insert(id) {
            continue;
        }
        let Some(node) = module.dom_nodes.get(id.0 as usize) else {
            continue;
        };
        match node {
            DomNode::Element { children, .. } => work.extend(children.iter().copied()),
            DomNode::Fragment { children, .. } => work.extend(children.iter().copied()),
            DomNode::Conditional {
                then_children,
                else_children,
                ..
            } => {
                work.extend(then_children.iter().copied());
                work.extend(else_children.iter().copied());
            }
            DomNode::Loop { body, .. } => work.extend(body.iter().copied()),
            _ => {}
        }
    }

    for id in &reachable {
        let Some(node) = module.dom_nodes.get(id.0 as usize) else {
            continue;
        };
        let DomNode::Element { tag, attrs, .. } = node else {
            continue;
        };
        let is_link = tag == "a" || tag == "link";
        if !is_link {
            continue;
        }
        for (key, val) in attrs {
            if key == "href" || key == "to" {
                // Strip surrounding quotes: DOM arena stores string literals as `"/path"`.
                let href = val.trim().trim_matches('"').trim_matches('\'');
                if href.starts_with('/') {
                    referenced.insert(href.to_string());
                    if !known_patterns.contains(href) {
                        out.push(WebIrDiagnostic {
                            code: "web_ir_validate.route.broken_link".to_string(),
                            message: format!(
                                "Link href '{}' does not match any declared route pattern",
                                href
                            ),
                            span: None,
                            category: Some("route".to_string()),
                        });
                    }
                }
            }
        }
    }

    // Warn on routes with no inbound literal link (unreachable).
    for (pattern, _) in &route_entries {
        if pattern == "/" {
            continue; // root is always reachable
        }
        if !referenced.contains(pattern.as_str()) {
            out.push(WebIrDiagnostic {
                code: "web_ir_validate.route.unreachable".to_string(),
                message: format!(
                    "Route '{}' has no inbound link in the DOM (may be unreachable via dynamic navigation)",
                    pattern
                ),
                span: None,
                category: Some("route".to_string()),
            });
        }
    }
}

fn collect_route_entries(contracts: &[RouteContract], out: &mut Vec<(String, Option<String>)>) {
    for c in contracts {
        let component = c
            .meta
            .get("component")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        out.push((c.pattern.clone(), component));
        if !c.children.is_empty() {
            collect_route_entries(&c.children, out);
        }
    }
}

/// TASK-5.1: reject literal CSS color and dimension values regardless of token registry.
/// Fires unconditionally so that raw `#rrggbb` / `rgb(…)` / `42px` values are always
/// compile errors, not just warnings gated on a loaded registry.
fn check_literal_value(
    prop: &str,
    decl_val: &StyleDeclarationValue,
    out: &mut Vec<WebIrDiagnostic>,
) {
    match decl_val {
        StyleDeclarationValue::Raw(s) => {
            let s = s.trim();
            let is_hex_color = s.starts_with('#') && {
                let rest = &s[1..];
                (rest.len() == 3 || rest.len() == 4 || rest.len() == 6 || rest.len() == 8)
                    && rest.chars().all(|c| c.is_ascii_hexdigit())
            };
            let is_functional_color = s.starts_with("rgb(")
                || s.starts_with("rgba(")
                || s.starts_with("hsl(")
                || s.starts_with("hsla(");
            let is_dimension = {
                let suffixes = ["px", "rem", "em", "%", "vh", "vw", "vmin", "vmax"];
                suffixes.iter().any(|suf| {
                    s.ends_with(suf) && s[..s.len() - suf.len()].trim().parse::<f64>().is_ok()
                })
            };
            if is_hex_color || is_functional_color {
                out.push(WebIrDiagnostic {
                    code: "web_ir_validate.style.literal_color_value".to_string(),
                    message: format!(
                        "Literal color value on property '{}'. Use a design token (tokens.<name>) instead.",
                        prop
                    ),
                    span: None,
                    category: Some("style".to_string()),
                });
            } else if is_dimension {
                out.push(WebIrDiagnostic {
                    code: "web_ir_validate.style.literal_dimension_value".to_string(),
                    message: format!(
                        "Literal dimension value '{}' on property '{}'. Use a design token instead.",
                        s, prop
                    ),
                    span: None,
                    category: Some("style".to_string()),
                });
            }
        }
        StyleDeclarationValue::Color(color) => {
            let is_literal = matches!(
                color,
                CssColor::Hex(_)
                    | CssColor::Rgb(_, _, _)
                    | CssColor::Rgba(_, _, _, _)
                    | CssColor::Named(_)
                    | CssColor::Hsl(_, _, _)
            );
            if is_literal {
                out.push(WebIrDiagnostic {
                    code: "web_ir_validate.style.literal_color_value".to_string(),
                    message: format!(
                        "Literal color value on property '{}'. Use a design token (tokens.<name>) instead.",
                        prop
                    ),
                    span: None,
                    category: Some("style".to_string()),
                });
            }
        }
        StyleDeclarationValue::Length(_, _) => {
            out.push(WebIrDiagnostic {
                code: "web_ir_validate.style.literal_dimension_value".to_string(),
                message: format!(
                    "Literal dimension value on property '{}'. Use a design token instead.",
                    prop
                ),
                span: None,
                category: Some("style".to_string()),
            });
        }
        StyleDeclarationValue::TokenRef(_)
        | StyleDeclarationValue::Keyword(_)
        | StyleDeclarationValue::Number(_) => {}
    }
}

fn check_declaration_against_registry(
    prop: &str,
    decl_val: &StyleDeclarationValue,
    registry: &vox_compiler::tokens::TokenRegistry,
    out: &mut Vec<WebIrDiagnostic>,
) {
    match decl_val {
        StyleDeclarationValue::TokenRef(token_ref) => {
            // token_ref is stored as "vox-color-primary"; strip "vox-" to get registry key
            let lookup_key = token_ref.strip_prefix("vox-").unwrap_or(token_ref.as_str());
            if registry.lookup(lookup_key).is_none() {
                let suggestions = vox_compiler::tokens::suggest_tokens(lookup_key, registry);
                let hint = if suggestions.is_empty() {
                    String::new()
                } else {
                    format!(" Did you mean: {}?", suggestions.join(", "))
                };
                out.push(WebIrDiagnostic {
                    code: "web_ir_validate.style.unknown_token".to_string(),
                    message: format!(
                        "Unknown token reference '{}' (not defined in vox.tokens.json).{}",
                        token_ref, hint
                    ),
                    span: None,
                    category: Some("style".to_string()),
                });
            }
        }
        StyleDeclarationValue::Color(color) => {
            let is_color_prop = prop == "color"
                || prop.ends_with("color")
                || prop == "background"
                || prop == "fill"
                || prop == "stroke";
            if is_color_prop {
                let is_literal = matches!(
                    color,
                    CssColor::Hex(_)
                        | CssColor::Rgb(_, _, _)
                        | CssColor::Rgba(_, _, _, _)
                        | CssColor::Named(_)
                        | CssColor::Hsl(_, _, _)
                );
                if is_literal {
                    out.push(WebIrDiagnostic {
                        code: "web_ir_validate.style.raw_color_value".to_string(),
                        message: format!(
                            "Raw color literal on property '{}'. Use a design token (tokens.<name>) for compile-time contrast validation.",
                            prop
                        ),
                        span: None,
                        category: Some("style".to_string()),
                    });
                }
            }
        }
        _ => {}
    }
}

fn validate_scheduled_jobs(
    module: &WebIrModule,
    out: &mut Vec<WebIrDiagnostic>,
    metrics: &mut WebIrValidateMetrics,
) {
    for job in &module.scheduled_jobs {
        metrics.scheduled_jobs_checked += 1;
        if job.name.trim().is_empty() {
            out.push(WebIrDiagnostic {
                code: "web_ir_validate.scheduled.empty_name".to_string(),
                message: "ScheduledJobSpec.name must not be empty".to_string(),
                span: None,
                category: Some("scheduled".to_string()),
            });
        }
        if job.interval.trim().is_empty() {
            out.push(WebIrDiagnostic {
                code: "web_ir_validate.scheduled.empty_interval".to_string(),
                message: "ScheduledJobSpec.interval must not be empty".to_string(),
                span: None,
                category: Some("scheduled".to_string()),
            });
        }
    }
}

fn validate_interop(module: &WebIrModule, out: &mut Vec<WebIrDiagnostic>) {
    for node in &module.interop_nodes {
        match node {
            InteropNode::ReactComponentRef {
                component,
                import_source,
                ..
            } => {
                if component.is_empty() {
                    out.push(WebIrDiagnostic {
                        code: "web_ir_validate.interop.empty_component".to_string(),
                        message: "ReactComponentRef.component must not be empty".to_string(),
                        span: None,
                        category: Some("interop".to_string()),
                    });
                }
                if import_source.is_empty() {
                    out.push(WebIrDiagnostic {
                        code: "web_ir_validate.interop.empty_import_source".to_string(),
                        message: "ReactComponentRef.import_source must not be empty".to_string(),
                        span: None,
                        category: Some("interop".to_string()),
                    });
                }
            }
            InteropNode::ExternalModuleRef { specifier, .. } => {
                if specifier.is_empty() {
                    out.push(WebIrDiagnostic {
                        code: "web_ir_validate.interop.empty_external_specifier".to_string(),
                        message: "ExternalModuleRef.specifier must not be empty".to_string(),
                        span: None,
                        category: Some("interop".to_string()),
                    });
                }
            }
            InteropNode::EscapeHatchExpr { expr, reason, .. } => {
                if expr.is_empty() {
                    out.push(WebIrDiagnostic {
                        code: "web_ir_validate.interop.empty_escape_expr".to_string(),
                        message: "EscapeHatchExpr.expr must not be empty".to_string(),
                        span: None,
                        category: Some("interop".to_string()),
                    });
                }
                if reason.is_empty() {
                    out.push(WebIrDiagnostic {
                        code: "web_ir_validate.interop.empty_escape_reason".to_string(),
                        message: "EscapeHatchExpr.reason must not be empty".to_string(),
                        span: None,
                        category: Some("interop".to_string()),
                    });
                }
            }
        }
    }
}

/// Semicolon-joined diagnostic lines for `VOX_WEBIR_VALIDATE` gates (OP-0287); shared with codegen.
#[must_use]
pub fn format_web_ir_validate_failure(diags: &[WebIrDiagnostic]) -> String {
    diags
        .iter()
        .map(|d| {
            format!(
                "{} [{}]: {}",
                d.code,
                d.category.as_deref().unwrap_or("-"),
                d.message
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// TASK-6.3: walk DOM arena checking `data-vox-surface` attrs against registered surface pairs.
/// Fires `web_ir_validate.surface.unknown_surface` when the name is not in the registry.
fn validate_surface_refs(
    module: &WebIrModule,
    registry: &vox_compiler::tokens::TokenRegistry,
    out: &mut Vec<WebIrDiagnostic>,
) {
    for node in &module.dom_nodes {
        let DomNode::Element { attrs, .. } = node else {
            continue;
        };
        for (k, v) in attrs {
            if k != "data-vox-surface" {
                continue;
            }
            // Attr values are TS-expression strings (see lower.rs convention comment) —
            // a plain string literal arrives JSON-encoded with embedded quotes. Strip one
            // layer before registry lookup.
            let surface_name = v.trim_matches('"');
            if registry.lookup_surface(surface_name).is_none() {
                let known: Vec<&str> = registry.surface_pairs.keys().map(|s| s.as_str()).collect();
                let hint = if known.is_empty() {
                    String::new()
                } else {
                    format!(" Known surfaces: {}.", known.join(", "))
                };
                out.push(WebIrDiagnostic {
                    code: "web_ir_validate.surface.unknown_surface".to_string(),
                    message: format!(
                        "Unknown surface pair '{surface_name}' — not declared in vox.tokens.json.{hint}"
                    ),
                    span: None,
                    category: Some("surface".to_string()),
                });
            }
        }
    }
}

/// Run structural checks that should hold before any target emitter, with counters for gates (OP-0094).
#[must_use]
pub fn validate_web_ir_with_metrics(
    module: &WebIrModule,
) -> (Vec<WebIrDiagnostic>, WebIrValidateMetrics) {
    validate_web_ir_full(module, None)
}

/// Run structural checks including token registry validation.
///
/// Pass `Some(&registry)` to enable unknown-token errors and raw-color warnings (TASK-4.4).
/// Callers that have no project root (e.g. unit tests over isolated modules) can pass `None`.
#[must_use]
pub fn validate_web_ir_with_registry(
    module: &WebIrModule,
    registry: Option<&vox_compiler::tokens::TokenRegistry>,
) -> Vec<WebIrDiagnostic> {
    validate_web_ir_full(module, registry).0
}

/// Run structural checks that should hold before any target emitter.
#[must_use]
pub fn validate_web_ir(module: &WebIrModule) -> Vec<WebIrDiagnostic> {
    validate_web_ir_with_metrics(module).0
}

/// Returns true for diagnostics that are advisory (soft warnings, not build blockers).
///
/// Advisory diagnostics are informational — callers that gate builds should filter these out;
/// callers that surface all diagnostics (LSP, dashboards) should still include them.
///
/// A11y accessibility violations (`web_ir_validate.a11y.*`) are advisory so that the reactive
/// emitter still produces valid JSX when an input lacks a label.  The violations are surfaced as
/// compiler warnings / LSP diagnostics, but they must not replace emitted components with the
/// failure placeholder — that would make the app non-functional, which is worse than an
/// accessible-name gap.
#[must_use]
pub fn is_advisory_diagnostic(d: &WebIrDiagnostic) -> bool {
    matches!(
        d.code.as_str(),
        "web_ir_validate.style.raw_css_escape"
            | "web_ir_validate.overlay.duplicate_z"
            | "web_ir_validate.overlay.position_conflict"
            | "web_ir_validate.route.unreachable"
            | "web_ir_validate.a11y.input_missing_label"
            | "web_ir_validate.a11y.button_missing_name"
            | "web_ir_validate.a11y.anchor_missing_name"
            | "web_ir_validate.a11y.anchor_missing_href"
            | "web_ir_validate.a11y.img_missing_alt"
            | "web_ir_validate.a11y.role_button_missing_keyboard"
    ) || d.code.ends_with("_warning")
        // The a11y family is advisory so emit still produces JSX on a name gap —
        // EXCEPT the hard WCAG contrast failure, which can block while still emitting.
        || (d.code.starts_with("web_ir_validate.a11y.")
            && d.code != "web_ir_validate.a11y.insufficient_contrast")
}

/// Internal combined validator used by `validate_web_ir_with_metrics` and `validate_web_ir_with_registry`.
#[must_use]
fn validate_web_ir_full(
    module: &WebIrModule,
    registry: Option<&vox_compiler::tokens::TokenRegistry>,
) -> (Vec<WebIrDiagnostic>, WebIrValidateMetrics) {
    let mut out = Vec::new();
    let mut metrics = WebIrValidateMetrics::default();

    validate_dom_roots(module, &mut out, &mut metrics);
    validate_route_families(module, &mut out, &mut metrics);
    validate_route_reachability(module, &mut out, &mut metrics);
    validate_behaviors(module, &mut out, &mut metrics);
    validate_styles_with_registry(module, &mut out, &mut metrics, registry);
    validate_scheduled_jobs(module, &mut out, &mut metrics);
    validate_interop(module, &mut out);
    out.extend(super::validate_keys::validate_keys(module));
    super::validate_a11y::validate_a11y(module, &mut out);
    if let Some(reg) = registry {
        super::validate_a11y::validate_a11y_with_registry(module, reg, &mut out);
    }
    super::validate_overlay::validate_overlay(module, &mut out);

    (out, metrics)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_route(id: &str, pattern: &str, component: Option<&str>) -> RouteContract {
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

    #[test]
    fn insufficient_contrast_is_not_advisory() {
        // The <3:1 hard WCAG failure must block the build even though the rest of
        // the a11y family stays advisory (so emit still produces JSX).
        let hard = WebIrDiagnostic {
            code: "web_ir_validate.a11y.insufficient_contrast".to_string(),
            message: String::new(),
            span: None,
            category: Some("a11y".to_string()),
        };
        assert!(
            !is_advisory_diagnostic(&hard),
            "hard WCAG failure must block the build"
        );

        let soft = WebIrDiagnostic {
            code: "web_ir_validate.a11y.low_contrast".to_string(),
            message: String::new(),
            span: None,
            category: Some("a11y".to_string()),
        };
        assert!(is_advisory_diagnostic(&soft), "low_contrast stays advisory");

        let name_gap = WebIrDiagnostic {
            code: "web_ir_validate.a11y.img_missing_alt".to_string(),
            message: String::new(),
            span: None,
            category: Some("a11y".to_string()),
        };
        assert!(
            is_advisory_diagnostic(&name_gap),
            "accessible-name gaps stay advisory"
        );
    }

    #[test]
    fn surface_lookup_tolerates_quoted_attr_values() {
        use crate::web_ir::{DomNode, DomNodeId, WebIrModule};
        // Real lowering (lower.rs:280) stores data-vox-surface as a JSON-encoded
        // string literal — value carries embedded quotes (`"\"primary\""`). The
        // lookup sites must strip them or every registry lookup misses.
        let tokens_json = r##"{
            "color": { "background": "#ffffff", "primary": "#3a86ff" },
            "surface": {
                "primary": { "$surface_pair": true, "fg": "color.background", "bg": "color.primary" }
            }
        }"##;
        let registry = vox_compiler::tokens::TokenRegistry::load_from_str(tokens_json)
            .expect("parse tokens");
        let mut m = WebIrModule::default();
        m.dom_nodes.push(DomNode::Element {
            id: DomNodeId(0),
            tag: "section".to_string(),
            attrs: vec![("data-vox-surface".to_string(), "\"primary\"".to_string())],
            children: vec![],
            span: None,
        });
        m.view_roots.push(("Page".to_string(), DomNodeId(0)));

        let diags = validate_web_ir_with_registry(&m, Some(&registry));
        assert!(
            !diags
                .iter()
                .any(|d| d.code == "web_ir_validate.surface.unknown_surface"),
            "quoted surface value must resolve against the registry, got: {diags:?}"
        );
    }

    #[test]
    fn route_with_known_component_passes() {
        use crate::web_ir::{DomNode, DomNodeId, WebIrModule};
        let mut m = WebIrModule::default();
        m.dom_nodes.push(DomNode::Element {
            id: DomNodeId(0),
            tag: "div".to_string(),
            attrs: vec![],
            children: vec![],
            span: None,
        });
        m.view_roots.push(("HomePage".to_string(), DomNodeId(0)));
        m.route_nodes.push(RouteNode::RouteTree {
            routes: vec![make_route("route_0", "/", Some("HomePage"))],
            span: None,
        });
        let diags = validate_web_ir(&m);
        assert!(
            !diags
                .iter()
                .any(|d| d.code == "web_ir_validate.route.missing_component"),
            "component exists in view_roots — no missing_component diag expected"
        );
    }

    #[test]
    fn route_with_missing_component_warns() {
        use crate::web_ir::WebIrModule;
        let mut m = WebIrModule::default();
        // No view_roots, but route references "HomePage".
        m.route_nodes.push(RouteNode::RouteTree {
            routes: vec![make_route("route_0", "/home", Some("HomePage"))],
            span: None,
        });
        let diags = validate_web_ir(&m);
        assert!(
            diags
                .iter()
                .any(|d| d.code == "web_ir_validate.route.missing_component"),
            "missing component should produce diagnostic"
        );
    }

    #[test]
    fn root_route_not_flagged_as_unreachable() {
        use crate::web_ir::{DomNode, DomNodeId, WebIrModule};
        let mut m = WebIrModule::default();
        m.dom_nodes.push(DomNode::Element {
            id: DomNodeId(0),
            tag: "div".to_string(),
            attrs: vec![],
            children: vec![],
            span: None,
        });
        m.view_roots.push(("App".to_string(), DomNodeId(0)));
        m.route_nodes.push(RouteNode::RouteTree {
            routes: vec![make_route("route_0", "/", Some("App"))],
            span: None,
        });
        let diags = validate_web_ir(&m);
        assert!(
            !diags
                .iter()
                .any(|d| d.code == "web_ir_validate.route.unreachable"),
            "root / route should never be flagged as unreachable"
        );
    }

    #[test]
    fn link_element_prevents_unreachable_warning() {
        use crate::web_ir::{DomNode, DomNodeId, WebIrModule};
        let mut m = WebIrModule::default();
        m.view_roots.push(("About".to_string(), DomNodeId(0)));
        // Add a <link to="/about"> DOM node.
        m.dom_nodes.push(DomNode::Element {
            id: DomNodeId(0),
            tag: "link".to_string(),
            attrs: vec![("to".to_string(), "/about".to_string())],
            children: vec![],
            span: None,
        });
        m.route_nodes.push(RouteNode::RouteTree {
            routes: vec![make_route("route_about", "/about", Some("About"))],
            span: None,
        });
        let diags = validate_web_ir(&m);
        assert!(
            !diags
                .iter()
                .any(|d| d.code == "web_ir_validate.route.unreachable"),
            "<link to='/about'> makes /about route reachable"
        );
    }

    #[test]
    fn non_root_route_without_link_is_warned() {
        use crate::web_ir::{DomNode, DomNodeId, WebIrModule};
        let mut m = WebIrModule::default();
        m.dom_nodes.push(DomNode::Element {
            id: DomNodeId(0),
            tag: "div".to_string(),
            attrs: vec![],
            children: vec![],
            span: None,
        });
        m.view_roots.push(("About".to_string(), DomNodeId(0)));
        // No <link> nodes — route is reachable only by direct URL.
        m.route_nodes.push(RouteNode::RouteTree {
            routes: vec![make_route("route_about", "/about", Some("About"))],
            span: None,
        });
        let diags = validate_web_ir(&m);
        assert!(
            diags
                .iter()
                .any(|d| d.code == "web_ir_validate.route.unreachable"),
            "/about without any <link> should warn as potentially unreachable"
        );
    }

    #[test]
    fn orphan_link_does_not_suppress_unreachable_warning() {
        // An <a href="/about"> that is NOT reachable from any view root (orphan)
        // must not prevent the route.unreachable warning.
        use crate::web_ir::{DomNode, DomNodeId, WebIrModule};
        let mut m = WebIrModule::default();
        // DomNodeId(0) = root div for "About" component (linked to view_root).
        m.dom_nodes.push(DomNode::Element {
            id: DomNodeId(0),
            tag: "div".to_string(),
            attrs: vec![],
            children: vec![], // no children — the link node is NOT a child
            span: None,
        });
        // DomNodeId(1) = detached <a href="/about"> — NOT reachable from root.
        m.dom_nodes.push(DomNode::Element {
            id: DomNodeId(1),
            tag: "a".to_string(),
            attrs: vec![("href".to_string(), "/about".to_string())],
            children: vec![],
            span: None,
        });
        m.view_roots.push(("About".to_string(), DomNodeId(0)));
        m.route_nodes.push(RouteNode::RouteTree {
            routes: vec![make_route("route_about", "/about", Some("About"))],
            span: None,
        });
        let diags = validate_web_ir(&m);
        assert!(
            diags
                .iter()
                .any(|d| d.code == "web_ir_validate.route.unreachable"),
            "orphan <a href='/about'> should not suppress unreachable warning: {diags:?}"
        );
    }
}
