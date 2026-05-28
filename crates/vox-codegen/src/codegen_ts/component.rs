//! Classic `@component fn` → React TSX (AST path).
//!
//! Web IR ([`crate::web_ir`]) owns structural view metadata; JSX-shaped classic bodies also lower
//! into [`WebIrModule::view_roots`](crate::web_ir::WebIrModule::view_roots) for validate/preview,
//! while **this** module remains the AST → TSX codegen path (OP-0177+, OP-0179).
//!
//! **Compatibility mode (OP-0178):** React hook imports follow [`vox_compiler::react_bridge`] registry;
//! non-registry `use_*` hooks rely on user imports. **Pathways (OP-0184):** classic AST → `super::jsx`;
//! Path C → `super::reactive` (Web IR canonical `view:` emit; see module docs there).
//!
//! **Disposition (OP-0190):** shrinking direct JSX ownership means moving view strings toward Web IR
//! preview/`emit_tsx` parity tests first, then codegen cutover.
//!
//! **Props / contracts (OP-0180):** classic function parameters remain the generated TS `*Props`
//! interface first; mapping props into Web IR `behavior_nodes` stays Path C–aligned until classic
//! bodies fully share the reactive binding model.
//!
//! **Adapter notes (OP-S035):** classic AST codegen (`emit_jsx_*`) and Path C reactive (`super::reactive`)
//! both consume hook registries through shared helpers; new prop or hook wiring should
//! land in [`vox_compiler::react_bridge`] + Web IR lower/validate before growing this adapter.
//!
//! **Notes B/C (OP-S115 / S161 / S193):** classic props interfaces remain the codegen boundary until Path C
//! view maps fully share [`crate::web_ir::BehaviorNode`] naming.

use crate::codegen_ts::jsx::{
    emit_expr, emit_jsx_element, emit_jsx_fragment, emit_jsx_self_closing, emit_stmt,
};
use std::collections::BTreeSet;
use vox_compiler::ast::decl::FnDecl;
use vox_compiler::ast::expr::Expr;
use vox_compiler::ast::scalar_mapping::VoxScalar;
use vox_compiler::ast::stmt::Stmt;
use vox_compiler::react_bridge::{for_each_vox_hook_call_in_stmt, react_hook_export_for_vox_ident};

/// Generate a React component from WebIR when a lowered view root is available.
#[must_use]
pub fn generate_component_from_web_ir(
    func: &FnDecl,
    has_styles: bool,
    web: &crate::web_ir::WebIrModule,
) -> Option<(String, String)> {
    let view = crate::web_ir::emit_tsx::emit_component_view_tsx(web, &func.name)?;
    let name = &func.name;
    let filename = format!("{name}.tsx");
    let mut out = String::new();

    let mut vox_hooks_used: BTreeSet<String> = BTreeSet::new();
    for stmt in &func.body {
        for_each_vox_hook_call_in_stmt(stmt, &mut |hook_name, _span| {
            vox_hooks_used.insert(hook_name.to_string());
        });
    }
    let mut react_hooks: BTreeSet<&str> = BTreeSet::new();
    for vox_name in &vox_hooks_used {
        if let Some(react_name) = react_hook_export_for_vox_ident(vox_name.as_str()) {
            react_hooks.insert(react_name);
        }
    }

    if react_hooks.is_empty() {
        out.push_str("import React from \"react\";\n");
    } else {
        let hook_list: Vec<&&str> = react_hooks.iter().collect();
        out.push_str(&format!(
            "import React, {{ {} }} from \"react\";\n",
            hook_list.iter().map(|s| **s).collect::<Vec<_>>().join(", ")
        ));
    }
    if has_styles {
        out.push_str(&format!("import \"./{name}.css\";\n"));
    }
    out.push('\n');

    if !func.params.is_empty() {
        out.push_str(&format!("export interface {name}Props {{\n"));
        for param in &func.params {
            let ts_type = param
                .type_ann
                .as_ref()
                .map_or("any".to_string(), map_vox_type_to_ts);
            let optional = if param.default.is_some() { "?" } else { "" };
            out.push_str(&format!("  {}{optional}: {ts_type};\n", param.name));
        }
        out.push_str("}\n\n");
    }

    if func.params.is_empty() {
        out.push_str(&format!(
            "export function {name}(): React.ReactElement {{\n"
        ));
    } else {
        let params = func
            .params
            .iter()
            .map(|p| p.name.clone())
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!(
            "export function {name}({{ {params} }}: {name}Props): React.ReactElement {{\n"
        ));
    }

    for stmt in &func.body {
        match stmt {
            Stmt::Let { .. } | Stmt::Assign { .. } => {
                out.push_str(&emit_component_stmt(stmt));
            }
            Stmt::Expr { expr, .. } => match expr {
                Expr::Jsx(_) | Expr::JsxSelfClosing(_) | Expr::JsxFragment { .. } => {}
                Expr::Call { .. } | Expr::MethodCall { .. } => {
                    out.push_str(&emit_component_stmt(stmt));
                }
                _ => {
                    out.push_str(&emit_component_stmt(stmt));
                }
            },
            Stmt::Return { .. } => {}
            Stmt::While { .. } | Stmt::Loop { .. } | Stmt::Break { .. } | Stmt::Continue { .. } => {
                out.push_str(&emit_component_stmt(stmt));
            }
        }
    }

    out.push_str("  return (\n");
    for line in view.lines() {
        out.push_str("    ");
        out.push_str(line);
        out.push('\n');
    }
    out.push_str("  );\n}\n");
    Some((filename, out))
}

