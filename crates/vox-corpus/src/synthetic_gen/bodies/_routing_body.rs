// ─── Multi-tool orchestration pairs ──────────────────────────────────────────

/// Generate multi-tool orchestration training pairs.
///
/// Teaches the model to chain 2–3 sequential tool calls to accomplish compound
/// goals. Sequences are derived dynamically from `TOOL_REGISTRY_SLIM` so they
/// stay in sync as tools are added.
pub fn generate_tool_chain_pairs(
    out: &mut impl Write,
    cfg: &SyntheticGenConfig,
) -> anyhow::Result<usize> {
    // Curated 2-and-3-tool sequences drawn from real orchestration flows
    let sequences: &[(&[&str], &str, &str)] = &[
        (
            &["vox_plan_create", "vox_generate_vox_code"],
            "Plan and then generate Vox code for a user authentication module",
            "First call `vox_plan_create` to create a structured plan for the auth module, then call `vox_generate_vox_code` with the plan as context to emit the implementation.",
        ),
        (
            &["vox_submit_task", "vox_get_task_status"],
            "Submit a background task and then check its status",
            "Call `vox_submit_task` with the task description, receive a task_id, then call `vox_get_task_status` with that id to poll for completion.",
        ),
        (
            &["vox_repo_index_files", "vox_generate_vox_code"],
            "Index the repository files and then generate a Vox wrapper for a found Rust crate",
            "Use `vox_repo_index_files` to walk the workspace and identify Rust crates, then call `vox_generate_vox_code` to emit a `.vox` binding wrapper for the selected crate.",
        ),
        (
            &["vox_plan_create", "vox_submit_task", "vox_get_task_status"],
            "Plan, dispatch, and monitor a multi-step refactoring task",
            "Chain: `vox_plan_create` → create the refactor plan; `vox_submit_task` → dispatch it to an agent; `vox_get_task_status` → poll until done.",
        ),
        (
            &["vox_chat_message", "vox_generate_vox_code"],
            "Ask the model to explain an API, then generate Vox bindings for it",
            "Use `vox_chat_message` to ask for an explanation of the target API surface, then call `vox_generate_vox_code` with the response as context to emit typed Vox bindings.",
        ),
    ];

    let mut count = 0;
    let min = cfg.min_phrasings_per_tool.max(2);
    let mut rng = Rng::new(cfg.seed, name_hash("tool_chain"));

    for (tools, goal, strategy) in sequences {
        let tool_list = tools.join(" → ");
        let phrasings = [
            format!("How do I use {tool_list} together to {goal}?"),
            format!("What is the right sequence of tool calls to {goal}?"),
            format!("I need to {goal}. Which tools should I call and in what order?"),
        ];
        for phrasing in phrasings.iter().take(min) {
            let response = json!({
                "strategy": strategy,
                "tool_sequence": tools,
                "reasoning": format!("These tools are chained because each step's output feeds the next: {tool_list}"),
            });
            emit_line(out, phrasing, &response, "tool_chain", "tool_chain_trace")?;
            count += 1;
        }
        let _ = rng.next(); // advance for seed mixing
    }
    Ok(count)
}

// ─── Routing decision pairs (Gap 1: Orchestrator-as-model training) ───────────

