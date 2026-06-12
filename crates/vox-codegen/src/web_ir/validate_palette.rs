//! Color-vocabulary + pairwise-contrast validation for VUV style kwargs.
//!
//! The lowerer mirrors `color`/`bg`/`border_color` kwarg raw values into
//! `data-vox-color` / `data-vox-bg` / `data-vox-border-color` attrs (JSON-quoted,
//! same convention as `data-vox-surface`). This pass checks those values against
//! the vendored Tailwind palette (`contracts/tokens/tailwind-palette.v1.json`)
//! plus the project token registry, and — for resolvable fg/bg pairs — computes
//! the WCAG ratio.
//!
//! Codes:
//! - `web_ir_validate.style.unknown_color` (error) — value not in palette or registry.
//! - `web_ir_validate.a11y.insufficient_contrast` (error, <3:1) / `low_contrast`
//!   (warning, <4.5:1) — emitted when a node's fg and effective bg both resolve.

use std::collections::HashMap;
use std::sync::OnceLock;

use super::{DomNode, DomNodeId, WebIrDiagnostic, WebIrModule};

/// Vendored Tailwind default palette: `{ "version": 1, "colors": { "zinc.400": "#a1a1aa", … } }`.
pub const PALETTE_JSON: &str =
    include_str!("../../../../contracts/tokens/tailwind-palette.v1.json");

fn palette() -> &'static HashMap<String, String> {
    static PALETTE: OnceLock<HashMap<String, String>> = OnceLock::new();
    PALETTE.get_or_init(|| {
        let v: serde_json::Value =
            serde_json::from_str(PALETTE_JSON).expect("tailwind-palette.v1.json parses");
        v.get("colors")
            .and_then(|c| c.as_object())
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, val)| val.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default()
    })
}

/// Resolve a kwarg color value (`zinc.400`, `white`) to its hex via palette then registry.
/// Registry token keys are CSS-var style (`color-primary`); a kwarg `color.primary`
/// is normalized to that form for the fallback lookup.
#[must_use]
pub fn resolve_color(
    value: &str,
    registry: Option<&vox_compiler::tokens::TokenRegistry>,
) -> Option<String> {
    let v = value.trim_matches('"');
    if let Some(hex) = palette().get(v) {
        return Some(hex.clone());
    }
    if let Some(reg) = registry {
        let css_key = v.replace('.', "-");
        if let Some(hex) = reg.lookup(&css_key) {
            return Some(hex.to_string());
        }
    }
    None
}

