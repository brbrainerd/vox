//! Synthesize tool-use SFT rows from the real Vox CLI / skill surface.
//! H.2: uses TOOL_REGISTRY_SLIM + CLI_COMMANDS (build-time derived) + SkillRegistry YAML.
use crate::synthetic_gen::{CLI_COMMANDS, TOOL_REGISTRY_SLIM};
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
        result_json: json!({ "status": "ok" }).to_string(),
        success: true,
        followup_text: None,
        session_id: None,
    }
}

fn tool_record(task: &str, tool: &str, args: serde_json::Value) -> ToolTraceRecord {
    ToolTraceRecord {
        task_prompt: task.to_string(),
        tool_name: tool.to_string(),
        arguments_json: args.to_string(),
        result_json: json!({ "status": "ok" }).to_string(),
        success: true,
        followup_text: None,
        session_id: None,
    }
}

/// Generate a synthetic agentic corpus file for the active spoke.
///
/// Sources (H.2):
/// 1. `TOOL_REGISTRY_SLIM` — build-time slice of every registered MCP tool.
/// 2. `CLI_COMMANDS` — build-time slice of every `vox` CLI subcommand.
/// 3. Skill-management MCP tools (stable subset, always present).
/// 4. Optional: `SkillRegistry` YAML on disk for user-installed skill examples.
pub fn generate_agentic_synth_file(output_path: &Path) -> anyhow::Result<usize> {
    if let Some(p) = output_path.parent() {
        std::fs::create_dir_all(p)?;
    }

    let mut file = std::io::BufWriter::new(std::fs::File::create(output_path)?);
    let mut count = 0;
    let mut rng = crate::synthetic_gen::rng::Rng::new(42, 100);

    // 1. MCP tool examples from TOOL_REGISTRY_SLIM (build-time, always in sync)
    for tool_name in TOOL_REGISTRY_SLIM {
        let args = crate::synthetic_gen::tool_pairs::example_args_for_tool(tool_name, &mut rng);
        let rec = tool_record(
            &format!("Execute the {} tool", tool_name),
            tool_name,
            args,
        );
        writeln!(file, "{}", serde_json::to_string(&rec)?)?;
        count += 1;
    }

    // 2. CLI command examples from CLI_COMMANDS (build-time, always in sync)
    for (cmd, desc) in CLI_COMMANDS {
        let rec = synth_vox_command(desc, cmd, json!({}));
        writeln!(file, "{}", serde_json::to_string(&rec)?)?;
        count += 1;
    }

    // 3. Skill-management MCP tools (stable, hardcoded names to avoid import cycles)
    let skill_mgmt = [
        ("Install a new development skill", "vox_skill_install", json!({ "bundle_json": "{\"id\":\"vox-lint-fixer\",\"version\":\"1.0.0\"}" })),
        ("List all installed agent skills", "vox_skill_list", json!({})),
        ("Search for a git helper skill", "vox_skill_search", json!({ "query": "git helper" })),
        ("Uninstall an unused skill", "vox_skill_uninstall", json!({ "skill_id": "old-skill" })),
    ];
    for (task, tool, args) in &skill_mgmt {
        writeln!(file, "{}", serde_json::to_string(&tool_record(task, tool, args.clone()))?)?;
        count += 1;
    }

    // 4. Optional: SkillRegistry YAML — adds examples for user-installed skills if present
    //    (workspace root inferred from Cargo env; silently skipped if absent)
    if let Ok(workspace_root) = std::env::var("CARGO_MANIFEST_DIR")
        .map(std::path::PathBuf::from)
        .map(|p| p.ancestors().nth(2).unwrap_or(&p).to_path_buf())
    {
        let skill_yaml = workspace_root.join("contracts/skills/installed-skills.v1.yaml");
        if skill_yaml.exists() {
            if let Ok(raw) = std::fs::read_to_string(&skill_yaml) {
                if let Ok(val) = serde_yaml::from_str::<serde_json::Value>(&raw) {
                    if let Some(skills) = val.get("skills").and_then(|v| v.as_array()) {
                        for skill in skills {
                            let id = skill.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");
                            let desc = skill.get("description").and_then(|v| v.as_str()).unwrap_or(id);
                            let rec = tool_record(
                                &format!("Invoke the {} skill: {}", id, desc),
                                &format!("vox_skill_invoke_{}", id.replace('-', "_")),
                                json!({ "skill_id": id }),
                            );
                            writeln!(file, "{}", serde_json::to_string(&rec)?)?;
                            count += 1;
                        }
                    }
                }
            }
        }
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

    #[test]
    fn generate_file_produces_records() {
        let dir = tempfile::tempdir().expect("tempdir");
        let out = dir.path().join("agentic.jsonl");
        let n = generate_agentic_synth_file(&out).expect("generate");
        assert!(n > 0, "should produce at least one record");
        let lines: Vec<_> = std::fs::read_to_string(&out)
            .expect("read output")
            .lines()
            .filter(|l| !l.trim().is_empty())
            .collect::<Vec<_>>()
            .into_iter()
            .map(|l| l.to_string())
            .collect();
        assert_eq!(lines.len(), n, "line count matches returned count");
        // All lines must be valid JSON
        for line in &lines {
            serde_json::from_str::<serde_json::Value>(line).expect("valid JSON");
        }
    }

    #[test]
    fn registry_slim_drives_mcp_records() {
        assert!(!TOOL_REGISTRY_SLIM.is_empty(), "TOOL_REGISTRY_SLIM must not be empty");
        assert!(!CLI_COMMANDS.is_empty(), "CLI_COMMANDS must not be empty");
    }
}
