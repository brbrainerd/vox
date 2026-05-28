//! AST-backed JSX → TypeScript emission for classic `@component fn` and similar paths.
//!
//! **Legacy / compat (OP-0145, ADR 012):** structural view parity is [`crate::web_ir`]. This module
//! remains for AST-shaped trees; attribute names use [`crate::codegen_ts::hir_emit::compat`] so HIR,
//! Web IR, and AST paths share one React mapping matrix (`super::hir_emit::map_jsx_attr_name`).
//!
//! **Disposition (OP-0158):** this file remains the AST codegen surface for `@component fn` and
//! shared stmt/expr helpers; do not grow new JSX semantics here—extend Web IR instead.
//!
//! **Compatibility tags (OP-S031):** AST path must stay aligned with `super::hir_emit` (OP-S029) via
//! `map_jsx_attr_name`; new attrs or event spellings belong in `crate::codegen_ts::hir_emit::compat`
//! first, then Web IR lowering / validate.
//!
//! **Wrapper inventory B/C (OP-S077 / S167):** `emit_jsx_element` / `emit_jsx_self_closing` are the only
//! supported AST JSX emit entry points; extend [`crate::web_ir`] for new view semantics.

use crate::codegen_ts::hir_emit::{ts_string_literal, wrap_jsx_hir_child_expr};
use vox_compiler::ast::expr::{BinOp, Expr, JsxAttribute, JsxElement, JsxSelfClosingElement, UnOp};
use vox_compiler::ast::stmt::Stmt;

pub use crate::codegen_ts::hir_emit::compat::{map_jsx_attr_name, map_jsx_tag};

/// Emit a JSX element with children to TypeScript.
///
/// **Phase:** compat-legacy (OP-0150); prefer Web IR preview for structural parity work.
pub fn emit_jsx_element(el: &JsxElement, indent: usize) -> String {
    let pad = "  ".repeat(indent);
    let mut out = String::new();

    let view = transform_view_kwargs(&el.tag, &el.attributes);
    out.push_str(&format!("{pad}<{}", view.html_tag));
    out.push_str(&render_view_attrs(&view, &view.passthrough));
    out.push_str(">\n");

    for child in &el.children {
        out.push_str(&emit_jsx_child(child, indent + 1));
    }

    out.push_str(&format!("{pad}</{}>\n", view.html_tag));
    out
}

/// Emit a self-closing JSX element.
///
/// **Phase:** compat-legacy (OP-0150).
pub fn emit_jsx_self_closing(el: &JsxSelfClosingElement, indent: usize) -> String {
    let pad = "  ".repeat(indent);
    let view = transform_view_kwargs(&el.tag, &el.attributes);
    let mut out = format!("{pad}<{}", view.html_tag);
    out.push_str(&render_view_attrs(&view, &view.passthrough));
    out.push_str(" />\n");
    out
}

// ── VUV view-call lowering at AST emit time ─────────────────────────────────
//
// Mirrors `web_ir/lower.rs::apply_primitive_emission` so that AST-path codegen produces the same
// className output as the web_ir validator sees. Without this, `vox build` emitted raw kwargs
// (`<row pad_x={4}>`) instead of Tailwind classes — invalid HTML / non-rendering React.

/// Result of applying primitive resolution + universal-kwarg lowering to a view call.
struct ViewCallEmission {
    /// HTML tag to emit (e.g. `"div"` for `row`/`column`/`panel`, or the original tag if no
    /// primitive matched).
    html_tag: String,
    /// className expression as a TS expression string. May be a static string literal or a
    /// concatenation of literal + dynamic ternaries.
    class_expr: Option<String>,
    /// Attributes that did NOT participate in primitive lowering — emitted as-is.
    passthrough: Vec<JsxAttribute>,
}

const PRIMITIVE_CONSUMED_PROPS: &[&str] = &[
    "size", "weight", "align", "wrap", "variant", "level", "surface", "z",
];

