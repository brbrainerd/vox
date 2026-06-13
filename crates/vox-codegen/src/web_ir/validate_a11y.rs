//! TASK-5.3 — Accessibility validator (a11y).
//!
//! Checks structural accessibility rules against the DOM arena:
//!
//! - `<img>` without `alt` or `aria-hidden="true"` → error.
//! - `<button>` without accessible name (text child / aria-label / aria-labelledby) → error.
//! - `<a href=...>` without accessible name → error.
//! - Any element with `role="button"` without accessible name → error.
//! - Any element with `role="button"` without a keyboard handler → error.
//!
//! TASK-6.5 adds ancestor-chain contrast checking:
//! - `<p>` / `<h1>`–`<h6>` inside a surface with insufficient contrast → compile error.
//!
//! All codes are under `web_ir_validate.a11y.*`.

use super::{BehaviorNode, DomNode, DomNodeId, WebIrDiagnostic, WebIrModule};

// ---------------------------------------------------------------------------
// Internal aria inference types (Phase 6 will embed these in DomNode)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum AriaRole {
    Button,
    Link,
    Img,
    TextInput,
    #[allow(dead_code)] // Phase 6: checkbox/radio role checks
    Checkbox,
    #[allow(dead_code)] // Phase 6: checkbox/radio role checks
    Radio,
    Combobox,
    Generic,
}

/// Derive the implicit ARIA role from an element tag, following the HTML-AAM mapping.
fn implicit_role(tag: &str) -> AriaRole {
    match tag.to_ascii_lowercase().as_str() {
        "button" | "summary" => AriaRole::Button,
        "a" | "area" => AriaRole::Link,
        "img" => AriaRole::Img,
        "input" => AriaRole::TextInput,
        "select" => AriaRole::Combobox,
        _ => AriaRole::Generic,
    }
}

/// Check whether an attrs list contains a specific attribute name (case-insensitive key).
fn has_attr(attrs: &[(String, String)], name: &str) -> bool {
    attrs.iter().any(|(k, _)| k.eq_ignore_ascii_case(name))
}

/// True if the element carries an explicit accessible name via aria attributes.
fn has_aria_name(attrs: &[(String, String)]) -> bool {
    has_attr(attrs, "aria-label") || has_attr(attrs, "aria-labelledby")
}

/// Recursively check whether a set of child DOM nodes contains any non-empty text content
/// or expression nodes (which may produce text at runtime).
fn has_non_empty_text_child(module: &WebIrModule, child_ids: &[DomNodeId]) -> bool {
    for child_id in child_ids {
        let Some(node) = module.dom_nodes.get(child_id.0 as usize) else {
            continue;
        };
        match node {
            DomNode::Text { content, .. } if !content.trim().is_empty() => {
                return true;
            }
            DomNode::Element { children, .. } if has_non_empty_text_child(module, children) => {
                return true;
            }
            // Expression nodes ({label}, {count}, etc.) may produce text at runtime;
            // treat their presence as satisfying the accessible-name requirement.
            DomNode::Expr { .. } => return true,
            // Fragment, Conditional, Loop may contain text children — recurse.
            DomNode::Fragment { children, .. } if has_non_empty_text_child(module, children) => {
                return true;
            }
            DomNode::Conditional {
                then_children,
                else_children,
                ..
            } if (has_non_empty_text_child(module, then_children)
                || has_non_empty_text_child(module, else_children)) =>
            {
                return true;
            }
            DomNode::Loop { body, .. } if has_non_empty_text_child(module, body) => {
                return true;
            }
            _ => {}
        }
    }
    false
}

