//! Walk [`HirExpr`] trees to find every `HirExpr::DbTableOp` site (shared by runtime projection).

use crate::hir::{
    HirArg, HirDbQueryPlan, HirDbTableOp, HirExpr, HirModule, HirReactiveMember, HirStmt,
};

/// One occurrence of a lowered `db.Table.op(...)` expression.
pub(crate) struct DbTableOpSite<'a> {
    pub table: &'a str,
    pub op: HirDbTableOp,
    pub plan: Option<&'a HirDbQueryPlan>,
}

pub(crate) fn for_each_db_table_op_in_module(
    module: &HirModule,
    f: &mut impl FnMut(DbTableOpSite<'_>),
) {
    for_each_hir_expr_in_module(module, &mut |expr| {
        if let HirExpr::MethodCall(_, _, _, Some(plan), _) = expr {
            f(DbTableOpSite {
                table: &plan.table,
                op: plan.op,
                plan: Some(&**plan),
            });
        }
    });
}

pub fn for_each_hir_expr_in_module(module: &HirModule, f: &mut impl FnMut(&HirExpr)) {
    for fd in &module.functions {
        walk_stmts(&fd.body, f);
    }
    for fd in &module.tests {
        walk_stmts(&fd.body, f);
    }

    for sf in &module.endpoint_fns {
        walk_stmts(&sf.body, f);
    }
    for tool in &module.mcp_tools {
        walk_stmts(&tool.func.body, f);
    }
    for res in &module.mcp_resources {
        walk_stmts(&res.func.body, f);
    }
    for rc in &module.components {
        for m in &rc.members {
            walk_reactive_member(m, f);
        }
        if let Some(view) = &rc.view {
            walk_expr(view, f);
        }
    }
}

fn walk_reactive_member(member: &HirReactiveMember, f: &mut impl FnMut(&HirExpr)) {
    match member {
        HirReactiveMember::State(s) => walk_expr(&s.init, f),
        HirReactiveMember::Derived(d) => walk_expr(&d.expr, f),
        HirReactiveMember::Effect(e) => walk_expr(&e.body, f),
        HirReactiveMember::OnMount(m) => walk_expr(&m.body, f),
        HirReactiveMember::OnCleanup(c) => walk_expr(&c.body, f),
        HirReactiveMember::Stmt(s) => walk_stmt(s, f),
    }
}

fn walk_reactive_member_mut(member: &mut HirReactiveMember, f: &mut impl FnMut(&mut HirExpr)) {
    match member {
        HirReactiveMember::State(s) => walk_expr_mut(&mut s.init, f),
        HirReactiveMember::Derived(d) => walk_expr_mut(&mut d.expr, f),
        HirReactiveMember::Effect(e) => walk_expr_mut(&mut e.body, f),
        HirReactiveMember::OnMount(m) => walk_expr_mut(&mut m.body, f),
        HirReactiveMember::OnCleanup(c) => walk_expr_mut(&mut c.body, f),
        HirReactiveMember::Stmt(s) => walk_stmt_mut(s, f),
    }
}

pub fn for_each_hir_expr_in_module_mut(module: &mut HirModule, f: &mut impl FnMut(&mut HirExpr)) {
    for fd in &mut module.functions {
        walk_stmts_mut(&mut fd.body, f);
    }
    for fd in &mut module.tests {
        walk_stmts_mut(&mut fd.body, f);
    }

    for sf in &mut module.endpoint_fns {
        walk_stmts_mut(&mut sf.body, f);
    }
    for tool in &mut module.mcp_tools {
        walk_stmts_mut(&mut tool.func.body, f);
    }
    for res in &mut module.mcp_resources {
        walk_stmts_mut(&mut res.func.body, f);
    }
    for rc in &mut module.components {
        for m in &mut rc.members {
            walk_reactive_member_mut(m, f);
        }
        if let Some(view) = &mut rc.view {
            walk_expr_mut(view, f);
        }
    }
}

fn walk_stmts(stmts: &[HirStmt], f: &mut impl FnMut(&HirExpr)) {
    for s in stmts {
        walk_stmt(s, f);
    }
}

fn walk_stmt(stmt: &HirStmt, f: &mut impl FnMut(&HirExpr)) {
    match stmt {
        HirStmt::Let { value, .. } => walk_expr(value, f),
        HirStmt::Assign { target, value, .. } => {
            walk_expr(target, f);
            walk_expr(value, f);
        }
        HirStmt::Return { value: Some(v), .. } => walk_expr(v, f),
        HirStmt::Return { value: None, .. } => {}
        HirStmt::Expr { expr, .. } => walk_expr(expr, f),
        HirStmt::While {
            condition, body, ..
        } => {
            walk_expr(condition, f);
            walk_stmts(body, f);
        }
        HirStmt::Loop { body, .. } => walk_stmts(body, f),
        HirStmt::Break { .. } | HirStmt::Continue { .. } => {}
    }
}

fn walk_expr(expr: &HirExpr, f: &mut impl FnMut(&HirExpr)) {
    f(expr);
    match expr {
        HirExpr::ObjectLit(fields, _) => {
            for (_, v) in fields {
                walk_expr(v, f);
            }
        }
        HirExpr::ListLit(items, _) | HirExpr::TupleLit(items, _) => {
            for it in items {
                walk_expr(it, f);
            }
        }
        HirExpr::Binary(_, l, r, _) => {
            walk_expr(l.as_ref(), f);
            walk_expr(r.as_ref(), f);
        }
        HirExpr::Unary(_, o, _) => walk_expr(o.as_ref(), f),
        HirExpr::Call(callee, args, _, _) => {
            walk_expr(callee.as_ref(), f);
            for a in args {
                walk_arg(a, f);
            }
        }
        HirExpr::MethodCall(obj, _, args, _, _) => {
            walk_expr(obj.as_ref(), f);
            for a in args {
                walk_arg(a, f);
            }
        }
        HirExpr::FieldAccess(o, _, _) => walk_expr(o.as_ref(), f),
        HirExpr::Match(subj, arms, _) => {
            walk_expr(subj.as_ref(), f);
            for arm in arms {
                if let Some(g) = &arm.guard {
                    walk_expr(g.as_ref(), f);
                }
                walk_expr(arm.body.as_ref(), f);
            }
        }
        HirExpr::If(cond, then_b, else_b, _) => {
            walk_expr(cond.as_ref(), f);
            walk_stmts(then_b, f);
            if let Some(else_stmts) = else_b {
                walk_stmts(else_stmts, f);
            }
        }
        HirExpr::For(_, _, it, body, _, _) => {
            walk_expr(it.as_ref(), f);
            walk_expr(body.as_ref(), f);
        }
        HirExpr::Lambda(_, _, body, _, _) => walk_expr(body.as_ref(), f),
        HirExpr::Spawn(t, _) => walk_expr(t.as_ref(), f),
        HirExpr::With(b, o, _) => {
            walk_expr(b.as_ref(), f);
            walk_expr(o.as_ref(), f);
        }
        HirExpr::Jsx(el) => {
            for a in &el.attributes {
                walk_expr(&a.value, f);
            }
            for c in &el.children {
                walk_expr(c, f);
            }
        }
        HirExpr::JsxSelfClosing(el) => {
            for a in &el.attributes {
                walk_expr(&a.value, f);
            }
        }
        HirExpr::JsxFragment(children, _) => {
            for c in children {
                walk_expr(c, f);
            }
        }
        HirExpr::Block(stmts, _) => walk_stmts(stmts, f),
        HirExpr::Try(t) => walk_expr(t.target.as_ref(), f),
        HirExpr::Index(obj, idx, _) => {
            walk_expr(obj.as_ref(), f);
            walk_expr(idx.as_ref(), f);
        }
        HirExpr::AsyncView(v) => {
            if let Some(arm) = &v.fetching_arm {
                walk_expr(arm, f);
            }
            if let Some(arm) = &v.empty_arm {
                walk_expr(arm, f);
            }
            if let Some(arm) = &v.error_arm {
                walk_expr(arm, f);
            }
            if let Some(arm) = &v.ok_arm {
                walk_expr(arm, f);
            }
        }
        HirExpr::IntLit(..)
        | HirExpr::FloatLit(..)
        | HirExpr::StringLit(..)
        | HirExpr::BoolLit(..)
        | HirExpr::Ident(..)
        | HirExpr::DecimalLit(..)
        | HirExpr::WorkflowVersion(_) => {}
    }
}

fn walk_arg(arg: &HirArg, f: &mut impl FnMut(&HirExpr)) {
    walk_expr(&arg.value, f);
}

fn walk_stmts_mut(stmts: &mut [HirStmt], f: &mut impl FnMut(&mut HirExpr)) {
    for s in stmts {
        walk_stmt_mut(s, f);
    }
}

fn walk_stmt_mut(stmt: &mut HirStmt, f: &mut impl FnMut(&mut HirExpr)) {
    match stmt {
        HirStmt::Let { value, .. } => walk_expr_mut(value, f),
        HirStmt::Assign { target, value, .. } => {
            walk_expr_mut(target, f);
            walk_expr_mut(value, f);
        }
        HirStmt::Return { value: Some(v), .. } => walk_expr_mut(v, f),
        HirStmt::Return { value: None, .. } => {}
        HirStmt::Expr { expr, .. } => walk_expr_mut(expr, f),
        HirStmt::While {
            condition, body, ..
        } => {
            walk_expr_mut(condition, f);
            walk_stmts_mut(body, f);
        }
        HirStmt::Loop { body, .. } => walk_stmts_mut(body, f),
        HirStmt::Break { .. } | HirStmt::Continue { .. } => {}
    }
}

fn walk_expr_mut(expr: &mut HirExpr, f: &mut impl FnMut(&mut HirExpr)) {
    f(expr);
    match expr {
        HirExpr::ObjectLit(fields, _) => {
            for (_, v) in fields {
                walk_expr_mut(v, f);
            }
        }
        HirExpr::ListLit(items, _) | HirExpr::TupleLit(items, _) => {
            for it in items {
                walk_expr_mut(it, f);
            }
        }
        HirExpr::Binary(_, l, r, _) => {
            walk_expr_mut(l.as_mut(), f);
            walk_expr_mut(r.as_mut(), f);
        }
        HirExpr::Unary(_, o, _) => walk_expr_mut(o.as_mut(), f),
        HirExpr::Call(callee, args, _, _) => {
            walk_expr_mut(callee.as_mut(), f);
            for a in args {
                walk_arg_mut(a, f);
            }
        }
        HirExpr::MethodCall(obj, _, args, _, _) => {
            walk_expr_mut(obj.as_mut(), f);
            for a in args {
                walk_arg_mut(a, f);
            }
        }
        HirExpr::FieldAccess(o, _, _) => walk_expr_mut(o.as_mut(), f),
        HirExpr::Match(subj, arms, _) => {
            walk_expr_mut(subj.as_mut(), f);
            for arm in arms {
                if let Some(g) = &mut arm.guard {
                    walk_expr_mut(g.as_mut(), f);
                }
                walk_expr_mut(arm.body.as_mut(), f);
            }
        }
        HirExpr::If(cond, then_b, else_b, _) => {
            walk_expr_mut(cond.as_mut(), f);
            walk_stmts_mut(then_b, f);
            if let Some(else_stmts) = else_b {
                walk_stmts_mut(else_stmts, f);
            }
        }
        HirExpr::For(_, _, it, body, _, _) => {
            walk_expr_mut(it.as_mut(), f);
            walk_expr_mut(body.as_mut(), f);
        }
        HirExpr::Lambda(_, _, body, _, _) => walk_expr_mut(body.as_mut(), f),

        HirExpr::Spawn(t, _) => walk_expr_mut(t.as_mut(), f),
        HirExpr::With(b, o, _) => {
            walk_expr_mut(b.as_mut(), f);
            walk_expr_mut(o.as_mut(), f);
        }
        HirExpr::Jsx(el) => {
            for a in &mut el.attributes {
                walk_expr_mut(&mut a.value, f);
            }
            for c in &mut el.children {
                walk_expr_mut(c, f);
            }
        }
        HirExpr::JsxSelfClosing(el) => {
            for a in &mut el.attributes {
                walk_expr_mut(&mut a.value, f);
            }
        }
        HirExpr::JsxFragment(children, _) => {
            for c in children {
                walk_expr_mut(c, f);
            }
        }
        HirExpr::Block(stmts, _) => walk_stmts_mut(stmts, f),
        HirExpr::Try(t) => walk_expr_mut(t.target.as_mut(), f),
        HirExpr::Index(obj, idx, _) => {
            walk_expr_mut(obj.as_mut(), f);
            walk_expr_mut(idx.as_mut(), f);
        }
        HirExpr::AsyncView(v) => {
            if let Some(arm) = &mut v.fetching_arm {
                walk_expr_mut(arm, f);
            }
            if let Some(arm) = &mut v.empty_arm {
                walk_expr_mut(arm, f);
            }
            if let Some(arm) = &mut v.error_arm {
                walk_expr_mut(arm, f);
            }
            if let Some(arm) = &mut v.ok_arm {
                walk_expr_mut(arm, f);
            }
        }
        HirExpr::IntLit(..)
        | HirExpr::FloatLit(..)
        | HirExpr::StringLit(..)
        | HirExpr::BoolLit(..)
        | HirExpr::Ident(..)
        | HirExpr::DecimalLit(..)
        | HirExpr::WorkflowVersion(_) => {}
    }
}

fn walk_arg_mut(arg: &mut HirArg, f: &mut impl FnMut(&mut HirExpr)) {
    walk_expr_mut(&mut arg.value, f);
}
