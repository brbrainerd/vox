//! P2-T2 acceptance: `workflow.version("change-id", min, max)` parses and lowers correctly.

use vox_compiler::ast::decl::Module;
use vox_compiler::ast::expr::{Expr, WorkflowVersionCall};
use vox_compiler::ast::stmt::Stmt;
use vox_compiler::hir::{HirExpr, HirStmt, HirWorkflowVersion, lower_module};
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;

fn parse_src(src: &str) -> Module {
    parse(lex(src)).expect("parse")
}

fn collect_workflow_version_calls(m: &Module) -> Vec<WorkflowVersionCall> {
    let mut out = Vec::new();
    for decl in &m.declarations {
        if let vox_compiler::ast::decl::Decl::Workflow(w) = decl {
            for stmt in &w.body {
                collect_from_stmt(stmt, &mut out);
            }
        }
    }
    out
}

fn collect_from_stmt(stmt: &Stmt, out: &mut Vec<WorkflowVersionCall>) {
    match stmt {
        Stmt::Let { value, .. } => collect_from_expr(value, out),
        Stmt::Expr { expr, .. } => collect_from_expr(expr, out),
        Stmt::Return { value: Some(e), .. } => collect_from_expr(e, out),
        _ => {}
    }
}

fn collect_from_expr(expr: &Expr, out: &mut Vec<WorkflowVersionCall>) {
    if let Expr::WorkflowVersion(call) = expr {
        out.push(call.clone());
    }
}

#[test]
fn parses_workflow_version_call_with_min_max() {
    let src = r#"
        workflow wf() to int {
            let v = workflow.version("change-1", 1, 2)
            return 0
        }
    "#;
    let module = parse_src(src);
    let calls = collect_workflow_version_calls(&module);
    assert_eq!(
        calls.len(),
        1,
        "should find exactly one workflow.version call"
    );
    assert_eq!(calls[0].change_id, "change-1");
    assert_eq!(calls[0].min, 1);
    assert_eq!(calls[0].max, 2);
}

#[test]
fn workflow_version_lowers_to_hir() {
    let src = r#"
        workflow wf() to int {
            let v = workflow.version("change-1", 1, 2)
            return 0
        }
    "#;
    let module = parse_src(src);
    let hir = lower_module(&module);
    let wf = hir
        .functions
        .iter()
        .find(|f| f.name == "wf")
        .expect("workflow wf present");
    let has_version = wf.body.iter().any(|stmt| {
        if let HirStmt::Let { value, .. } = stmt {
            matches!(value, HirExpr::WorkflowVersion(HirWorkflowVersion { change_id, .. }) if change_id == "change-1")
        } else {
            false
        }
    });
    assert!(
        has_version,
        "HIR body should contain HirExpr::WorkflowVersion"
    );
}