/// Walk a JSX-shaped attribute list and split it into:
///   - className contributions (literal + dynamic) merged into a single `class_expr`,
///   - passthrough attributes preserved verbatim.
///
/// If the tag is a known UI primitive, prepend its base classes. Any author-supplied
/// `class`/`className` attribute is concatenated last so it overrides defaults.
fn transform_view_kwargs(tag: &str, attrs: &[JsxAttribute]) -> ViewCallEmission {
    // Mirror the HIR-emit path: pass static-literal per-primitive kwargs (size/weight/align/wrap/
    // variant/level/surface) into primitives::resolve so per-primitive logic runs.
    // `text(size="xs")` → `text-xs`; `heading(level=1)` → `<h1>`. Without this, those classes
    // were silently dropped from the AST emit path.
    let mut static_per_primitive: Vec<(String, String)> = Vec::new();
    for attr in attrs {
        if !PRIMITIVE_CONSUMED_PROPS.contains(&attr.name.as_str()) {
            continue;
        }
        let v = match unwrap_block(&attr.value) {
            Expr::StringLit { value, .. } => Some(value.clone()),
            Expr::BoolLit { value, .. } => Some(value.to_string()),
            Expr::IntLit { value, .. } => Some(value.to_string()),
            _ => None,
        };
        if let Some(v) = v {
            static_per_primitive.push((attr.name.clone(), v));
        }
    }
    let primitive_emission = crate::web_ir::primitives::resolve(tag, &static_per_primitive);
    let html_tag = primitive_emission
        .as_ref()
        .map(|e| e.html_tag.to_string())
        // Non-primitive fallback: route through map_jsx_tag so snake_case SVG forms
        // (radial_gradient → radialGradient, fe_gaussian_blur → feGaussianBlur, etc.)
        // emit canonical camelCase. Plain HTML/SVG tags pass through unchanged.
        .unwrap_or_else(|| map_jsx_tag(tag).to_string());
    // Author kwarg names — used to suppress primitive base classes on the same Tailwind axis.
    let author_kwargs: Vec<&str> = attrs.iter().map(|a| a.name.as_str()).collect();
    let mut class_pieces: Vec<String> = primitive_emission
        .as_ref()
        .map(|e| {
            e.base_classes
                .iter()
                .filter(|c| {
                    !crate::web_ir::primitives::primitive_base_class_overridden(c, &author_kwargs)
                })
                .map(|c| format!("\"{c}\""))
                .collect()
        })
        .unwrap_or_default();
    let mut passthrough: Vec<JsxAttribute> = Vec::with_capacity(attrs.len());

    for attr in attrs {
        let name = attr.name.as_str();
        if name == "class" || name == "className" {
            // Author-supplied className takes lowest priority — concatenated after primitive base
            // and typed kwargs so overrides land last in Tailwind's class string.
            class_pieces.push(emit_jsx_attr_value(&attr.value));
            continue;
        }
        if PRIMITIVE_CONSUMED_PROPS.contains(&name) {
            // Per-primitive kwarg consumed by web_ir::primitives::resolve at validation time —
            // the AST emit here doesn't recompute the primitive class for these. Drop the attr
            // (don't passthrough); the primitive-base classes already cover the common case.
            continue;
        }
        if let Some(piece) = kwarg_to_class_expr(name, &attr.value) {
            class_pieces.push(piece);
            continue;
        }
        // Universal style kwargs that resolve to no class (e.g. `border=false`,
        // `italic=false`) are intentionally consumed by the lowering — passing them
        // through would emit invalid JSX (`<div border={false}>`).
        if crate::web_ir::primitives::UNIVERSAL_STYLE_KWARGS.contains(&name) {
            continue;
        }
        passthrough.push(attr.clone());
    }

    let class_expr = if class_pieces.is_empty() {
        None
    } else if class_pieces.len() == 1 {
        Some(class_pieces.into_iter().next().unwrap())
    } else {
        Some(format!(
            "[{}].filter(Boolean).join(\" \")",
            class_pieces.join(", ")
        ))
    };

    ViewCallEmission {
        html_tag,
        class_expr,
        passthrough,
    }
}

/// Render the attribute portion of a JSX opening tag: a leading className (if any) followed by
/// the passthrough attributes. Returns a string starting with a space, suitable for splicing into
/// `<tag …` or `<tag … />`.
fn render_view_attrs(view: &ViewCallEmission, passthrough: &[JsxAttribute]) -> String {
    let mut out = String::new();
    if let Some(ref ce) = view.class_expr {
        out.push_str(&format!(" className={{{ce}}}"));
    }
    for attr in passthrough {
        if attr.name == "bind" {
            let (value_str, onchange_str) = expand_bind_attribute(&attr.value);
            out.push_str(&format!(
                " value={{{value_str}}} onChange={{{onchange_str}}}"
            ));
        } else {
            let react_name = map_jsx_attr_name(&attr.name);
            let value = emit_jsx_attr_value(&attr.value);
            out.push_str(&format!(" {react_name}={{{value}}}"));
        }
    }
    out
}