/// Levenshtein-nearest palette/registry color name, if within edit distance 3.
fn suggest_color(input: &str, registry: Option<&vox_compiler::tokens::TokenRegistry>) -> Option<String> {
    let mut best: Option<(usize, String)> = None;
    let candidates = palette()
        .keys()
        .map(|s| s.as_str())
        .chain(registry.into_iter().flat_map(|r| r.all_keys()));
    for cand in candidates {
        let d = levenshtein(input, cand);
        if best.as_ref().is_none_or(|(bd, _)| d < *bd) {
            best = Some((d, cand.to_string()));
        }
    }
    best.filter(|(d, _)| *d <= 3).map(|(_, s)| s)
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for (i, &ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur[j + 1] = (prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// Validate color-bearing kwargs against the palette/registry, and check pairwise
/// WCAG contrast for nodes whose fg + effective bg both resolve.
pub fn validate_palette(
    module: &WebIrModule,
    registry: Option<&vox_compiler::tokens::TokenRegistry>,
    out: &mut Vec<WebIrDiagnostic>,
) {
    // Vocabulary check: every color-bearing kwarg value must resolve.
    for node in &module.dom_nodes {
        let DomNode::Element { attrs, .. } = node else {
            continue;
        };
        for (k, v) in attrs {
            let kwarg = match k.as_str() {
                "data-vox-color" => "color",
                "data-vox-bg" => "bg",
                "data-vox-border-color" => "border_color",
                _ => continue,
            };
            let value = v.trim_matches('"');
            if resolve_color(value, registry).is_none() {
                let hint = match suggest_color(value, registry) {
                    Some(s) => format!(" Did you mean '{s}'?"),
                    None => String::new(),
                };
                out.push(WebIrDiagnostic {
                    code: "web_ir_validate.style.unknown_color".to_string(),
                    message: format!(
                        "Unknown color '{value}' for `{kwarg}` — not in the Tailwind palette or the project token registry. Use a palette name (e.g. zinc.400) or a declared token, not a raw hex.{hint}"
                    ),
                    span: None,
                    category: Some("style".to_string()),
                });
            }
        }
    }

    // Pairwise WCAG contrast (A4): walk the arena tracking the nearest effective bg.
    check_contrast(module, registry, out);
}

/// Build a child→parent map and walk each view root, threading the nearest resolved
/// background so a node carrying `data-vox-color` can be paired against it.
fn check_contrast(
    module: &WebIrModule,
    registry: Option<&vox_compiler::tokens::TokenRegistry>,
    out: &mut Vec<WebIrDiagnostic>,
) {
    for (_, root) in &module.view_roots {
        walk_contrast(module, *root, None, registry, out);
    }
}

fn walk_contrast(
    module: &WebIrModule,
    id: DomNodeId,
    inherited_bg: Option<String>,
    registry: Option<&vox_compiler::tokens::TokenRegistry>,
    out: &mut Vec<WebIrDiagnostic>,
) {
    let Some(node) = module.dom_nodes.get(id.0 as usize) else {
        return;
    };
    let DomNode::Element { attrs, children, .. } = node else {
        return;
    };

    // Effective bg for this node: own data-vox-bg, else own surface bg token, else inherited.
    let own_bg_hex = attrs
        .iter()
        .find(|(k, _)| k == "data-vox-bg")
        .and_then(|(_, v)| resolve_color(v.trim_matches('"'), registry))
        .or_else(|| surface_bg_hex(attrs, registry));
    let effective_bg = own_bg_hex.or(inherited_bg);

    // If this node sets a text color and we know the bg, check the ratio.
    if let Some(fg_raw) = attrs.iter().find(|(k, _)| k == "data-vox-color").map(|(_, v)| v) {
        if let (Some(fg_hex), Some(bg_hex)) =
            (resolve_color(fg_raw.trim_matches('"'), registry), effective_bg.clone())
        {
            if let Some(ratio) = vox_compiler::tokens::wcag21_contrast_ratio(&fg_hex, &bg_hex) {
                let fg = fg_raw.trim_matches('"');
                if ratio < 3.0 {
                    out.push(WebIrDiagnostic {
                        code: "web_ir_validate.a11y.insufficient_contrast".to_string(),
                        message: format!(
                            "Text color '{fg}' ({fg_hex}) on background ({bg_hex}) has contrast {ratio:.2}:1 — below the 3:1 hard floor. Pick a darker/lighter pair (WCAG AA needs 4.5:1 for body text)."
                        ),
                        span: None,
                        category: Some("a11y".to_string()),
                    });
                } else if ratio < 4.5 {
                    out.push(WebIrDiagnostic {
                        code: "web_ir_validate.a11y.low_contrast".to_string(),
                        message: format!(
                            "Text color '{fg}' on background is {ratio:.2}:1 — below WCAG AA 4.5:1 for body text."
                        ),
                        span: None,
                        category: Some("a11y".to_string()),
                    });
                }
            }
        }
    }

    for child in children {
        walk_contrast(module, *child, effective_bg.clone(), registry, out);
    }
}

/// Resolve a node's surface (`data-vox-surface`) to its background hex via the registry.
fn surface_bg_hex(
    attrs: &[(String, String)],
    registry: Option<&vox_compiler::tokens::TokenRegistry>,
) -> Option<String> {
    let reg = registry?;
    let surface = attrs
        .iter()
        .find(|(k, _)| k == "data-vox-surface")
        .map(|(_, v)| v.trim_matches('"'))?;
    let pair = reg.lookup_surface(surface)?;
    reg.lookup(&pair.bg_key).map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web_ir::{DomNode, DomNodeId, WebIrModule};

    fn module_with_attrs(attrs: Vec<(&str, &str)>) -> WebIrModule {
        let mut m = WebIrModule::default();
        m.dom_nodes.push(DomNode::Element {
            id: DomNodeId(0),
            tag: "span".to_string(),
            attrs: attrs
                .into_iter()
                .map(|(k, v)| (k.to_string(), format!("\"{v}\"")))
                .collect(),
            children: vec![],
            span: None,
        });
        m.view_roots.push(("V".to_string(), DomNodeId(0)));
        m
    }

    fn parent_child(parent_attrs: Vec<(&str, &str)>, child_attrs: Vec<(&str, &str)>) -> WebIrModule {
        let enc = |a: Vec<(&str, &str)>| {
            a.into_iter()
                .map(|(k, v)| (k.to_string(), format!("\"{v}\"")))
                .collect::<Vec<_>>()
        };
        let mut m = WebIrModule::default();
        m.dom_nodes.push(DomNode::Element {
            id: DomNodeId(0),
            tag: "div".to_string(),
            attrs: enc(parent_attrs),
            children: vec![DomNodeId(1)],
            span: None,
        });
        m.dom_nodes.push(DomNode::Element {
            id: DomNodeId(1),
            tag: "span".to_string(),
            attrs: enc(child_attrs),
            children: vec![],
            span: None,
        });
        m.view_roots.push(("V".to_string(), DomNodeId(0)));
        m
    }

    // ── A3: vocabulary ──────────────────────────────────────────────────────

    #[test]
    fn known_palette_color_passes() {
        let m = module_with_attrs(vec![("data-vox-color", "zinc.400")]);
        let mut out = vec![];
        validate_palette(&m, None, &mut out);
        assert!(
            !out.iter().any(|d| d.code == "web_ir_validate.style.unknown_color"),
            "{out:?}"
        );
    }

    #[test]
    fn unknown_color_is_rejected_with_suggestion() {
        let m = module_with_attrs(vec![("data-vox-color", "zink.400")]);
        let mut out = vec![];
        validate_palette(&m, None, &mut out);
        let d = out
            .iter()
            .find(|d| d.code == "web_ir_validate.style.unknown_color")
            .expect("unknown color must be rejected");
        assert!(d.message.contains("zinc.400"), "did-you-mean expected: {}", d.message);
    }

    #[test]
    fn raw_hex_is_rejected() {
        let m = module_with_attrs(vec![("data-vox-bg", "#aaaaaa")]);
        let mut out = vec![];
        validate_palette(&m, None, &mut out);
        assert!(out.iter().any(|d| d.code == "web_ir_validate.style.unknown_color"));
    }

    // ── A4: pairwise contrast ───────────────────────────────────────────────

    #[test]
    fn gray_text_on_white_panel_is_a_hard_error() {
        let m = parent_child(vec![("data-vox-bg", "white")], vec![("data-vox-color", "gray.300")]);
        let mut out = vec![];
        validate_palette(&m, None, &mut out);
        assert!(
            out.iter().any(|d| d.code == "web_ir_validate.a11y.insufficient_contrast"),
            "gray.300 on white is ~1.5:1 and must hard-fail, got: {out:?}"
        );
    }

    #[test]
    fn marginal_contrast_warns_not_errors() {
        // gray.500 (#6b7280) on gray.100 (#f3f4f6) ≈ 4.2:1 → AA-fail but >3:1.
        let m = parent_child(vec![("data-vox-bg", "gray.100")], vec![("data-vox-color", "gray.500")]);
        let mut out = vec![];
        validate_palette(&m, None, &mut out);
        assert!(out.iter().any(|d| d.code == "web_ir_validate.a11y.low_contrast"), "{out:?}");
        assert!(!out.iter().any(|d| d.code == "web_ir_validate.a11y.insufficient_contrast"), "{out:?}");
    }

    #[test]
    fn high_contrast_pair_passes() {
        let m = parent_child(vec![("data-vox-bg", "white")], vec![("data-vox-color", "zinc.900")]);
        let mut out = vec![];
        validate_palette(&m, None, &mut out);
        assert!(out.iter().all(|d| !d.code.contains("contrast")), "{out:?}");
    }

    #[test]
    fn no_known_background_means_no_check() {
        let m = parent_child(vec![], vec![("data-vox-color", "gray.300")]);
        let mut out = vec![];
        validate_palette(&m, None, &mut out);
        assert!(out.iter().all(|d| !d.code.contains("contrast")), "{out:?}");
    }
}