/// Generate a React component from a Vox @component function declaration.
/// Returns (filename, content) tuple.
pub fn generate_component(func: &FnDecl, has_styles: bool) -> (String, String) {
    let name = &func.name;
    let filename = format!("{name}.tsx");
    let mut out = String::new();

    // Collect hook names referenced in the component body.
    let mut vox_hooks_used: BTreeSet<String> = BTreeSet::new();
    for stmt in &func.body {
        for_each_vox_hook_call_in_stmt(stmt, &mut |name, _span| {
            vox_hooks_used.insert(name.to_string());
        });
    }

    // Map Vox snake_case hook names → React camelCase imports.
    let mut react_hooks: BTreeSet<&str> = BTreeSet::new();
    for vox_name in &vox_hooks_used {
        if let Some(react_name) = react_hook_export_for_vox_ident(vox_name.as_str()) {
            react_hooks.insert(react_name);
        }
        // Custom use_* hooks (not in registry) are not React built-ins;
        // they are imported separately below if the user imports them.
    }

    // Emit React import — only include hooks actually used.
    if react_hooks.is_empty() {
        out.push_str("import React from \"react\";\n\n");
    } else {
        let hook_list: Vec<&&str> = react_hooks.iter().collect();
        out.push_str(&format!(
            "import React, {{ {} }} from \"react\";\n\n",
            hook_list.iter().map(|s| **s).collect::<Vec<_>>().join(", ")
        ));
    }
    if has_styles {
        out.push_str(&format!("import \"./{name}.css\";\n\n"));
    }

    // Mobile bridge import
    let mut uses_mobile = false;
    for stmt in &func.body {
        if uses_mobile_ident_in_stmt(stmt) {
            uses_mobile = true;
            break;
        }
    }
    if uses_mobile {
        // Assume default std.mobile for standard component imports
        out.push_str("import { mobile } from \"./mobile-utils\";\n\n");
    }

    // Props interface
    if !func.params.is_empty() {
        out.push_str(&format!("export interface {name}Props {{\n"));
        for param in &func.params {
            let ts_type = param
                .type_ann
                .as_ref()
                .map_or("any".to_string(), map_vox_type_to_ts);
            let optional = if param.default.is_some() { "?" } else { "" };
            out.push_str(&format!("  {}{optional}: {ts_type};\n", param.name));
        }
        out.push_str("}\n\n");
    }

    // Function component
    if func.params.is_empty() {
        out.push_str(&format!(
            "export function {name}(): React.ReactElement {{\n"
        ));
    } else {
        // Destructure props
        let param_names: Vec<String> = func
            .params
            .iter()
            .map(|p| {
                if let Some(ref default) = p.default {
                    format!("{} = {}", p.name, emit_expr(default))
                } else {
                    p.name.clone()
                }
            })
            .collect();
        out.push_str(&format!(
            "export function {name}({{ {} }}: {name}Props): React.ReactElement {{\n",
            param_names.join(", ")
        ));
    }

    // Body: emit all non-return, non-JSX statements, then find the JSX return
    let mut jsx_return: Option<String> = None;

    for stmt in &func.body {
        match stmt {
            Stmt::Let { .. } | Stmt::Assign { .. } => {
                out.push_str(&emit_component_stmt(stmt));
            }
            Stmt::Expr { expr, .. } => {
                match expr {
                    Expr::Jsx(el) => {
                        // This is the return JSX
                        jsx_return = Some(emit_jsx_element(el, 2));
                    }
                    Expr::JsxSelfClosing(el) => {
                        jsx_return = Some(emit_jsx_self_closing(el, 2));
                    }
                    Expr::JsxFragment { children, .. } => {
                        jsx_return = Some(emit_jsx_fragment(children, 2));
                    }
                    Expr::Call { .. } | Expr::MethodCall { .. } => {
                        out.push_str(&emit_component_stmt(stmt));
                    }
                    _ => {
                        out.push_str(&emit_component_stmt(stmt));
                    }
                }
            }
            Stmt::Return {
                value: Some(expr), ..
            } => {
                jsx_return = Some(format!("    {}", emit_expr(expr)));
            }
            Stmt::While { .. } | Stmt::Loop { .. } | Stmt::Break { .. } | Stmt::Continue { .. } => {
                out.push_str(&emit_component_stmt(stmt));
            }
            _ => {}
        }
    }

    // Emit JSX return
    if let Some(jsx) = jsx_return {
        out.push_str(&format!("  return (\n{jsx}  );\n"));
    }

    out.push_str("}\n");

    (filename, out)
}

