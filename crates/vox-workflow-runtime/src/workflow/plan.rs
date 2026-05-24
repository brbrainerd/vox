//! Walk HIR to build a linear activity plan from workflow statements.

use anyhow::Context;
use std::collections::HashSet;
use vox_compiler::hir::{HirBinOp, HirExpr, HirModule, HirStmt, HirUnOp, HirWorkflowVersion};

use super::types::{PlannedActivity, PopuliHttpOp, ReplayNode, WorkflowReplayIr};

/// Extract ordered activity call names from a workflow's statements.
pub fn plan_workflow_activities(
    hir: &HirModule,
    workflow_name: &str,
) -> anyhow::Result<Vec<PlannedActivity>> {
    let ir = plan_workflow_replay_ir(hir, workflow_name)?;
    Ok(ir
        .nodes
        .into_iter()
        .filter_map(|node| match node {
            ReplayNode::Activity(activity) => Some(activity),
            ReplayNode::WorkflowPatch { .. } => None,
        })
        .collect())
}

/// Build replay-oriented linear IR from workflow statements.
///
/// Locates the named workflow in the HIR (by `DurabilityKind::Workflow`),
/// then walks its statements to extract a linear activity call sequence.
/// Only calls to functions declared as `activity` (DurabilityKind::Activity)
/// are planned; helper/pure functions in the same pool are ignored.
///
/// Returns `Err` if the workflow is not found or the body contains
/// unsupported constructs for deterministic replay.
pub fn plan_workflow_replay_ir(
    hir: &HirModule,
    workflow_name: &str,
) -> anyhow::Result<WorkflowReplayIr> {
    use vox_compiler::hir::nodes::DurabilityKind;

    // Build a set of declared activity names so the planner skips plain helpers.
    let activity_names: HashSet<&str> = hir
        .functions
        .iter()
        .filter(|f| f.durability == Some(DurabilityKind::Activity))
        .map(|f| f.name.as_str())
        .collect();

    let wf = hir
        .functions
        .iter()
        .find(|f| f.durability == Some(DurabilityKind::Workflow) && f.name == workflow_name)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "workflow `{workflow_name}` not found in HIR; \
                 declare it with the `workflow` keyword"
            )
        })?;

    let mut out = Vec::new();
    let ctx = ActivityWithOpts::default();
    let mut branch_counter = 0usize;
    collect_activity_calls_from_stmts(
        workflow_name,
        &wf.body,
        &ctx,
        &activity_names,
        &mut out,
        &mut branch_counter,
    )?;

    Ok(WorkflowReplayIr { nodes: out })
}

#[derive(Clone, Default, Debug)]
struct ActivityWithOpts {
    activity_id: Option<String>,
    timeout_ms: Option<u64>,
    mesh_key: Option<String>,
    retries: u32,
    initial_backoff_ms: Option<u64>,
    required_labels: Option<Vec<String>>,
    is_detached: bool,
}

impl ActivityWithOpts {
    fn merged_with(&self, opts: &HirExpr) -> anyhow::Result<Self> {
        let mut n = self.clone();
        let HirExpr::ObjectLit(fields, _) = opts else {
            return Ok(n);
        };
        for (k, v) in fields {
            match k.as_str() {
                "activity_id" | "id" => {
                    if let HirExpr::StringLit(s, _) = v {
                        n.activity_id = Some(s.clone());
                    }
                }
                "timeout" => {
                    n.timeout_ms = Some(parse_timeout_ms(v)?);
                }
                "mens" => {
                    if let HirExpr::StringLit(s, _) = v {
                        n.mesh_key = Some(s.clone());
                    }
                }
                "retries" => {
                    n.retries = parse_retries(v)?;
                }
                "initial_backoff" => {
                    n.initial_backoff_ms = Some(parse_timeout_ms(v)?);
                }
                "labels" => {
                    if let HirExpr::ListLit(items, _) = v {
                        let mut labels = Vec::new();
                        for it in items {
                            if let HirExpr::StringLit(s, _) = it {
                                labels.push(s.clone());
                            }
                        }
                        n.required_labels = Some(labels);
                    }
                }
                "detach" => {
                    if let HirExpr::BoolLit(b, _) = v {
                        n.is_detached = *b;
                    }
                }
                _ => {}
            }
        }
        Ok(n)
    }
}

