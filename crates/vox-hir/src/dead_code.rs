use crate::hir::*;
use std::collections::HashSet;
use vox_ast::span::Span;

pub fn check_dead_code(module: &HirModule) -> Vec<(String, Span)> {
    let mut warnings = Vec::new();
    let mut used = HashSet::new();

    // Collect all referenced identifiers
    for f in &module.functions {
        visit_fn(f, &mut used);
    }
    for t in &module.tests {
        visit_fn(t, &mut used);
    }
    for sf in &module.server_fns {
        visit_fn_body(&sf.body, &mut used);
    }
    for route in &module.routes {
        for stmt in &route.body {
            visit_stmt(stmt, &mut used);
        }
    }
    for w in &module.workflows {
        visit_fn_body(&w.body, &mut used);
    }
    for a in &module.activities {
        visit_fn_body(&a.body, &mut used);
    }
    for actor in &module.actors {
        for handler in &actor.handlers {
            visit_fn_body(&handler.body, &mut used);
        }
    }
    for imp in &module.impls {
        used.insert(imp.trait_name.clone());
        for m in &imp.methods {
            visit_fn(m, &mut used);
        }
    }
    for m in &module.mcp_tools {
        visit_fn(&m.func, &mut used);
    }
    for q in &module.queries {
        visit_fn(&q.func, &mut used);
    }
    for m in &module.mutations {
        visit_fn(&m.func, &mut used);
    }
    for a in &module.actions {
        visit_fn(&a.func, &mut used);
    }
    for s in &module.skills {
        visit_fn(&s.func, &mut used);
    }
    for a in &module.agents {
        visit_fn(&a.func, &mut used);
    }
    for na in &module.native_agents {
        used.insert(na.name.clone());
        for h in &na.handlers {
            visit_fn_body(&h.body, &mut used);
        }
        for m in &na.migrations {
            visit_fn_body(&m.body, &mut used);
        }
    }
    for msg in &module.messages {
        used.insert(msg.name.clone());
    }
    for s in &module.scheduled {
        visit_fn(&s.func, &mut used);
    }
    for f in &module.fixtures {
        visit_fn(f, &mut used);
    }
    for c in &module.contexts {
        if let Some(e) = &c.default_expr {
            visit_expr(e, &mut used);
        }
    }
    for h in &module.hooks {
        visit_fn_body(&h.body, &mut used);
    }
    for p in &module.providers {
        visit_fn(&p.func, &mut used);
    }

    // Now emit warnings for anything unused and not public
    for f in &module.functions {
        if !f.is_pub && f.name != "main" && !f.is_component && !used.contains(&f.name) {
            warnings.push((format!("function `{}` is never used", f.name), f.span));
        }
    }

    for tbl in &module.tables {
        if !tbl.is_pub && !used.contains(&tbl.name) {
            warnings.push((format!("table `{}` is never used", tbl.name), tbl.span));
        }
    }

    for ty in &module.types {
        if !ty.is_pub && !used.contains(&ty.name) {
            warnings.push((format!("type `{}` is never used", ty.name), ty.span));
        }
    }

    for c in &module.contexts {
        if !used.contains(&c.name) {
            warnings.push((format!("context `{}` is never used", c.name), c.span));
        }
    }

    for h in &module.hooks {
        if !used.contains(&h.name) {
            warnings.push((format!("hook `{}` is never used", h.name), h.span));
        }
    }

    warnings
}

fn visit_fn(f: &HirFn, used: &mut HashSet<String>) {
    visit_fn_body(&f.body, used);
}

fn visit_fn_body(body: &[HirStmt], used: &mut HashSet<String>) {
    for stmt in body {
        visit_stmt(stmt, used);
    }
}

fn visit_stmt(stmt: &HirStmt, used: &mut HashSet<String>) {
    match stmt {
        HirStmt::Let { value, .. } => visit_expr(value, used),
        HirStmt::Assign { target, value, .. } => {
            visit_expr(target, used);
            visit_expr(value, used);
        }
        HirStmt::Return { value: Some(v), .. } => {
            visit_expr(v, used);
        }
        HirStmt::Return { value: None, .. } => {}
        HirStmt::Expr { expr, .. } => visit_expr(expr, used),
        HirStmt::Emit { value, .. } => visit_expr(value, used),
        HirStmt::For { iterable, body, .. } => {
            visit_expr(iterable, used);
            for stmt in body {
                visit_stmt(stmt, used);
            }
        }
        _ => {}
    }
}

fn visit_expr(expr: &HirExpr, used: &mut HashSet<String>) {
    match expr {
        HirExpr::Ident(name, _) => {
            used.insert(name.clone());
        }
        HirExpr::Call(callee, args, _, _) => {
            visit_expr(callee, used);
            for arg in args {
                visit_expr(&arg.value, used);
            }
        }
        HirExpr::Binary(_, left, right, _) => {
            visit_expr(left, used);
            visit_expr(right, used);
        }
        HirExpr::Unary(_, operand, _) => {
            visit_expr(operand, used);
        }
        HirExpr::MethodCall(obj, _, args, _) => {
            visit_expr(obj, used);
            for arg in args {
                visit_expr(&arg.value, used);
            }
        }
        HirExpr::FieldAccess(obj, _, _) => {
            visit_expr(obj, used);
        }
        HirExpr::Match(subj, arms, _) => {
            visit_expr(subj, used);
            for arm in arms {
                if let HirPattern::Constructor(name, _, _) = &arm.pattern {
                    used.insert(name.clone());
                }
                visit_expr(&arm.body, used);
            }
        }
        HirExpr::If(cond, then_body, else_body, _) => {
            visit_expr(cond, used);
            for stmt in then_body {
                visit_stmt(stmt, used);
            }
            if let Some(stmts) = else_body {
                for stmt in stmts {
                    visit_stmt(stmt, used);
                }
            }
        }
        HirExpr::For(_, iter, body, _) => {
            visit_expr(iter, used);
            visit_expr(body, used);
        }
        HirExpr::While {
            condition, body, ..
        } => {
            visit_expr(condition, used);
            for stmt in body {
                visit_stmt(stmt, used);
            }
        }
        HirExpr::Loop { body, .. } => {
            for stmt in body {
                visit_stmt(stmt, used);
            }
        }
        HirExpr::TryCatch {
            body, catch_body, ..
        } => {
            for stmt in body {
                visit_stmt(stmt, used);
            }
            for stmt in catch_body {
                visit_stmt(stmt, used);
            }
        }
        HirExpr::Block(stmts, _) | HirExpr::StreamBlock(stmts, _) => {
            for stmt in stmts {
                visit_stmt(stmt, used);
            }
        }
        HirExpr::Lambda(_, _, body, _) => {
            visit_expr(body, used);
        }
        HirExpr::Spawn(target, _) | HirExpr::Await(target, _) => {
            visit_expr(target, used);
        }
        HirExpr::With(body, opts, _) => {
            visit_expr(body, used);
            visit_expr(opts, used);
        }
        HirExpr::Pipe(left, right, _) => {
            visit_expr(left, used);
            visit_expr(right, used);
        }
        HirExpr::ObjectLit(fields, _) => {
            for (_, err) in fields {
                visit_expr(err, used);
            }
        }
        HirExpr::ListLit(items, _) => {
            for item in items {
                visit_expr(item, used);
            }
        }
        _ => {}
    }
}
