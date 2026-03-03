>>> MAIN STARTED
# Vox MCP Tool Reference

Total Tools: 102

| Tool Name | Description |
|-----------|-------------|
| `vox_submit_task` | Submit a new task to the orchestrator. Routes to the best agent by file affinity. |
| `vox_task_status` | Get the current status of a specific task by ID. |
| `vox_orchestrator_status` | Get a full snapshot of the orchestrator state: agents, queues, and completed tasks. |
| `vox_orchestrator_start` | Start the AgentFleet runtime programmatically from an OpenCode session. |
| `vox_complete_task` | Mark a task as completed, releasing its file locks. |
| `vox_fail_task` | Mark a task as failed with a reason string. |
| `vox_check_file_owner` | Check which agent currently owns a given file path. |
| `vox_validate_file` | Validate a .vox file using the full compiler pipeline (lexer → parser → typeck → HIR). |
| `vox_run_tests` | Run cargo test for a specific crate, optionally filtered by test name. |
| `vox_check_workspace` | Run cargo check for the entire workspace and return diagnostics. |
| `vox_test_all` | Run cargo test for the entire workspace. |
| `vox_publish_message` | Publish a message to the bulletin board for all agents to receive. |
| `vox_set_context` | Set a key-value pair in the shared orchestrator context store. Supports TTL. |
| `vox_get_context` | Retrieve a value from the shared context. |
| `vox_list_context` | List available context keys by prefix. |
| `vox_context_budget` | Get the token budget status and summarize recommendation for an agent. |
| `vox_handoff_context` | Handoff summarized context from one agent to another. |
| `vox_check_mood` | Returns the current gamification mood and status of the agent companion. |
| `vox_agent_status` | Returns current agent state, activity, mood, queue depth. |
| `vox_agent_continue` | Triggers auto-continuation for idle agents. |
| `vox_agent_assess` | Evaluates remaining work, returns completion estimate. |
| `vox_agent_handoff` | Passes plan/context from one agent to another. |
| `vox_queue_status` | Returns the specific queue and tasks for an agent. |
| `vox_lock_status` | Returns a list of all current file locks. |
| `vox_budget_status` | Returns token usage and approximate costs across all agents. |
| `vox_cancel_task` | Cancels an active or queued task. |
| `vox_rebalance` | Rebalances tasks dynamically across agents. |
| `vox_agent_events` | Streams event history for agents. |
| `vox_my_files` | Returns all files currently owned by the specified agent. |
| `vox_claim_file` | Request ownership of a specific file. |
| `vox_transfer_file` | Transfer ownership of a file to another agent. |
| `vox_ask_agent` | Ask another agent a question. |
| `vox_answer_question` | Answer a pending question from another agent. |
| `vox_pending_questions` | List all questions waiting for my answer. |
| `vox_broadcast` | Broadcast a message to all agents on the board. |
| `vox_memory_store` | Persist a key-value fact to long-term memory (MEMORY.md). |
| `vox_memory_recall` | Retrieve a fact from long-term memory by key. |
| `vox_memory_search` | Search daily logs and MEMORY.md for a keyword query. |
| `vox_memory_log` | Append an entry to today's daily memory log. |
| `vox_memory_list_keys` | List all section keys from MEMORY.md. |
| `vox_knowledge_query` | Query the knowledge graph (VoxDB) for related concepts by keyword. |
| `vox_skill_install` | Install a skill from a VoxSkillBundle JSON payload. |
| `vox_skill_uninstall` | Uninstall an installed skill by ID. |
| `vox_skill_list` | List all installed skills. |
| `vox_skill_search` | Search installed skills by keyword. |
| `vox_skill_info` | Get detailed info on a specific skill by ID. |
| `vox_skill_parse` | Parse a SKILL.md and preview its manifest before installing. |
| `vox_compaction_status` | Get current context token usage and whether compaction is recommended. |
| `vox_session_create` | Create a new persistent session for an agent. |
| `vox_session_list` | List all active sessions with state and token usage. |
| `vox_session_reset` | Reset a session's conversation history (keeps metadata). |
| `vox_session_compact` | Replace a session's history with a summary string. |
| `vox_session_info` | Get detailed info about a specific session. |
| `vox_session_cleanup` | Tick lifecycle and remove archived sessions. |
| `vox_preference_get` | Get a user preference value by key from VoxDb. |
| `vox_preference_set` | Set a user preference key to a value in VoxDb. |
| `vox_preference_list` | List all user preferences, optionally filtered by a key prefix. |
| `vox_learn_pattern` | Record a learned behavioral pattern with confidence score. |
| `vox_behavior_record` | Record a user behavior event and receive pattern suggestions. |
| `vox_behavior_summary` | Analyze recent behavior and summarize detected patterns. |
| `vox_memory_save_db` | Persist a typed memory fact to VoxDb agent_memory table. |
| `vox_memory_recall_db` | Recall typed memory facts for an agent from VoxDb. |
| `vox_build_crate` | Run cargo build for a crate or the whole workspace. |
| `vox_lint_crate` | Run cargo clippy for a crate or whole workspace. |
| `vox_coverage_report` | Get code coverage report for a crate using cargo-llvm-cov. |
| `vox_cancel_task` | Cancel a queued task before it starts. |
| `vox_reorder_task` | Change the priority of a queued task. |
| `vox_drain_agent` | Remove all queued tasks from an agent without retiring it. |
| `vox_cost_history` | Get a time-series cost breakdown of operations. |
| `vox_file_graph` | Get a JSON graph of all files and their owning agents (affinity map). |
| `vox_config_get` | Get the current runtime orchestrator configuration. |
| `vox_config_set` | Update the orchestrator configuration dynamically (pass fields to update). |
| `vox_map_opencode_session` | Map an OpenCode session ID to an existing orchestrator agent. |
| `vox_poll_events` | Poll recent orchestrator events for all agents. |
| `vox_submit_task` | Submit a new task to the Vox Orchestrator. |
| `vox_heartbeat` | Send an active heartbeat from an OpenCode session. |
| `vox_record_cost` | Record a cost event from an OpenCode session token usage. |
| `vox_git_log` | Show recent git commits (default: last 10). |
| `vox_git_diff` | Show uncommitted git diff for a file or the whole tree. |
| `vox_git_status` | Get current git working tree status. |
| `vox_git_blame` | Show line-by-line git blame for a file. |
| `vox_snapshot_list` | List recent file snapshots for an agent (auto-captured before/after tool calls). |
| `vox_snapshot_diff` | Show the file-level diff between two snapshots. |
| `vox_snapshot_restore` | Restore files to a previous snapshot state. |
| `vox_oplog` | Show recent operations (tool calls, edits, task transitions) with undo support. |
| `vox_undo` | Undo the last operation or a specific operation by ID. |
| `vox_redo` | Redo a previously undone operation. |
| `vox_conflicts` | List active file conflicts between agents. |
| `vox_resolve_conflict` | Resolve a file conflict (take-left, take-right, or defer). |
| `vox_conflict_diff` | Show the N-way diff of a conflict. |
| `vox_workspace_create` | Create an isolated workspace for an agent to edit without interference. |
| `vox_workspace_merge` | Merge an agent's workspace changes back to main. |
| `vox_workspace_status` | Show files modified in an agent's workspace. |
| `vox_change_create` | Start tracking a new logical change (stable ID across edits). |
| `vox_change_log` | Show the history of a change across snapshots and agents. |
| `vox_vcs_status` | Get unified VCS status: snapshots, oplog, conflicts, workspaces, and changes. |
| `vox_a2a_send` | Send a targeted A2A message from one agent to another. |
| `vox_a2a_inbox` | Read unacknowledged messages in an agent's inbox. |
| `vox_a2a_ack` | Acknowledge a message in an agent's inbox. |
| `vox_a2a_broadcast` | Broadcast an A2A message to all agents except sender. |
| `vox_a2a_history` | Query the A2A message audit trail. |
| `vox_generate_code` | Generate validated Vox code from a natural language prompt using the fine-tuned QWEN model. Returns code with syntax validation. |
