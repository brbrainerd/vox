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

use std::collections::{BTreeMap, HashSet};
use vox_compiler::hir::{
    HirExpr, HirJsxAttr, HirJsxElement, HirReactiveComponent, HirReactiveMember, HirStmt,
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
    } else if joined.contains(&"rounded-lg")
        && joined.contains(&"border")
        && joined.contains(&"p-4")
    {
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
fn emit_hir_expr_inline_with_state(expr: &HirExpr, state_names: &HashSet<String>) -> String {
    let ctx = crate::codegen_ts::hir_emit::EmitCtx::new(state_names);
    crate::codegen_ts::hir_emit::emit_hir_expr(expr, &ctx)
}

fn emit_hir_expr_inline(expr: &HirExpr) -> String {
    let empty_states: HashSet<String> = HashSet::new();
    emit_hir_expr_inline_with_state(expr, &empty_states)
}

/// Emit an event handler — wraps the lowered body in an arrow function so the
/// JSX `onPress={...}` attribute receives a callable, not an immediate
/// invocation. State assignments (`n = n + 1`) inside the body are rewritten
/// to setter calls (`set_n(n + 1)`) by the shared HIR emit when the variable
/// appears in `state_names`.
fn emit_event_handler_with_state(expr: &HirExpr, state_names: &HashSet<String>) -> String {
    let body = emit_hir_expr_inline_with_state(expr, state_names);
    // The shared HIR → TS emit wraps block expressions in an IIFE because they
    // appear in expression position elsewhere (where a value is expected).
    // For an event handler we want the lambda itself, not the invocation —
    // strip the outer `(...)()` if present.
    let arrow_body = strip_iife_wrapper(&body);
    let trimmed = arrow_body.trim_start();
    if trimmed.starts_with("() =>") || trimmed.starts_with("async () =>") {
        // Already a clean arrow function.
        arrow_body.to_string()
    } else if trimmed.starts_with("{") {
        format!("() => {arrow_body}")
    } else {
        format!("() => ({arrow_body})")
    }
}

/// Detect and unwrap `(EXPR)()` where `EXPR` is an arrow lambda. Returns the inner
/// arrow expression so callers can use it as an event handler. Leaves any other
/// shape untouched.
fn strip_iife_wrapper(body: &str) -> &str {
    let trimmed = body.trim();
    if !trimmed.ends_with(")()") {
        return body;
    }
    // Confirm the leading `(` matches the second-to-last `)` (i.e. balanced).
    if !trimmed.starts_with('(') {
        return body;
    }
    let inner = &trimmed[1..trimmed.len() - 3]; // strip leading `(` and trailing `)()`
    let inner_trim = inner.trim_start();
    if inner_trim.starts_with("() =>") || inner_trim.starts_with("async () =>") {
        inner
    } else {
        body
    }
}

fn attr_value<'a>(attrs: &'a [HirJsxAttr], name: &str) -> Option<&'a HirExpr> {
    attrs.iter().find(|a| a.name == name).map(|a| &a.value)
}

/// Pull a literal integer out of a HirExpr (used for `heading(level=1)`).
fn extract_int_literal(expr: &HirExpr) -> Option<i64> {
    if let HirExpr::IntLit(n, _) = expr {
        Some(*n)
    } else {
        None
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
    diagnostics: &mut Vec<WebIrDiagnostic>,
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
    let on_press = handler_attr.map(|h| emit_event_handler_with_state(h, state_names));

    let children: Vec<RnNode> = el
        .children
        .iter()
        .map(|c| hir_view_child_to_rn(c, state_names, diagnostics))
        .collect();

    match el.tag.as_str() {
        // ── VUV abstract container primitives ────────────────────────────────
        "column" | "stack" | "div" => RnNode::View {
            style_key: style_key.or(Some("col".to_string())),
            children,
        },
        "row" => RnNode::View {
            style_key: style_key.or(Some("row".to_string())),
            children,
        },
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
                .map(|h| emit_event_handler_with_state(h, state_names)),
            placeholder_ts: attr_value(&el.attributes, "placeholder").map(emit_hir_expr_inline),
        },

        other => {
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
    }
}

fn hir_view_child_to_rn(
    child: &HirExpr,
    state_names: &HashSet<String>,
    diagnostics: &mut Vec<WebIrDiagnostic>,
) -> RnNode {
    match child {
        HirExpr::Jsx(el) => jsx_to_rn(el, state_names, diagnostics),
        HirExpr::JsxSelfClosing(sc) => jsx_to_rn(
            &HirJsxElement {
                tag: sc.tag.clone(),
                attributes: sc.attributes.clone(),
                children: vec![],
                span: sc.span,
            },
            state_names,
            diagnostics,
        ),
        HirExpr::JsxFragment(children, _) => RnNode::View {
            style_key: None,
            children: children
                .iter()
                .map(|c| hir_view_child_to_rn(c, state_names, diagnostics))
                .collect(),
        },
        HirExpr::StringLit(s, _) => RnNode::StringLit(s.clone()),
        other => RnNode::Expr(emit_hir_expr_inline_with_state(other, state_names)),
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
            for c in children {
                collect_used_styles(c, out);
            }
        }
        RnNode::Image { style_key, .. } | RnNode::TextInput { style_key, .. } => {
            if let Some(k) = style_key {
                out.insert(k.clone());
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
        RnNode::StringLit(s) => RnNode::StringLit(s.clone()),
        RnNode::Expr(e) => RnNode::Expr(e.clone()),
    }
}

/// StyleSheet entries indexed by the keys [`class_string_to_style_key`] returns.
fn emit_styles_block(used: &std::collections::BTreeSet<String>) -> String {
    let table: BTreeMap<&str, &str> = BTreeMap::from([
        ("col", "{ flexDirection: \"column\", gap: 12 }"),
        (
            "row",
            "{ flexDirection: \"row\", gap: 12, alignItems: \"center\" }",
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
    if members
        .iter()
        .any(|m| matches!(m, HirReactiveMember::State(_)))
    {
        hooks.insert("useState");
    }
    if members
        .iter()
        .any(|m| matches!(m, HirReactiveMember::Effect(_)))
        || members
            .iter()
            .any(|m| matches!(m, HirReactiveMember::OnMount(_)))
    {
        hooks.insert("useEffect");
    }
    if members
        .iter()
        .any(|m| matches!(m, HirReactiveMember::Derived(_)))
    {
        hooks.insert("useMemo");
    }
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

/// Emit a single component file.
pub fn emit_rn_component(
    rc: &HirReactiveComponent,
    diagnostics: &mut Vec<WebIrDiagnostic>,
) -> (String, String) {
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
    out.push_str(
        "import { View, Text, Pressable, Image, TextInput, StyleSheet } from \"react-native\";\n",
    );
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

    // Body: state + prelude
    out.push_str(&emit_state_declarations(&rc.members));
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

    // Collect state names so the shared HIR → TS lowering rewrites `n = expr`
    // (mutation) to `set_n(expr)` (React setter) inside handler bodies. Without
    // this the emitted handler would mutate the variable directly, which RN
    // ignores between renders.
    let state_names: HashSet<String> = rc
        .members
        .iter()
        .filter_map(|m| match m {
            HirReactiveMember::State(s) => Some(s.name.clone()),
            _ => None,
        })
        .collect();

    let rn_root = hir_view_child_to_rn(view_root, &state_names, diagnostics);
    let mut used_styles = std::collections::BTreeSet::new();
    collect_used_styles(&rn_root, &mut used_styles);

    out.push_str("  return (\n");
    let rendered = emit_rn_node(&rn_root, 2);
    out.push_str(&rendered);
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
