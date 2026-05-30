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
use vox_compiler::hir::{HirExpr, HirJsxAttr, HirJsxElement, HirReactiveComponent, HirReactiveMember, HirStmt};

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
    let joined: Vec<&str> = class_tokens.iter().copied().collect();
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
    } else if joined.contains(&"rounded-lg") && joined.contains(&"border") && joined.contains(&"p-4") {
        Some("panel")
    } else {
        None
    }
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
    let body = emit_hir_expr_inline_with_state(expr, state_names, endpoint_params);
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
    if trimmed.starts_with('(')
        && trimmed.ends_with(')')
        && {
            let inner = &trimmed[1..trimmed.len() - 1];
            let inner_trim = inner.trim_start();
            inner_trim.starts_with("() =>") || inner_trim.starts_with("async () =>")
        }
    {
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
) -> RnNode {
    let class_tokens_owned = attr_value(&el.attributes, "className")
        .map(extract_class_tokens)
        .unwrap_or_default();
    let class_refs: Vec<&str> = class_tokens_owned.iter().map(|s| s.as_str()).collect();
    let style_key = class_string_to_style_key(&class_refs).map(String::from);

    // Recognize both snake_case (`on_click`) and camelCase (`onClick`) attribute names —
    // depending on which lowering produced the HIR, either may appear.
    let handler_attr = attr_value(&el.attributes, "on_click")
        .or_else(|| attr_value(&el.attributes, "onClick"));
    let on_press = handler_attr.map(|h| emit_event_handler_with_state(h, state_names, endpoint_params));

    let children: Vec<RnNode> = el
        .children
        .iter()
        .map(|c| hir_view_child_to_rn(c, state_names, endpoint_params, diagnostics))
        .collect();

    match el.tag.as_str() {
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
            RnNode::View { style_key, children }
        }
    }
}

