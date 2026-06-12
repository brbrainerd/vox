//! Vox `component Name() { state ...; view: ... }` → React Native TSX.
//!
//! Strategy: walk the HIR `view:` expression, translating React-DOM-flavored
//! `HirExpr::Jsx` / `HirExpr::JsxSelfClosing` nodes into an abstract `RnNode` tree,
//! then emit TSX referencing React Native primitives (`View`, `Text`, `Pressable`,
//! `Image`, `TextInput`) with a `StyleSheet.create({...})` block at the bottom.
//!
//! HIR-level note: the parser/lowerer maps VUV view-call syntax (`column()`,
//! `heading(level=1)`, `button(on_click=...)`) onto HTML-flavored tags (`div`,
//! `h1`-`h6`, `button`) with React-style attribute names (`className`, `onClick`).
//! This module reverses that, mapping back to platform-native equivalents.
//! The single source of truth — the HIR — never changes between targets.

use std::collections::{BTreeMap, HashMap, HashSet};
use vox_compiler::hir::{
    HirExpr, HirImport, HirJsxAttr, HirJsxElement, HirReactiveComponent, HirReactiveMember, HirStmt,
};

use crate::web_ir::WebIrDiagnostic;

/// Abstract React Native node, post-translation from React DOM HIR.
enum RnNode {
    /// `<View style={styles.<key>}>...</View>` — flex container.
    View {
        style_key: Option<String>,
        children: Vec<RnNode>,
    },
    /// `<Text style={styles.<key>}>...content...</Text>`.
    /// Children may be other `RnNode::Text` (interpolations) or plain strings.
    Text {
        style_key: Option<String>,
        children: Vec<RnNode>,
    },
    /// `<Pressable style={styles.<key>} onPress={<handler>}>...</Pressable>`.
    /// String children are auto-wrapped in `<Text>`.
    Pressable {
        style_key: Option<String>,
        handler_ts: Option<String>,
        children: Vec<RnNode>,
    },
    /// `<Image source={{ uri: <src> }} accessibilityLabel={<alt>} style={...} />`.
    Image {
        src_ts: String,
        alt_ts: Option<String>,
        style_key: Option<String>,
    },
    /// `<TextInput style={styles.<key>} value={<value>} onChangeText={<handler>} />`.
    TextInput {
        style_key: Option<String>,
        value_ts: Option<String>,
        on_change_ts: Option<String>,
        placeholder_ts: Option<String>,
    },
    /// `{<iter>.map((<item>, <index>) => <body>)}` — VUV `for x in xs key=x.id { ... }`.
    /// The body is RECURSIVELY translated to RN nodes so loops never emit DOM tags
    /// like `<div>` / `<p>` inside an RN tree.
    Loop {
        iterator_ts: String,
        item_name: String,
        index_name: Option<String>,
        key_ts: Option<String>,
        body: Vec<RnNode>,
    },
    /// `<UserComponent attr1={expr1} attr2={expr2}>...</UserComponent>` — a call
    /// to a user-declared `component` inside a view. The tag is PascalCase
    /// per React convention; attributes pass through as JSX `{...}` blocks
    /// (no style remapping — user components manage their own props).
    CustomComponent {
        tag_name: String,
        attributes: Vec<(String, String)>,
        children: Vec<RnNode>,
    },
    /// `<ScrollView horizontal>...</ScrollView>` — `row(scroll: "horizontal")`.
    /// A single non-wrapping line that scrolls instead of overflowing.
    ScrollRow { children: Vec<RnNode> },
    /// `<Modal transparent visible={…} animationType="…">…</Modal>` — the RN
    /// representation of the `modal` / `drawer` tier primitives (B4). `drawer` uses
    /// `slide` animation; `modal` uses `fade`. `visible_ts` comes from the `open`
    /// kwarg (defaults to `true`).
    RnModal {
        animation: &'static str,
        visible_ts: String,
        children: Vec<RnNode>,
    },
    /// `<Link href={<href>} style={styles.<key>}>...</Link>` — expo-router
    /// navigation. VUV `link(href="/x") { "label" }`. String children are
    /// auto-wrapped in `<Text>` (same as Pressable) so the label is tappable.
    Link {
        href_ts: String,
        style_key: Option<String>,
        children: Vec<RnNode>,
    },
    /// A plain JS-style string literal inside a `<Text>` — gets quoted/escaped.
    StringLit(String),
    /// A raw TypeScript expression to interpolate — emitted inside `{...}` braces.
    Expr(String),
}

/// Map a React-DOM-flavored Tailwind class string to a canonical RN style key.
/// The key is opaque; the matching definition is in [`emit_styles_block`].
fn class_string_to_style_key(class_tokens: &[&str]) -> Option<&'static str> {
    if class_tokens.is_empty() {
        return None;
    }
    let joined: Vec<&str> = class_tokens.to_vec();
    if joined.contains(&"flex") && joined.contains(&"flex-col") {
        Some("col")
    } else if joined.contains(&"flex") && joined.contains(&"flex-row") {
        Some("row")
    } else if joined.contains(&"text-3xl") && joined.contains(&"font-semibold") {
        Some("h1")
    } else if joined.contains(&"text-2xl") && joined.contains(&"font-semibold") {
        Some("h2")
    } else if joined.contains(&"text-xl") && joined.contains(&"font-semibold") {
        Some("h3")
    } else if joined.contains(&"text-base") {
        Some("body")
    } else if joined.contains(&"text-sm") && joined.contains(&"font-medium") {
        // shadcn-flavored button — long class lists end with "h-10 px-4 py-2"
        if joined.contains(&"bg-primary") {
            Some("btn_primary")
        } else if joined.contains(&"bg-secondary") {
            Some("btn_secondary")
        } else {
            Some("btn_primary")
        }
    } else if joined.contains(&"rounded-lg")
        && joined.contains(&"border")
        && joined.contains(&"p-4")
    {
        Some("panel")
    } else {
        None
    }
}

/// Map Tailwind utility-class tokens to React Native `StyleSheet` property pairs.
///
/// This is the B3 fix for the "style kwargs silently dropped on mobile" gap: kwargs
/// like `bg=blue.600` / `pad=8` / `color=zinc.50` lower to Tailwind classes
/// (`bg-blue-600 p-8 text-zinc-50`) on the web path, but the RN emitter previously
/// only recognized ~7 whole-class combos and dropped the rest. Here we translate the
/// granular utilities — colors (resolved to hex via the shared palette), the 4px
/// spacing scale, radius, font weight/size, opacity, width/height — into real RN
/// style props so mobile honors the same authoring vocabulary as web.
///
/// Returns `(prop, value)` pairs where `value` is already RN-literal-ready
/// (quoted strings keep their quotes, numbers are bare).
fn tailwind_tokens_to_rn_props(tokens: &[&str]) -> Vec<(String, String)> {
    let mut props: Vec<(String, String)> = Vec::new();
    for &tok in tokens {
        if let Some(pair) = tailwind_token_to_rn_prop(tok) {
            props.push(pair);
        }
    }
    props
}

/// Tailwind spacing step (1 = 0.25rem = 4px).
fn spacing_px(n: &str) -> Option<i32> {
    n.parse::<f32>().ok().map(|v| (v * 4.0).round() as i32)
}

/// Resolve a Tailwind color suffix (`zinc-400`, `white`) to a hex string via the
/// shared palette. Tailwind uses `hue-shade`; the palette keys are `hue.shade`.
fn resolve_tw_color(suffix: &str) -> Option<String> {
    let palette_name = suffix.replacen('-', ".", 1);
    crate::web_ir::validate_palette::resolve_color(&palette_name, None)
        .or_else(|| crate::web_ir::validate_palette::resolve_color(suffix, None))
}

