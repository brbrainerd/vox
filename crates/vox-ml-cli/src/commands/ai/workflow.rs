use anyhow::{Context, Result};
use std::path::Path;
#[cfg(feature = "workflow-runtime")]
use std::sync::Arc;
#[cfg(feature = "workflow-runtime")]
use uuid::Uuid;
use vox_cli_core::cli_actions::WorkflowAction;

/// Dispatch workflow actions.
pub async fn run(action: WorkflowAction) -> Result<()> {
    match action {
        WorkflowAction::List { file } => list(&file).await,
        WorkflowAction::Inspect { file, name } => inspect(&file, &name).await,
        WorkflowAction::Check { file } => check(&file).await,
        WorkflowAction::Run {
            file,
            name,
            args,
            run_id,
            mesh,
        } => {
            run_workflow(
                &file,
                &name,
                args.as_deref().unwrap_or("[]"),
                run_id.as_deref(),
                mesh,
            )
            .await
        }
    }
}

/// List all workflows and activities defined in a .vox file.
///
/// Compiles the file, then reports all functions with `DurabilityKind::Workflow`
/// or `DurabilityKind::Activity`.
pub async fn list(file: &Path) -> Result<()> {
    use vox_compiler::hir::nodes::DurabilityKind;

    let result = crate::pipeline::run_frontend(file, false).await?;

    let workflows: Vec<_> = result
        .hir
        .functions
        .iter()
        .filter(|f| f.durability == Some(DurabilityKind::Workflow))
        .map(|f| f.name.as_str())
        .collect();
    let activities: Vec<_> = result
        .hir
        .functions
        .iter()
        .filter(|f| f.durability == Some(DurabilityKind::Activity))
        .map(|f| f.name.as_str())
        .collect();

    if workflows.is_empty() && activities.is_empty() {
        println!("No workflows or activities found in {}", file.display());
        println!("  Add an 'activity' or 'workflow' block to your .vox file.");
    } else {
        for name in &workflows {
            println!("workflow  {name}");
        }
        for name in &activities {
            println!("activity  {name}");
        }
    }

    Ok(())
}

/// Show type-checked info about a specific workflow.
pub async fn inspect(file: &Path, workflow_name: &str) -> Result<()> {
    use vox_compiler::hir::nodes::DurabilityKind;

    let result = crate::pipeline::run_frontend(file, false).await?;
    let errors = result.error_count();
    crate::pipeline::print_diagnostics(&result, file, false);
    if errors > 0 {
        anyhow::bail!("{} type error(s) found", errors);
    }
    let wf = result
        .hir
        .functions
        .iter()
        .find(|f| f.durability == Some(DurabilityKind::Workflow) && f.name == workflow_name)
        .ok_or_else(|| {
            anyhow::anyhow!("workflow `{workflow_name}` not found in {}", file.display())
        })?;

    let params: Vec<String> = wf
        .params
        .iter()
        .map(|p| format!("{}: {:?}", p.name, p.type_ann))
        .collect();
    let ret = wf
        .return_type
        .as_ref()
        .map(|t| format!("{:?}", t))
        .unwrap_or_else(|| "Unit".to_string());

    println!("workflow  {}", wf.name);
    println!("params    {}", wf.params.len());
    println!(
        "  Signature: workflow {}({}) to {}",
        wf.name,
        params.join(", "),
        ret
    );

    let activities: Vec<_> = result
        .hir
        .functions
        .iter()
        .filter(|f| f.durability == Some(DurabilityKind::Activity))
        .collect();

    println!("  Activities in this file: {}", activities.len());
    println!();

    if !activities.is_empty() {
        println!("  Available activities:");
        for act in &activities {
            let act_params: Vec<String> = act
                .params
                .iter()
                .map(|p| format!("{}: {:?}", p.name, p.type_ann))
                .collect();
            println!(
                "    {} ({}) \u{2014} callable with current 'with' options like retries, timeout, activity_id, mens",
                act.name,
                act_params.join(", ")
            );
        }
        println!();
    }

    println!("  Tip: current 'with' option handling:");
    println!(
        "    retries: int         \u{2014} honored for interpreted mesh_* activity execution; local interpreted steps are still journal-only no-ops"
    );
    println!(
        "    timeout: str         \u{2014} parsed today for interpreted runtime paths such as mesh_* activity execution"
    );
    println!("    activity_id: str     \u{2014} used for durable skip/resume and idempotency keys");
    println!("    id: str              \u{2014} alias for activity_id in the interpreted planner");
    println!(
        "    mens: str            \u{2014} selects mesh_* control op override such as \"join\" or \"snapshot\""
    );
    println!(
        "    initial_backoff: str \u{2014} honored for interpreted mesh_* retries; local interpreted steps do not execute user activity bodies yet"
    );

    Ok(())
}