/// Try to convert a (kwarg, value-expression) pair into a TS expression string evaluating to a
/// className fragment. Static-literal values resolve directly; `if/else` expressions recurse and
/// emit ternaries; other dynamic shapes return `None` (caller will pass them through as raw).
fn kwarg_to_class_expr(kwarg: &str, expr: &Expr) -> Option<String> {
    match unwrap_block(expr) {
        Expr::StringLit { value, .. } => {
            let classes = crate::web_ir::primitives::resolve_universal_kwarg(kwarg, value)?;
            if classes.is_empty() {
                return None;
            }
            Some(format!("\"{}\"", classes.join(" ")))
        }
        Expr::BoolLit { value, .. } => {
            let v = value.to_string();
            let classes = crate::web_ir::primitives::resolve_universal_kwarg(kwarg, &v)?;
            if classes.is_empty() {
                return None;
            }
            Some(format!("\"{}\"", classes.join(" ")))
        }
        Expr::IntLit { value, .. } => {
            let v = value.to_string();
            let classes = crate::web_ir::primitives::resolve_universal_kwarg(kwarg, &v)?;
            if classes.is_empty() {
                return None;
            }
            Some(format!("\"{}\"", classes.join(" ")))
        }
        Expr::FloatLit { value, .. } => {
            let v = value.to_string();
            let classes = crate::web_ir::primitives::resolve_universal_kwarg(kwarg, &v)?;
            if classes.is_empty() {
                return None;
            }
            Some(format!("\"{}\"", classes.join(" ")))
        }
        Expr::DecimalLit { value, .. } => {
            let classes = crate::web_ir::primitives::resolve_universal_kwarg(kwarg, value)?;
            if classes.is_empty() {
                return None;
            }
            Some(format!("\"{}\"", classes.join(" ")))
        }
        Expr::If {
            condition,
            then_body,
            else_body,
            ..
        } => {
            // `bg=if cond { "x" } else { "y" }` → `(cond ? "bg-x" : "bg-y")`. Pull the trailing
            // expression of each branch (Stmt::Expr) and recurse; bail to None if either branch
            // doesn't end in a single expression we can resolve.
            let then_expr = single_trailing_expr(then_body)?;
            let else_body = else_body.as_ref()?;
            let else_expr = single_trailing_expr(else_body)?;
            let then_class = kwarg_to_class_expr(kwarg, then_expr)?;
            let else_class = kwarg_to_class_expr(kwarg, else_expr)?;
            let cond_str = emit_expr(condition);
            Some(format!("({cond_str} ? {then_class} : {else_class})"))
        }
        // Recognized kwarg with an unrecognized expression shape — caller falls back to passing
        // the attribute through as-is so the user sees the raw kwarg in the output and can fix it.
        _ if crate::web_ir::primitives::UNIVERSAL_STYLE_KWARGS.contains(&kwarg) => None,
        _ => None,
    }
}

/// Return the single trailing expression of a statement body, if the body is exactly one
/// `Stmt::Expr`. Used for `if`/`else` branch resolution at view-call sites.
fn single_trailing_expr(body: &[Stmt]) -> Option<&Expr> {
    if body.len() != 1 {
        return None;
    }
    if let Stmt::Expr { expr, .. } = &body[0] {
        Some(expr)
    } else {
        None
    }
}

/// Emit a JSX attribute value expression.
fn emit_jsx_attr_value(expr: &Expr) -> String {
    match unwrap_block(expr) {
        Expr::StringLit { value, .. } => {
            // Check if string contains interpolation like {msg.role}
            if value.contains('{') && value.contains('}') {
                // Convert to template literal: "message {msg.role}" -> `message ${msg.role}`
                let template = value.replace('{', "${").to_string();
                format!("`{template}`")
            } else {
                ts_string_literal(value)
            }
        }
        Expr::Ident { name, .. } => name.clone(),
        Expr::Lambda { params, body, .. } => {
            let param_names: Vec<String> = params.iter().map(|p| p.name.clone()).collect();
            let body_str = emit_expr(body);
            format!("({}) => {body_str}", param_names.join(", "))
        }
        other => emit_expr(other),
    }
}

