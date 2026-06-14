use crate::hir::*;
use std::collections::HashSet;
use crate::ast::span::Span;

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
    for m in &module.mcp_resources {
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

    for p in &module.providers {
        visit_fn(&p.func, &mut used);
    }

    // Now emit warnings for anything unused and not public
    for f in &module.functions {
        if !f.is_pub && f.name != "main" && !used.contains(&f.name) {
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
        HirStmt::While { condition, body, .. } => {
            visit_expr(condition, used);
            for s in body {
                visit_stmt(s, used);
            }
        }
        HirStmt::Loop { body, .. } => {
            for s in body {
                visit_stmt(s, used);
            }
        }
        HirStmt::Break { .. } | HirStmt::Continue { .. } => {}
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
        HirExpr::MethodCall(obj, _, args, _, _) => {
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
        HirExpr::For(_, _, iter, body, _, _) => {
            visit_expr(iter, used);
            visit_expr(body, used);
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
        HirExpr::Lambda(_, _, body, _, _) => {
            visit_expr(body, used);
        }
        HirExpr::Spawn(target, _) | HirExpr::Await(target, _) => {
            visit_expr(target, used);
        }
        HirExpr::With(body, opts, _) => {
            visit_expr(body, used);
            visit_expr(opts, used);
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
        HirExpr::Index(obj, idx, _) => {
            visit_expr(obj, used);
            visit_expr(idx, used);
        }
        _ => {}
    }
}

#[cfg(test)]
mod semcov_wave1c_tests {
    #![allow(unused_imports)]
    use super::*;
    use crate::ast::span::Span;
    use crate::hir::*;

    #[test]
    fn private_unused_function_is_flagged_dead() {
        let f = crate::hir::HirFn {
            id: crate::hir::DefId(0),
            name: "foo".into(),
            generics: vec![],
            params: vec![],
            return_type: None,
            body: vec![],
            is_async: false,
            is_pub: false,
            is_mobile_native: false,
            is_pure: false,
            is_reactive: false,
            is_versioned: false,
            is_remote: false,
            is_llm: false,
            llm_model: None,
            ai_structured_output: None,
            ai_fixture: None,
            embed: None,
            is_deprecated: false,
            schedule_interval: None,
            durability: None,
            actor_state_fields: vec![],
            capabilities: vec![],
            postconditions: vec![],
            ts_extern_module: None,
            generated_hash: None,
            span: crate::ast::span::Span::new(0, 1),
            inference_model: None,
            training_step: true,
            distributed_train: None,
        };
        let module = HirModule {
            functions: vec![f],
            ..Default::default()
        };

        let warnings = check_dead_code(&module);

        assert_eq!(warnings.len(), 1, "exactly one dead-code warning expected");
        assert_eq!(warnings[0].0, "function `foo` is never used");
    }

    #[test]
    fn unused_private_fn_is_reported_as_dead_code() {
        let dead = HirFn {
            id: DefId(0),
            name: "helper".into(),
            generics: vec![],
            params: vec![],
            return_type: None,
            body: vec![],
            is_async: false,
            is_pub: false,
            is_mobile_native: false,
            is_pure: false,
            is_reactive: false,
            is_versioned: false,
            is_remote: false,
            is_llm: false,
            llm_model: None,
            ai_structured_output: None,
            ai_fixture: None,
            embed: None,
            is_deprecated: false,
            schedule_interval: None,
            durability: None,
            actor_state_fields: vec![],
            capabilities: vec![],
            postconditions: vec![],
            ts_extern_module: None,
            generated_hash: None,
            span: Span::new(0, 1),
            inference_model: None,
            training_step: false,
            distributed_train: None,
        };
        let module = HirModule {
            functions: vec![dead],
            ..Default::default()
        };
        let warnings = check_dead_code(&module);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].0, "function `helper` is never used");
        assert_eq!(warnings[0].1, Span::new(0, 1));
    }

    #[test]
    fn pub_fn_and_empty_module_are_not_dead_code() {
        // A pub function is never flagged even if unused.
        let exported = HirFn {
            id: DefId(0),
            name: "api".into(),
            generics: vec![],
            params: vec![],
            return_type: None,
            body: vec![],
            is_async: false,
            is_pub: true,
            is_mobile_native: false,
            is_pure: false,
            is_reactive: false,
            is_versioned: false,
            is_remote: false,
            is_llm: false,
            llm_model: None,
            ai_structured_output: None,
            ai_fixture: None,
            embed: None,
            is_deprecated: false,
            schedule_interval: None,
            durability: None,
            actor_state_fields: vec![],
            capabilities: vec![],
            postconditions: vec![],
            ts_extern_module: None,
            generated_hash: None,
            span: Span::new(0, 1),
            inference_model: None,
            training_step: false,
            distributed_train: None,
        };
        let module = HirModule {
            functions: vec![exported],
            ..Default::default()
        };
        assert!(check_dead_code(&module).is_empty());
        // An entirely empty module yields no warnings.
        assert!(check_dead_code(&HirModule::default()).is_empty());
    }
}