/// Type-check a workflow file through the full Vox compiler pipeline.
pub async fn check(file: &Path) -> Result<()> {
    let result: crate::pipeline::FrontendResult = crate::pipeline::run_frontend(file, false)
        .await
        .map_err(|e| anyhow::anyhow!("Workflow check failed: {}", e))?;

    let warnings = result.warning_count();
    let errors = result.error_count();

    crate::pipeline::print_diagnostics(&result, file, false);

    if errors == 0 {
        println!("✓ {} — {} warning(s)", file.display(), warnings);
        Ok(())
    } else {
        anyhow::bail!("{} type error(s) found", errors)
    }
}

/// Execute a workflow by building the project and running the generated binary in workflow mode.
///
/// When the project has workflows, the generated main checks `VOX_RUN_WORKFLOW` at startup.
/// If set, it runs the named workflow and exits instead of starting the HTTP server.
/// Use `--args '["a","b",42]'` to pass JSON-typed workflow arguments.
pub async fn run_workflow(
    file: &Path,
    workflow_name: &str,
    args_json: &str,
    requested_run_id: Option<&str>,
    mesh: bool,
) -> Result<()> {
    if mesh {
        #[cfg(feature = "populi")]
        {
            let _ = vox_populi::publish_local_registry_best_effort();
            let _ = vox_populi::http_lifecycle::populi_http_join_best_effort(
                vox_populi::populi_registration_record_for_process(),
                "vox workflow run",
            )
            .await;
        }
    }

    use vox_compiler::hir::nodes::DurabilityKind;
    let result = crate::pipeline::run_frontend(file, false).await?;
    let errors = result.error_count();
    if errors > 0 {
        crate::pipeline::print_diagnostics(&result, file, false);
        anyhow::bail!("{} type error(s) found", errors);
    }
    let _wf = result
        .hir
        .functions
        .iter()
        .find(|f| f.durability == Some(DurabilityKind::Workflow) && f.name == workflow_name)
        .with_context(|| {
            format!(
                "Workflow '{}' not found in {}",
                workflow_name,
                file.display()
            )
        })?;

    if let Err(e) = serde_json::from_str::<Vec<serde_json::Value>>(args_json) {
        anyhow::bail!(
            "Invalid --args JSON (must be array, e.g. [\"a\",42]): {}",
            e
        );
    }

    #[cfg(feature = "workflow-runtime")]
    {
        use vox_cli_core::workflow_journal_codex;
        let run_id = resolve_workflow_run_id(workflow_name, requested_run_id)?;
        let resume_requested = requested_run_id.is_some();
        let db = Arc::new(
            vox_db::VoxDb::connect_default()
                .await
                .context("Failed to connect to VoxDB for workflow tracking")?,
        );
        let mut tracker = if let Some((plan_session_id, plan_node_id, plan_version)) =
            resolve_plan_context_from_env()
        {
            vox_workflow_runtime::VoxDbTracker::new(db, run_id.clone()).with_plan_context(
                plan_session_id,
                plan_node_id,
                plan_version,
            )
        } else {
            vox_workflow_runtime::VoxDbTracker::new(db, run_id.clone())
        };

        if resume_requested {
            println!(
                "Resuming interpreted workflow '{}' with run id '{}'.",
                workflow_name, run_id
            );
        } else {
            println!(
                "Starting interpreted workflow '{}' with new run id '{}'.",
                workflow_name, run_id
            );
        }

        let journal = vox_workflow_runtime::interpret_workflow_durable(
            &result.hir,
            workflow_name,
            &mut tracker,
        )
        .await?;
        for entry in &journal {
            workflow_journal_codex::persist_workflow_journal_entry_opt(workflow_name, entry).await;
        }
        println!(
            "{}",
            serde_json::to_string_pretty(&journal).unwrap_or_else(|_| "[]".to_string())
        );
        println!(
            "Workflow '{}' completed (interpreted runtime).",
            workflow_name
        );
        println!("Durable run id: {}", run_id);
        Ok(())
    }

    #[cfg(not(feature = "workflow-runtime"))]
    {
        let _ = requested_run_id;
        // Build the project (same as vox run)
        let out_dir = std::path::PathBuf::from("dist");
        vox_cli::commands::build::run(
            file,
            &out_dir,
            None,
            None,
            false,
            false,
            vox_cli_core::cli_args::BuildMode::App,
            vox_cli::RustAppShell::default(),
            None,
            None,
        )
        .await?;

        let generated_dir = std::path::PathBuf::from("target").join("generated");
        let shared_target = vox_cli::fs_utils::run_target_dir_for_workspace(Some(&generated_dir));

        // Run workflow: set env so generated binary executes workflow and exits
        let extra_env: Vec<(String, String)> = vec![
            ("VOX_RUN_WORKFLOW".to_string(), workflow_name.to_string()),
            ("VOX_WORKFLOW_ARGS".to_string(), args_json.to_string()),
        ];

        let req = vox_cli_core::build_service::CargoRequest::run(
            generated_dir,
            Some(shared_target),
            vec!["--".to_string()],
            extra_env,
        );
        let output = vox_cli_core::build_service::run_cargo(&req)
            .context("Failed to execute workflow (cargo run in generated directory)")?;

        if !output.status.success() {
            std::io::Write::write_all(&mut std::io::stderr(), &output.stderr).ok();
            anyhow::bail!(
                "Workflow execution failed with exit code: {:?}",
                output.status.code()
            );
        }

        println!("Workflow '{}' completed successfully.", workflow_name);
        Ok(())
    }
}