/// Expand a `bind={expr}` attribute into (value_expr, onChange_handler).
///
/// - `bind={email}` → `value={email}` + `onChange={(e) => setEmail(e.target.value)}`
/// - `bind={form.email}` → `value={form.email}` + `onChange={(e) => setForm({...form, email: e.target.value})}`
fn expand_bind_attribute(expr: &Expr) -> (String, String) {
    match unwrap_block(expr) {
        Expr::Ident { name, .. } => {
            let setter = format!("set_{name}");
            (name.clone(), format!("(e) => {setter}(e.target.value)"))
        }
        Expr::FieldAccess { object, field, .. } => {
            let obj_str = emit_expr(object);
            let value_str = format!("{obj_str}.{field}");
            // Derive setter from object name
            let setter = if let Expr::Ident { name, .. } = object.as_ref() {
                format!("set_{name}")
            } else {
                format!("set_{}", emit_expr(object))
            };
            let onchange = format!("(e) => {setter}({{...{obj_str}, {field}: e.target.value}})");
            (value_str, onchange)
        }
        _ => {
            // Fallback: treat as opaque expression
            let val = emit_expr(expr);
            (val.clone(), "(e) => {}".to_string())
        }
    }
}

/// Emit a JSX fragment `<>children</>`.
pub fn emit_jsx_fragment(children: &[Expr], indent: usize) -> String {
    let pad = "  ".repeat(indent);
    let mut out = format!("{pad}<>\n");
    for child in children {
        out.push_str(&emit_jsx_child(child, indent + 1));
    }
    out.push_str(&format!("{pad}</>\n"));
    out
}

/// Emit a JSX child expression.
fn emit_jsx_child(expr: &Expr, indent: usize) -> String {
    let pad = "  ".repeat(indent);
    let unwrapped = unwrap_block(expr);
    match unwrapped {
        Expr::Jsx(el) => emit_jsx_element(el, indent),
        Expr::JsxSelfClosing(el) => emit_jsx_self_closing(el, indent),
        Expr::JsxFragment { children, .. } => emit_jsx_fragment(children, indent),
        Expr::For {
            binding,
            index,
            iterable,
            body,
            key,
            ..
        } => {
            let iter_str = emit_expr(iterable);
            let mut body_str = emit_jsx_child(body, indent + 1);
            // Default index name when the user wrote `for x in arr` (no index binding).
            // The leading underscore signals "unused" by JS convention and avoids clashing
            // with a user-named `i` in an outer scope.
            let idx = index.as_deref().unwrap_or("_i");
            // Inject `key={expr}` into the first JSX element in the body string.
            if let Some(k) = key {
                let key_attr = format!(" key={{{}}}", emit_expr(k));
                body_str = inject_key_into_jsx(body_str, &key_attr);
            }
            format!("{pad}{{{iter_str}.map(({binding}, {idx}) => (\n{body_str}{pad}))}}\n")
        }
        Expr::If {
            condition,
            then_body,
            else_body,
            ..
        } => {
            // In JSX children context, emit if-else as a ternary expression.
            let cond_str = emit_expr(condition);
            let then_part = jsx_branch_to_ternary_str(then_body, indent + 1);
            let else_part = match else_body.as_deref() {
                Some(stmts) => jsx_branch_to_ternary_str(stmts, indent + 1),
                None => "null".to_string(),
            };
            format!("{pad}{{{cond_str}\n{pad}  ? {then_part}\n{pad}  : {else_part}}}\n")
        }
        _ => format!("{pad}{}\n", wrap_jsx_hir_child_expr(emit_expr(unwrapped))),
    }
}

/// Extract the single JSX expression (or nested ternary) from an if-else branch statement list.
fn jsx_branch_to_ternary_str(stmts: &[Stmt], indent: usize) -> String {
    if let [Stmt::Expr { expr, .. }] = stmts {
        let u = unwrap_block(expr);
        return match u {
            Expr::JsxSelfClosing(_) | Expr::Jsx(_) | Expr::JsxFragment { .. } => {
                emit_jsx_child(u, indent).trim().to_string()
            }
            Expr::If {
                condition,
                then_body,
                else_body,
                ..
            } => {
                let pad = "  ".repeat(indent);
                let cond_str = emit_expr(condition);
                let then_part = jsx_branch_to_ternary_str(then_body, indent + 1);
                let else_part = match else_body.as_deref() {
                    Some(s) => jsx_branch_to_ternary_str(s, indent + 1),
                    None => "null".to_string(),
                };
                format!("({cond_str}\n{pad}  ? {then_part}\n{pad}  : {else_part})")
            }
            _ => {
                let s = emit_expr(u);
                if s.trim_start().starts_with('<') {
                    s
                } else {
                    format!("{{{s}}}")
                }
            }
        };
    }
    "null".to_string()
}