/// Run structural a11y checks on the DOM arena.
pub fn validate_a11y(module: &WebIrModule, out: &mut Vec<WebIrDiagnostic>) {
    for node in &module.dom_nodes {
        let DomNode::Element {
            id: elem_id,
            tag,
            attrs,
            children,
            ..
        } = node
        else {
            continue;
        };

        let get_a = |name: &str| -> Option<&str> {
            attrs
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(name))
                .map(|(_, v)| v.as_str())
        };

        match tag.as_str() {
            "img" => {
                let has_alt = attrs.iter().any(|(k, _)| k == "alt");
                let aria_hidden = get_a("aria-hidden") == Some("true");
                if !has_alt && !aria_hidden {
                    out.push(WebIrDiagnostic {
                        code: "web_ir_validate.a11y.img_missing_alt".to_string(),
                        message: "img element requires an `alt` attribute or `aria-hidden=\"true\"` for decorative images".to_string(),
                        span: None,
                        category: Some("a11y".to_string()),
                    });
                }
            }
            "button" => {
                check_accessible_name(module, tag, attrs, children, out);
            }
            "a" => {
                let has_href = attrs
                    .iter()
                    .any(|(k, _)| k.eq_ignore_ascii_case("href") || k.eq_ignore_ascii_case("to"));
                if !has_href {
                    out.push(WebIrDiagnostic {
                        code: "web_ir_validate.a11y.anchor_missing_href".to_string(),
                        message: "`a` element has no `href` attribute — navigation links must have a destination or use a `<button>` instead".to_string(),
                        span: None,
                        category: Some("a11y".to_string()),
                    });
                }
                if has_href {
                    check_accessible_name(module, tag, attrs, children, out);
                }
            }
            "input" => {
                let has_label =
                    has_aria_name(attrs) || attrs.iter().any(|(k, _)| k.eq_ignore_ascii_case("id"));
                if !has_label {
                    out.push(WebIrDiagnostic {
                        code: "web_ir_validate.a11y.input_missing_label".to_string(),
                        message: "`input` element requires an `aria-label`, `aria-labelledby`, or associated `<label>` (via `id`)".to_string(),
                        span: None,
                        category: Some("a11y".to_string()),
                    });
                }
            }
            _ => {
                if let Some(role) = get_a("role")
                    && role == "button"
                {
                    // Only check role="button" on non-button elements
                    if !matches!(implicit_role(tag), AriaRole::Button) {
                        check_accessible_name(module, tag, attrs, children, out);
                        check_keyboard_handler(module, *elem_id, out);
                    }
                }
            }
        }
    }
}

fn check_accessible_name(
    module: &WebIrModule,
    tag: &str,
    attrs: &[(String, String)],
    children: &[DomNodeId],
    out: &mut Vec<WebIrDiagnostic>,
) {
    let has_aria = has_aria_name(attrs);
    if has_aria {
        return;
    }
    if has_non_empty_text_child(module, children) {
        return;
    }
    out.push(WebIrDiagnostic {
        code: "web_ir_validate.a11y.interactive_missing_label".to_string(),
        message: format!(
            "`{}` element has no accessible name — add text content, `aria-label`, or `aria-labelledby`",
            tag
        ),
        span: None,
        category: Some("a11y".to_string()),
    });
}

/// For `role="button"` elements: require at least one keyboard event handler in the module
/// targeting this element (or a module-level catch-all keyboard handler).
fn check_keyboard_handler(
    module: &WebIrModule,
    elem_id: DomNodeId,
    out: &mut Vec<WebIrDiagnostic>,
) {
    let is_keyboard = |event: &str| event == "keydown" || event == "keyup" || event == "keypress";

    let has_handler = module.behavior_nodes.iter().any(|b| {
        if let BehaviorNode::EventHandler {
            event, target_dom, ..
        } = b
        {
            if !is_keyboard(event) {
                return false;
            }
            // Accept: handler targeting this element OR a catch-all (None target).
            target_dom.is_none_or(|t| t == elem_id)
        } else {
            false
        }
    });

    if !has_handler {
        out.push(WebIrDiagnostic {
            code: "web_ir_validate.a11y.role_button_missing_keyboard".to_string(),
            message: "element with `role=\"button\"` requires a `keydown` or `keyup` handler for keyboard accessibility".to_string(),
            span: None,
            category: Some("a11y".to_string()),
        });
    }
}

/// TASK-6.5: Walk the DOM from each view root, tracking the current surface pair context.
/// At `p` / `h1`–`h6` nodes with an active surface, compute WCAG 2.1 contrast and emit
/// `web_ir_validate.a11y.insufficient_contrast` (error < 3:1) or
/// `web_ir_validate.a11y.low_contrast` (warning < 4.5:1 for body text).
pub fn validate_a11y_with_registry(
    module: &WebIrModule,
    registry: &vox_compiler::tokens::TokenRegistry,
    out: &mut Vec<WebIrDiagnostic>,
) {
    for (_name, root_id) in &module.view_roots {
        walk_contrast(module, *root_id, None, registry, out);
    }
}