/// Emit a statement inside a React component body.
fn emit_component_stmt(stmt: &Stmt) -> String {
    match stmt {
        Stmt::Let { pattern, value, .. } => {
            let pat = emit_component_pattern(pattern);
            let val = emit_component_expr(value);
            format!("  const {pat} = {val};\n")
        }
        Stmt::Expr { expr, .. } => {
            // Check for nested function definitions
            if let Expr::Block { .. } = expr {
                return emit_component_expr(expr);
            }
            format!("  {};\n", emit_component_expr(expr))
        }
        _ => emit_stmt(stmt, 1),
    }
}

/// Emit an expression in component context with React-specific transformations.
fn emit_component_expr(expr: &Expr) -> String {
    match expr {
        Expr::Call { callee, args, .. } => {
            let callee_str = match callee.as_ref() {
                Expr::Ident { name, .. } => {
                    // Map Vox stdlib names to React equivalents
                    match name.as_str() {
                        "use_state" => "useState".to_string(),
                        "use_effect" => "useEffect".to_string(),
                        "use_memo" => "useMemo".to_string(),
                        "use_ref" => "useRef".to_string(),
                        "use_callback" => "useCallback".to_string(),
                        other => other.to_string(),
                    }
                }
                other => emit_expr(other),
            };
            let args_str: Vec<String> = args.iter().map(|a| emit_expr(&a.value)).collect();
            format!("{callee_str}({})", args_str.join(", "))
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
                        "((path: string) => {{ throw new Error(\"Speech.transcribe is backend-only (Vox Oratio / Candle Whisper). Use @server or POST /api/audio/transcribe; see examples/oratio/codexAudioTranscribe.ts.\"); }})({path_js} as string)"
                    );
                }
                if name == "Speech" && method == "transcribe_microphone" && args.is_empty() {
                    return "mobile.transcribe_microphone()".to_string();
                }
            }
            let obj = emit_component_expr(object);
            if method == "append" && args.len() == 1 {
                return format!("[...{obj}, {}]", args_str[0]);
            }
            format!("{obj}.{method}({})", args_str.join(", "))
        }
        Expr::Match { subject, arms, .. } => {
            // For HTTP.post results in a component, emit try/catch
            let subj = emit_component_expr(subject);
            let mut out = String::new();
            out.push_str(&format!(
                "(async () => {{\n    try {{\n      const _result = await {subj};\n"
            ));
            if let Some(ok_arm) = arms.first() {
                out.push_str(&format!("      {};\n", emit_expr(&ok_arm.body)));
            }
            out.push_str("    } catch (_err) {\n");
            if arms.len() > 1 {
                out.push_str(&format!("      {};\n", emit_expr(&arms[1].body)));
            }
            out.push_str("    }\n  })()");
            out
        }
        _ => emit_expr(expr),
    }
}

fn emit_component_pattern(pattern: &vox_compiler::ast::pattern::Pattern) -> String {
    match pattern {
        vox_compiler::ast::pattern::Pattern::Ident { name, .. } => name.clone(),
        vox_compiler::ast::pattern::Pattern::Tuple { elements, .. } => {
            let elems: Vec<String> = elements.iter().map(emit_component_pattern).collect();
            format!("[{}]", elems.join(", "))
        }
        vox_compiler::ast::pattern::Pattern::Wildcard { .. } => "_".to_string(),
        _ => "_".to_string(),
    }
}

