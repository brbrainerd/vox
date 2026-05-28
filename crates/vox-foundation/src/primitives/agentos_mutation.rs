//! Canonical mapping from MCP tool names to coarse `mutation_kind` strings used by AgentOS
//! (orchestrator policy overlay, ACI envelopes, checkpoint hints).
//!
//! SSOT for [`mutation_kind_for_tool`]; keep in sync with `contracts/aci/agent-computer-interface.v1.yaml`.

/// Returns the `mutation_kind` string for `contracts/aci/agent-computer-interface.v1.schema.json`.
#[must_use]
pub fn mutation_kind_for_tool(name: &str) -> &'static str {
    match name {
        // External / network / shell / browser / codegen that may bill
        "vox_run_shell"
        | "vox_deploy"
        | "vox_generate_code"
        | "vox_speech_to_code"
        | "vox_visual_rag_query"
        | "vox_browser_open"
        | "vox_browser_close"
        | "vox_browser_goto"
        | "vox_browser_click"
        | "vox_browser_fill"
        | "vox_browser_wait_for"
        | "vox_browser_text"
        | "vox_browser_screenshot"
        | "vox_browser_html"
        | "vox_browser_extract"
        | "vox_browser_extract_json"
        | "vox_browser_act"
        | "vox_openclaw_gateway_call"
        | "vox_openclaw_notify"
        | "vox_openclaw_subscribe"
        | "vox_openclaw_import_skill"
        | "vox_submit_task"
        | "vox_schola_submit" => "external_side_effect",

        // Local workspace mutation
        "vox_write_file"
        | "vox_patch_file"
        | "vox_inline_edit_file"
        | "vox_multi_replace"
        | "vox_multi_replace_file"
        | "vox_delete_file"
        | "vox_snapshot_restore"
        | "vox_undo"
        | "vox_redo"
        | "vox_resolve_conflict"
        | "vox_workspace_merge"
        | "vox_change_create"
        | "vox_commit_create"
        | "vox_push"
        | "vox_pr_open"
        | "vox_force_push"
        | "vox_branch_delete"
        | "vox_project_init"
        | "vox_complete_task"
        | "vox_fail_task"
        | "vox_doubt_task" => "local_mutation",

        _ => {
            if name.starts_with("vox_db_") && name.contains("upsert")
                || name.contains("append")
                || name.contains("insert")
            {
                return "local_mutation";
            }
            if name.starts_with("vox_openclaw_") {
                return "external_side_effect";
            }
            "read_only"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_status_is_read_only() {
        assert_eq!(mutation_kind_for_tool("vox_git_status"), "read_only");
    }

    #[test]
    fn run_shell_is_external() {
        assert_eq!(
            mutation_kind_for_tool("vox_run_shell"),
            "external_side_effect"
        );
    }

    #[test]
    fn write_file_is_local_mutation() {
        assert_eq!(mutation_kind_for_tool("vox_write_file"), "local_mutation");
    }
}