fn walk_contrast(
    module: &WebIrModule,
    node_id: DomNodeId,
    current_surface: Option<(String, String)>,
    registry: &vox_compiler::tokens::TokenRegistry,
    out: &mut Vec<WebIrDiagnostic>,
) {
    let Some(node) = module.dom_nodes.get(node_id.0 as usize) else {
        return;
    };
    match node {
        DomNode::Element {
            tag,
            attrs,
            children,
            ..
        } => {
            // Inherit or update surface context from data-vox-surface attr.
            let surface_ctx: Option<(String, String)> = if let Some(surface_name) = attrs
                .iter()
                .find(|(k, _)| k == "data-vox-surface")
                // Attr values are JSON-encoded TS-expression strings (lower.rs) — strip quotes.
                .map(|(_, v)| v.as_str().trim_matches('"'))
            {
                registry
                    .lookup_surface(surface_name)
                    .map(|e| (e.fg_key.clone(), e.bg_key.clone()))
                    .or_else(|| current_surface.clone())
            } else {
                current_surface.clone()
            };

            if let Some((ref fg_key, ref bg_key)) = surface_ctx {
                check_text_contrast(tag, fg_key, bg_key, registry, out);
            }

            for child_id in children {
                walk_contrast(module, *child_id, surface_ctx.clone(), registry, out);
            }
        }
        DomNode::Fragment { children, .. } => {
            for child_id in children {
                walk_contrast(module, *child_id, current_surface.clone(), registry, out);
            }
        }
        DomNode::Conditional {
            then_children,
            else_children,
            ..
        } => {
            for child_id in then_children.iter().chain(else_children.iter()) {
                walk_contrast(module, *child_id, current_surface.clone(), registry, out);
            }
        }
        DomNode::Loop { body, .. } => {
            for child_id in body {
                walk_contrast(module, *child_id, current_surface.clone(), registry, out);
            }
        }
        DomNode::Text { .. } | DomNode::Slot { .. } | DomNode::Expr { .. } => {}
    }
}

