//! Unified tool registry and dispatcher for the Vox MCP server.

use crate::params::{SubmitTaskParams, TaskStatusParams, ToolResult};
use crate::server::ServerState;

pub mod chat_tools;
pub mod compiler_tools;
pub mod db_tools;
pub mod git_tools;
pub mod task_tools;
pub mod vcs_tools;

/// Names and descriptions of all available tools.
pub const TOOL_REGISTRY: &[(&str, &str)] = &[
    (
        "vox_submit_task",
        "Submit a new task to the orchestrator. Routes to the best agent by file affinity.",
    ),
    (
        "vox_task_status",
        "Get the current status of a specific task by ID.",
    ),
    (
        "vox_orchestrator_status",
        "Get a full snapshot of the orchestrator state: agents, queues, and completed tasks.",
    ),
    (
        "vox_orchestrator_start",
        "Start the AgentFleet runtime programmatically from a Vox agent session.",
    ),
    (
        "vox_complete_task",
        "Mark a task as completed, releasing its file locks.",
    ),
    (
        "vox_fail_task",
        "Mark a task as failed with a reason string.",
    ),
    (
        "vox_check_file_owner",
        "Check which agent currently owns a given file path.",
    ),
    (
        "vox_validate_file",
        "Validate a .vox file using the full compiler pipeline (lexer → parser → typeck → HIR).",
    ),
    (
        "vox_run_tests",
        "Run cargo test for a specific crate, optionally filtered by test name.",
    ),
    (
        "vox_check_workspace",
        "Run cargo check for the entire workspace and return diagnostics.",
    ),
    (
        "vox_test_all",
        "Run cargo test for the entire workspace.",
    ),
    (
        "vox_publish_message",
        "Publish a message to the bulletin board for all agents to receive.",
    ),
    (
        "vox_set_context",
        "Set a key-value pair in the shared orchestrator context store. Supports TTL.",
    ),
    (
        "vox_get_context",
        "Retrieve a value from the shared context.",
    ),
    (
        "vox_list_context",
        "List available context keys by prefix.",
    ),
    (
        "vox_context_budget",
        "Get the token budget status and summarize recommendation for an agent.",
    ),
    (
        "vox_handoff_context",
        "Handoff summarized context from one agent to another.",
    ),
    (
        "vox_check_mood",
        "Returns the current gamification mood and status of the agent companion.",
    ),
    (
        "vox_agent_status",
        "Returns current agent state, activity, mood, queue depth.",
    ),
    (
        "vox_agent_continue",
        "Triggers auto-continuation for idle agents.",
    ),
    (
        "vox_agent_assess",
        "Evaluates remaining work, returns completion estimate.",
    ),
    (
        "vox_agent_handoff",
        "Passes plan/context from one agent to another.",
    ),
    (
        "vox_queue_status",
        "Returns the specific queue and tasks for an agent.",
    ),
    (
        "vox_lock_status",
        "Returns a list of all current file locks.",
    ),
    (
        "vox_budget_status",
        "Returns token usage and approximate costs across all agents.",
    ),
    (
        "vox_cancel_task",
        "Cancels an active or queued task.",
    ),
    (
        "vox_rebalance",
        "Rebalances tasks dynamically across agents.",
    ),
    (
        "vox_agent_events",
        "Streams event history for agents.",
    ),
    (
        "vox_my_files",
        "Returns all files currently owned by the specified agent.",
    ),
    (
        "vox_claim_file",
        "Request ownership of a specific file.",
    ),
    (
        "vox_transfer_file",
        "Transfer ownership of a file to another agent.",
    ),
    (
        "vox_ask_agent",
        "Ask another agent a question.",
    ),
    (
        "vox_answer_question",
        "Answer a pending question from another agent.",
    ),
    (
        "vox_pending_questions",
        "List all questions waiting for my answer.",
    ),
    (
        "vox_broadcast",
        "Broadcast a message to all agents on the board.",
    ),
    (
        "vox_memory_store",
        "Persist a key-value fact to long-term memory (MEMORY.md).",
    ),
    (
        "vox_memory_recall",
        "Retrieve a fact from long-term memory by key.",
    ),
    (
        "vox_memory_search",
        "Search daily logs and MEMORY.md for a keyword query.",
    ),
    (
        "vox_memory_log",
        "Append an entry to today's daily memory log.",
    ),
    (
        "vox_memory_list_keys",
        "List all section keys from MEMORY.md.",
    ),
    (
        "vox_knowledge_query",
        "Query the knowledge graph (VoxDB) for related concepts by keyword.",
    ),
    (
        "vox_skill_install",
        "Install a skill from a VoxSkillBundle JSON payload.",
    ),
    (
        "vox_skill_uninstall",
        "Uninstall an installed skill by ID.",
    ),
    (
        "vox_skill_list",
        "List all installed skills.",
    ),
    (
        "vox_skill_search",
        "Search installed skills by keyword.",
    ),
    (
        "vox_skill_info",
        "Get detailed info on a specific skill by ID.",
    ),
    (
        "vox_skill_parse",
        "Parse a SKILL.md and preview its manifest before installing.",
    ),
    (
        "vox_compaction_status",
        "Get current context token usage and whether compaction is recommended.",
    ),
    (
        "vox_session_create",
        "Create a new persistent session for an agent.",
    ),
    (
        "vox_session_list",
        "List all active sessions with state and token usage.",
    ),
    (
        "vox_session_reset",
        "Reset a session's conversation history (keeps metadata).",
    ),
    (
        "vox_session_compact",
        "Replace a session's history with a summary string.",
    ),
    (
        "vox_session_info",
        "Get detailed info about a specific session.",
    ),
    (
        "vox_session_cleanup",
        "Tick lifecycle and remove archived sessions.",
    ),
    (
        "vox_preference_get",
        "Get a user preference value by key from VoxDb.",
    ),
    (
        "vox_preference_set",
        "Set a user preference key to a value in VoxDb.",
    ),
    (
        "vox_preference_list",
        "List all user preferences, optionally filtered by a key prefix.",
    ),
    (
        "vox_learn_pattern",
        "Record a learned behavioral pattern with confidence score.",
    ),
    (
        "vox_behavior_record",
        "Record a user behavior event and receive pattern suggestions.",
    ),
    (
        "vox_behavior_summary",
        "Analyze recent behavior and summarize detected patterns.",
    ),
    (
        "vox_memory_save_db",
        "Persist a typed memory fact to VoxDb agent_memory table.",
    ),
    (
        "vox_memory_recall_db",
        "Recall typed memory facts for an agent from VoxDb.",
    ),
    (
        "vox_build_crate",
        "Run cargo build for a crate or the whole workspace.",
    ),
    (
        "vox_lint_crate",
        "Run cargo clippy for a crate or whole workspace.",
    ),
    (
        "vox_coverage_report",
        "Get code coverage report for a crate using cargo-llvm-cov.",
    ),
    (
        "vox_reorder_task",
        "Change the priority of a queued task.",
    ),
    (
        "vox_drain_agent",
        "Remove all queued tasks from an agent without retiring it.",
    ),
    (
        "vox_cost_history",
        "Get a time-series cost breakdown of operations.",
    ),
    (
        "vox_file_graph",
        "Get a JSON graph of all files and their owning agents (affinity map).",
    ),
    (
        "vox_config_get",
        "Get the current runtime orchestrator and toolchain configuration.",
    ),
    (
        "vox_get_config",
        "Canonical alias for vox_config_get. Returns toolchain + orchestrator config merged.",
    ),
    (
        "vox_config_set",
        "Update the orchestrator configuration dynamically (pass fields to update).",
    ),
    (
        "vox_set_config",
        "Canonical alias for vox_config_set.",
    ),
    (
        "vox_map_agent_session",
        "Map a Vox agent session ID to an existing orchestrator agent.",
    ),
    (
        "vox_poll_events",
        "Poll recent orchestrator events for all agents.",
    ),
    (
        "vox_heartbeat",
        "Send an active heartbeat from a Vox agent session.",
    ),
    (
        "vox_record_cost",
        "Record a cost event from a Vox agent session token usage.",
    ),
    (
        "vox_git_log",
        "Show recent git commits (default: last 10).",
    ),
    (
        "vox_git_diff",
        "Show uncommitted git diff for a file or the whole tree.",
    ),
    (
        "vox_git_status",
        "Get current git working tree status.",
    ),
    ("vox_git_blame", "Show line-by-line git blame for a file."),
    ("vox_snapshot_list", "List recent file snapshots for an agent."),
    ("vox_snapshot_diff", "Show the file-level diff between two snapshots."),
    ("vox_snapshot_restore", "Restore files to a previous snapshot state."),
    ("vox_oplog", "Show recent operations with undo support."),
    ("vox_undo", "Undo the last operation or a specific operation by ID."),
    ("vox_redo", "Redo a previously undone operation."),
    ("vox_conflicts", "List active file conflicts between agents."),
    ("vox_resolve_conflict", "Resolve a file conflict."),
    ("vox_conflict_diff", "Show the N-way diff of a conflict."),
    ("vox_workspace_create", "Create an isolated workspace for an agent."),
    ("vox_workspace_merge", "Merge an agent's workspace changes back to main."),
    ("vox_workspace_status", "Show files modified in an agent's workspace."),
    ("vox_change_create", "Start tracking a new logical change."),
    ("vox_change_log", "Show the history of a change."),
    ("vox_vcs_status", "Get unified VCS status."),
    ("vox_a2a_send", "Send a targeted A2A message from one agent to another."),
    ("vox_a2a_inbox", "Read unacknowledged messages in an agent's inbox."),
    ("vox_a2a_ack", "Acknowledge a message in an agent's inbox."),
    ("vox_a2a_broadcast", "Broadcast an A2A message to all agents."),
    ("vox_a2a_history", "Query the A2A message audit trail."),
    ("vox_db_schema", "Return the complete database schema digest as JSON."),
    ("vox_db_relationships", "Return the entity-relationship graph for the database."),
    ("vox_db_data_flow", "Return the data flow map."),
    ("vox_db_sample_data", "Fetch sample data from a given database table."),
    ("vox_db_explain_query", "Explain a query or mutation in plain English."),
    ("vox_db_suggest_query", "Suggest the correct Vox query expression for an intent."),
    ("vox_generate_code", "Generate validated Vox code from a prompt."),
    // ── Chat & Inline AI ──────────────────────────────────────────────────────
    ("vox_chat_message", "Send a chat message to the Vox AI. Resolves @mentions, injects editor context, queries LLM, persists history."),
    ("vox_chat_history", "Retrieve the full chat history for the current session."),
    ("vox_inline_edit", "AI inline edit on a file range. Editor sends current text; Rust queries LLM and returns replacement."),
    ("vox_plan", "Generate a Cursor-style structured task plan for a goal. Optionally writes PLAN.md to workspace root."),
];

