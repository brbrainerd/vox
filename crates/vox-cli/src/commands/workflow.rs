//! `vox workflow` — inspect and validate Vox workflows and activities.

use anyhow::{Context, Result};
use std::path::PathBuf;

/// List all workflows and activities defined in a .vox file.
pub async fn list(file: &PathBuf) -> Result<()> {
    let result = crate::pipeline::run_frontend(file, false).await?;
    let hir = &result.hir;

    if hir.activities.is_empty() && hir.workflows.is_empty() {
        println!("No workflows or activities found in {}", file.display());
        println!("  Add an 'activity' or 'workflow' block to your .vox file.");
        return Ok(());
    }

    if !hir.activities.is_empty() {
        println!("Activities ({}):", hir.activities.len());
        for act in &hir.activities {
            let params: Vec<String> = act
                .params
                .iter()
                .map(|p| format!("{}: {:?}", p.name, p.type_ann))
                .collect();
            let ret = act
                .return_type
                .as_ref()
                .map(|t| format!("{:?}", t))
                .unwrap_or_else(|| "Unit".to_string());
            println!("  activity {}({}) to {}", act.name, params.join(", "), ret);
        }
        println!();
    }

    if !hir.workflows.is_empty() {
        println!("Workflows ({}):", hir.workflows.len());
        for wf in &hir.workflows {
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
            println!("  workflow {}({}) to {}", wf.name, params.join(", "), ret);
        }
    }

    Ok(())
}

/// Show type-checked info about a specific workflow.
pub async fn inspect(file: &PathBuf, workflow_name: &str) -> Result<()> {
    let result = crate::pipeline::run_frontend(file, false).await?;
    let hir = &result.hir;

    let wf = hir
        .workflows
        .iter()
        .find(|w| w.name == workflow_name)
        .with_context(|| {
            format!(
                "Workflow '{}' not found in {}",
                workflow_name,
                file.display()
            )
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

    println!("Workflow: {}", wf.name);
    println!(
        "  Signature: workflow {}({}) to {}",
        wf.name,
        params.join(", "),
        ret
    );
    println!("  Activities in this file: {}", hir.activities.len());
    println!();

    if !hir.activities.is_empty() {
        println!("  Available activities:");
        for act in &hir.activities {
            let act_params: Vec<String> = act
                .params
                .iter()
                .map(|p| format!("{}: {:?}", p.name, p.type_ann))
                .collect();
            println!(
                "    {} ({}) — callable with 'with {{ retries, timeout, activity_id }}'",
                act.name,
                act_params.join(", ")
            );
        }
        println!();
    }

    println!("  Tip: 'with' options supported:");
    println!("    retries: int         — retry attempts on failure");
    println!("    timeout: str         — e.g. \"30s\", \"5m\"");
    println!("    initial_backoff: str — delay before first retry e.g. \"500ms\"");
    println!("    activity_id: str     — unique ID for deduplication / idempotency");

    Ok(())
}

/// Type-check a workflow file through the full Vox compiler pipeline.
pub async fn check(file: &PathBuf) -> Result<()> {
    let result = crate::pipeline::run_frontend(file, false)
        .await
        .map_err(|e| anyhow::anyhow!("Workflow check failed: {}", e))?;

    let warnings = result.warning_count();
    let errors = result.error_count();

    crate::pipeline::print_diagnostics(&result, file, false);

    if errors == 0 {
        println!(
            "✓ {} — {} activity(ies), {} workflow(s), {} warning(s)",
            file.display(),
            result.hir.activities.len(),
            result.hir.workflows.len(),
            warnings,
        );
        Ok(())
    } else {
        anyhow::bail!("{} type error(s) found", errors)
    }
}

/// Execute a workflow (currently a stub before durable execution runtime is implemented).
pub async fn run(file: &PathBuf, workflow_name: &str) -> Result<()> {
    let result = crate::pipeline::run_frontend(file, false).await?;
    let _wf = result
        .hir
        .workflows
        .iter()
        .find(|w| w.name == workflow_name)
        .with_context(|| {
            format!(
                "Workflow '{}' not found in {}",
                workflow_name,
                file.display()
            )
        })?;

    println!("Attempting to execute workflow: {}", workflow_name);
    println!(">>> NOTICE: Vox Workflow Engine (crates/vox-workflow-runtime) is an upcoming architectural milestone.");
    println!(">>> Durable execution (retry, timeout, activity journal, crash recovery) is a work in progress.");
    println!(">>> The execution will be treated as dry-run mode for now.");
    println!("Dry-run completed successfully.");

    Ok(())
}