fn tailwind_token_to_rn_prop(tok: &str) -> Option<(String, String)> {
    let q = |s: String| format!("\"{s}\"");
    // Colors.
    if let Some(c) = tok.strip_prefix("text-") {
        if let Some(hex) = resolve_tw_color(c) {
            return Some(("color".into(), q(hex)));
        }
        // Font sizes (text-xs … text-3xl).
        let fs = match c {
            "xs" => Some(12),
            "sm" => Some(14),
            "base" => Some(16),
            "lg" => Some(18),
            "xl" => Some(20),
            "2xl" => Some(24),
            "3xl" => Some(30),
            _ => None,
        };
        if let Some(px) = fs {
            return Some(("fontSize".into(), px.to_string()));
        }
        return None;
    }
    if let Some(c) = tok.strip_prefix("bg-") {
        return resolve_tw_color(c).map(|hex| ("backgroundColor".into(), q(hex)));
    }
    if let Some(c) = tok.strip_prefix("border-") {
        if let Some(hex) = resolve_tw_color(c) {
            return Some(("borderColor".into(), q(hex)));
        }
        return None;
    }
    // Font weight.
    match tok {
        "font-bold" => return Some(("fontWeight".into(), q("700".into()))),
        "font-semibold" => return Some(("fontWeight".into(), q("600".into()))),
        "font-medium" => return Some(("fontWeight".into(), q("500".into()))),
        "italic" => return Some(("fontStyle".into(), q("italic".into()))),
        _ => {}
    }
    // Spacing scale: p/px/py/pt/pb/pl/pr, m*, gap.
    let spacing: &[(&str, &str)] = &[
        ("px-", "paddingHorizontal"),
        ("py-", "paddingVertical"),
        ("pt-", "paddingTop"),
        ("pb-", "paddingBottom"),
        ("pl-", "paddingLeft"),
        ("pr-", "paddingRight"),
        ("p-", "padding"),
        ("mx-", "marginHorizontal"),
        ("my-", "marginVertical"),
        ("mt-", "marginTop"),
        ("mb-", "marginBottom"),
        ("ml-", "marginLeft"),
        ("mr-", "marginRight"),
        ("m-", "margin"),
        ("gap-", "gap"),
    ];
    for (prefix, prop) in spacing {
        if let Some(n) = tok.strip_prefix(prefix) {
            if let Some(px) = spacing_px(n) {
                return Some(((*prop).into(), px.to_string()));
            }
        }
    }
    // Border radius.
    if tok == "rounded" {
        return Some(("borderRadius".into(), "4".into()));
    }
    if let Some(sz) = tok.strip_prefix("rounded-") {
        let r = match sz {
            "sm" => Some(2),
            "md" => Some(6),
            "lg" => Some(8),
            "xl" => Some(12),
            "2xl" => Some(16),
            "full" => Some(9999),
            other => other.parse::<i32>().ok().map(|n| n * 4),
        };
        return r.map(|px| ("borderRadius".into(), px.to_string()));
    }
    // Opacity.
    if let Some(o) = tok.strip_prefix("opacity-") {
        if let Ok(n) = o.parse::<f32>() {
            return Some(("opacity".into(), format!("{:.2}", n / 100.0)));
        }
    }
    // Width / height (full → "100%", numeric → 4px scale).
    for (prefix, prop) in [("w-", "width"), ("h-", "height")] {
        if let Some(n) = tok.strip_prefix(prefix) {
            if n == "full" {
                return Some((prop.into(), q("100%".into())));
            }
            if let Some(px) = spacing_px(n) {
                return Some((prop.into(), px.to_string()));
            }
        }
    }
    None
}

/// Map VUV universal style *kwargs* (`bg`, `color`, `pad`, `gap`, `radius`, …) on a
/// view-call element directly to RN `StyleSheet` props. The RN emit path sees raw
/// kwargs (the web path folds them into `className`, but RN bypasses Web IR), so this
/// is the primary B3 source; the className mapper above is a fallback for pre-folded
/// shapes.
fn kwargs_to_rn_props(attrs: &[HirJsxAttr]) -> Vec<(String, String)> {
    let q = |s: String| format!("\"{s}\"");
    let val = |name: &str| -> Option<String> {
        attr_value(attrs, name).and_then(|e| {
            literal_string_value(e).or_else(|| extract_int_literal(e).map(|i| i.to_string()))
        })
    };
    let color = |v: &str| crate::web_ir::validate_palette::resolve_color(v, None);
    let mut out: Vec<(String, String)> = Vec::new();
    let mut push = |k: &str, v: String| out.push((k.to_string(), v));

    if let Some(v) = val("bg").and_then(|v| color(&v)) {
        push("backgroundColor", q(v));
    }
    if let Some(v) = val("color").and_then(|v| color(&v)) {
        push("color", q(v));
    }
    if let Some(v) = val("border_color").and_then(|v| color(&v)) {
        push("borderColor", q(v));
    }
    // Spacing kwargs → 4px scale.
    let spacing: &[(&str, &str)] = &[
        ("pad", "padding"),
        ("pad_x", "paddingHorizontal"),
        ("pad_y", "paddingVertical"),
        ("pad_t", "paddingTop"),
        ("pad_b", "paddingBottom"),
        ("pad_l", "paddingLeft"),
        ("pad_r", "paddingRight"),
        ("mb", "marginBottom"),
        ("mt", "marginTop"),
        ("ml", "marginLeft"),
        ("mr", "marginRight"),
        ("mx", "marginHorizontal"),
        ("my", "marginVertical"),
        ("gap", "gap"),
        ("gap_x", "columnGap"),
        ("gap_y", "rowGap"),
    ];
    for (kw, prop) in spacing {
        if let Some(px) = val(kw).as_deref().and_then(spacing_px) {
            push(prop, px.to_string());
        }
    }
    // Radius (named scale or numeric step).
    if let Some(r) = val("radius") {
        let px = match r.as_str() {
            "sm" => Some(2),
            "md" => Some(6),
            "lg" => Some(8),
            "xl" => Some(12),
            "2xl" => Some(16),
            "full" => Some(9999),
            other => other.parse::<i32>().ok().map(|n| n * 4),
        };
        if let Some(px) = px {
            push("borderRadius", px.to_string());
        }
    }
    if let Some(o) = val("opacity").and_then(|v| v.parse::<f32>().ok()) {
        push("opacity", format!("{:.2}", o / 100.0));
    }
    out
}

/// Render RN style prop pairs as an object-literal body (no surrounding braces).
fn rn_props_to_object_body(props: &[(String, String)]) -> String {
    props
        .iter()
        .map(|(k, v)| format!("{k}: {v}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Extract the class tokens from a `className={[...].filter(Boolean).join(" ")}` attribute.
/// Returns an empty vec when the expression doesn't match the known emit shape.
fn extract_class_tokens(attr_value: &HirExpr) -> Vec<String> {
    // The HIR for `className={["a","b"].filter(Boolean).join(" ")}` is a method-call
    // chain over a list literal. Walk the chain back to the original list.
    let list_expr = match attr_value {
        HirExpr::MethodCall(receiver, method, _args, _, _) if method == "join" => {
            // .join(" ") — recurse to find .filter() target.
            if let HirExpr::MethodCall(inner, m2, _, _, _) = receiver.as_ref() {
                if m2 == "filter" {
                    inner.as_ref()
                } else {
                    receiver.as_ref()
                }
            } else {
                receiver.as_ref()
            }
        }
        _ => attr_value,
    };
    if let HirExpr::ListLit(items, _) = list_expr {
        items
            .iter()
            .filter_map(|e| match e {
                HirExpr::StringLit(s, _) => Some(s.clone()),
                _ => None,
            })
            .collect()
    } else {
        Vec::new()
    }
}

/// Emit a JS literal / interpolation for a HirExpr suitable for placing inside
/// `{...}` braces in JSX. Reuses the shared HIR → TS expression lowering so the
/// two emit targets cannot diverge on expression-level syntax (split-brain
/// protection: both targets call the SAME `emit_hir_expr`).
fn emit_hir_expr_inline_with_state(
    expr: &HirExpr,
    state_names: &HashSet<String>,
    endpoint_params: &HashMap<String, Vec<String>>,
) -> String {
    // `with_endpoints` (not `new`) so positional endpoint calls
    // (`record_event("mood", ...)`) are rewritten to the named-object form
    // (`record_event({ event_kind: "mood", ... })`) that `vox-client.ts`
    // expects — same rewrite the web reactive emit performs.
    let ctx = crate::codegen_ts::hir_emit::EmitCtx::with_endpoints(state_names, endpoint_params);
    crate::codegen_ts::hir_emit::emit_hir_expr(expr, &ctx)
}

fn emit_hir_expr_inline(expr: &HirExpr) -> String {
    let empty_states: HashSet<String> = HashSet::new();
    let empty_endpoints: HashMap<String, Vec<String>> = HashMap::new();
    emit_hir_expr_inline_with_state(expr, &empty_states, &empty_endpoints)
}

/// Emit an event handler — produces a clean arrow lambda for the JSX
/// `onPress={...}` attribute. State assignments (`n = n + 1`) inside the
/// body are rewritten to setter calls (`set_n(n + 1)`) by the shared HIR
/// emit when the variable appears in `state_names`.
fn emit_event_handler_with_state(
    expr: &HirExpr,
    state_names: &HashSet<String>,
    endpoint_params: &HashMap<String, Vec<String>>,
) -> String {
    // Thread the async client-fn names (every `@query`/`@mutation` endpoint) so a
    // handler body that calls one is emitted as an `async () => { … await … }`
    // arrow by the shared lambda lowering (`EmitCtx::handler_await`). Without this
    // the body would emit sync nested IIFEs and a `match record_event(...) {…}`
    // would discriminate on the pending Promise — never running the Ok/Error arm
    // (and, for a chained Save, never firing the inner mutation). Same async
    // machinery the web reactive emit uses via `view_ctx`.
    let async_fn_names: HashSet<String> = endpoint_params.keys().cloned().collect();
    let ctx = crate::codegen_ts::hir_emit::EmitCtx::with_async_and_endpoints(
        state_names,
        &async_fn_names,
        endpoint_params,
    );
    let body = crate::codegen_ts::hir_emit::emit_hir_expr(expr, &ctx);
    extract_or_wrap_arrow(&body).into_owned()
}

/// Normalize a JS expression string into a callable arrow lambda suitable for an
/// event handler. Handles four input shapes:
///
///   1. `(() => EXPR)()` / `(() => { ... })()` — IIFE invocation. Strip the
///      outer `(...)()` to recover the inner arrow lambda directly.
///   2. `(() => EXPR)` / `(() => { ... })` — parenthesized arrow lambda.
///      Strip the outer parens; the inner arrow is the handler.
///   3. `() => EXPR` / `async () => EXPR` — already clean. Use unchanged.
///   4. Anything else — wrap as `() => (BODY)` (for an expression) or
///      `() => BODY` (for a `{ ... }` block).
fn extract_or_wrap_arrow(body: &str) -> std::borrow::Cow<'_, str> {
    let trimmed = body.trim();

    // Shape 1: `(() => ...)()` — IIFE.
    if trimmed.ends_with(")()") && trimmed.starts_with('(') {
        let inner = &trimmed[1..trimmed.len() - 3];
        let inner_trim = inner.trim_start();
        if inner_trim.starts_with("() =>") || inner_trim.starts_with("async () =>") {
            return std::borrow::Cow::Owned(inner.to_string());
        }
    }

    // Shape 2: `(() => ...)` — parenthesized arrow. Verify the leading `(` and
    // a balancing trailing `)` actually pair (no `()()` chain hiding inside).
    if trimmed.starts_with('(') && trimmed.ends_with(')') && {
        let inner = &trimmed[1..trimmed.len() - 1];
        let inner_trim = inner.trim_start();
        inner_trim.starts_with("() =>") || inner_trim.starts_with("async () =>")
    } {
        let inner = &trimmed[1..trimmed.len() - 1];
        return std::borrow::Cow::Owned(inner.to_string());
    }

    // Shape 3: already a clean arrow lambda.
    if trimmed.starts_with("() =>") || trimmed.starts_with("async () =>") {
        return std::borrow::Cow::Borrowed(body);
    }

    // Shape 4: bare expression or block — wrap.
    if trimmed.starts_with('{') {
        std::borrow::Cow::Owned(format!("() => {body}"))
    } else {
        std::borrow::Cow::Owned(format!("() => ({body})"))
    }
}

fn attr_value<'a>(attrs: &'a [HirJsxAttr], name: &str) -> Option<&'a HirExpr> {
    attrs.iter().find(|a| a.name == name).map(|a| &a.value)
}