fn parse_timeout_ms(expr: &HirExpr) -> anyhow::Result<u64> {
    match expr {
        HirExpr::IntLit(ms, _) if *ms >= 0 => Ok(*ms as u64),
        HirExpr::IntLit(_, _) => {
            anyhow::bail!("workflow `timeout` must be a non-negative integer")
        }
        HirExpr::StringLit(s, _) => {
            parse_duration_ms_str(s).with_context(|| format!("invalid workflow timeout {:?}", s))
        }
        _ => anyhow::bail!("workflow `timeout` must be an int (milliseconds) or duration string"),
    }
}

fn parse_retries(expr: &HirExpr) -> anyhow::Result<u32> {
    match expr {
        HirExpr::IntLit(n, _) if *n >= 0 => {
            u32::try_from(*n).map_err(|_| anyhow::anyhow!("workflow `retries` is too large"))
        }
        HirExpr::IntLit(_, _) => anyhow::bail!("workflow `retries` must be a non-negative integer"),
        _ => anyhow::bail!("workflow `retries` must be an int"),
    }
}

/// Adapt the shared SSOT duration parser to the `u64`-millisecond shape the
/// planner stores. Delegates to
/// [`crate::duration_literal::parse_duration_str`] so the runtime and the
/// `vox-codegen` emit agree on every unit (ms / s / m / h / d) and on error
/// messages (ADR-041 M-7).
///
/// **Behavior note (post-M-7 consolidation, 2026-05-24):** a bare integer like
/// `workflow_wait(30)` is now interpreted as 30 **seconds** (the cron-style
/// ergonomic default the shared parser uses), where the legacy local parser
/// here interpreted it as 30 **milliseconds**. No golden uses the bare-integer
/// form so blast radius is zero, but if a workflow author wants milliseconds
/// they must write `workflow_wait("30ms")` explicitly.
fn parse_duration_ms_str(s: &str) -> anyhow::Result<u64> {
    crate::duration_literal::parse_duration_str(s)
        .map(|d| d.as_millis().min(u64::MAX as u128) as u64)
        .map_err(|e| anyhow::anyhow!("{e}"))
}

fn parse_populi_control_op(s: &str) -> anyhow::Result<PopuliHttpOp> {
    match s.trim() {
        "noop" => Ok(PopuliHttpOp::Noop),
        "join" => Ok(PopuliHttpOp::Join),
        "snapshot" => Ok(PopuliHttpOp::Snapshot),
        "heartbeat" => Ok(PopuliHttpOp::Heartbeat),
        "dispatch" => Ok(PopuliHttpOp::Dispatch),
        "wait" => Ok(PopuliHttpOp::Wait),
        other => anyhow::bail!(
            "unknown workflow mens control {:?}; expected noop|join|snapshot|heartbeat|dispatch|wait",
            other
        ),
    }
}

fn resolve_populi_http_op(name: &str, mesh_key: Option<&str>) -> anyhow::Result<PopuliHttpOp> {
    if let Some(k) = mesh_key {
        return parse_populi_control_op(k);
    }
    match name {
        "mesh_noop" => Ok(PopuliHttpOp::Noop),
        "mesh_join" => Ok(PopuliHttpOp::Join),
        "mesh_snapshot" => Ok(PopuliHttpOp::Snapshot),
        "mesh_dispatch" => Ok(PopuliHttpOp::Dispatch),
        "mesh_wait" => Ok(PopuliHttpOp::Wait),
        _ if name.starts_with("mesh_") => Ok(PopuliHttpOp::Heartbeat),
        _ => Ok(PopuliHttpOp::Heartbeat),
    }
}

fn collect_activity_calls_from_stmts(
    workflow_name: &str,
    stmts: &[HirStmt],
    ctx: &ActivityWithOpts,
    activity_names: &HashSet<&str>,
    out: &mut Vec<ReplayNode>,
    branch_counter: &mut usize,
) -> anyhow::Result<()> {
    for s in stmts {
        match s {
            HirStmt::Let { value, .. } => collect_from_expr(
                workflow_name,
                value,
                ctx,
                activity_names,
                out,
                branch_counter,
            )?,
            HirStmt::Assign { value, .. } => collect_from_expr(
                workflow_name,
                value,
                ctx,
                activity_names,
                out,
                branch_counter,
            )?,
            HirStmt::Return { value, .. } => {
                if let Some(v) = value {
                    collect_from_expr(workflow_name, v, ctx, activity_names, out, branch_counter)?;
                }
            }
            HirStmt::Expr { expr, .. } => collect_from_expr(
                workflow_name,
                expr,
                ctx,
                activity_names,
                out,
                branch_counter,
            )?,
            HirStmt::While { .. } | HirStmt::Loop { .. } => {
                anyhow::bail!(
                    "workflow `{workflow_name}`: interpreted durable planning does not support `while` or `loop` statements"
                )
            }
            HirStmt::Break { .. } | HirStmt::Continue { .. } => {
                anyhow::bail!(
                    "workflow `{workflow_name}`: interpreted durable planning does not support `break` or `continue`"
                )
            }
        }
    }
    Ok(())
}

