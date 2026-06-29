//! Map normalized CodeRabbit items to a markdown/json task checklist.

use std::path::Path;

use anyhow::Result;
use vox_code_audit::rules::{Finding, Severity};
use vox_code_audit::task_queue::{Priority, TaskQueue};

use super::ingest::NormalizedReviewItem;

fn map_severity(s: &str) -> Severity {
    match s.to_lowercase().as_str() {
        "critical" => Severity::Critical,
        "error" => Severity::Error,
        "warning" => Severity::Warning,
        _ => Severity::Info,
    }
}

fn to_finding(item: &NormalizedReviewItem) -> Finding {
    let rule_id = format!("coderabbit/{}", item.category);
    let message = if item.details.is_empty() {
        item.title.clone()
    } else {
        format!("{}\n\n{}", item.title, item.details)
    };
    Finding {
        rule_id,
        diagnostic_id: None,
        rule_name: format!("CodeRabbit: {}", item.category),
        severity: map_severity(&item.severity),
        file: std::path::PathBuf::from(&item.file_path),
        line: item.line,
        column: 0,
        message,
        suggestion: item.suggested_fix.clone(),
        alternatives: Vec::new(),
        rationale: None,
        context: String::new(),
        confidence: None,
        evidence: None,
    }
}

fn build_task_queue(items: &[NormalizedReviewItem]) -> TaskQueue {
    let findings: Vec<Finding> = items.iter().map(to_finding).collect();
    let mut fix_suggestions = Vec::new();
    for f in &findings {
        let priority = match f.severity {
            Severity::Critical | Severity::Error => Priority::Immediate,
            Severity::Warning => Priority::NextSession,
            Severity::Info => Priority::Backlog,
        };
        let prompt = format!(
            "In file `{}` at line {}: {}.\n\nReview and fix this CodeRabbit finding. {}",
            f.file.display(),
            f.line,
            f.message,
            f.suggestion
                .as_deref()
                .unwrap_or("Apply the suggested fix if available.")
        );
        fix_suggestions.push(vox_code_audit::task_queue::FixSuggestion {
            rule_id: f.rule_id.clone(),
            location: format!("{}:{}", f.file.display(), f.line),
            prompt,
            auto_fixable: false,
            priority,
        });
    }
    TaskQueue {
        total_findings: findings.len(),
        fix_suggestions,
    }
}

/// Run tasks command: ingest (if pr given), map, emit markdown/json.
pub async fn run_tasks(
    pr_number: Option<u64>,
    format: &str,
    persist: bool,
    path: &Path,
) -> Result<()> {
    let items = if let Some(pr) = pr_number {
        super::ingest::ingest_pr(pr, path).await?
    } else {
        anyhow::bail!("PR number required (e.g. `vox review coderabbit tasks 42`)");
    };

    if items.is_empty() {
        eprintln!("No CodeRabbit findings for PR");
        return Ok(());
    }

    let queue = build_task_queue(&items);

    if persist {
        let cr_dir = path.join(".coderabbit");
        std::fs::create_dir_all(&cr_dir).ok();
        let tasks_path = cr_dir.join("tasks.json");
        let json = queue.to_json();
        if let Err(e) = std::fs::write(&tasks_path, &json) {
            eprintln!("[persist] Failed to write tasks.json: {}", e);
        } else {
            eprintln!(
                "[persist] Saved actionable Toestub tasks to: {}",
                tasks_path.display()
            );
        }
    }

    match format.to_lowercase().as_str() {
        "markdown" | "md" => {
            println!("{}", queue.to_markdown_checklist());
        }
        "json" => {
            println!("{}", queue.to_json());
        }
        _ => {
            println!("{}", queue.to_markdown_checklist());
        }
    }

    Ok(())
}
