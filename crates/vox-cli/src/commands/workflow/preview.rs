//! `vox workflow preview` — dry-run schedule projection (P1-T8).
//!
//! Runs lex → parse → lower → typeck on the target file, then walks the named
//! workflow's HIR body to produce a flat list of "would-call" activities.
//! No I/O is performed; user-supplied args appear as opaque placeholders.

use anyhow::{Context, Result};
use owo_colors::OwoColorize;
use serde::Serialize;
use std::path::PathBuf;

use vox_compiler::hir::nodes::durability::DurabilityKind;
use vox_compiler::hir::{HirExpr, HirModule, HirStmt};

#[derive(clap::Args, Debug)]
pub struct WorkflowPreviewArgs {
    /// Workflow target: `path/to/file.vox::workflow_name`.
    pub target: String,
    /// Emit JSON instead of human-readable text.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Serialize)]
pub struct PreviewedActivity {
    pub name: String,
    pub is_remote: bool,
    pub durability: String,
    pub children: Vec<PreviewedActivity>,
}

#[derive(Debug, Serialize)]
pub struct PreviewedWorkflow {
    pub workflow: String,
    pub steps: Vec<PreviewedActivity>,
}

/// Project a workflow schedule from a Vox source string (used in tests).
pub fn project_workflow_from_source(source: &str, wf_name: &str) -> Result<PreviewedWorkflow> {
    use vox_compiler::hir::lower_module;
    use vox_compiler::lexer::cursor::lex;
    use vox_compiler::parser::parse;
    let tokens = lex(source);
    let module = parse(tokens).map_err(|_| anyhow::anyhow!("parse error"))?;
    let hir = lower_module(&module);
    project_workflow(&hir, wf_name)
}

pub async fn run(args: &WorkflowPreviewArgs) -> Result<()> {
    let (path, wf_name) =
        parse_target(&args.target).with_context(|| format!("invalid target `{}`", args.target))?;
    let result = crate::pipeline::run_frontend(&path, false).await?;
    if result.has_errors() {
        anyhow::bail!(
            "preview aborted: {} error(s) in {}",
            result.error_count(),
            path.display()
        );
    }
    let projected = project_workflow(&result.hir, &wf_name)
        .with_context(|| format!("workflow `{wf_name}` not found in {}", path.display()))?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&projected)?);
    } else {
        render_tree(&projected);
    }
    Ok(())
}

fn parse_target(t: &str) -> Result<(PathBuf, String)> {
    let (path_str, fn_name) = t
        .rsplit_once("::")
        .ok_or_else(|| anyhow::anyhow!("expected `path/to/file.vox::workflow_name`, got `{t}`"))?;
    Ok((PathBuf::from(path_str), fn_name.to_string()))
}

fn project_workflow(hir: &HirModule, wf_name: &str) -> Result<PreviewedWorkflow> {
    let wf = hir
        .functions
        .iter()
        .find(|f| f.name == wf_name)
        .ok_or_else(|| anyhow::anyhow!("workflow `{wf_name}` not found"))?;
    if !matches!(wf.durability, Some(DurabilityKind::Workflow)) {
        anyhow::bail!("function `{wf_name}` is not a `workflow`");
    }
    let steps = walk_stmts(&wf.body, hir);
    Ok(PreviewedWorkflow {
        workflow: wf_name.to_string(),
        steps,
    })
}

fn walk_stmts(stmts: &[HirStmt], hir: &HirModule) -> Vec<PreviewedActivity> {
    let mut out = Vec::new();
    for s in stmts {
        match s {
            HirStmt::Expr { expr, .. }
            | HirStmt::Let { value: expr, .. }
            | HirStmt::Assign { value: expr, .. } => walk_expr(expr, hir, &mut out),
            HirStmt::Return { value: Some(e), .. } => walk_expr(e, hir, &mut out),
            HirStmt::While { body, .. } | HirStmt::Loop { body, .. } => {
                out.extend(walk_stmts(body, hir));
            }
            _ => {}
        }
    }
    out
}

fn walk_expr(e: &HirExpr, hir: &HirModule, out: &mut Vec<PreviewedActivity>) {
    match e {
        HirExpr::Call(callee, args, _, _) => {
            if let HirExpr::Ident(name, _) = callee.as_ref() {
                if let Some(target) = hir.functions.iter().find(|f| &f.name == name) {
                    let is_activity = target.is_remote
                        || matches!(target.durability, Some(DurabilityKind::Activity))
                        || name.starts_with("__side_effect_");
                    if is_activity {
                        let children = walk_stmts(&target.body, hir);
                        out.push(PreviewedActivity {
                            name: target.name.clone(),
                            is_remote: target.is_remote,
                            durability: target
                                .durability
                                .as_ref()
                                .map(|d| d.label().to_string())
                                .unwrap_or_else(|| "fn".to_string()),
                            children,
                        });
                    }
                }
            }
            for a in args {
                walk_expr(&a.value, hir, out);
            }
        }
        HirExpr::Block(stmts, _) => {
            out.extend(walk_stmts(stmts, hir));
        }
        HirExpr::If(cond, then_stmts, else_stmts, _) => {
            walk_expr(cond, hir, out);
            out.extend(walk_stmts(then_stmts, hir));
            if let Some(els) = else_stmts {
                out.extend(walk_stmts(els, hir));
            }
        }
        _ => {}
    }
}

fn render_tree(p: &PreviewedWorkflow) {
    println!("{} {}", "workflow".green().bold(), p.workflow.bold());
    println!("  {}:", "schedule".bold());
    if p.steps.is_empty() {
        println!("    (no activities)");
    } else {
        for s in &p.steps {
            render_step(s, 4);
        }
    }
}

fn render_step(s: &PreviewedActivity, indent: usize) {
    let pad = " ".repeat(indent);
    let where_ = if s.is_remote {
        "mesh".cyan().to_string()
    } else {
        s.durability.dimmed().to_string()
    };
    println!("{pad}- {} [{}]", s.name.bold(), where_);
    for c in &s.children {
        render_step(c, indent + 4);
    }
}