fn collect_from_expr(
    workflow_name: &str,
    expr: &HirExpr,
    ctx: &ActivityWithOpts,
    activity_names: &HashSet<&str>,
    out: &mut Vec<ReplayNode>,
    branch_counter: &mut usize,
) -> anyhow::Result<()> {
    match expr {
        HirExpr::With(inner, opts, _) => {
            let merged = ctx.merged_with(opts)?;
            collect_from_expr(
                workflow_name,
                inner,
                &merged,
                activity_names,
                out,
                branch_counter,
            )?;
        }
        HirExpr::Call(callee, args, _, _) => {
            if let HirExpr::Ident(name, _) = &**callee {
                // Always traverse args first so nested activity/mesh calls inside
                // helper(send_email()) or charge(render_invoice()) are not silently
                // dropped from the replay plan.
                for arg in args {
                    collect_from_expr(
                        workflow_name,
                        &arg.value,
                        ctx,
                        activity_names,
                        out,
                        branch_counter,
                    )?;
                }

                if name == "workflow_wait" {
                    let wait_ms = parse_workflow_wait_ms(args)?;
                    out.push(ReplayNode::Activity(PlannedActivity {
                        name: "__durable_timer_wait".to_string(),
                        mens: false,
                        activity_id: ctx.activity_id.clone(),
                        timeout_ms: Some(wait_ms),
                        retries: 0,
                        initial_backoff_ms: None,
                        populi_op: PopuliHttpOp::Noop,
                        required_labels: None,
                        is_detached: false,
                        arguments: vec![],
                        dedup_window_ms: None,
                    }));
                    return Ok(());
                }
                if name == "workflow_wait_signal" {
                    let signal_key = parse_workflow_signal_key(args)?;
                    out.push(ReplayNode::Activity(PlannedActivity {
                        name: format!("__durable_signal_wait:{signal_key}"),
                        mens: false,
                        activity_id: ctx.activity_id.clone(),
                        timeout_ms: None,
                        retries: ctx.retries,
                        initial_backoff_ms: ctx.initial_backoff_ms,
                        populi_op: PopuliHttpOp::Noop,
                        required_labels: None,
                        is_detached: false,
                        arguments: vec![],
                        dedup_window_ms: None,
                    }));
                    return Ok(());
                }
                // Only plan calls to declared activities (DurabilityKind::Activity) or
                // built-in mesh_* ops. Plain helper/pure functions share the same pool
                // and must not be recorded as durable steps.
                let is_mesh = name.starts_with("mesh_");
                if !is_mesh && !activity_names.contains(name.as_str()) {
                    return Ok(());
                }
                if ctx.mesh_key.is_some() && !is_mesh {
                    anyhow::bail!(
                        "workflow `{workflow_name}`: `mens` in `with {{ … }}` only applies to mesh_* activities (got `{name}`)"
                    );
                }
                let populi_op = resolve_populi_http_op(name, ctx.mesh_key.as_deref())?;
                out.push(ReplayNode::Activity(PlannedActivity {
                    name: name.clone(),
                    mens: is_mesh,
                    activity_id: ctx.activity_id.clone(),
                    timeout_ms: ctx.timeout_ms,
                    retries: ctx.retries,
                    initial_backoff_ms: ctx.initial_backoff_ms,
                    populi_op,
                    required_labels: ctx.required_labels.clone(),
                    is_detached: ctx.is_detached,
                    arguments: vec![],
                    dedup_window_ms: None,
                }));
            } else {
                collect_from_expr(
                    workflow_name,
                    callee,
                    ctx,
                    activity_names,
                    out,
                    branch_counter,
                )?;
                for arg in args {
                    collect_from_expr(
                        workflow_name,
                        &arg.value,
                        ctx,
                        activity_names,
                        out,
                        branch_counter,
                    )?;
                }
            }
        }
        HirExpr::If(cond, then_stmts, else_stmts, _) => {
            let take_then = eval_const_bool(cond).with_context(|| {
                format!(
                    "workflow `{workflow_name}`: interpreted durable planning supports `if` only when the condition is a deterministic literal expression (bool/int/string literals with basic logical/comparison operators)"
                )
            })?;
            let branch_id = *branch_counter;
            *branch_counter = branch_counter.saturating_add(1);
            out.push(ReplayNode::Activity(PlannedActivity {
                name: if take_then {
                    "__branch_decision_then".to_string()
                } else {
                    "__branch_decision_else".to_string()
                },
                mens: false,
                activity_id: Some(format!("{workflow_name}-branch-{branch_id}")),
                timeout_ms: None,
                retries: 0,
                initial_backoff_ms: None,
                populi_op: PopuliHttpOp::Noop,
                required_labels: None,
                is_detached: false,
                arguments: vec![],
                dedup_window_ms: None,
            }));
            if take_then {
                collect_activity_calls_from_stmts(
                    workflow_name,
                    then_stmts,
                    ctx,
                    activity_names,
                    out,
                    branch_counter,
                )?
            } else if let Some(else_branch) = else_stmts {
                collect_activity_calls_from_stmts(
                    workflow_name,
                    else_branch,
                    ctx,
                    activity_names,
                    out,
                    branch_counter,
                )?;
            }
        }
        HirExpr::Block(stmts, _) => collect_activity_calls_from_stmts(
            workflow_name,
            stmts,
            ctx,
            activity_names,
            out,
            branch_counter,
        )?,
        HirExpr::Binary(_, a, b, _) => {
            collect_from_expr(workflow_name, a, ctx, activity_names, out, branch_counter)?;
            collect_from_expr(workflow_name, b, ctx, activity_names, out, branch_counter)?;
        }
        HirExpr::Unary(_, a, _) => {
            collect_from_expr(workflow_name, a, ctx, activity_names, out, branch_counter)?
        }
        HirExpr::Match(_, _, _) => anyhow::bail!(
            "workflow `{workflow_name}`: interpreted durable planning currently supports only linear activity plans; `match` branches are not replay-safe yet"
        ),
        HirExpr::MethodCall(recv, _, args, _, _) => {
            collect_from_expr(
                workflow_name,
                recv,
                ctx,
                activity_names,
                out,
                branch_counter,
            )?;
            for a in args {
                collect_from_expr(
                    workflow_name,
                    &a.value,
                    ctx,
                    activity_names,
                    out,
                    branch_counter,
                )?;
            }
        }
        HirExpr::FieldAccess(recv, _, _) => collect_from_expr(
            workflow_name,
            recv,
            ctx,
            activity_names,
            out,
            branch_counter,
        )?,
        HirExpr::Lambda(_, _, body, _, _) => collect_from_expr(
            workflow_name,
            body,
            ctx,
            activity_names,
            out,
            branch_counter,
        )?,
        HirExpr::For(_, _, iter, body, _, _) => {
            // Replay-safe bounded loops: literal lists are deterministic and can be unrolled.
            const MAX_STATIC_LOOP_UNROLL: usize = 64;
            match iter.as_ref() {
                HirExpr::ListLit(items, _) => {
                    if items.len() > MAX_STATIC_LOOP_UNROLL {
                        anyhow::bail!(
                            "workflow `{workflow_name}`: static `for` loop exceeds max unroll ({MAX_STATIC_LOOP_UNROLL})"
                        );
                    }
                    for _ in items {
                        collect_from_expr(
                            workflow_name,
                            body,
                            ctx,
                            activity_names,
                            out,
                            branch_counter,
                        )?;
                    }
                }
                _ => anyhow::bail!(
                    "workflow `{workflow_name}`: interpreted durable planning supports `for` only over literal list values (bounded deterministic replay)"
                ),
            }
        }
        HirExpr::Spawn(inner, _) => collect_from_expr(
            workflow_name,
            inner,
            ctx,
            activity_names,
            out,
            branch_counter,
        )?,
        // `?` operator: walk the inner expression so activity calls like
        // `charge_card(amount)?` are not silently dropped from the plan.
        // (The error-propagation semantics are interpreter-side; the planner
        // only cares about which activities run.)
        HirExpr::Try(t) => collect_from_expr(
            workflow_name,
            &t.target,
            ctx,
            activity_names,
            out,
            branch_counter,
        )?,
        HirExpr::ListLit(items, _) => {
            for it in items {
                collect_from_expr(workflow_name, it, ctx, activity_names, out, branch_counter)?;
            }
        }
        HirExpr::ObjectLit(fields, _) => {
            for (_, v) in fields {
                collect_from_expr(workflow_name, v, ctx, activity_names, out, branch_counter)?;
            }
        }
        HirExpr::WorkflowVersion(HirWorkflowVersion {
            change_id,
            min,
            max,
            ..
        }) => {
            out.push(super::types::ReplayNode::WorkflowPatch {
                change_id: change_id.clone(),
                min: *min,
                max: *max,
            });
        }
        _ => {}
    }
    Ok(())
}

