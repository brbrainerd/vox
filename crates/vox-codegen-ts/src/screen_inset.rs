//! Shared screen-root inset rules for web and React Native emitters.

use vox_compiler::hir::{HirExpr, HirJsxAttr};

fn attr_value<'a>(attrs: &'a [HirJsxAttr], name: &str) -> Option<&'a HirExpr> {
    attrs.iter().find(|a| a.name == name).map(|a| &a.value)
}

/// True when a screen-root view opts OUT of default edge padding via `bleed`
/// (present, and not literally `false`). Shared with the web reactive emit so
/// both targets honor the same opt-out.
pub fn root_view_bleeds(view_root: &HirExpr) -> bool {
    let attrs: &[HirJsxAttr] = match view_root {
        HirExpr::Jsx(el) => &el.attributes,
        HirExpr::JsxSelfClosing(sc) => &sc.attributes,
        _ => return false,
    };
    match attr_value(attrs, "bleed") {
        Some(HirExpr::BoolLit(false, _)) => false,
        Some(HirExpr::StringLit(s, _)) if s == "false" => false,
        Some(_) => true,
        None => false,
    }
}