fn check_text_contrast(
    tag: &str,
    fg_key: &str,
    bg_key: &str,
    registry: &vox_compiler::tokens::TokenRegistry,
    out: &mut Vec<WebIrDiagnostic>,
) {
    let is_text_tag = matches!(tag, "p" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6");
    if !is_text_tag {
        return;
    }
    let Some(fg_hex) = registry.lookup(fg_key) else {
        return;
    };
    let Some(bg_hex) = registry.lookup(bg_key) else {
        return;
    };
    let Some(ratio) = vox_compiler::tokens::wcag21_contrast_ratio(fg_hex, bg_hex) else {
        return;
    };

    // h1–h3 are large text (≥18pt regular or ≥14pt bold): WCAG 2.1 minimum 3:1.
    // p, h4–h6 are body text: warn <4.5:1, error <3:1.
    let is_large = matches!(tag, "h1" | "h2" | "h3");
    let (error_threshold, warn_threshold): (f64, f64) =
        if is_large { (3.0, 3.0) } else { (3.0, 4.5) };

    if ratio < error_threshold {
        out.push(WebIrDiagnostic {
            code: "web_ir_validate.a11y.insufficient_contrast".to_string(),
            message: format!(
                "`<{tag}>` contrast {ratio:.2}:1 is below {error_threshold:.1}:1 minimum (WCAG 2.1 §1.4.3)"
            ),
            span: None,
            category: Some("a11y".to_string()),
        });
    } else if ratio < warn_threshold {
        out.push(WebIrDiagnostic {
            code: "web_ir_validate.a11y.low_contrast".to_string(),
            message: format!(
                "`<{tag}>` contrast {ratio:.2}:1 is below recommended {warn_threshold:.1}:1 (WCAG 2.1 §1.4.3)"
            ),
            span: None,
            category: Some("a11y".to_string()),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web_ir::{DomNodeId, WebIrModule};

    fn elem(id: u32, tag: &str, attrs: Vec<(&str, &str)>, children: Vec<u32>) -> DomNode {
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

    fn text(content: &str) -> DomNode {
        DomNode::Text {
            content: content.to_string(),
            span: None,
        }
    }

    fn module_with_nodes(nodes: Vec<DomNode>) -> WebIrModule {
        WebIrModule {
            dom_nodes: nodes,
            ..Default::default()
        }
    }

    #[test]
    fn img_without_alt_is_error() {
        let m = module_with_nodes(vec![elem(0, "img", vec![("src", "logo.png")], vec![])]);
        let diags = {
            let mut out = Vec::new();
            validate_a11y(&m, &mut out);
            out
        };
        assert!(
            diags
                .iter()
                .any(|d| d.code == "web_ir_validate.a11y.img_missing_alt"),
            "expected img_missing_alt: {diags:?}"
        );
    }

    #[test]
    fn img_with_alt_is_ok() {
        let m = module_with_nodes(vec![elem(
            0,
            "img",
            vec![("src", "x.png"), ("alt", "Logo")],
            vec![],
        )]);
        let mut out = Vec::new();
        validate_a11y(&m, &mut out);
        assert!(out.is_empty(), "unexpected: {out:?}");
    }

    #[test]
    fn img_with_aria_hidden_is_ok() {
        let m = module_with_nodes(vec![elem(
            0,
            "img",
            vec![("src", "deco.svg"), ("aria-hidden", "true")],
            vec![],
        )]);
        let mut out = Vec::new();
        validate_a11y(&m, &mut out);
        assert!(out.is_empty(), "unexpected: {out:?}");
    }

    #[test]
    fn button_without_label_is_error() {
        let m = module_with_nodes(vec![elem(0, "button", vec![], vec![])]);
        let mut out = Vec::new();
        validate_a11y(&m, &mut out);
        assert!(
            out.iter()
                .any(|d| d.code == "web_ir_validate.a11y.interactive_missing_label"),
            "expected interactive_missing_label: {out:?}"
        );
    }

    #[test]
    fn button_with_text_child_is_ok() {
        let m = module_with_nodes(vec![elem(0, "button", vec![], vec![1]), text("Submit")]);
        let mut out = Vec::new();
        validate_a11y(&m, &mut out);
        assert!(out.is_empty(), "unexpected: {out:?}");
    }

    #[test]
    fn button_with_aria_label_is_ok() {
        let m = module_with_nodes(vec![elem(
            0,
            "button",
            vec![("aria-label", "Close")],
            vec![],
        )]);
        let mut out = Vec::new();
        validate_a11y(&m, &mut out);
        assert!(out.is_empty(), "unexpected: {out:?}");
    }

    #[test]
    fn anchor_without_href_emits_warning_and_skips_label_check() {
        let m = module_with_nodes(vec![elem(0, "a", vec![], vec![])]);
        let mut out = Vec::new();
        validate_a11y(&m, &mut out);
        // anchor without href gets a warning about the missing href
        assert!(
            out.iter()
                .any(|d| d.code == "web_ir_validate.a11y.anchor_missing_href"),
            "expected anchor_missing_href warning: {out:?}"
        );
        // but the accessible-label check is skipped (only runs when href is present)
        assert!(
            !out.iter()
                .any(|d| d.code == "web_ir_validate.a11y.interactive_missing_label"),
            "label check should be skipped for anchor without href: {out:?}"
        );
    }

    #[test]
    fn anchor_with_href_and_no_label_is_error() {
        let m = module_with_nodes(vec![elem(0, "a", vec![("href", "/about")], vec![])]);
        let mut out = Vec::new();
        validate_a11y(&m, &mut out);
        assert!(
            out.iter()
                .any(|d| d.code == "web_ir_validate.a11y.interactive_missing_label"),
            "expected interactive_missing_label: {out:?}"
        );
    }

    #[test]
    fn role_button_without_keyboard_handler_is_error() {
        let m = module_with_nodes(vec![elem(
            0,
            "div",
            vec![("role", "button"), ("aria-label", "Toggle")],
            vec![],
        )]);
        let mut out = Vec::new();
        validate_a11y(&m, &mut out);
        assert!(
            out.iter()
                .any(|d| d.code == "web_ir_validate.a11y.role_button_missing_keyboard"),
            "expected role_button_missing_keyboard: {out:?}"
        );
    }

    #[test]
    fn role_button_with_keyboard_handler_is_ok() {
        let mut m = module_with_nodes(vec![elem(
            0,
            "div",
            vec![("role", "button"), ("aria-label", "Toggle")],
            vec![],
        )]);
        m.behavior_nodes.push(BehaviorNode::EventHandler {
            target_dom: Some(DomNodeId(0)),
            event: "keydown".to_string(),
            handler: "handleKey".to_string(),
            span: None,
        });
        let mut out = Vec::new();
        validate_a11y(&m, &mut out);
        assert!(
            out.iter()
                .all(|d| d.code != "web_ir_validate.a11y.role_button_missing_keyboard"),
            "unexpected keyboard error: {out:?}"
        );
    }

    // ---- TASK-6.5 contrast tests ----

    fn registry_with_surface(
        surface_name: &str,
        fg_hex: &str,
        bg_hex: &str,
    ) -> vox_compiler::tokens::TokenRegistry {
        let mut reg = vox_compiler::tokens::TokenRegistry::default();
        reg.by_css_var
            .insert("fg-test".to_string(), fg_hex.to_string());
        reg.by_css_var
            .insert("bg-test".to_string(), bg_hex.to_string());
        reg.surface_pairs.insert(
            surface_name.to_string(),
            vox_compiler::tokens::SurfacePairEntry {
                fg_key: "fg-test".to_string(),
                bg_key: "bg-test".to_string(),
            },
        );
        reg
    }

    fn module_with_surface_and_tag(surface_name: &str, text_tag: &str) -> WebIrModule {
        let mut m = WebIrModule::default();
        m.dom_nodes.push(DomNode::Element {
            id: DomNodeId(0),
            tag: "div".to_string(),
            attrs: vec![("data-vox-surface".to_string(), surface_name.to_string())],
            children: vec![DomNodeId(1)],
            span: None,
        });
        m.dom_nodes.push(DomNode::Element {
            id: DomNodeId(1),
            tag: text_tag.to_string(),
            attrs: vec![],
            children: vec![],
            span: None,
        });
        m.view_roots.push(("Page".to_string(), DomNodeId(0)));
        m
    }

    #[test]
    fn passing_contrast_no_error() {
        // #1d3557 on #ffffff — deep navy, ratio ≈ 12.6:1
        let reg = registry_with_surface("dark", "#1d3557", "#ffffff");
        let m = module_with_surface_and_tag("dark", "p");
        let mut out = Vec::new();
        validate_a11y_with_registry(&m, &reg, &mut out);
        assert!(out.is_empty(), "unexpected diagnostics: {out:?}");
    }

    #[test]
    fn insufficient_contrast_emits_error() {
        // #cccccc on #ffffff — ratio ≈ 1.6:1, below the 3:1 error threshold
        let reg = registry_with_surface("low", "#cccccc", "#ffffff");
        let m = module_with_surface_and_tag("low", "p");
        let mut out = Vec::new();
        validate_a11y_with_registry(&m, &reg, &mut out);
        assert!(
            out.iter()
                .any(|d| d.code == "web_ir_validate.a11y.insufficient_contrast"),
            "expected insufficient_contrast: {out:?}"
        );
    }

    #[test]
    fn low_contrast_body_text_emits_warning() {
        // #888888 on #ffffff — ratio ≈ 3.5:1, between 3:1 and 4.5:1 → low_contrast warning for body text
        let reg = registry_with_surface("mid", "#888888", "#ffffff");
        let m = module_with_surface_and_tag("mid", "p");
        let mut out = Vec::new();
        validate_a11y_with_registry(&m, &reg, &mut out);
        assert!(
            out.iter()
                .any(|d| d.code == "web_ir_validate.a11y.low_contrast"),
            "expected low_contrast warning for p at ~3.5:1: {out:?}"
        );
    }

    #[test]
    fn no_surface_context_skips_contrast_check() {
        // p element with no ancestor data-vox-surface → no contrast check regardless
        let reg = registry_with_surface("any", "#cccccc", "#ffffff");
        let mut m = WebIrModule::default();
        m.dom_nodes.push(DomNode::Element {
            id: DomNodeId(0),
            tag: "p".to_string(),
            attrs: vec![],
            children: vec![],
            span: None,
        });
        m.view_roots.push(("Page".to_string(), DomNodeId(0)));
        let mut out = Vec::new();
        validate_a11y_with_registry(&m, &reg, &mut out);
        assert!(
            out.is_empty(),
            "should skip check without surface context: {out:?}"
        );
    }

    #[test]
    fn heading_large_text_passes_at_3_to_1() {
        // h1 uses large-text threshold (3:1); #888888 on #ffffff ≈ 3.5:1 → passes
        let reg = registry_with_surface("dark", "#888888", "#ffffff");
        let m = module_with_surface_and_tag("dark", "h1");
        let mut out = Vec::new();
        validate_a11y_with_registry(&m, &reg, &mut out);
        assert!(
            out.is_empty(),
            "h1 at ~3.5:1 should pass large-text 3:1 threshold: {out:?}"
        );
    }
}