/// True when a screen-root view opts OUT of default edge padding via `bleed`
/// (present, and not literally `false`). Shared with the web reactive emit so
/// both targets honor the same opt-out.
pub(crate) fn root_view_bleeds(view_root: &HirExpr) -> bool {
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

/// True if the component's view contains a `link(...)` / `<a>` element, so the
/// emitter knows to `import { Link } from "expo-router"`.
fn component_uses_link(rc: &HirReactiveComponent) -> bool {
    fn expr_has_link(e: &HirExpr) -> bool {
        match e {
            HirExpr::Jsx(el) => {
                el.tag == "link" || el.tag == "a" || el.children.iter().any(expr_has_link)
            }
            HirExpr::JsxSelfClosing(sc) => sc.tag == "link" || sc.tag == "a",
            HirExpr::JsxFragment(children, _) => children.iter().any(expr_has_link),
            HirExpr::For(_, _, iter, body, _, _) => expr_has_link(iter) || expr_has_link(body),
            HirExpr::Block(stmts, _) => stmts.iter().any(stmt_has_link),
            HirExpr::If(c, t, e2, _) => {
                expr_has_link(c)
                    || t.iter().any(stmt_has_link)
                    || e2.as_ref().is_some_and(|s| s.iter().any(stmt_has_link))
            }
            _ => false,
        }
    }
    fn stmt_has_link(s: &HirStmt) -> bool {
        match s {
            HirStmt::Expr { expr, .. } | HirStmt::Let { value: expr, .. } => expr_has_link(expr),
            _ => false,
        }
    }
    rc.view.as_ref().is_some_and(expr_has_link)
}

/// Pull a literal integer out of a HirExpr (used for `heading(level=1)`).
fn extract_int_literal(expr: &HirExpr) -> Option<i64> {
    if let HirExpr::IntLit(n, _) = expr {
        Some(*n)
    } else {
        None
    }
}

/// Pull a literal string value out of a HirExpr, unwrapping a single-expr block
/// (used for `row(scroll: "horizontal")`).
fn literal_string_value(expr: &HirExpr) -> Option<String> {
    match expr {
        HirExpr::StringLit(s, _) => Some(s.clone()),
        HirExpr::Block(stmts, _) if stmts.len() == 1 => {
            if let HirStmt::Expr { expr, .. } = &stmts[0] {
                literal_string_value(expr)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Translate an `HirJsxElement` (raw VUV names OR React-DOM-flavored — both shapes are
/// accepted so this path works whether the HIR view came from the reactive component
/// declaration directly or post-WebIR-lowering) to an `RnNode`.
///
/// VUV abstract names (`column`, `row`, `stack`, `heading`, `panel`, `text`, `button`,
/// `image`, `text_input`) are the primary shape. The React-DOM forms (`div`, `h1`,
/// `button`, ...) are also recognized so this lowering survives any future change
/// to where in the pipeline the VUV→DOM lowering happens.
fn jsx_to_rn(
    el: &HirJsxElement,
    state_names: &HashSet<String>,
    endpoint_params: &HashMap<String, Vec<String>>,
    diagnostics: &mut Vec<WebIrDiagnostic>,
    dyn_styles: &mut BTreeMap<String, String>,
) -> RnNode {
    let class_tokens_owned = attr_value(&el.attributes, "className")
        .map(extract_class_tokens)
        .unwrap_or_default();
    let class_refs: Vec<&str> = class_tokens_owned.iter().map(|s| s.as_str()).collect();
    let style_key = class_string_to_style_key(&class_refs).map(String::from);

    // Recognize both snake_case (`on_click`) and camelCase (`onClick`) attribute names —
    // depending on which lowering produced the HIR, either may appear.
    let handler_attr =
        attr_value(&el.attributes, "on_click").or_else(|| attr_value(&el.attributes, "onClick"));
    let on_press =
        handler_attr.map(|h| emit_event_handler_with_state(h, state_names, endpoint_params));

    let children: Vec<RnNode> = el
        .children
        .iter()
        .map(|c| hir_view_child_to_rn(c, state_names, endpoint_params, diagnostics, dyn_styles))
        .collect();

    let styled = match el.tag.as_str() {
        // ── VUV abstract container primitives ────────────────────────────────
        "column" | "stack" | "div" => RnNode::View {
            style_key: style_key.or(Some("col".to_string())),
            children,
        },
        "row" => {
            // `row(scroll: "horizontal")` (or "x") → a single non-wrapping line
            // in a horizontal ScrollView (the right UX for nav bars). A bare
            // `row` uses the wrap-by-default `row` style and can never overflow.
            let scroll = attr_value(&el.attributes, "scroll")
                .and_then(literal_string_value)
                .unwrap_or_default();
            if scroll == "horizontal" || scroll == "x" {
                RnNode::ScrollRow { children }
            } else {
                RnNode::View {
                    style_key: style_key.or(Some("row".to_string())),
                    children,
                }
            }
        }
        "panel" => RnNode::View {
            style_key: style_key.or(Some("panel".to_string())),
            children,
        },

        // ── Overlay-family tier primitives (B4) ──────────────────────────────
        // `overlay` is the transparent portal host on web; on RN it's just an
        // in-place container (children render where declared).
        "overlay" => RnNode::View {
            style_key,
            children,
        },
        // `modal` / `drawer` → react-native <Modal>. `open` drives `visible`.
        "modal" | "drawer" => {
            let visible_ts = attr_value(&el.attributes, "open")
                .map(|e| emit_hir_expr_inline_with_state(e, state_names, endpoint_params))
                .unwrap_or_else(|| "true".to_string());
            let animation = if el.tag == "drawer" { "slide" } else { "fade" };
            RnNode::RnModal {
                animation,
                visible_ts,
                children,
            }
        }
        // `toast` → a transient absolutely-positioned banner near the bottom.
        "toast" => RnNode::View {
            style_key: Some("toast".to_string()),
            children,
        },

        // ── VUV abstract text primitives ─────────────────────────────────────
        "heading" => {
            // `heading(level=N)` — map the integer level to a header style key.
            let level = attr_value(&el.attributes, "level")
                .and_then(extract_int_literal)
                .unwrap_or(1);
            let level_key = match level {
                1 => "h1",
                2 => "h2",
                3 => "h3",
                _ => "h3", // h4+ render at h3 weight for now; can extend later.
            };
            RnNode::Text {
                style_key: Some(level_key.to_string()),
                children,
            }
        }
        "h1" => RnNode::Text {
            style_key: Some("h1".to_string()),
            children,
        },
        "h2" => RnNode::Text {
            style_key: Some("h2".to_string()),
            children,
        },
        "h3" | "h4" | "h5" | "h6" => RnNode::Text {
            style_key: Some("h3".to_string()),
            children,
        },
        "text" | "p" => RnNode::Text {
            style_key: style_key.or(Some("body".to_string())),
            children,
        },
        "span" => RnNode::Text {
            style_key: None,
            children,
        },

        // ── VUV abstract interactive primitives ──────────────────────────────
        "button" => RnNode::Pressable {
            style_key: style_key.or(Some("btn_primary".to_string())),
            handler_ts: on_press,
            children,
        },
        "image" | "img" => {
            let src = attr_value(&el.attributes, "src")
                .map(emit_hir_expr_inline)
                .unwrap_or_else(|| "\"\"".to_string());
            let alt = attr_value(&el.attributes, "alt").map(emit_hir_expr_inline);
            RnNode::Image {
                src_ts: src,
                alt_ts: alt,
                style_key,
            }
        }
        "text_input" | "input" => RnNode::TextInput {
            style_key,
            value_ts: attr_value(&el.attributes, "value").map(emit_hir_expr_inline),
            on_change_ts: attr_value(&el.attributes, "on_change")
                .or_else(|| attr_value(&el.attributes, "onChange"))
                .map(|h| emit_event_handler_with_state(h, state_names, endpoint_params)),
            placeholder_ts: attr_value(&el.attributes, "placeholder").map(emit_hir_expr_inline),
        },

        // VUV `link(href="/x") { "label" }` → expo-router `<Link>`. Without this
        // every navigation link rendered as an inert `<View>` and the app could
        // not move between routes.
        "link" | "a" => {
            let href_ts = attr_value(&el.attributes, "href")
                .map(emit_hir_expr_inline)
                .unwrap_or_else(|| "\"/\"".to_string());
            RnNode::Link {
                href_ts,
                style_key,
                children,
            }
        }

        other => {
            // PascalCase tag → user-declared `component Name(...) { view: ... }` call.
            // React's JSX convention is that any tag starting with an uppercase
            // letter is a component reference, not a built-in element. Pass
            // through every attribute as a JSX `{<expr>}` block so callers like
            // `EntryCard(label=item)` lower to `<EntryCard label={item}/>`.
            if other.chars().next().is_some_and(|c| c.is_ascii_uppercase()) {
                let attrs: Vec<(String, String)> = el
                    .attributes
                    .iter()
                    .filter(|a| a.name != "className") // className handled via style_key on built-ins; not for custom
                    .map(|a| {
                        (
                            a.name.clone(),
                            emit_hir_expr_inline_with_state(&a.value, state_names, endpoint_params),
                        )
                    })
                    .collect();
                return RnNode::CustomComponent {
                    tag_name: other.to_string(),
                    attributes: attrs,
                    children,
                };
            }
            // Lowercase unknown tag → genuinely unsupported. Surface the diagnostic
            // (not a silent stub) and fall back to a bare `<View>` so the build
            // still produces something inspectable.
            diagnostics.push(WebIrDiagnostic {
                code: "vox/codegen/rn-unsupported-tag".to_string(),
                message: format!(
                    "RN lowering does not yet handle tag `<{other}>` — falling back to `<View>`. \
                     Add a mapping to crates/vox-codegen/src/codegen_ts/rn/component.rs."
                ),
                span: None,
                category: Some("codegen".to_string()),
            });
            RnNode::View {
                style_key,
                children,
            }
        }
    };
    // B3: fold any granular style props (colors/spacing/radius/…) that the semantic
    // table didn't capture into the node's style, so mobile honors the same kwarg
    // vocabulary as web instead of dropping it. Sources: raw kwargs (primary) +
    // className tokens (fallback for pre-folded shapes).
    let mut extra = kwargs_to_rn_props(&el.attributes);
    extra.extend(tailwind_tokens_to_rn_props(&class_refs));
    apply_dyn_style_to_node(styled, &extra, dyn_styles)
}

/// Rewrite a styled leaf node's `style_key` to a synthetic merged entry when it
/// carries extra inline props. Containers/text/pressables only; structural nodes
/// (ScrollRow, CustomComponent, …) pass through unchanged.
fn apply_dyn_style_to_node(
    node: RnNode,
    extra: &[(String, String)],
    dyn_styles: &mut BTreeMap<String, String>,
) -> RnNode {
    match node {
        RnNode::View { style_key, children } => RnNode::View {
            style_key: apply_dyn_style(style_key, extra, dyn_styles),
            children,
        },
        RnNode::Text { style_key, children } => RnNode::Text {
            style_key: apply_dyn_style(style_key, extra, dyn_styles),
            children,
        },
        RnNode::Pressable { style_key, handler_ts, children } => RnNode::Pressable {
            style_key: apply_dyn_style(style_key, extra, dyn_styles),
            handler_ts,
            children,
        },
        other => other,
    }
}

fn hir_view_child_to_rn(
    child: &HirExpr,
    state_names: &HashSet<String>,
    endpoint_params: &HashMap<String, Vec<String>>,
    diagnostics: &mut Vec<WebIrDiagnostic>,
    dyn_styles: &mut BTreeMap<String, String>,
) -> RnNode {
    match child {
        HirExpr::Jsx(el) => jsx_to_rn(el, state_names, endpoint_params, diagnostics, dyn_styles),
        HirExpr::JsxSelfClosing(sc) => jsx_to_rn(
            &HirJsxElement {
                tag: sc.tag.clone(),
                attributes: sc.attributes.clone(),
                children: vec![],
                span: sc.span,
            },
            state_names,
            endpoint_params,
            diagnostics,
            dyn_styles,
        ),
        HirExpr::JsxFragment(children, _) => RnNode::View {
            style_key: None,
            children: children
                .iter()
                .map(|c| hir_view_child_to_rn(c, state_names, endpoint_params, diagnostics, dyn_styles))
                .collect(),
        },
        // VUV `for item, i in items key=item.id { <body> }` — recurse so the body
        // gets RN-translated, never falls through to the shared React-DOM emit.
        // Without this, the shared emit would produce `<div>` / `<p>` inside an
        // RN tree (split-brain bug).
        HirExpr::For(item, index, iter, body, key, _) => {
            let iterator_ts = emit_hir_expr_inline_with_state(iter, state_names, endpoint_params);
            let key_ts = key
                .as_ref()
                .map(|k| emit_hir_expr_inline_with_state(k, state_names, endpoint_params));
            // The body is either a single JSX element or a Block of statements.
            // Both are recursively walked so every child renders in RN form.
            let body_nodes = match body.as_ref() {
                HirExpr::Block(stmts, _) => stmts
                    .iter()
                    .filter_map(|s| match s {
                        HirStmt::Expr { expr, .. } => Some(hir_view_child_to_rn(
                            expr,
                            state_names,
                            endpoint_params,
                            diagnostics,
                            dyn_styles,
                        )),
                        _ => None,
                    })
                    .collect(),
                other => vec![hir_view_child_to_rn(
                    other,
                    state_names,
                    endpoint_params,
                    diagnostics,
                    dyn_styles,
                )],
            };
            RnNode::Loop {
                iterator_ts,
                item_name: item.clone(),
                index_name: index.clone(),
                key_ts,
                body: body_nodes,
            }
        }
        HirExpr::StringLit(s, _) => RnNode::StringLit(s.clone()),
        other => RnNode::Expr(emit_hir_expr_inline_with_state(
            other,
            state_names,
            endpoint_params,
        )),
    }
}

/// Walk RN tree to gather every distinct style key that needs a StyleSheet entry.
fn collect_used_styles(node: &RnNode, out: &mut std::collections::BTreeSet<String>) {
    match node {
        RnNode::View {
            style_key,
            children,
        } => {
            if let Some(k) = style_key {
                out.insert(k.clone());
            }
            for c in children {
                collect_used_styles(c, out);
            }
        }
        RnNode::Text {
            style_key,
            children,
        } => {
            if let Some(k) = style_key {
                out.insert(k.clone());
            }
            for c in children {
                collect_used_styles(c, out);
            }
        }
        RnNode::Pressable {
            style_key,
            children,
            ..
        } => {
            if let Some(k) = style_key {
                out.insert(k.clone());
            }
            // Emission wraps any bare string/expr child in `<Text style={styles.btn_text}>`
            // (see `wrap_pressable_text_children`), so the style is genuinely used.
            if children
                .iter()
                .any(|c| matches!(c, RnNode::StringLit(_) | RnNode::Expr(_)))
            {
                out.insert("btn_text".to_string());
            }
            for c in children {
                collect_used_styles(c, out);
            }
        }
        RnNode::Image { style_key, .. } | RnNode::TextInput { style_key, .. } => {
            if let Some(k) = style_key {
                out.insert(k.clone());
            }
        }
        RnNode::Loop { body, .. } => {
            for c in body {
                collect_used_styles(c, out);
            }
        }
        RnNode::CustomComponent { children, .. } => {
            for c in children {
                collect_used_styles(c, out);
            }
        }
        RnNode::Link {
            style_key,
            children,
            ..
        } => {
            if let Some(k) = style_key {
                out.insert(k.clone());
            }
            // Link text children are wrapped in `<Text style={styles.btn_text}>` at emit.
            if children
                .iter()
                .any(|c| matches!(c, RnNode::StringLit(_) | RnNode::Expr(_)))
            {
                out.insert("btn_text".to_string());
            }
            for c in children {
                collect_used_styles(c, out);
            }
        }
        RnNode::ScrollRow { children } => {
            out.insert("row_scroll_content".to_string());
            for c in children {
                collect_used_styles(c, out);
            }
        }
        RnNode::RnModal { children, .. } => {
            out.insert("modal_backdrop".to_string());
            for c in children {
                collect_used_styles(c, out);
            }
        }
        RnNode::StringLit(_) | RnNode::Expr(_) => {}
    }
}

/// Wrap any non-Text children inside `<Pressable>` in `<Text>` so the RN runtime
/// doesn't error with "Text strings must be rendered within a `<Text>` component."
fn wrap_pressable_text_children(children: Vec<RnNode>) -> Vec<RnNode> {
    let mut out = Vec::with_capacity(children.len());
    for c in children {
        match c {
            RnNode::StringLit(s) => out.push(RnNode::Text {
                style_key: Some("btn_text".to_string()),
                children: vec![RnNode::StringLit(s)],
            }),
            RnNode::Expr(e) => out.push(RnNode::Text {
                style_key: Some("btn_text".to_string()),
                children: vec![RnNode::Expr(e)],
            }),
            other => out.push(other),
        }
    }
    out
}

fn emit_rn_node(node: &RnNode, indent: usize) -> String {
    let pad = "  ".repeat(indent);
    match node {
        RnNode::View {
            style_key,
            children,
        } => {
            let style_attr = style_key
                .as_ref()
                .map(|k| format!(" style={{styles.{k}}}"))
                .unwrap_or_default();
            if children.is_empty() {
                return format!("{pad}<View{style_attr} />\n");
            }
            let mut inner = String::new();
            for c in children {
                inner.push_str(&emit_rn_node(c, indent + 1));
            }
            format!("{pad}<View{style_attr}>\n{inner}{pad}</View>\n")
        }
        RnNode::Text {
            style_key,
            children,
        } => {
            let style_attr = style_key
                .as_ref()
                .map(|k| format!(" style={{styles.{k}}}"))
                .unwrap_or_default();
            // Text children get inlined for tighter output.
            let mut inner = String::new();
            for c in children {
                inner.push_str(&emit_text_child_inline(c));
            }
            format!("{pad}<Text{style_attr}>{inner}</Text>\n")
        }
        RnNode::Pressable {
            style_key,
            handler_ts,
            children,
        } => {
            let style_attr = style_key
                .as_ref()
                .map(|k| format!(" style={{styles.{k}}}"))
                .unwrap_or_default();
            let press_attr = handler_ts
                .as_ref()
                .map(|h| format!(" onPress={{{h}}}"))
                .unwrap_or_default();
            let wrapped = wrap_pressable_text_children(
                children
                    .iter()
                    .map(|c| match c {
                        RnNode::StringLit(s) => RnNode::StringLit(s.clone()),
                        RnNode::Expr(e) => RnNode::Expr(e.clone()),
                        other => clone_rn_node(other),
                    })
                    .collect(),
            );
            let mut inner = String::new();
            for c in &wrapped {
                inner.push_str(&emit_rn_node(c, indent + 1));
            }
            format!("{pad}<Pressable{style_attr}{press_attr}>\n{inner}{pad}</Pressable>\n")
        }
        RnNode::Image {
            src_ts,
            alt_ts,
            style_key,
        } => {
            let style_attr = style_key
                .as_ref()
                .map(|k| format!(" style={{styles.{k}}}"))
                .unwrap_or_default();
            let alt_attr = alt_ts
                .as_ref()
                .map(|a| format!(" accessibilityLabel={{{a}}}"))
                .unwrap_or_default();
            format!("{pad}<Image source={{{{ uri: {src_ts} }}}}{alt_attr}{style_attr} />\n")
        }
        RnNode::TextInput {
            style_key,
            value_ts,
            on_change_ts,
            placeholder_ts,
        } => {
            let style_attr = style_key
                .as_ref()
                .map(|k| format!(" style={{styles.{k}}}"))
                .unwrap_or_default();
            let value_attr = value_ts
                .as_ref()
                .map(|v| format!(" value={{{v}}}"))
                .unwrap_or_default();
            let onc_attr = on_change_ts
                .as_ref()
                .map(|h| format!(" onChangeText={{{h}}}"))
                .unwrap_or_default();
            let ph_attr = placeholder_ts
                .as_ref()
                .map(|p| format!(" placeholder={{{p}}}"))
                .unwrap_or_default();
            format!("{pad}<TextInput{style_attr}{value_attr}{onc_attr}{ph_attr} />\n")
        }
        RnNode::Loop {
            iterator_ts,
            item_name,
            index_name,
            key_ts,
            body,
        } => {
            // RN renders a JSX-expression loop as `{iter.map((item, i) => (<body/>))}`.
            // The first body node must carry a `key={...}` attribute or React warns;
            // we inject it onto the first element in the rendered body.
            let params = match index_name {
                Some(idx) => format!("({item_name}: any, {idx}: number)"),
                None => format!("({item_name}: any)"),
            };
            let mut inner = String::new();
            for c in body {
                inner.push_str(&emit_rn_node(c, indent + 2));
            }
            // A `.map(...)` arrow must return a single element. When the loop body
            // lowers to multiple sibling nodes, wrap them in a keyed `<View>` (always
            // imported, and unlike the `<>` shorthand it can carry the `key`). A
            // single-node body injects `key` into that node directly.
            let body_render = if body.len() > 1 {
                let key_attr = key_ts
                    .as_ref()
                    .map(|k| format!(" key={{{k}}}"))
                    .unwrap_or_default();
                format!("{pad}  <View{key_attr}>\n{inner}{pad}  </View>\n")
            } else if let Some(k) = key_ts {
                inject_key_into_first_element(inner, k)
            } else {
                inner
            };
            format!("{pad}{{{iterator_ts}.map({params} => (\n{body_render}{pad}  ))}}\n")
        }
        RnNode::CustomComponent {
            tag_name,
            attributes,
            children,
        } => {
            let attr_str = if attributes.is_empty() {
                String::new()
            } else {
                let mut s = String::new();
                for (name, expr) in attributes {
                    s.push(' ');
                    s.push_str(name);
                    s.push('=');
                    s.push('{');
                    s.push_str(expr);
                    s.push('}');
                }
                s
            };
            if children.is_empty() {
                return format!("{pad}<{tag_name}{attr_str} />\n");
            }
            let mut inner = String::new();
            for c in children {
                inner.push_str(&emit_rn_node(c, indent + 1));
            }
            format!("{pad}<{tag_name}{attr_str}>\n{inner}{pad}</{tag_name}>\n")
        }
        RnNode::Link {
            href_ts,
            style_key,
            children,
        } => {
            let style_attr = style_key
                .as_ref()
                .map(|k| format!(" style={{styles.{k}}}"))
                .unwrap_or_default();
            // Bare string/expr children must be wrapped in <Text> (same rule as
            // Pressable) so they render and are tappable inside the Link.
            let wrapped = wrap_pressable_text_children(
                children
                    .iter()
                    .map(|c| match c {
                        RnNode::StringLit(s) => RnNode::StringLit(s.clone()),
                        RnNode::Expr(e) => RnNode::Expr(e.clone()),
                        other => clone_rn_node(other),
                    })
                    .collect(),
            );
            let mut inner = String::new();
            for c in &wrapped {
                inner.push_str(&emit_rn_node(c, indent + 1));
            }
            format!("{pad}<Link href={{{href_ts}}}{style_attr}>\n{inner}{pad}</Link>\n")
        }
        RnNode::ScrollRow { children } => {
            let mut inner = String::new();
            for c in children {
                inner.push_str(&emit_rn_node(c, indent + 1));
            }
            format!(
                "{pad}<ScrollView horizontal showsHorizontalScrollIndicator={{false}} contentContainerStyle={{styles.row_scroll_content}}>\n{inner}{pad}</ScrollView>\n"
            )
        }
        RnNode::RnModal {
            animation,
            visible_ts,
            children,
        } => {
            let mut inner = String::new();
            for c in children {
                inner.push_str(&emit_rn_node(c, indent + 2));
            }
            // transparent + a centered backdrop View is the standard RN dialog shell.
            format!(
                "{pad}<Modal transparent animationType=\"{animation}\" visible={{{visible_ts}}}>\n{pad}  <View style={{styles.modal_backdrop}}>\n{inner}{pad}  </View>\n{pad}</Modal>\n"
            )
        }
        RnNode::StringLit(s) => {
            // String at View context — wrap defensively (RN forbids bare strings outside <Text>).
            let lit = serde_json::to_string(s).unwrap_or_else(|_| "\"\"".to_string());
            format!("{pad}<Text>{{{lit}}}</Text>\n")
        }
        RnNode::Expr(e) => format!("{pad}{{{e}}}\n"),
    }
}

fn emit_text_child_inline(child: &RnNode) -> String {
    match child {
        RnNode::StringLit(s) => {
            let lit = serde_json::to_string(s).unwrap_or_else(|_| "\"\"".to_string());
            format!("{{{lit}}}")
        }
        RnNode::Expr(e) => format!("{{{e}}}"),
        RnNode::Text { children, .. } => children
            .iter()
            .map(emit_text_child_inline)
            .collect::<String>(),
        _ => String::new(),
    }
}

fn clone_rn_node(n: &RnNode) -> RnNode {
    match n {
        RnNode::View {
            style_key,
            children,
        } => RnNode::View {
            style_key: style_key.clone(),
            children: children.iter().map(clone_rn_node).collect(),
        },
        RnNode::Text {
            style_key,
            children,
        } => RnNode::Text {
            style_key: style_key.clone(),
            children: children.iter().map(clone_rn_node).collect(),
        },
        RnNode::Pressable {
            style_key,
            handler_ts,
            children,
        } => RnNode::Pressable {
            style_key: style_key.clone(),
            handler_ts: handler_ts.clone(),
            children: children.iter().map(clone_rn_node).collect(),
        },
        RnNode::Image {
            src_ts,
            alt_ts,
            style_key,
        } => RnNode::Image {
            src_ts: src_ts.clone(),
            alt_ts: alt_ts.clone(),
            style_key: style_key.clone(),
        },
        RnNode::TextInput {
            style_key,
            value_ts,
            on_change_ts,
            placeholder_ts,
        } => RnNode::TextInput {
            style_key: style_key.clone(),
            value_ts: value_ts.clone(),
            on_change_ts: on_change_ts.clone(),
            placeholder_ts: placeholder_ts.clone(),
        },
        RnNode::Loop {
            iterator_ts,
            item_name,
            index_name,
            key_ts,
            body,
        } => RnNode::Loop {
            iterator_ts: iterator_ts.clone(),
            item_name: item_name.clone(),
            index_name: index_name.clone(),
            key_ts: key_ts.clone(),
            body: body.iter().map(clone_rn_node).collect(),
        },
        RnNode::CustomComponent {
            tag_name,
            attributes,
            children,
        } => RnNode::CustomComponent {
            tag_name: tag_name.clone(),
            attributes: attributes.clone(),
            children: children.iter().map(clone_rn_node).collect(),
        },
        RnNode::Link {
            href_ts,
            style_key,
            children,
        } => RnNode::Link {
            href_ts: href_ts.clone(),
            style_key: style_key.clone(),
            children: children.iter().map(clone_rn_node).collect(),
        },
        RnNode::ScrollRow { children } => RnNode::ScrollRow {
            children: children.iter().map(clone_rn_node).collect(),
        },
        RnNode::RnModal {
            animation,
            visible_ts,
            children,
        } => RnNode::RnModal {
            animation,
            visible_ts: visible_ts.clone(),
            children: children.iter().map(clone_rn_node).collect(),
        },
        RnNode::StringLit(s) => RnNode::StringLit(s.clone()),
        RnNode::Expr(e) => RnNode::Expr(e.clone()),
    }
}

/// Insert `key={<expr>}` into the opening tag of the first JSX element in `inner`.
/// Used to satisfy React's `key` requirement on `.map(...)` children — without this
/// users get the noisy "Each child in a list should have a unique key" warning
/// at runtime even though their Vox source declared `key=...`.
fn inject_key_into_first_element(inner: String, key_ts: &str) -> String {
    let bytes = inner.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'<' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_alphabetic() {
            break;
        }
        i += 1;
    }
    if i >= bytes.len() {
        return inner;
    }
    let mut j = i + 1;
    while j < bytes.len() && bytes[j] != b'>' {
        j += 1;
    }
    if j >= bytes.len() {
        return inner;
    }
    let insert_at = if j > 0 && bytes[j - 1] == b'/' {
        j - 1
    } else {
        j
    };
    let key_attr = format!(" key={{{key_ts}}}");
    let mut out = String::with_capacity(inner.len() + key_attr.len());
    out.push_str(&inner[..insert_at]);
    out.push_str(&key_attr);
    out.push_str(&inner[insert_at..]);
    out
}

/// StyleSheet entries indexed by the keys [`class_string_to_style_key`] returns.
/// The fixed semantic StyleSheet table (keys returned by [`class_string_to_style_key`]
/// and the per-primitive defaults). Lifted so [`apply_dyn_style`] can merge a node's
/// extra inline props onto its semantic base.
fn semantic_style_table() -> BTreeMap<&'static str, &'static str> {
    BTreeMap::from([
        ("col", "{ flexDirection: \"column\", gap: 12 }"),
        // Screen-root wrapper: default horizontal edge padding (opt out with `bleed`).
        ("screen", "{ flex: 1, paddingHorizontal: 16 }"),
        // `row` wraps by default so children can never run off the right edge
        // (RN children default to flexShrink:0). `columnGap`/`rowGap` keep
        // spacing correct once wrapped. A `row(scroll: "horizontal")` opts into
        // a single non-wrapping scrollable line instead (see `row_scroll_content`).
        (
            "row",
            "{ flexDirection: \"row\", flexWrap: \"wrap\", columnGap: 12, rowGap: 12, alignItems: \"center\" }",
        ),
        (
            "row_scroll_content",
            "{ flexDirection: \"row\", columnGap: 12, alignItems: \"center\" }",
        ),
        ("h1", "{ fontSize: 30, fontWeight: \"600\" }"),
        ("h2", "{ fontSize: 24, fontWeight: \"600\" }"),
        ("h3", "{ fontSize: 20, fontWeight: \"600\" }"),
        ("body", "{ fontSize: 14 }"),
        (
            "btn_primary",
            "{ backgroundColor: \"#0a7ea4\", paddingVertical: 10, paddingHorizontal: 16, borderRadius: 6, alignItems: \"center\" }",
        ),
        (
            "btn_secondary",
            "{ backgroundColor: \"#e5e7eb\", paddingVertical: 10, paddingHorizontal: 16, borderRadius: 6, alignItems: \"center\" }",
        ),
        ("btn_text", "{ color: \"white\", fontWeight: \"500\" }"),
        (
            "panel",
            "{ padding: 16, backgroundColor: \"#f5f5f5\", borderRadius: 8, borderWidth: 1, borderColor: \"#e5e7eb\" }",
        ),
        // Centered dim backdrop for the RN <Modal> dialog shell (B4).
        (
            "modal_backdrop",
            "{ flex: 1, justifyContent: \"center\", alignItems: \"center\", padding: 24, backgroundColor: \"rgba(0,0,0,0.5)\" }",
        ),
        // Transient bottom banner for the `toast` tier primitive (B4).
        (
            "toast",
            "{ position: \"absolute\", left: 16, right: 16, bottom: 32, padding: 12, borderRadius: 8, backgroundColor: \"#18181b\" }",
        ),
    ])
}

/// Strip the outer `{ … }` of a StyleSheet object literal, returning the inner body.
fn style_object_body(def: &str) -> &str {
    def.trim().trim_start_matches('{').trim_end_matches('}').trim().trim_end_matches(',').trim()
}

/// Build a deterministic, JS-ident-safe synthetic style key from style props.
fn synthetic_style_key(props: &[(String, String)]) -> String {
    let body: String = props
        .iter()
        .map(|(k, v)| format!("{k}_{v}"))
        .collect::<Vec<_>>()
        .join("_")
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    format!("d_{body}")
}

/// B3: if a node carries granular style props the semantic table doesn't cover
/// (`backgroundColor`, `padding`, `color`, …), merge them onto the node's semantic
/// base into a synthetic StyleSheet entry and return its key. Registers the merged
/// def in `dyn_styles` for [`emit_styles_block`]. Returns the original `style_key`
/// unchanged when there are no extra props.
fn apply_dyn_style(
    style_key: Option<String>,
    extra: &[(String, String)],
    dyn_styles: &mut BTreeMap<String, String>,
) -> Option<String> {
    if extra.is_empty() {
        return style_key;
    }
    let extra_body = rn_props_to_object_body(extra);
    let base_body = style_key
        .as_deref()
        .and_then(|k| semantic_style_table().get(k).map(|d| style_object_body(d).to_string()))
        .filter(|b| !b.is_empty());
    let merged = match base_body {
        Some(b) => format!("{{ {b}, {extra_body} }}"),
        None => format!("{{ {extra_body} }}"),
    };
    let key = synthetic_style_key(extra);
    dyn_styles.insert(key.clone(), merged);
    Some(key)
}

fn emit_styles_block(
    used: &std::collections::BTreeSet<String>,
    dyn_styles: &BTreeMap<String, String>,
) -> String {
    let table = semantic_style_table();
    let mut entries: Vec<String> = Vec::new();
    // Always include `btn_text` if any Pressable was emitted (used by wrap_pressable_text_children).
    let mut keys: std::collections::BTreeSet<String> = used.clone();
    if keys.iter().any(|k| k.starts_with("btn_")) {
        keys.insert("btn_text".to_string());
    }
    for k in keys {
        if let Some(def) = table.get(k.as_str()) {
            entries.push(format!("  {k}: {def}"));
        } else if let Some(def) = dyn_styles.get(&k) {
            entries.push(format!("  {k}: {def}"));
        }
    }
    if entries.is_empty() {
        return String::new();
    }
    format!(
        "const styles = StyleSheet.create({{\n{},\n}});\n",
        entries.join(",\n")
    )
}

/// Emit the `useState` declarations for the component's reactive state.
fn emit_state_declarations(members: &[HirReactiveMember]) -> String {
    let mut out = String::new();
    for m in members {
        if let HirReactiveMember::State(s) = m {
            let initial = emit_hir_expr_inline(&s.init);
            let setter = format!("set_{}", s.name);
            out.push_str(&format!(
                "  const [{name}, {setter}] = useState({initial});\n",
                name = s.name,
            ));
            // Suppress unused-setter warnings in tsc strict mode: the setter is used in handlers.
            let _ = setter;
        }
    }
    out
}

/// Detect which React hooks the component body references so we can build the import line.
fn detect_react_hooks(members: &[HirReactiveMember], view: Option<&HirExpr>) -> Vec<&'static str> {
    let mut hooks = std::collections::BTreeSet::new();
    if members
        .iter()
        .any(|m| matches!(m, HirReactiveMember::State(_)))
    {
        hooks.insert("useState");
    }
    if members.iter().any(|m| {
        matches!(
            m,
            HirReactiveMember::Effect(_)
                | HirReactiveMember::OnMount(_)
                | HirReactiveMember::OnCleanup(_)
        )
    }) {
        hooks.insert("useEffect");
    }
    // `Derived` members emit a plain `const x = expr` (recomputed each render,
    // see `emit_lifecycle_hooks`), so no `useMemo` import is needed.
    let _ = view; // currently no view-driven hooks
    hooks.into_iter().collect()
}

/// Emit `prelude` (let, hook calls, etc.) — currently passes through statements.
fn emit_prelude(members: &[HirReactiveMember]) -> String {
    let mut out = String::new();
    for m in members {
        if let HirReactiveMember::Stmt(HirStmt::Let { pattern, value, .. }) = m {
            let pat = match pattern {
                vox_compiler::hir::HirPattern::Ident(name, _) => name.clone(),
                _ => "_unsupported".to_string(),
            };
            let val = emit_hir_expr_inline(value);
            out.push_str(&format!("  const {pat} = {val};\n"));
        }
    }
    out
}

/// Emit lifecycle hooks — `effect`, `on mount`, `on cleanup`, and `derived`
/// members — as React hooks, mirroring the web reactive emit so both targets
/// load data the same way. Endpoint calls inside these bodies are awaited
/// (the ctx is async-aware) and wrapped in a fire-and-forget async IIFE when
/// needed, since a `useEffect` callback must stay synchronous.
///
/// Without this, the RN target dropped lifecycle members entirely — a component
/// whose `on mount:` loads data would render its initial state forever.
fn emit_lifecycle_hooks(
    members: &[HirReactiveMember],
    state_names: &HashSet<String>,
    endpoint_params: &HashMap<String, Vec<String>>,
) -> String {
    use crate::codegen_ts::hir_emit::{
        EmitCtx, emit_block_stmts, emit_hir_expr, wrap_effect_body_if_async,
    };

    // All `@endpoint` fns are async; their names drive `await` insertion.
    let endpoint_names: HashSet<String> = endpoint_params.keys().cloned().collect();
    let async_ctx =
        EmitCtx::with_async_and_endpoints(state_names, &endpoint_names, endpoint_params);
    let plain_ctx = EmitCtx::with_endpoints(state_names, endpoint_params);

    let mut out = String::new();
    for m in members {
        match m {
            HirReactiveMember::Effect(e) => {
                let stmts = emit_block_stmts(&e.body, &async_ctx, 2);
                let body = wrap_effect_body_if_async(&stmts, 2);
                out.push_str(&format!("  useEffect(() => {{\n{body}  }}, []);\n"));
            }
            HirReactiveMember::OnMount(o) => {
                let stmts = emit_block_stmts(&o.body, &async_ctx, 2);
                let body = wrap_effect_body_if_async(&stmts, 2);
                out.push_str(&format!("  useEffect(() => {{\n{body}  }}, []);\n"));
            }
            HirReactiveMember::OnCleanup(c) => {
                let stmts = emit_block_stmts(&c.body, &async_ctx, 2);
                let body = wrap_effect_body_if_async(&stmts, 2);
                out.push_str(&format!("  useEffect(() => () => {{\n{body}  }}, []);\n"));
            }
            HirReactiveMember::Derived(d) => {
                // Recompute each render — correct without dep tracking (the web
                // target memoizes; RN keeps it simple and always-fresh).
                let expr = emit_hir_expr(&d.expr, &plain_ctx);
                out.push_str(&format!("  const {} = {};\n", d.name, expr));
            }
            _ => {}
        }
    }
    out
}

/// Emit a single component file.
///
/// `known_components` is the set of all `component` names in the module and
/// `endpoint_params` maps each `@query`/`@mutation`/`@server` fn name to its
/// ordered parameter names. The keys drive cross-file `import` statements
/// (sibling components from `./Name`, endpoint fns from `./vox-client`); the
/// values drive the positional→named-object rewrite for endpoint calls inside
/// handlers. Both mirror the web reactive emit exactly, so the two targets
/// stay in lockstep on what a component pulls in and how it calls endpoints.
pub fn emit_rn_component(
    rc: &HirReactiveComponent,
    known_components: &HashSet<String>,
    form_names: &HashSet<String>,
    endpoint_params: &HashMap<String, Vec<String>>,
    screen_root_names: &HashSet<String>,
    es_imports: &[HirImport],
    diagnostics: &mut Vec<WebIrDiagnostic>,
) -> (String, String) {
    use crate::codegen_ts::reactive::collect_component_import_refs;

    let endpoint_names: HashSet<String> = endpoint_params.keys().cloned().collect();
    let mut out = String::new();
    let hooks = detect_react_hooks(&rc.members, rc.view.as_ref());

    // Imports
    if hooks.is_empty() {
        out.push_str("import React from \"react\";\n");
    } else {
        out.push_str(&format!(
            "import React, {{ {} }} from \"react\";\n",
            hooks.join(", ")
        ));
    }
    out.push_str("import { View, Text, Pressable, Image, TextInput, ScrollView, Modal, StyleSheet } from \"react-native\";\n");
    // `mobile` namespace import — only when this component's view or members
    // reference the `mobile` identifier (or `Speech.transcribe_microphone`,
    // which lowers to it). Mirrors the web target's auto-import in
    // `crates/vox-codegen/src/codegen_ts/component.rs`.
    if super::mobile_utils::component_uses_mobile(rc) {
        out.push_str("import { mobile } from \"./mobile-utils\";\n");
    }
    // expo-router `<Link>` for `link(href=...)` navigation.
    if component_uses_link(rc) {
        out.push_str("import { Link } from \"expo-router\";\n");
    }

    // Phase 5 S7: external React/TS component imports (`import react …`). Shared
    // with the web target via `emit_react_es_import_lines` so both agree. JS-only
    // RN component libraries (e.g. Tamagui) work; native-module libraries still
    // require a native rebuild (autolinking) — that diagnostic is gated on the
    // per-library SSOT (separate slice), not emitted here.
    let react_es = crate::codegen_ts::reactive::emit_react_es_import_lines(es_imports);
    out.push_str(&react_es);
    // Phase 5 SSOT: mandatory-provider guidance for known RN libs (Paper/Tamagui).
    out.push_str(&crate::codegen_ts::reactive::emit_external_lib_support(
        es_imports, true,
    ));

    // Cross-file imports: sibling components (`<NavBar />` → `./NavBar`) and
    // endpoint fns this component calls (`record_event(...)` → `./vox-client`),
    // collected anywhere in the view or member bodies. Shared with the web
    // reactive emit via `collect_component_import_refs` so both targets agree.
    let (comp_refs, endpoint_refs) =
        collect_component_import_refs(rc, known_components, &endpoint_names);
    for comp in &comp_refs {
        // `@form` components live in `forms.tsx`; sibling components in `./Name`.
        if form_names.contains(comp) {
            out.push_str(&format!("import {{ {comp} }} from \"./forms\";\n"));
        } else {
            out.push_str(&format!("import {{ {comp} }} from \"./{comp}\";\n"));
        }
    }
    if !endpoint_refs.is_empty() {
        out.push_str(&format!(
            "import {{ {} }} from \"./vox-client\";\n",
            endpoint_refs.join(", ")
        ));
    }
    out.push('\n');

    // Props interface
    if !rc.params.is_empty() {
        out.push_str(&format!("export interface {}Props {{\n", rc.name));
        for p in &rc.params {
            let ty = p
                .type_ann
                .as_ref()
                .map(hir_type_to_ts)
                .unwrap_or_else(|| "any".to_string());
            out.push_str(&format!("  {}: {};\n", p.name, ty));
        }
        out.push_str("}\n\n");
    }

    // Function header
    if rc.params.is_empty() {
        out.push_str(&format!(
            "export function {}(): React.ReactElement {{\n",
            rc.name
        ));
    } else {
        let params = rc
            .params
            .iter()
            .map(|p| p.name.clone())
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!(
            "export function {}({{ {params} }}: {}Props): React.ReactElement {{\n",
            rc.name, rc.name
        ));
    }

    // Collect state names so the shared HIR → TS lowering rewrites `n = expr`
    // (mutation) to `set_n(expr)` (React setter) inside handler and lifecycle
    // bodies. Without this the emitted code would mutate the variable directly,
    // which RN ignores between renders.
    let state_names: HashSet<String> = rc
        .members
        .iter()
        .filter_map(|m| match m {
            HirReactiveMember::State(s) => Some(s.name.clone()),
            _ => None,
        })
        .collect();

    // Body: state declarations, then lifecycle hooks (effects / on-mount /
    // on-cleanup / derived), then prelude. Lifecycle hooks are what load data
    // — e.g. `on mount: { total = health_event_count() }` becomes a
    // `useEffect` that awaits the async endpoint and calls `set_total`.
    out.push_str(&emit_state_declarations(&rc.members));
    out.push_str(&emit_lifecycle_hooks(
        &rc.members,
        &state_names,
        endpoint_params,
    ));
    out.push_str(&emit_prelude(&rc.members));

    // View
    let view_root = match &rc.view {
        Some(v) => v,
        None => {
            diagnostics.push(WebIrDiagnostic {
                code: "vox/codegen/rn-missing-view".to_string(),
                message: format!(
                    "Component `{}` has no `view:` declaration; emitting a null view.",
                    rc.name
                ),
                span: None,
                category: Some("codegen".to_string()),
            });
            out.push_str("  return <View />;\n}\n");
            return (format!("{}.tsx", rc.name), out);
        }
    };

    let mut dyn_styles: BTreeMap<String, String> = BTreeMap::new();
    let rn_root = hir_view_child_to_rn(
        view_root,
        &state_names,
        endpoint_params,
        diagnostics,
        &mut dyn_styles,
    );
    let mut used_styles = std::collections::BTreeSet::new();
    collect_used_styles(&rn_root, &mut used_styles);

    // Screen-root components get default horizontal edge padding so content
    // doesn't kiss the device edges. Applied as an outer wrapper View on the
    // screen root only (never on nested components like NavBar), and skipped
    // when the root view opts out with `bleed`.
    let pad_screen = screen_root_names.contains(&rc.name) && !root_view_bleeds(view_root);
    out.push_str("  return (\n");
    if pad_screen {
        used_styles.insert("screen".to_string());
        out.push_str("    <View style={styles.screen}>\n");
        out.push_str(&emit_rn_node(&rn_root, 3));
        out.push_str("    </View>\n");
    } else {
        out.push_str(&emit_rn_node(&rn_root, 2));
    }
    out.push_str("  );\n}\n\n");

    // StyleSheet block
    let styles_block = emit_styles_block(&used_styles, &dyn_styles);
    if !styles_block.is_empty() {
        out.push_str(&styles_block);
    }

    (format!("{}.tsx", rc.name), out)
}

fn hir_type_to_ts(ty: &vox_compiler::hir::HirType) -> String {
    use vox_compiler::hir::HirType;
    match ty {
        HirType::Named(n) => match n.as_str() {
            "int" => "number".to_string(),
            "float" => "number".to_string(),
            "str" | "string" => "string".to_string(),
            "bool" => "boolean".to_string(),
            "Unit" => "void".to_string(),
            other => other.to_string(),
        },
        HirType::Generic(name, args) => {
            let args_str: Vec<String> = args.iter().map(hir_type_to_ts).collect();
            match name.as_str() {
                "list" | "List" => format!("{}[]", args_str.join(", ")),
                "Option" => format!("{} | undefined", args_str.join(", ")),
                _ => format!("{}<{}>", name, args_str.join(", ")),
            }
        }
        HirType::Function(params, ret) => {
            let p: Vec<String> = params
                .iter()
                .enumerate()
                .map(|(i, t)| format!("arg{i}: {}", hir_type_to_ts(t)))
                .collect();
            format!("({}) => {}", p.join(", "), hir_type_to_ts(ret))
        }
        HirType::Tuple(elems) => {
            let es: Vec<String> = elems.iter().map(hir_type_to_ts).collect();
            format!("[{}]", es.join(", "))
        }
        HirType::Unit => "void".to_string(),
        HirType::Decimal => "string".to_string(),
    }
}

#[cfg(test)]
mod b3_style_tests {
    use super::*;

    #[test]
    fn colors_resolve_to_hex() {
        assert_eq!(
            tailwind_token_to_rn_prop("text-zinc-400"),
            Some(("color".into(), "\"#a1a1aa\"".into()))
        );
        assert_eq!(
            tailwind_token_to_rn_prop("bg-blue-600"),
            Some(("backgroundColor".into(), "\"#2563eb\"".into()))
        );
        assert_eq!(
            tailwind_token_to_rn_prop("bg-white"),
            Some(("backgroundColor".into(), "\"#fff\"".into()))
        );
    }

    #[test]
    fn spacing_uses_4px_scale() {
        assert_eq!(
            tailwind_token_to_rn_prop("p-4"),
            Some(("padding".into(), "16".into()))
        );
        assert_eq!(
            tailwind_token_to_rn_prop("px-8"),
            Some(("paddingHorizontal".into(), "32".into()))
        );
        assert_eq!(
            tailwind_token_to_rn_prop("gap-2"),
            Some(("gap".into(), "8".into()))
        );
    }

    #[test]
    fn radius_weight_size_opacity() {
        assert_eq!(
            tailwind_token_to_rn_prop("rounded-lg"),
            Some(("borderRadius".into(), "8".into()))
        );
        assert_eq!(
            tailwind_token_to_rn_prop("font-bold"),
            Some(("fontWeight".into(), "\"700\"".into()))
        );
        assert_eq!(
            tailwind_token_to_rn_prop("text-sm"),
            Some(("fontSize".into(), "14".into()))
        );
        assert_eq!(
            tailwind_token_to_rn_prop("opacity-50"),
            Some(("opacity".into(), "0.50".into()))
        );
    }

    #[test]
    fn unknown_tokens_are_ignored() {
        assert_eq!(tailwind_token_to_rn_prop("flex"), None);
        assert_eq!(tailwind_token_to_rn_prop("not-a-class"), None);
    }

    #[test]
    fn multi_token_collects_all() {
        let props = tailwind_tokens_to_rn_props(&["bg-white", "p-4", "text-zinc-900"]);
        assert!(props.contains(&("backgroundColor".into(), "\"#fff\"".into())));
        assert!(props.contains(&("padding".into(), "16".into())));
        assert!(props.contains(&("color".into(), "\"#18181b\"".into())));
    }
}

#[cfg(test)]
mod b3_wiring_tests {
    use super::*;
    use vox_compiler::lexer::lex;
    use vox_compiler::parser::parse;
    use vox_compiler::hir::lower_module;

    /// End-to-end: a mobile component with custom color/spacing kwargs must emit a
    /// StyleSheet entry carrying those props (not silently drop them).
    #[test]
    fn custom_kwargs_reach_rn_stylesheet() {
        let src = r#"
component Card() {
    state n: int = 0
    view: panel(bg="blue.600", pad="8") {
        text(color="zinc.50") { "hi" }
    }
}
"#;
        let module = parse(lex(src)).expect("parse");
        let hir = lower_module(&module);
        let rc = hir.components.iter().find(|c| c.name == "Card").expect("Card");
        let (_name, tsx) = emit_rn_component(
            rc,
            &HashSet::new(),
            &HashSet::new(),
            &HashMap::new(),
            &HashSet::new(),
            &[],
            &mut vec![],
        );
        assert!(
            tsx.contains("backgroundColor: \"#2563eb\""),
            "bg=blue.600 must reach the StyleSheet; got:\n{tsx}"
        );
        assert!(
            tsx.contains("padding: 32"),
            "pad=8 must reach the StyleSheet (8*4); got:\n{tsx}"
        );
        assert!(
            tsx.contains("color: \"#fafafa\""),
            "color=zinc.50 must reach the StyleSheet; got:\n{tsx}"
        );
    }
}