/// Map a Vox type expression to a TypeScript type string.
pub fn map_vox_type_to_ts(ty: &vox_compiler::ast::types::TypeExpr) -> String {
    match ty {
        vox_compiler::ast::types::TypeExpr::Named { name, .. } => {
            if let Some(s) = VoxScalar::parse(name) {
                s.as_ts_primitive().to_string()
            } else {
                match name.as_str() {
                    "Element" => "React.ReactElement".to_string(),
                    "Unit" => "void".to_string(),
                    other => other.to_string(),
                }
            }
        }
        vox_compiler::ast::types::TypeExpr::Generic { name, args, .. } => {
            let args_str: Vec<String> = args.iter().map(map_vox_type_to_ts).collect();
            match name.as_str() {
                "list" => format!("{}[]", args_str.join(", ")),
                "Result" => format!("Result<{}>", args_str.join(", ")),
                "Option" => format!("{} | undefined", args_str.join(", ")),
                _ => format!("{}<{}>", name, args_str.join(", ")),
            }
        }
        vox_compiler::ast::types::TypeExpr::Function {
            params,
            return_type,
            ..
        } => {
            let params_str: Vec<String> = params
                .iter()
                .enumerate()
                .map(|(i, p)| format!("arg{i}: {}", map_vox_type_to_ts(p)))
                .collect();
            format!(
                "({}) => {}",
                params_str.join(", "),
                map_vox_type_to_ts(return_type)
            )
        }
        vox_compiler::ast::types::TypeExpr::Tuple { elements, .. } => {
            let elems: Vec<String> = elements.iter().map(map_vox_type_to_ts).collect();
            format!("[{}]", elems.join(", "))
        }
        vox_compiler::ast::types::TypeExpr::Unit { .. } => "void".to_string(),
        vox_compiler::ast::types::TypeExpr::Infer { .. } => "any".to_string(),
        vox_compiler::ast::types::TypeExpr::Decimal { .. } => "string".to_string(),
    }
}

fn uses_mobile_ident_in_stmt(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Let { value, .. } => uses_mobile_ident_in_expr(value),
        Stmt::Assign { target, value, .. } => {
            uses_mobile_ident_in_expr(target) || uses_mobile_ident_in_expr(value)
        }
        Stmt::Return { value, .. } => value.as_ref().is_some_and(uses_mobile_ident_in_expr),
        Stmt::Expr { expr, .. } => uses_mobile_ident_in_expr(expr),
        Stmt::While {
            condition, body, ..
        } => uses_mobile_ident_in_expr(condition) || body.iter().any(uses_mobile_ident_in_stmt),
        Stmt::Loop { body, .. } => body.iter().any(uses_mobile_ident_in_stmt),
        _ => false,
    }
}

fn uses_mobile_ident_in_expr(expr: &Expr) -> bool {
    match expr {
        Expr::Ident { name, .. } => name == "mobile",
        Expr::Call { callee, args, .. } => {
            uses_mobile_ident_in_expr(callee)
                || args.iter().any(|a| uses_mobile_ident_in_expr(&a.value))
        }
        Expr::MethodCall { object, args, .. } => {
            uses_mobile_ident_in_expr(object)
                || args.iter().any(|a| uses_mobile_ident_in_expr(&a.value))
        }
        Expr::FieldAccess { object, .. } => uses_mobile_ident_in_expr(object),
        Expr::Binary { left, right, .. } => {
            uses_mobile_ident_in_expr(left) || uses_mobile_ident_in_expr(right)
        }
        Expr::Unary { operand, .. } => uses_mobile_ident_in_expr(operand),
        Expr::Block { stmts, .. } => stmts.iter().any(uses_mobile_ident_in_stmt),
        Expr::If {
            condition,
            then_body,
            else_body,
            ..
        } => {
            uses_mobile_ident_in_expr(condition)
                || then_body.iter().any(uses_mobile_ident_in_stmt)
                || else_body
                    .as_ref()
                    .is_some_and(|b| b.iter().any(uses_mobile_ident_in_stmt))
        }
        Expr::ObjectLit { fields, .. } => fields.iter().any(|(_, v)| uses_mobile_ident_in_expr(v)),
        Expr::ListLit { elements, .. } | Expr::TupleLit { elements, .. } => {
            elements.iter().any(uses_mobile_ident_in_expr)
        }
        Expr::Try { target, .. } | Expr::Spawn { target, .. } => uses_mobile_ident_in_expr(target),
        _ => false,
    }
}