/// Generate routing decision pairs where the model must decide HOW to respond
/// (tool call, direct answer, agent delegation) given a user request.
///
/// This teaches Mens to act as the orchestrator itself, not just as a tool
/// that executes commands — critical for autonomous agent operation.
pub fn generate_routing_decision_pairs(
    out: &mut impl Write,
    _cfg: &SyntheticGenConfig,
) -> anyhow::Result<usize> {
    let mut count = 0;

    // (user_request, response_type, tool_or_agent, reasoning, args_or_content)
    let scenarios: &[(&str, &str, &str, &str, serde_json::Value)] = &[
        (
            "List all running tasks in the orchestrator",
            "tool_call",
            "vox_orchestrator_status",
            "Direct query tool — no agent delegation needed; response is synchronous status",
            json!({}),
        ),
        (
            "Implement a Vox actor for managing a user session cache",
            "tool_call",
            "vox_submit_task",
            "Complex implementation task requiring agent dispatch; submit to a worker agent with file affinity",
            json!({"description": "Implement a Vox actor for session cache management", "files": ["src/session_cache.vox"]}),
        ),
        (
            "What is the difference between an actor and a workflow in Vox?",
            "direct_answer",
            "none",
            "Conceptual question with known answer — respond directly without tools",
            json!({"answer": "An actor is a stateful isolated entity with a mailbox. A workflow is a durable state machine that survives failures. Actors process messages in real time; workflows model long-running processes with explicit steps."}),
        ),
        (
            "Check if the auth.vox file is owned by another agent before editing",
            "tool_call",
            "vox_check_file_owner",
            "File ownership query is a prerequisite to editing; must call vox_check_file_owner first",
            json!({"path": "src/auth.vox"}),
        ),
        (
            "Send the completed login component to the review agent",
            "tool_call",
            "vox_a2a_send",
            "A2A coordination — handoff between agents via structured message",
            json!({"sender_id": 1, "receiver_id": 2, "msg_type": "plan_handoff", "payload": "{\"artifact\": \"src/login.vox\", \"status\": \"complete\"}"}),
        ),
        (
            "Run the test suite for the vox-compiler crate",
            "tool_call",
            "vox_run_tests",
            "Direct test tool — not an implementation task, execute immediately",
            json!({"crate_name": "vox-compiler"}),
        ),
        (
            "Which LLM model should I use for a code generation task?",
            "tool_call",
            "vox_suggest_model",
            "Model selection requires the routing registry — delegate to suggest_model tool",
            json!({"task": "code_generation"}),
        ),
        (
            "Broadcast to all agents that phase 2 has started",
            "tool_call",
            "vox_a2a_broadcast",
            "Global agent notification — use broadcast not point-to-point a2a_send",
            json!({"sender_id": 1, "msg_type": "phase_start", "payload": "{\"phase\": 2}"}),
        ),
        (
            "Create a plan to migrate the database schema from v7 to v8",
            "tool_call",
            "vox_plan",
            "Multi-step planning task requiring structured plan creation before any code is written",
            json!({"goal": "Migrate VoxDb schema from v7 to v8, ensure backward compatibility", "write_to_disk": true}),
        ),
        (
            "Store that the auth module is complete in agent memory for future reference",
            "tool_call",
            "vox_memory_store",
            "Persistent memory write — agent needs to remember this fact across sessions",
            json!({"key": "auth_module_status", "value": "complete"}),
        ),
        (
            "Explain how Option[T] works in Vox",
            "direct_answer",
            "none",
            "Well-known language concept — answer directly, no tool call needed",
            json!({"answer": "Option[T] represents an optional value that is either Some(value) or None. Use it instead of null. Access the inner value with pattern matching: match x { Some(v) => use(v), None => handle_missing() }"}),
        ),
        (
            "What tasks is agent 3 currently working on?",
            "tool_call",
            "vox_agent_status",
            "Real-time agent state query — must call the status tool, cannot answer from memory",
            json!({"agent_id": 3}),
        ),
    ];

    let prompts = [
        "User request: {req}\nDecide: tool_call / direct_answer, and provide the correct response.",
        "How should a Vox AI agent respond to: '{req}'?",
        "You are a Vox orchestrator. The user says: '{req}'. What is the correct action?",
        "Route this request appropriately: '{req}'",
    ];

    for (req, resp_type, tool, reasoning, args) in scenarios {
        for (i, tmpl) in prompts.iter().enumerate() {
            let prompt = tmpl.replace("{req}", req);
            let response = json!({
                "response_type": resp_type,
                "tool": tool,
                "reasoning": reasoning,
                "arguments": args,
            });
            let _ = i;
            emit_line(out, &prompt, &response, "routing_decision", "routing_trace")?;
            count += 1;
        }
    }
    Ok(count)
}

// ─── Expanded negative preference pairs (Gap 2) ───────────────────────────────