#[cfg(feature = "workflow-runtime")]
fn resolve_plan_context_from_env() -> Option<(String, String, i64)> {
    let plan_session_id = std::env::var("VOX_PLAN_SESSION_ID")
        .ok()?
        .trim()
        .to_string();
    let plan_node_id = std::env::var("VOX_PLAN_NODE_ID").ok()?.trim().to_string();
    if plan_session_id.is_empty() || plan_node_id.is_empty() {
        return None;
    }
    let plan_version = std::env::var("VOX_PLAN_VERSION")
        .ok()
        .and_then(|v| v.trim().parse::<i64>().ok())
        .unwrap_or(1);
    Some((plan_session_id, plan_node_id, plan_version.max(1)))
}

#[cfg(feature = "workflow-runtime")]
fn resolve_workflow_run_id(workflow_name: &str, requested_run_id: Option<&str>) -> Result<String> {
    if let Some(run_id) = requested_run_id {
        let trimmed = run_id.trim();
        if trimmed.is_empty() {
            anyhow::bail!("--run-id must not be empty");
        }
        return Ok(trimmed.to_string());
    }

    Ok(format!("wf-{workflow_name}-{}", Uuid::new_v4()))
}

#[cfg(all(test, feature = "workflow-runtime"))]
mod tests {
    use super::{resolve_plan_context_from_env, resolve_workflow_run_id};

    #[test]
    fn resolve_workflow_run_id_uses_explicit_value() {
        let run_id = resolve_workflow_run_id("checkout", Some("resume-123")).expect("run id");
        assert_eq!(run_id, "resume-123");
    }

    #[test]
    fn resolve_workflow_run_id_generates_fresh_value() {
        let run_a = resolve_workflow_run_id("checkout", None).expect("run id");
        let run_b = resolve_workflow_run_id("checkout", None).expect("run id");
        assert!(run_a.starts_with("wf-checkout-"));
        assert!(run_b.starts_with("wf-checkout-"));
        assert_ne!(run_a, run_b);
    }

    #[test]
    fn resolve_workflow_run_id_rejects_blank_value() {
        let err = resolve_workflow_run_id("checkout", Some("   ")).expect_err("blank run id");
        assert!(err.to_string().contains("must not be empty"));
    }

    #[test]
    fn resolve_plan_context_from_env_reads_expected_values() {
        std::env::set_var("VOX_PLAN_SESSION_ID", "session-1");
        std::env::set_var("VOX_PLAN_NODE_ID", "node-a");
        std::env::set_var("VOX_PLAN_VERSION", "3");
        let ctx = resolve_plan_context_from_env().expect("plan context");
        assert_eq!(ctx.0, "session-1");
        assert_eq!(ctx.1, "node-a");
        assert_eq!(ctx.2, 3);
        std::env::remove_var("VOX_PLAN_SESSION_ID");
        std::env::remove_var("VOX_PLAN_NODE_ID");
        std::env::remove_var("VOX_PLAN_VERSION");
    }
}