fn parse_workflow_wait_ms(args: &[vox_compiler::hir::HirArg]) -> anyhow::Result<u64> {
    let Some(first) = args.first() else {
        anyhow::bail!("workflow_wait requires one duration argument");
    };
    parse_timeout_ms(&first.value)
}

fn parse_workflow_signal_key(args: &[vox_compiler::hir::HirArg]) -> anyhow::Result<String> {
    let Some(first) = args.first() else {
        anyhow::bail!("workflow_wait_signal requires one string signal key argument");
    };
    let HirExpr::StringLit(value, _) = &first.value else {
        anyhow::bail!("workflow_wait_signal key must be a string literal");
    };
    let key = value.trim();
    if key.is_empty() {
        anyhow::bail!("workflow_wait_signal key must not be empty");
    }
    Ok(key.to_string())
}

fn eval_const_bool(expr: &HirExpr) -> anyhow::Result<bool> {
    match expr {
        HirExpr::BoolLit(v, _) => Ok(*v),
        HirExpr::IntLit(v, _) => Ok(*v != 0),
        HirExpr::StringLit(v, _) => Ok(!v.is_empty()),
        HirExpr::Unary(HirUnOp::Not, inner, _) => Ok(!eval_const_bool(inner)?),
        HirExpr::Binary(op, lhs, rhs, _) => match op {
            HirBinOp::And => Ok(eval_const_bool(lhs)? && eval_const_bool(rhs)?),
            HirBinOp::Or => Ok(eval_const_bool(lhs)? || eval_const_bool(rhs)?),
            HirBinOp::Is => eval_const_eq(lhs, rhs),
            HirBinOp::Isnt => Ok(!eval_const_eq(lhs, rhs)?),
            HirBinOp::Lt => eval_const_ord(lhs, rhs, |a, b| a < b),
            HirBinOp::Lte => eval_const_ord(lhs, rhs, |a, b| a <= b),
            HirBinOp::Gt => eval_const_ord(lhs, rhs, |a, b| a > b),
            HirBinOp::Gte => eval_const_ord(lhs, rhs, |a, b| a >= b),
            _ => anyhow::bail!("unsupported binary operator in deterministic `if` condition"),
        },
        _ => anyhow::bail!("unsupported expression in deterministic `if` condition"),
    }
}

fn eval_const_eq(lhs: &HirExpr, rhs: &HirExpr) -> anyhow::Result<bool> {
    match (lhs, rhs) {
        (HirExpr::BoolLit(a, _), HirExpr::BoolLit(b, _)) => Ok(a == b),
        (HirExpr::IntLit(a, _), HirExpr::IntLit(b, _)) => Ok(a == b),
        (HirExpr::StringLit(a, _), HirExpr::StringLit(b, _)) => Ok(a == b),
        _ => anyhow::bail!("`is`/`isnt` supports only bool/int/string literal comparisons"),
    }
}

fn eval_const_ord(
    lhs: &HirExpr,
    rhs: &HirExpr,
    cmp: impl FnOnce(i64, i64) -> bool,
) -> anyhow::Result<bool> {
    match (lhs, rhs) {
        (HirExpr::IntLit(a, _), HirExpr::IntLit(b, _)) => Ok(cmp(*a, *b)),
        _ => anyhow::bail!("ordering comparisons support only int literals"),
    }
}