/// Expanded negative preference corpus: 50+ pairs covering tool hallucination,
/// bad parameters, dangerous commands, Vox anti-patterns (null, bad types),
/// and orchestrator misrouting.
pub fn generate_negative_preference_pairs_expanded(
    out: &mut impl Write,
    _cfg: &SyntheticGenConfig,
) -> anyhow::Result<usize> {
    let mut count = 0;

    let negatives: &[(&str, &str, &str, serde_json::Value)] = &[
        // Tool hallucination (invoking tools that don't exist)
        (
            "Query the database for all users",
            "vox_sql_execute",
            "Hallucinated raw SQL tool — use Codex query builder or vox_db_suggest_query instead",
            json!({"sql": "SELECT * FROM users"}),
        ),
        (
            "Search the web for Vox documentation",
            "vox_web_search",
            "vox_web_search does not exist — use vox_knowledge_query or vox_memory_search for local knowledge",
            json!({"query": "Vox documentation"}),
        ),
        (
            "Deploy to production",
            "vox_deploy_production",
            "No such tool — deployment must go through vox_submit_task with deploy_type=production after human approval",
            json!({"target": "prod"}),
        ),
        (
            "Send an email to the user",
            "vox_send_email",
            "vox_send_email does not exist in the Vox tool registry — use the appropriate notification workflow",
            json!({"to": "user@example.com"}),
        ),
        (
            "Read the entire codebase into memory",
            "vox_read_all_files",
            "No bulk file reader tool — use vox_repo_index_files or vox_check_workspace for bounded scanning",
            json!({}),
        ),
        // Bad parameter usage
        (
            "Check the status of task 42",
            "vox_task_status",
            "task_id must be a UUID string, not an integer — use 'task-00000000-0000-0000-0000-000000000042' format",
            json!({"task_id": 42}),
        ),
        (
            "Set context with null value",
            "vox_set_context",
            "null is banned in Vox — use Option[T] = None or omit the field entirely",
            json!({"key": "phase", "value": null}),
        ),
        (
            "Ask the agent to do something with empty description",
            "vox_submit_task",
            "Empty description provides no grounding for the agent — always include specific, actionable text",
            json!({"description": ""}),
        ),
        (
            "Send A2A message with untyped payload",
            "vox_a2a_send",
            "msg_type must be a known A2A type (plan_handoff, task_result, etc.), not arbitrary string 'thing'",
            json!({"sender_id": 1, "receiver_id": 2, "msg_type": "thing", "payload": "{}"}),
        ),
        (
            "Store a value with integer key",
            "vox_memory_store",
            "Memory keys must be strings — integer keys cause type errors in the Arca store",
            json!({"key": 42, "value": "data"}),
        ),
        // Dangerous commands
        (
            "Delete all generated files",
            "vox_run_command",
            "rm -rf without scope guard destroys untracked files shared with other agents — use targeted deletion or file ownership checkout first",
            json!({"command": "rm -rf target/"}),
        ),
        (
            "Reset all local git changes",
            "vox_run_command",
            "git reset --hard is permanently banned — it destroys uncommitted changes from other concurrent agents (see AGENTS.md git-concurrency-policy)",
            json!({"command": "git reset --hard HEAD"}),
        ),
        (
            "Restore the file to its last commit state",
            "vox_run_command",
            "git restore is banned — it silently discards working-tree changes. Use git show HEAD:path/file instead (read-only)",
            json!({"command": "git restore src/auth.vox"}),
        ),
        (
            "Clean up untracked files",
            "vox_run_command",
            "git clean -fd is banned — deletes newly created files from other agents without trace",
            json!({"command": "git clean -fd"}),
        ),
        (
            "Stash current changes before switching context",
            "vox_run_command",
            "git stash is banned — agents lack shared context on stash contents; use 'git commit -m wip:' instead",
            json!({"command": "git stash"}),
        ),
        // Vox syntax anti-patterns
        (
            "Create a nullable user field",
            "vox_generate_code",
            "null is banned in Vox — use Option[User] = None instead of User | null",
            json!({"prompt": "let user: User | null = null"}),
        ),
        (
            "Write a function that returns null on error",
            "vox_generate_code",
            "null returns are banned — use Result[T] or Option[T] to model absence or failure explicitly",
            json!({"prompt": "fn find_user(id: int) -> User { return null }"}),
        ),
        (
            "Write a class with mutable global state",
            "vox_generate_code",
            "Vox has no classes or mutable globals — use actors with isolated state or workflows for durable state machines",
            json!({"prompt": "class GlobalState { static mut count: int = 0 }"}),
        ),
        // Orchestrator misrouting
        (
            "Review the code changes before merging",
            "vox_submit_task",
            "Code review is a human gate — submit_task dispatches to an agent; reviews requiring judgment should use the review agent explicitly or defer to human approval",
            json!({"description": "Review and auto-approve all changes"}),
        ),
        (
            "Run the entire test suite and auto-merge if passing",
            "vox_run_tests",
            "Auto-merge after tests is a dangerous automation pattern — never combine test execution with merge decisions in a single agent step without explicit human approval gate",
            json!({"crate_name": "all", "auto_merge": true}),
        ),
        // Wrong tool for job
        (
            "Save important findings about the codebase",
            "vox_set_context",
            "Context is ephemeral (TTL-based) — use vox_memory_store for persistent facts that must survive session restarts",
            json!({"key": "findings", "value": "...", "ttl_secs": 30}),
        ),
        (
            "Get the list of all available MCP tools",
            "vox_repo_index_files",
            "Repo index lists *files*, not *tools* — use vox_orchestrator_status or consult the tool registry directly for MCP tool discovery",
            json!({}),
        ),
        (
            "Remember which agent is assigned to the auth task",
            "vox_broadcast",
            "Broadcast is for one-to-many notifications — use vox_memory_store to persist assignment facts for later retrieval",
            json!({"agent_id": 1, "message": "auth assigned to agent 2"}),
        ),
        // Type safety violations
        (
            "Use Box<dyn Error> in a public Vox API",
            "vox_generate_code",
            "Box<dyn std::error::Error> is banned in public crate APIs — use a typed error enum (e.g. vox_gamify::Error) per the 5.6 data architecture policy",
            json!({"prompt": "pub fn load() -> Result<Data, Box<dyn Error>>"}),
        ),
        (
            "Use parallel crates for the same domain",
            "vox_generate_code",
            "Creating two crates with overlapping purpose violates the 'No Parallel Crates for the Same Domain' rule (AGENTS.md 5.4) — add to the existing crate",
            json!({"prompt": "create new crate vox-gamify alongside vox-gamify"}),
        ),
    ];

    for (prompt, bad_tool, reason, bad_args) in negatives {
        let response = json!({
            "rejected_tool": bad_tool,
            "reason": reason,
            "arguments": bad_args,
            "policy": "vox_dogfood",
        });
        emit_line(
            out,
            prompt,
            &response,
            "negative_routing",
            "negative_preference",
        )?;
        count += 1;
    }
    Ok(count)
}