/// Helper to unwrap a single expression block created by { }.
fn unwrap_block(expr: &Expr) -> &Expr {
    if let Expr::Block { stmts, .. } = expr {
        if stmts.len() == 1 {
            if let Stmt::Expr { expr: inner, .. } = &stmts[0] {
                return inner;
            }
        }
    }
    expr
}

/// Emit a Vox expression as TypeScript.
pub fn emit_expr(expr: &Expr) -> String {
    match expr {
        Expr::IntLit { value, .. } => value.to_string(),
        Expr::FloatLit { value, .. } => value.to_string(),
        Expr::StringLit { value, .. } => ts_string_literal(value),
        Expr::BoolLit { value, .. } => value.to_string(),
        Expr::DecimalLit { value, .. } => ts_string_literal(value),
        Expr::Ident { name, .. } => name.clone(),
        Expr::ObjectLit { fields, .. } => {
            let pairs: Vec<String> = fields
                .iter()
                .map(|(k, v)| format!("{k}: {}", emit_expr(v)))
                .collect();
            format!("{{ {} }}", pairs.join(", "))
        }
        Expr::ListLit { elements, .. } => {
            let elems: Vec<String> = elements.iter().map(emit_expr).collect();
            format!("[{}]", elems.join(", "))
        }
        Expr::TupleLit { elements, .. } => {
            let elems: Vec<String> = elements.iter().map(emit_expr).collect();
            format!("[{}]", elems.join(", "))
        }
        Expr::Binary {
            op, left, right, ..
        } => {
            let l = emit_expr(left);
            let r = emit_expr(right);
            let op_str = match op {
                BinOp::Add => "+",
                BinOp::Sub => "-",
                BinOp::Mul => "*",
                BinOp::Div => "/",
                BinOp::Lt => "<",
                BinOp::Gt => ">",
                BinOp::Lte => "<=",
                BinOp::Gte => ">=",
                BinOp::And => "&&",
                BinOp::Or => "||",
                BinOp::Is => "===",
                BinOp::Isnt => "!==",
                BinOp::Mod => "%",
                BinOp::Pipe => "|>", // handled separately in practice
            };
            if matches!(op, BinOp::Pipe) {
                format!("{r}({l})")
            } else {
                format!("{l} {op_str} {r}")
            }
        }
        Expr::Unary { op, operand, .. } => {
            let inner = emit_expr(operand);
            match op {
                UnOp::Not => format!("!{inner}"),
                UnOp::Neg => format!("-{inner}"),
            }
        }
        Expr::Call { callee, args, .. } => {
            let callee_str = emit_expr(callee);
            let args_str: Vec<String> = args
                .iter()
                .map(|a| {
                    if let Some(ref name) = a.name {
                        // Named args become object property
                        format!("{name}: {}", emit_expr(&a.value))
                    } else {
                        emit_expr(&a.value)
                    }
                })
                .collect();
            // Handle named args: if any arg has a name, wrap in object
            let has_named = args.iter().any(|a| a.name.is_some());
            if has_named {
                let positional: Vec<String> = args
                    .iter()
                    .filter(|a| a.name.is_none())
                    .map(|a| emit_expr(&a.value))
                    .collect();
                let named: Vec<String> = args
                    .iter()
                    .filter(|a| a.name.is_some())
                    .map(|a| format!("{}: {}", a.name.as_ref().unwrap(), emit_expr(&a.value)))
                    .collect();
                let mut all_args = positional;
                if !named.is_empty() {
                    all_args.push(format!("{{ {} }}", named.join(", ")));
                }
                format!("{callee_str}({})", all_args.join(", "))
            } else {
                format!("{callee_str}({})", args_str.join(", "))
            }
        }
        Expr::MethodCall {
            object,
            method,
            args,
            ..
        } => {
            let args_str: Vec<String> = args.iter().map(|a| emit_expr(&a.value)).collect();
            if let Expr::Ident { name, .. } = object.as_ref() {
                if name == "Speech" && method == "transcribe" {
                    let path_js = args_str
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "\"\"".to_string());
                    return format!(
                        "((path: string) => {{ throw new Error(\"Speech.transcribe is backend-only (Vox Oratio / Candle Whisper). Use a @server function or POST /api/audio/transcribe with JSON {{ path }}; see examples/oratio/codexAudioTranscribe.ts.\"); }})({path_js} as string)"
                    );
                }
                if name == "Speech" && method == "transcribe_microphone" && args.is_empty() {
                    return "mobile.transcribe_microphone()".to_string();
                }
            }
            let obj = emit_expr(object);
            // Special case: list.append(x) -> [...list, x]
            if method == "append" && args.len() == 1 {
                return format!("[...{obj}, {}]", args_str[0]);
            }
            format!("{obj}.{method}({})", args_str.join(", "))
        }
        Expr::FieldAccess { object, field, .. } => {
            let obj = emit_expr(object);
            // In React, e.value should be e.target.value for input events
            if field == "value" {
                if let Expr::Ident { .. } = object.as_ref() {
                    return format!("{obj}.target.value");
                }
            }
            format!("{obj}.{field}")
        }
        Expr::Match { subject, arms, .. } => {
            // For HTTP results, emit try/catch
            let subj = emit_expr(subject);
            let mut out = String::new();
            out.push_str(&format!(
                "await (async () => {{\n  const _match = {subj};\n"
            ));
            for (i, arm) in arms.iter().enumerate() {
                let cond = match &arm.pattern {
                    vox_compiler::ast::pattern::Pattern::Constructor { name, .. } => {
                        format!("_match._tag === \"{name}\"")
                    }
                    _ => "true".to_string(),
                };
                let keyword = if i == 0 { "if" } else { "else if" };
                out.push_str(&format!(
                    "  {keyword} ({cond}) {{ return {}; }}\n",
                    emit_expr(&arm.body)
                ));
            }
            out.push_str("})()");
            out
        }
        Expr::Lambda { params, body, .. } => {
            let param_names: Vec<String> = params.iter().map(|p| p.name.clone()).collect();
            format!("({}) => {}", param_names.join(", "), emit_expr(body))
        }
        Expr::Pipe { left, right, .. } => {
            format!("{}({})", emit_expr(right), emit_expr(left))
        }
        Expr::Spawn { target, .. } => {
            format!("new {}Actor()", emit_expr(target))
        }
        Expr::Jsx(el) => emit_jsx_element(el, 0),
        Expr::JsxSelfClosing(el) => emit_jsx_self_closing(el, 0),
        Expr::JsxFragment { children, .. } => emit_jsx_fragment(children, 0),
        Expr::For {
            binding,
            index,
            iterable,
            body,
            key,
            ..
        } => {
            // Default index name when the user wrote `for x in arr` (no index binding).
            // The leading underscore signals "unused" by JS convention and avoids clashing
            // with a user-named `i` in an outer scope.
            let idx = index.as_deref().unwrap_or("_i");
            let mut body_str = emit_expr(body);
            // Inject key prop into the body expression if present.
            if let Some(k) = key {
                let key_attr = format!(" key={{{}}}", emit_expr(k));
                body_str = inject_key_into_jsx(body_str, &key_attr);
            }
            format!(
                "{}.map(({binding}, {idx}) => {body_str})",
                emit_expr(iterable),
            )
        }
        Expr::If {
            condition,
            then_body,
            else_body,
            ..
        } => {
            let cond = emit_expr(condition);
            let then_str: Vec<String> = then_body.iter().map(|s| emit_stmt(s, 1)).collect();
            let mut out = format!("if ({cond}) {{\n{}", then_str.join(""));
            if let Some(else_stmts) = else_body {
                let else_str: Vec<String> = else_stmts.iter().map(|s| emit_stmt(s, 1)).collect();
                out.push_str(&format!("}} else {{\n{}", else_str.join("")));
            }
            out.push('}');
            out
        }
        Expr::StringInterp { parts, .. } => {
            let mut out = String::from("`");
            for part in parts {
                match part {
                    vox_compiler::ast::expr::StringPart::Literal(s) => out.push_str(s),
                    vox_compiler::ast::expr::StringPart::Interpolation(e) => {
                        out.push_str(&format!("${{{}}}", emit_expr(e)));
                    }
                }
            }
            out.push('`');
            out
        }
        Expr::Block { stmts, .. } => {
            let lines: Vec<String> = stmts.iter().map(|s| emit_stmt(s, 0)).collect();
            lines.join("")
        }
        Expr::With { operand, .. } => emit_expr(operand),
        Expr::Try { target, .. } => {
            let inner = emit_expr(target);
            format!(
                "(() => {{ const _v = {inner}; if (_v._tag === \"Ok\") return _v.value; throw _v; }})()"
            )
        }
        Expr::Index { object, index, .. } => {
            format!("{}[{}]", emit_expr(object), emit_expr(index))
        }
        // side_effect desugars to a synthesised activity call at HIR time; should not
        // reach the AST-level TS codegen path.
        Expr::SideEffect { .. } => "__side_effect__(/* P1-T7 */)".to_string(),
        // WorkflowVersion is not representable as a TS expression in this emit path
        Expr::WorkflowVersion(_) => String::new(),
    }
}

