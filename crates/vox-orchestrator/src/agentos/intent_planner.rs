//! Compiles a natural-language intent into a bounded list of suggested MCP tool names (stub planner).

/// Returns up to `max_steps` canonical tool names as a conservative starting plan.
pub fn plan_intent(intent: &str, max_steps: usize) -> Vec<&'static str> {
    let lower = intent.to_ascii_lowercase();
    let mut out: Vec<&'static str> = Vec::new();

    if lower.contains("status") || lower.contains("diff") {
        out.push("vox_git_status");
    }
    if lower.contains("test") || lower.contains("cargo test") {
        out.push("vox_run_tests");
    }
    if lower.contains("validate") || lower.contains("typecheck") || lower.contains("check") {
        out.push("vox_validate_file");
    }
    if lower.contains("write") || lower.contains("patch") || lower.contains("edit") {
        out.push("vox_write_file");
    }
    if out.is_empty() {
        out.push("vox_repo_status");
    }

    out.truncate(max_steps.max(1));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_tests_intent() {
        let p = plan_intent("run cargo tests on vox-cli", 4);
        assert!(p.contains(&"vox_run_tests"));
    }

    #[test]
    fn detects_write_intent() {
        let p = plan_intent("patch the readme", 4);
        assert!(p.contains(&"vox_write_file"));
    }
}
