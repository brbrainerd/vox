//! Synthesize tool-use SFT rows from the real Vox CLI / skill surface.
use crate::tool_workflow_corpus::ToolTraceRecord;
use serde_json::json;
use std::io::Write;
use std::path::Path;

/// One synthetic supervised tool call over a REAL Vox CLI command (e.g.
/// "vox ci affected-crates"). `command` MUST be a command verified to exist;
/// `args` is the JSON arguments object.
pub fn synth_vox_command(task: &str, command: &str, args: serde_json::Value) -> ToolTraceRecord {
    ToolTraceRecord {
        task_prompt: task.to_string(),
        tool_name: command.to_string(),
        arguments_json: args.to_string(),
        result_json: serde_json::json!({ "status": "ok" }).to_string(),
        success: true,
        followup_text: None,
        session_id: None,
    }
}

/// Generate a synthetic agentic corpus file for the active spoke.
pub fn generate_agentic_synth_file(output_path: &Path) -> anyhow::Result<usize> {
    if let Some(p) = output_path.parent() {
        std::fs::create_dir_all(p)?;
    }

    let mut file = std::io::BufWriter::new(std::fs::File::create(output_path)?);
    let mut count = 0;

    // 1. Generate CLI command examples
    let cli_examples = vec![
        (
            "Find which crates a change affects",
            "vox ci affected-crates",
            json!({ "base": "origin/main" }),
        ),
        (
            "Run command compliance checks",
            "vox ci command-check",
            json!({}),
        ),
        ("Show workspace status", "vox status", json!({})),
        (
            "Run tests for a crate",
            "vox test",
            json!({ "crate_name": "vox-cli" }),
        ),
        ("Sync ignore files", "vox ci sync-ignore-files", json!({})),
        (
            "Lint commit messages",
            "vox ci commit-lint",
            json!({ "revision": "HEAD~1" }),
        ),
    ];

    for (task, cmd, args) in cli_examples {
        let rec = synth_vox_command(task, cmd, args);
        writeln!(file, "{}", serde_json::to_string(&rec)?)?;
        count += 1;
    }

    // 2. Generate skill discovery and installation examples
    let skill_examples = vec![
        (
            "Install a new development skill",
            "vox_skill_install",
            json!({ "bundle_json": "{\"id\":\"vox-lint-fixer\",\"version\":\"1.0.0\"}" }),
        ),
        (
            "List all installed agent skills",
            "vox_skill_list",
            json!({}),
        ),
        (
            "Search for a git helper skill",
            "vox_skill_search",
            json!({ "query": "git helper" }),
        ),
        (
            "Uninstall an unused skill",
            "vox_skill_uninstall",
            json!({ "skill_id": "old-skill" }),
        ),
    ];

    for (task, tool, args) in skill_examples {
        let rec = ToolTraceRecord {
            task_prompt: task.to_string(),
            tool_name: tool.to_string(),
            arguments_json: args.to_string(),
            result_json: json!({ "status": "ok" }).to_string(),
            success: true,
            followup_text: None,
            session_id: None,
        };
        writeln!(file, "{}", serde_json::to_string(&rec)?)?;
        count += 1;
    }

    // 3. Generate examples for MCP registry tools
    let mut rng = crate::synthetic_gen::rng::Rng::new(42, 100);
    for entry in vox_mcp_registry::TOOL_REGISTRY {
        let args = crate::synthetic_gen::tool_pairs::example_args_for_tool(entry.name, &mut rng);
        let task = format!("Execute the {} tool to help with the task", entry.name);
        let rec = ToolTraceRecord {
            task_prompt: task,
            tool_name: entry.name.to_string(),
            arguments_json: args.to_string(),
            result_json: json!({ "status": "ok" }).to_string(),
            success: true,
            followup_text: None,
            session_id: None,
        };
        writeln!(file, "{}", serde_json::to_string(&rec)?)?;
        count += 1;
    }

    file.flush()?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synth_targets_real_vox_command() {
        let r = synth_vox_command(
            "Find which crates a change affects",
            "vox ci affected-crates",
            serde_json::json!({ "base": "origin/main" }),
        );
        assert_eq!(r.tool_name, "vox ci affected-crates");
        assert!(r.arguments_json.contains("origin/main"));
    }
}
