use vox_ast::expr::{BinOp, Expr, JsxElement, JsxSelfClosingElement, UnOp};
use vox_ast::stmt::Stmt;

/// Map Vox JSX attribute names to React attribute names.
pub fn map_jsx_attr_name(name: &str) -> &str {
    match name {
        "class" => "className",
        "on_click" => "onClick",
        "on_change" => "onChange",
        "on_submit" => "onSubmit",
        "on_input" => "onInput",
        "on_key_down" => "onKeyDown",
        "on_key_up" => "onKeyUp",
        "on_mouse_enter" => "onMouseEnter",
        "on_mouse_leave" => "onMouseLeave",
        "for" => "htmlFor",
        "tab_index" => "tabIndex",
        other => other,
    }
}

/// Emit a JSX element with children to TypeScript.
pub fn emit_jsx_element(el: &JsxElement, indent: usize) -> String {
    let pad = "  ".repeat(indent);
    let mut out = String::new();

    out.push_str(&format!("{pad}<{}", el.tag));

    // Attributes
    for attr in &el.attributes {
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
    out.push_str(">\n");

    // Children
    for child in &el.children {
        out.push_str(&emit_jsx_child(child, indent + 1));
    }

    out.push_str(&format!("{pad}</{}>\n", el.tag));
    out
}

/// Emit a self-closing JSX element.
pub fn emit_jsx_self_closing(el: &JsxSelfClosingElement, indent: usize) -> String {
    let pad = "  ".repeat(indent);
    let mut out = format!("{pad}<{}", el.tag);

    for attr in &el.attributes {
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
    out.push_str(" />\n");
    out
}

/// Emit a JSX attribute value expression.
fn emit_jsx_attr_value(expr: &Expr) -> String {
    match expr {
        Expr::StringLit { value, .. } => {
            // Check if string contains interpolation like {msg.role}
            if value.contains('{') && value.contains('}') {
                // Convert to template literal: "message {msg.role}" -> `message ${msg.role}`
                let template = value.replace('{', "${").to_string();
                format!("`{template}`")
            } else {
                format!("\"{value}\"")
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
    match expr {
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

/// Emit a JSX child expression.
fn emit_jsx_child(expr: &Expr, indent: usize) -> String {
    let pad = "  ".repeat(indent);
    match expr {
        Expr::Jsx(el) => emit_jsx_element(el, indent),
        Expr::JsxSelfClosing(el) => emit_jsx_self_closing(el, indent),
        Expr::For {
            binding,
            iterable,
            body,
            ..
        } => {
            let iter_str = emit_expr(iterable);
            let body_str = emit_jsx_child(body, indent + 1);
            format!("{pad}{{{iter_str}.map(({binding}, _i) => (\n{body_str}{pad}))}}\n")
        }
        Expr::StringLit { value, .. } => format!("{pad}{value}\n"),
        Expr::Ident { name, .. } => format!("{pad}{name}\n"),
        _ => format!("{pad}{{{}}}\n", emit_expr(expr)),
    }
}

/// Emit a Vox expression as TypeScript.
pub fn emit_expr(expr: &Expr) -> String {
    match expr {
        Expr::IntLit { value, .. } => value.to_string(),
        Expr::FloatLit { value, .. } => value.to_string(),
        Expr::StringLit { value, .. } => format!("\"{value}\""),
        Expr::BoolLit { value, .. } => value.to_string(),
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
            let obj = emit_expr(object);
            let args_str: Vec<String> = args.iter().map(|a| emit_expr(&a.value)).collect();
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
                    vox_ast::pattern::Pattern::Constructor { name, .. } => {
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
        Expr::For {
            binding,
            iterable,
            body,
            ..
        } => {
            format!(
                "{}.map(({binding}) => {})",
                emit_expr(iterable),
                emit_expr(body)
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
                    vox_ast::expr::StringPart::Literal(s) => out.push_str(s),
                    vox_ast::expr::StringPart::Interpolation(e) => {
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
    }
}

/// Emit a Vox statement as TypeScript.
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
    }
}

/// Emit a pattern as TypeScript destructuring.
pub fn emit_pattern_public(pattern: &vox_ast::pattern::Pattern) -> String {
    emit_pattern(pattern)
}

/// Emit a pattern as TypeScript destructuring.
fn emit_pattern(pattern: &vox_ast::pattern::Pattern) -> String {
    match pattern {
        vox_ast::pattern::Pattern::Ident { name, .. } => name.clone(),
        vox_ast::pattern::Pattern::Tuple { elements, .. } => {
            let elems: Vec<String> = elements.iter().map(emit_pattern).collect();
            format!("[{}]", elems.join(", "))
        }
        vox_ast::pattern::Pattern::Wildcard { .. } => "_".to_string(),
        vox_ast::pattern::Pattern::Constructor { name, fields, .. } => {
            if fields.is_empty() {
                name.clone()
            } else {
                let f: Vec<String> = fields.iter().map(emit_pattern).collect();
                format!("{name}({})", f.join(", "))
            }
        }
        vox_ast::pattern::Pattern::Literal { value, .. } => emit_expr(value),
    }
}