fn hir_view_child_to_rn(
    child: &HirExpr,
    state_names: &HashSet<String>,
    endpoint_params: &HashMap<String, Vec<String>>,
    diagnostics: &mut Vec<WebIrDiagnostic>,
) -> RnNode {
    match child {
        HirExpr::Jsx(el) => jsx_to_rn(el, state_names, endpoint_params, diagnostics),
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
        ),
        HirExpr::JsxFragment(children, _) => RnNode::View {
            style_key: None,
            children: children
                .iter()
                .map(|c| hir_view_child_to_rn(c, state_names, endpoint_params, diagnostics))
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
                        )),
                        _ => None,
                    })
                    .collect(),
                other => vec![hir_view_child_to_rn(
                    other,
                    state_names,
                    endpoint_params,
                    diagnostics,
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
        RnNode::View { style_key, children } => {
            if let Some(k) = style_key {
                out.insert(k.clone());
            }
            for c in children {
                collect_used_styles(c, out);
            }
        }
        RnNode::Text { style_key, children } => {
            if let Some(k) = style_key {
                out.insert(k.clone());
            }
            for c in children {
                collect_used_styles(c, out);
            }
        }
        RnNode::Pressable { style_key, children, .. } => {
            if let Some(k) = style_key {
                out.insert(k.clone());
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
        RnNode::Link { style_key, children, .. } => {
            if let Some(k) = style_key {
                out.insert(k.clone());
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
        RnNode::View { style_key, children } => {
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
        RnNode::Text { style_key, children } => {
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
            let wrapped = wrap_pressable_text_children(children.iter().map(|c| match c {
                RnNode::StringLit(s) => RnNode::StringLit(s.clone()),
                RnNode::Expr(e) => RnNode::Expr(e.clone()),
                other => clone_rn_node(other),
            }).collect());
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
            let inner_with_key = if let Some(k) = key_ts {
                inject_key_into_first_element(inner, k)
            } else {
                inner
            };
            format!(
                "{pad}{{{iterator_ts}.map({params} => (\n{inner_with_key}{pad}  ))}}\n"
            )
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
        RnNode::View { style_key, children } => RnNode::View {
            style_key: style_key.clone(),
            children: children.iter().map(clone_rn_node).collect(),
        },
        RnNode::Text { style_key, children } => RnNode::Text {
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
    let insert_at = if j > 0 && bytes[j - 1] == b'/' { j - 1 } else { j };
    let key_attr = format!(" key={{{key_ts}}}");
    let mut out = String::with_capacity(inner.len() + key_attr.len());
    out.push_str(&inner[..insert_at]);
    out.push_str(&key_attr);
    out.push_str(&inner[insert_at..]);
    out
}

/// StyleSheet entries indexed by the keys [`class_string_to_style_key`] returns.
fn emit_styles_block(used: &std::collections::BTreeSet<String>) -> String {
    let table: BTreeMap<&str, &str> = BTreeMap::from([
        ("col", "{ flexDirection: \"column\", gap: 12 }"),
        // Screen-root wrapper: default horizontal edge padding (opt out with `bleed`).
        ("screen", "{ flex: 1, paddingHorizontal: 16 }"),
        // `row` wraps by default so children can never run off the right edge
        // (RN children default to flexShrink:0). `columnGap`/`rowGap` keep
        // spacing correct once wrapped. A `row(scroll: "horizontal")` opts into
        // a single non-wrapping scrollable line instead (see `row_scroll_content`).
        ("row", "{ flexDirection: \"row\", flexWrap: \"wrap\", columnGap: 12, rowGap: 12, alignItems: \"center\" }"),
        ("row_scroll_content", "{ flexDirection: \"row\", columnGap: 12, alignItems: \"center\" }"),
        ("h1", "{ fontSize: 30, fontWeight: \"600\" }"),
        ("h2", "{ fontSize: 24, fontWeight: \"600\" }"),
        ("h3", "{ fontSize: 20, fontWeight: \"600\" }"),
        ("body", "{ fontSize: 14 }"),
        ("btn_primary", "{ backgroundColor: \"#0a7ea4\", paddingVertical: 10, paddingHorizontal: 16, borderRadius: 6, alignItems: \"center\" }"),
        ("btn_secondary", "{ backgroundColor: \"#e5e7eb\", paddingVertical: 10, paddingHorizontal: 16, borderRadius: 6, alignItems: \"center\" }"),
        ("btn_text", "{ color: \"white\", fontWeight: \"500\" }"),
        ("panel", "{ padding: 16, backgroundColor: \"#f5f5f5\", borderRadius: 8, borderWidth: 1, borderColor: \"#e5e7eb\" }"),
    ]);
    let mut entries: Vec<String> = Vec::new();
    // Always include `btn_text` if any Pressable was emitted (used by wrap_pressable_text_children).
    let mut keys: std::collections::BTreeSet<String> = used.clone();
    if keys.iter().any(|k| k.starts_with("btn_")) {
        keys.insert("btn_text".to_string());
    }
    for k in keys {
        if let Some(def) = table.get(k.as_str()) {
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
    if members.iter().any(|m| matches!(m, HirReactiveMember::State(_))) {
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
        if let HirReactiveMember::Stmt(stmt) = m {
            if let HirStmt::Let { pattern, value, .. } = stmt {
                let pat = match pattern {
                    vox_compiler::hir::HirPattern::Ident(name, _) => name.clone(),
                    _ => "_unsupported".to_string(),
                };
                let val = emit_hir_expr_inline(value);
                out.push_str(&format!("  const {pat} = {val};\n"));
            }
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
    endpoint_params: &HashMap<String, Vec<String>>,
    screen_root_names: &HashSet<String>,
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
    out.push_str("import { View, Text, Pressable, Image, TextInput, ScrollView, StyleSheet } from \"react-native\";\n");
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

    // Cross-file imports: sibling components (`<NavBar />` → `./NavBar`) and
    // endpoint fns this component calls (`record_event(...)` → `./vox-client`),
    // collected anywhere in the view or member bodies. Shared with the web
    // reactive emit via `collect_component_import_refs` so both targets agree.
    let (comp_refs, endpoint_refs) =
        collect_component_import_refs(rc, known_components, &endpoint_names);
    for comp in &comp_refs {
        out.push_str(&format!("import {{ {comp} }} from \"./{comp}\";\n"));
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
    out.push_str(&emit_lifecycle_hooks(&rc.members, &state_names, endpoint_params));
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

    let rn_root = hir_view_child_to_rn(view_root, &state_names, endpoint_params, diagnostics);
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
    let styles_block = emit_styles_block(&used_styles);
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
        _ => "any".to_string(),
    }
}