pub fn tool_registry() -> Vec<rmcp::model::Tool> {
    TOOL_REGISTRY
        .iter()
        .map(|(n, d)| rmcp::model::Tool {
            name: std::borrow::Cow::Owned(n.to_string()),
            description: Some(std::borrow::Cow::Owned(d.to_string())),
            input_schema: std::sync::Arc::new(serde_json::Map::new()),
            output_schema: None,
            meta: None,
            annotations: None,
            execution: None,
            icons: None,
            title: None,
        })
        .collect()
}

pub async fn handle_tool_call(
    state: &ServerState,
    name: &str,
    args: serde_json::Value,
) -> Result<String, anyhow::Error> {
    match name {
        "vox_submit_task" => Ok(task_tools::submit_task(state, serde_json::from_value(args)?).await),
        "vox_task_status" => Ok(task_tools::task_status(state, serde_json::from_value(args)?).await),
        "vox_orchestrator_status" => Ok(crate::orchestrator_tools::orchestrator_status(state).await),
        "vox_orchestrator_start" => Ok(crate::orchestrator_tools::orchestrator_start(state).await),
        "vox_complete_task" => Ok(task_tools::complete_task(state, serde_json::from_value(args)?).await),
        "vox_fail_task" => Ok(task_tools::fail_task(state, serde_json::from_value(args)?).await),
        "vox_check_file_owner" => Ok(crate::orchestrator_tools::check_file_owner(state, args.get("path").and_then(|v| v.as_str()).unwrap_or(".")).await),

        "vox_validate_file" => Ok(compiler_tools::validate_file(serde_json::from_value(args)?)),
        "vox_run_tests" => Ok(compiler_tools::run_tests(serde_json::from_value(args)?)),
        "vox_check_workspace" => Ok(compiler_tools::check_workspace()),
        "vox_test_all" => Ok(compiler_tools::test_all()),
        "vox_publish_message" => Ok(task_tools::publish_message(state, serde_json::from_value(args)?).await),

        "vox_git_log" => Ok(git_tools::git_log(args.get("max_commits").and_then(|v| v.as_u64()).map(|n| n as usize))),
        "vox_git_diff" => Ok(git_tools::git_diff(args.get("path").and_then(|v| v.as_str()))),
        "vox_git_status" => Ok(git_tools::git_status()),
        "vox_git_blame" => Ok(git_tools::git_blame(args.get("path").and_then(|v| v.as_str()).unwrap_or("."))),

        "vox_snapshot_list" => Ok(vcs_tools::snapshot_list(state, args).await),
        "vox_snapshot_diff" => Ok(vcs_tools::snapshot_diff(state, args).await),
        "vox_snapshot_restore" => Ok(vcs_tools::snapshot_restore(state, args).await),
        "vox_oplog" => Ok(vcs_tools::oplog_list(state, args).await),
        "vox_undo" => Ok(vcs_tools::oplog_undo(state, args).await),
        "vox_redo" => Ok(vcs_tools::oplog_redo(state, args).await),
        "vox_conflicts" => Ok(vcs_tools::conflicts_list(state).await),
        "vox_resolve_conflict" => Ok(vcs_tools::resolve_conflict(state, args).await),
        "vox_conflict_diff" => Ok(vcs_tools::conflicts_list(state).await),
        "vox_workspace_create" => Ok(vcs_tools::workspace_create(state, args).await),
        "vox_workspace_merge" => Ok(vcs_tools::workspace_merge(state, args).await),
        "vox_workspace_status" => Ok(vcs_tools::workspace_status(state, args).await),
        "vox_change_create" => Ok(vcs_tools::change_create(state, args).await),
        "vox_change_log" => Ok(vcs_tools::change_log(state, args).await),
        "vox_vcs_status" => Ok(crate::orchestrator_tools::vcs_status(state).await),

        "vox_db_schema" => Ok(db_tools::vox_db_schema(args)),
        "vox_db_relationships" => Ok(db_tools::vox_db_relationships(args)),
        "vox_db_data_flow" => Ok(db_tools::vox_db_data_flow(args)),
        "vox_db_sample_data" => Ok(db_tools::vox_db_sample_data(state, args).await),
        "vox_db_explain_query" => Ok(db_tools::vox_db_explain_query(args).await),
        "vox_db_suggest_query" => Ok(db_tools::vox_db_suggest_query(args).await),

        "vox_generate_code" => Ok(compiler_tools::generate_vox_code(args).await),
        "vox_build_crate" => Ok(compiler_tools::build_crate(args.get("crate_name").and_then(|v| v.as_str()))),
        "vox_lint_crate" => Ok(compiler_tools::lint_crate(args.get("crate_name").and_then(|v| v.as_str()))),
        "vox_coverage_report" => Ok(compiler_tools::coverage_report(args.get("crate_name").and_then(|v| v.as_str()))),

        // ── Chat & Inline AI ──────────────────────────────────────────────
        "vox_chat_message" => Ok(chat_tools::chat_message(state, serde_json::from_value(args)?).await),
        "vox_chat_history" => Ok(chat_tools::chat_history(state).await),
        "vox_inline_edit" => Ok(chat_tools::inline_edit(state, serde_json::from_value(args)?).await),
        "vox_plan" => Ok(chat_tools::plan_goal(state, serde_json::from_value(args)?).await),

        // Delegate others to existing modules
        "vox_my_files" => Ok(crate::affinity::my_files(state, serde_json::from_value(args)?).await),
        "vox_claim_file" => Ok(crate::affinity::claim_file(state, serde_json::from_value(args)?).await),
        "vox_transfer_file" => Ok(crate::affinity::transfer_file(state, serde_json::from_value(args)?).await),

        "vox_ask_agent" => Ok(crate::qa::ask_agent(state, serde_json::from_value(args)?).await),
        "vox_answer_question" => Ok(crate::qa::answer_question(state, serde_json::from_value(args)?).await),
        "vox_pending_questions" => Ok(crate::qa::pending_questions(state, serde_json::from_value(args)?).await),
        "vox_broadcast" => Ok(crate::qa::broadcast(state, serde_json::from_value(args)?).await),

        "vox_memory_store" => Ok(crate::memory::memory_store(state, serde_json::from_value(args)?).await),
        "vox_memory_recall" => Ok(crate::memory::memory_recall(state, serde_json::from_value(args)?).await),
        "vox_memory_search" => Ok(crate::memory::memory_search(state, serde_json::from_value(args)?).await),
        "vox_memory_log" => Ok(crate::memory::memory_daily_log(state, serde_json::from_value(args)?).await),
        "vox_memory_list_keys" => Ok(crate::memory::memory_list_keys(state).await),
        "vox_knowledge_query" => Ok(crate::memory::knowledge_query(state, serde_json::from_value(args)?).await),
        "vox_memory_save_db" => Ok(crate::memory::memory_save_db(state, serde_json::from_value(args)?).await),
        "vox_memory_recall_db" => Ok(crate::memory::memory_recall_db(state, serde_json::from_value(args)?).await),

        "vox_compaction_status" => Ok(crate::memory::compaction_status(state, serde_json::from_value(args)?).await),
        "vox_session_create" => Ok(crate::memory::session_create(state, serde_json::from_value(args)?).await),
        "vox_session_list" => Ok(crate::memory::session_list(state).await),
        "vox_session_reset" => Ok(crate::memory::session_reset(state, serde_json::from_value(args)?).await),
        "vox_session_compact" => Ok(crate::memory::session_compact(state, serde_json::from_value(args)?).await),
        "vox_session_info" => Ok(crate::memory::session_info(state, serde_json::from_value(args)?).await),
        "vox_session_cleanup" => Ok(crate::memory::session_cleanup(state).await),

        "vox_preference_get" => Ok(crate::memory::preference_get(state, serde_json::from_value(args)?).await),
        "vox_preference_set" => Ok(crate::memory::preference_set(state, serde_json::from_value(args)?).await),
        "vox_preference_list" => Ok(crate::memory::preference_list(state, serde_json::from_value(args)?).await),
        "vox_learn_pattern" => Ok(crate::memory::learn_pattern(state, serde_json::from_value(args)?).await),
        "vox_behavior_record" => Ok(crate::memory::behavior_record(state, serde_json::from_value(args)?).await),
        "vox_behavior_summary" => Ok(crate::memory::behavior_summary(state, serde_json::from_value(args)?).await),

        "vox_check_mood" => Ok(crate::gamify::check_mood(state, serde_json::from_value(args)?).await),
        "vox_agent_status" => Ok(crate::gamify::agent_status(state, serde_json::from_value(args)?).await),
        "vox_agent_continue" => Ok(crate::gamify::agent_continue(state, serde_json::from_value(args)?).await),
        "vox_agent_assess" => Ok(crate::gamify::agent_assess(state, serde_json::from_value(args)?).await),
        "vox_agent_handoff" => Ok(crate::gamify::agent_handoff(state, serde_json::from_value(args)?).await),

        "vox_queue_status" => Ok(crate::orchestrator_tools::queue_status(state, serde_json::from_value(args)?).await),
        "vox_lock_status" => Ok(crate::orchestrator_tools::lock_status(state).await),
        "vox_budget_status" => Ok(crate::orchestrator_tools::budget_status(state).await),
        "vox_cancel_task" => Ok(crate::orchestrator_tools::cancel_task(state, serde_json::from_value(args)?).await),
        "vox_reorder_task" => Ok(crate::orchestrator_tools::reorder_task(state, serde_json::from_value(args)?).await),
        "vox_drain_agent" => Ok(crate::orchestrator_tools::drain_agent(state, serde_json::from_value(args)?).await),
        "vox_cost_history" => Ok(crate::orchestrator_tools::cost_history(state, serde_json::from_value(args)?).await),
        "vox_file_graph" => Ok(crate::orchestrator_tools::file_graph(state).await),
        "vox_config_get" | "vox_get_config" => Ok(crate::orchestrator_tools::config_get(state).await),
        "vox_config_set" | "vox_set_config" => Ok(crate::orchestrator_tools::config_set(state, args).await),
        "vox_map_agent_session" => Ok(crate::orchestrator_tools::map_agent_session(state, serde_json::from_value(args)?).await),
        "vox_poll_events" => Ok(crate::orchestrator_tools::poll_events(state, serde_json::from_value(args)?).await),
        "vox_heartbeat" => Ok(crate::orchestrator_tools::heartbeat(state, serde_json::from_value(args)?).await),
        "vox_record_cost" => Ok(crate::orchestrator_tools::record_cost(state, serde_json::from_value(args)?).await),
        "vox_rebalance" => Ok(crate::orchestrator_tools::rebalance(state).await),
        "vox_agent_events" => Ok(crate::orchestrator_tools::agent_events(state, serde_json::from_value(args)?).await),

        "vox_a2a_send" => Ok(crate::a2a::a2a_send(state, serde_json::from_value(args)?).await),
        "vox_a2a_inbox" => Ok(crate::a2a::a2a_inbox(state, serde_json::from_value(args)?).await),
        "vox_a2a_ack" => Ok(crate::a2a::a2a_ack(state, serde_json::from_value(args)?).await),
        "vox_a2a_broadcast" => Ok(crate::a2a::a2a_broadcast(state, serde_json::from_value(args)?).await),
        "vox_a2a_history" => Ok(crate::a2a::a2a_history(state, serde_json::from_value(args)?).await),

        "vox_skill_install" => Ok(crate::skills::skill_install(state, serde_json::from_value(args)?).await),
        "vox_skill_uninstall" => Ok(crate::skills::skill_uninstall(state, serde_json::from_value(args)?).await),
        "vox_skill_list" => Ok(crate::skills::skill_list(state)),
        "vox_skill_search" => Ok(crate::skills::skill_search(state, serde_json::from_value(args)?)),
        "vox_skill_info" => Ok(crate::skills::skill_info(state, serde_json::from_value(args)?)),
        "vox_skill_parse" => Ok(crate::skills::skill_parse(serde_json::from_value(args)?)),

        "vox_set_context" => Ok(crate::context::set_context(state, serde_json::from_value(args)?).await),
        "vox_get_context" => Ok(crate::context::get_context(state, serde_json::from_value(args)?).await),
        "vox_list_context" => Ok(crate::context::list_context(state, serde_json::from_value(args)?).await),
        "vox_context_budget" => Ok(crate::context::context_budget(state, serde_json::from_value(args)?).await),
        "vox_handoff_context" => Ok(crate::context::handoff_context(state, serde_json::from_value(args)?).await),

        _ => {
            // Check skill macro tools
            let skills = state.skill_registry.list(None);
            if let Some(skill) = skills.iter().find(|s| s.tools.contains(&name.to_string())) {
                if let Some(db) = &state.db {
                    if let Ok(Some(entry)) = db.store().get_skill_manifest(&skill.id).await {
                        let msg = format!(
                            "This tool is an instructional macro from skill '{}'.\n\nPlease read these instructions and perform the requested actions yourself:\n\n{}",
                            skill.name, entry.skill_md
                        );
                        return Ok(ToolResult::ok(msg).to_json());
                    }
                }
            }
            Err(anyhow::anyhow!("Unknown tool: {}", name))
        }
    }
}