/// Emit a Vox statement as TypeScript.
///
/// **Phase:** compat-legacy (OP-0150).
pub fn emit_stmt(stmt: &Stmt, indent: usize) -> String {
    let pad = "  ".repeat(indent);
    match stmt {
        Stmt::Let {
            pattern,
            value,
            mutable,
            ..
        } => {
            let keyword = if *mutable { "let" } else { "const" };
            let pat = emit_pattern(pattern);
            let val = emit_expr(value);
            format!("{pad}{keyword} {pat} = {val};\n")
        }
        Stmt::Assign { target, value, .. } => {
            format!("{pad}{} = {};\n", emit_expr(target), emit_expr(value))
        }
        Stmt::Return { value, .. } => {
            if let Some(v) = value {
                format!("{pad}return {};\n", emit_expr(v))
            } else {
                format!("{pad}return;\n")
            }
        }
        Stmt::Expr { expr, .. } => {
            format!("{pad}{};\n", emit_expr(expr))
        }
        Stmt::While {
            condition, body, ..
        } => {
            let cond = emit_expr(condition);
            let body_str: Vec<String> = body.iter().map(|s| emit_stmt(s, indent + 1)).collect();
            format!("{pad}while ({cond}) {{\n{}{pad}}}\n", body_str.join(""))
        }
        Stmt::Loop { body, .. } => {
            let body_str: Vec<String> = body.iter().map(|s| emit_stmt(s, indent + 1)).collect();
            format!("{pad}while (true) {{\n{}{pad}}}\n", body_str.join(""))
        }
        Stmt::Break { .. } => format!("{pad}break;\n"),
        Stmt::Continue { .. } => format!("{pad}continue;\n"),
    }
}

