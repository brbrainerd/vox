# Crate API: vox-mcp

## Module: `vox-mcp\src\a2a.rs`

A2A (Agent-to-Agent) MCP tools — send, inbox, ack, broadcast, history.


### `fn a2a_send`

Send a targeted A2A message from one agent to another.


### `fn a2a_inbox`

Read unacknowledged messages in an agent's inbox.


### `fn a2a_ack`

Acknowledge a message in an agent's inbox.


### `fn a2a_broadcast`

Broadcast an A2A message to all agents except sender.


### `fn a2a_history`

Query the A2A audit trail.


### `fn set_context`

Set a key-value pair in the shared orchestrator context.


### `fn get_context`

Retrieve a value from the shared context.


### `fn list_context`

List available context keys by prefix.


### `fn context_budget`

Get the token budget status for an agent.


### `fn handoff_context`

Handoff summarized context from one agent to another.


### `struct ToolResult`

A standard envelope for all tool responses.


## Module: `vox-mcp\src\main.rs`

# vox-mcp binary

MCP server entry point. Runs on stdio for Vox Agent integration.

Startup flow:
1. Initialize logging → stderr (stdout reserved for MCP protocol)
2. Load orchestrator config from `Vox.toml` (or defaults)
3. Create shared `ServerState` with the orchestrator
4. Start MCP server on stdio via `rmcp::transport::stdio()`


## Module: `vox-mcp\src\memory.rs`

MCP tools for the persistent memory system.


### `fn memory_store`

Persist a key-value fact to long-term memory (MEMORY.md + VoxDb).


### `fn memory_recall`

Retrieve a fact from long-term memory by key.


### `fn memory_search`

Search memory (daily logs + MEMORY.md) by keyword.


### `fn memory_daily_log`

Append an entry to today's daily memory log.


### `fn memory_list_keys`

List all memory keys from MEMORY.md.


### `fn knowledge_query`

Query the knowledge graph by keyword.


### `fn compaction_status`

Get current context window usage and compaction recommendation.


### `struct SessionInfo`

Response type for session info.


### `fn session_create`

Create a new session for an agent.


### `fn session_list`

List all sessions.


### `fn session_reset`

Reset a session (clear history, keep metadata).


### `fn session_compact`

Compact a session with a summary.


### `fn session_info`

Get info about a specific session.


### `fn session_cleanup`

Cleanup archived sessions.


### `fn preference_get`

Get a user preference from VoxDb.


### `fn preference_set`

Set a user preference in VoxDb.


### `fn preference_list`

List user preferences from VoxDb, optionally filtered by key prefix.


### `fn learn_pattern`

Store a learned behavior pattern in VoxDb.


### `fn behavior_record`

Record a user behavior event and get triggered suggestions.


### `fn behavior_summary`

Analyze all behavior events for a user and return learned patterns summary.


### `fn memory_save_db`

Persist a fact directly into VoxDb agent_memory table.


### `fn memory_recall_db`

Recall facts from VoxDb agent_memory table.


### `fn vcs_status`

Unified VCS status: snapshots, oplog, conflicts, workspaces, and changes.


## Module: `vox-mcp\src\skills.rs`

MCP tools for the vox-skills marketplace.


### `struct SkillInfo`

Response shape for skill info.


## Module: `vox-mcp\src\tools.rs`

Tool handler implementations for the Vox MCP server.

Each public function corresponds to an MCP tool that AI agents can invoke.


### `fn submit_task`

Submit a new task to the orchestrator.

Routes the task to the best agent based on file affinity, acquires locks,
and enqueues it for processing.


### `fn task_status`

Get the current status of a specific task.


### `fn orchestrator_status`

Get a full snapshot of the orchestrator's state.


### `fn complete_task`

Mark a task as completed, releasing its file locks.


### `fn fail_task`

Mark a task as failed with a reason.


### `fn check_file_owner`

Check which agent owns a given file path.


### `fn validate_file`

Validate a .vox file using the full compiler pipeline (lexer → parser → typeck → HIR).


### `fn run_tests`

Run `cargo test` for a specific crate.


### `fn check_workspace`

Run `cargo check` for the entire workspace.


### `fn test_all`

Run `cargo test` for the entire workspace.


### `fn build_crate`

Run `cargo build` for a crate or the whole workspace.


### `fn lint_crate`

Run `cargo clippy` and TOESTUB for a crate or the whole workspace.


### `fn coverage_report`

Run `cargo llvm-cov` or `cargo tarpaulin` for code coverage.


### `fn git_log`

Run `git log` to show recent commits.


### `fn git_diff`

Run `git diff` for a file or the whole working tree.


### `fn git_status`

Run `git status` to see working tree status.


### `fn git_blame`

Run `git blame` for a specific file.


### `fn snapshot_list`

List recent snapshots for an agent.


### `fn snapshot_diff`

Show diff between two snapshots.


### `fn oplog_list`

List recent operations from the operation log.


### `fn oplog_undo`

Undo an operation.


### `fn oplog_redo`

Redo an operation.


### `fn conflicts_list`

List active conflicts.


### `fn resolve_conflict`

Resolve a conflict.


### `fn workspace_create`

Create a workspace for an agent.


### `fn workspace_status`

Show workspace status.


### `fn workspace_merge`

Merge workspace back to main.


### `fn change_create`

Create a new logical change.


### `fn change_log`

Show history of a change.


### `fn publish_message`

Publish a message to the bulletin board for all agents to receive.


### `fn generate_vox_code`

Generate validated Vox code using the QWEN inference server.

Calls the inference server at localhost:7863 to generate code from a prompt,
with automatic syntax validation and self-correction.


### `fn vox_db_schema`

Return the complete schema digest for a .vox file as JSON.

This is the primary LLM context tool — it tells AI models exactly
what tables, fields, indexes, and relationships exist in the database.


### `fn vox_db_relationships`

Return the entity-relationship graph: auto-detected Id<X> references between tables.


### `fn vox_db_data_flow`

Return the data flow map: which queries read which tables, which mutations write.


### `fn tool_registry`

Return full list of capabilities to the Vox agent client


### `fn handle_tool_call`

Routes from string name to underlying function