/// Emit a pattern as TypeScript destructuring.
/// Inject a `key` attribute into the first JSX element tag in the given string.
///
/// Looks for the first `<Tag` sequence and inserts `key_attr` (e.g. ` key={expr}`)
/// before the first `>` or `/>` of that opening tag. Falls back to returning the
/// original string unchanged if no suitable insertion point is found.
fn inject_key_into_jsx(jsx: String, key_attr: &str) -> String {
    // Find the first '<' that starts a JSX element (skip '{' expression wrappers).
    if let Some(lt_pos) = jsx.find('<') {
        let after_lt = &jsx[lt_pos..];
        // Find the end of the opening tag — first '>' or '/' before '>'.
        // We want to insert before the first '>' or '/>' in the opening tag.
        if let Some(rel_end) = after_lt.find(['>', '/']) {
            let insert_at = lt_pos + rel_end;
            return format!("{}{}{}", &jsx[..insert_at], key_attr, &jsx[insert_at..]);
        }
    }
    jsx
}

fn emit_pattern(pattern: &vox_compiler::ast::pattern::Pattern) -> String {
    match pattern {
        vox_compiler::ast::pattern::Pattern::Ident { name, .. } => name.clone(),
        vox_compiler::ast::pattern::Pattern::Tuple { elements, .. } => {
            let elems: Vec<String> = elements.iter().map(emit_pattern).collect();
            format!("[{}]", elems.join(", "))
        }
        vox_compiler::ast::pattern::Pattern::Wildcard { .. } => "_".to_string(),
        vox_compiler::ast::pattern::Pattern::Constructor { name, fields, .. } => {
            if fields.is_empty() {
                name.clone()
            } else {
                let f: Vec<String> = fields.iter().map(emit_pattern).collect();
                format!("{name}({})", f.join(", "))
            }
        }
        vox_compiler::ast::pattern::Pattern::Literal { value, .. } => emit_expr(value),
    }
}
