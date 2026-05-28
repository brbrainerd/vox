//! JSON Schema fragments for MCP tool `input_schema` (draft-07 subset).
//!
//! Keep shapes aligned with [`crate::params`], [`crate::memory`], [`crate::affinity`],
//! and [`super::chat_tools`] `Deserialize` structs. Unknown tools fall back to an empty map
//! (caller may treat as unconstrained JSON).
//!
//! ## Schemars-first vs hand-tuned
//!
//! Most strict object shapes use [`schemars::schema_for`] on the same Rust types handlers
//! deserialize. Remaining `parse_obj(...)` literals are **intentional overrides**: `Value`-parsed
//! tools (Oratio), `anyOf`/discriminated union chat payloads, VCS pass-through maps, and other
//! shapes that are awkward or misleading if derived verbatim.

use serde_json::{Map, Value};

macro_rules! derived_tool_schema {
    ($t:ty) => {{
        let settings = schemars::generate::SchemaSettings::draft07().with(|s| {
            s.inline_subschemas = true;
        });
        let schema_generator = settings.into_generator();
        let root = schema_generator.into_root_schema_for::<$t>();
        let serde_json::Value::Object(mut map) =
            serde_json::to_value(&root).expect(concat!("schema_for ", stringify!($t)))
        else {
            panic!(concat!(
                "JsonSchema root must be an object: ",
                stringify!($t)
            ));
        };
        map.remove("$schema");
        map
    }};
}

fn parse_obj(s: &str) -> Map<String, Value> {
    serde_json::from_str::<Value>(s)
        .ok()
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default()
}

/// Schema object for RMCP / MCP tool registration.
pub(super) fn tool_input_schema(name: &str) -> Map<String, Value> {
    let name = super::tool_aliases::canonical_tool_name(name);
    match name {
        // ── Oratio (already strict) ─────────────────────────────────────────
        // Schemas are pure JSON — no oratio-rerank types referenced; always available
        // so that TOOL_REGISTRY coverage test passes in all feature configurations.
        "vox_oratio_transcribe" => parse_obj(
            r#"{"type":"object","properties":{"path":{"type":"string","description":"Workspace-relative or absolute path to an audio or transcript file"},"language_hint":{"type":"string"},"profile":{"type":"string","enum":["conservative","balanced","aggressive"]},"debug_parser_payload":{"type":"boolean"}},"required":["path"],"additionalProperties":false}"#,
        ),
        "vox_oratio_listen" => parse_obj(
            r#"{"type":"object","properties":{"path":{"type":"string"},"session_id":{"type":"string"},"timeout_ms":{"type":"integer","minimum":1},"max_duration_ms":{"type":"integer","minimum":1},"inference_deadline_ms":{"type":"integer","minimum":1},"heartbeat_ms":{"type":"integer","minimum":1},"language_hint":{"type":"string"},"profile":{"type":"string","enum":["conservative","balanced","aggressive"]},"route_mode":{"type":"string","enum":["none","tool","chat","orchestrator"]},"debug_parser_payload":{"type":"boolean"},"emit_asr_refine_path":{"type":"string"},"llm_refinement":{"type":"boolean"},"llm_min_det_confidence":{"type":"number"},"llm_max_output_tokens":{"type":"integer","minimum":1}},"required":["path"],"additionalProperties":false}"#,
        ),
        "vox_oratio_status" => parse_obj(r#"{"type":"object","additionalProperties":false}"#),
        "vox_speech_to_code" => parse_obj(
            r#"{"type":"object","description":"Exactly one of `path` (audio/transcript file under workspace) or `prompt` (text only). Chains Oratio STT when path is set, then vox_generate_code.","properties":{"path":{"type":"string","description":"Workspace-relative audio file for Candle Whisper + refine"},"prompt":{"type":"string","description":"Skip STT; use as codegen prompt"},"language_hint":{"type":"string"},"profile":{"type":"string","enum":["conservative","balanced","aggressive"]},"debug_parser_payload":{"type":"boolean"},"route_mode":{"type":"string","enum":["none","tool","chat","orchestrator"]},"include_route":{"type":"boolean","description":"When true (default), run deterministic intent routing on refined transcript (path mode only)"},"validate":{"type":"boolean"},"max_retries":{"type":"integer","minimum":0,"maximum":5},"session_id":{"type":"string","description":"Shared with generate; defaults to new correlation if omitted"},"output_surface_mode":{"type":"string"},"emit_trace_path":{"type":"string","description":"Append one JSON line (speech_trace.schema.json fields) under this workspace-relative path"}},"additionalProperties":false}"#,
        ),

        // ── Tasks & bulletin ─────────────────────────────────────────────────
        "vox_submit_task" => derived_tool_schema!(crate::params::SubmitTaskParams),
        "vox_task_status" | "vox_cancel_task" | "vox_test_decision" => {
            derived_tool_schema!(crate::params::TaskStatusParams)
        }
        "vox_complete_task" => derived_tool_schema!(crate::params::CompleteTaskParams),
        "vox_fail_task" => derived_tool_schema!(crate::params::FailTaskParams),
        "vox_doubt_task" => derived_tool_schema!(crate::params::DoubtTaskParams),
        "vox_publish_message" => {
            derived_tool_schema!(crate::params::PublishMessageParams)
        }
        "vox_reorder_task" => derived_tool_schema!(crate::params::ReorderTaskParams),
        "vox_drain_agent" => derived_tool_schema!(crate::params::DrainAgentParams),
        "vox_spawn_agent" => derived_tool_schema!(crate::params::SpawnAgentParams),
        "vox_retire_agent" | "vox_pause_agent" | "vox_resume_agent" => {
            derived_tool_schema!(crate::params::AgentIdToolParams)
        }

        // ── Orchestrator status / no-arg tools ───────────────────────────────
        "vox_orchestrator_status"
        | "vox_orchestrator_persistence_outbox_lifecycle"
        | "vox_rebalance"
        | "vox_lock_status"
        | "vox_file_graph"
        | "vox_config_get"
        | "vox_openclaw_list_remote"
        | "vox_openclaw_discover"
        | "vox_openclaw_health"
        | "vox_openclaw_subscriptions"
        | "vox_session_list"
        | "vox_session_cleanup"
        | "vox_memory_list_keys"
        | "vox_skill_list"
        | "vox_test_all"
        | "vox_check_workspace"
        | "vox_get_active_model"
        | "vox_language_surface"
        | "vox_capability_model_manifest"
        | "vox_pipeline_status"
        | "vox_decorator_registry"
        | "vox_builtin_registry"
        | "vox_workspace_modules"
        | "vox_export_grammar_ebnf"
        | "vox_a2a_tasks" => parse_obj(r#"{"type":"object","additionalProperties":false}"#),
        "vox_secrets_doctor" => parse_obj(
            r#"{
            "type": "object",
            "properties": {
                "workflow": { "type": "string", "enum": ["Chat", "Mcp", "Publish", "Review", "DbRemote", "MensMesh"] },
                "profile": { "type": "string", "enum": ["Dev", "Ci", "Prod", "Mobile"] }
            },
            "additionalProperties": false
        }"#,
        ),

        // Handler ignores args today; keep the schema strict so clients send `{}` only.
        "vox_orchestrator_start" => parse_obj(r#"{"type":"object","additionalProperties":false}"#),
        "vox_orchestrator_persistence_outbox_queue" => parse_obj(
            r#"{"type":"object","properties":{"lane":{"type":"string","description":"Optional lane filter (e.g. lineage/task_failed)"},"limit":{"type":"integer","minimum":1,"maximum":1000,"description":"Maximum rows returned from the tail of filtered queue"},"include_replay":{"type":"boolean","description":"Include replay payload blobs in returned rows (default true)"}},"additionalProperties":false}"#,
        ),
        // `params` is `serde_json::Value` — derive would need a custom `schema_with`; keep explicit.
        "vox_openclaw_gateway_call" => parse_obj(
            r#"{"type":"object","properties":{"method":{"type":"string","minLength":1},"params":{"description":"OpenClaw gateway params JSON object"}},"required":["method"],"additionalProperties":false}"#,
        ),
        "vox_openclaw_search_remote" => {
            derived_tool_schema!(crate::params::OpenClawSearchParams)
        }
        "vox_openclaw_import_skill" => {
            derived_tool_schema!(crate::params::OpenClawImportParams)
        }
        "vox_openclaw_subscribe" | "vox_openclaw_unsubscribe" => {
            derived_tool_schema!(crate::params::OpenClawDomainParams)
        }
        "vox_openclaw_notify" => {
            derived_tool_schema!(crate::params::OpenClawNotifyParams)
        }

        // ── Browser (CDP / chromiumoxide) ───────────────────────────────────
        "vox_browser_open" => derived_tool_schema!(crate::params::BrowserOpenParams),
        "vox_browser_close" => derived_tool_schema!(crate::params::BrowserPageParams),
        "vox_browser_goto" => derived_tool_schema!(crate::params::BrowserGotoParams),
        "vox_browser_click" | "vox_browser_text" => {
            derived_tool_schema!(crate::params::BrowserTargetParams)
        }
        "vox_browser_fill" => derived_tool_schema!(crate::params::BrowserFillParams),
        "vox_browser_wait_for" => derived_tool_schema!(crate::params::BrowserWaitParams),
        "vox_browser_html" => derived_tool_schema!(crate::params::BrowserHtmlParams),
        "vox_browser_screenshot" => {
            derived_tool_schema!(crate::params::BrowserScreenshotParams)
        }
        "vox_browser_extract" => {
            derived_tool_schema!(crate::params::BrowserExtractParams)
        }
        "vox_browser_extract_json" => {
            derived_tool_schema!(crate::params::BrowserExtractJsonParams)
        }
        "vox_browser_act" => derived_tool_schema!(crate::params::BrowserActParams),

        // ── Compiler / workspace ─────────────────────────────────────────────
        "vox_check" | "vox_validate_file" | "vox_compiler::ast_inspect" => {
            derived_tool_schema!(crate::params::ValidateFileParams)
        }
        "vox_validate_source" => {
            derived_tool_schema!(crate::params::ValidateSourceParams)
        }
        "vox_run_tests" => derived_tool_schema!(crate::params::RunTestsParams),
        "vox_build_crate" | "vox_lint_crate" | "vox_coverage_report" => {
            derived_tool_schema!(crate::params::OptionalCrateNameParams)
        }

        // ── Execution Budget ─────────────────────────────────────────────────
        "vox_exec_time_query" => parse_obj(
            r#"{"type":"object","properties":{"tool_key":{"type":"string","minLength":1},"repository_id":{"type":"string"},"window_days":{"type":"integer","minimum":1}},"required":["tool_key"],"additionalProperties":false}"#,
        ),
        "vox_exec_time_record" => parse_obj(
            r#"{"type":"object","properties":{"tool_key":{"type":"string","minLength":1},"repository_id":{"type":"string"},"duration_ms":{"type":"integer","minimum":0},"timeout_budget_ms":{"type":"integer","minimum":0},"outcome":{"type":"string","enum":["success","timeout","error"]}},"required":["tool_key","duration_ms"],"additionalProperties":false}"#,
        ),

        // ── File affinity ────────────────────────────────────────────────────
        "vox_check_file_owner" => parse_obj(
            r#"{"type":"object","properties":{"path":{"type":"string"}},"required":["path"],"additionalProperties":false}"#,
        ),
        "vox_my_files" => parse_obj(
            r#"{"type":"object","properties":{"agent_id":{"type":"integer","minimum":0}},"required":["agent_id"],"additionalProperties":false}"#,
        ),
        "vox_claim_file" => parse_obj(
            r#"{"type":"object","properties":{"agent_id":{"type":"integer","minimum":0},"path":{"type":"string"}},"required":["agent_id","path"],"additionalProperties":false}"#,
        ),
        "vox_transfer_file" => parse_obj(
            r#"{"type":"object","properties":{"from_agent":{"type":"integer","minimum":0},"to_agent":{"type":"integer","minimum":0},"path":{"type":"string"}},"required":["from_agent","to_agent","path"],"additionalProperties":false}"#,
        ),

        // ── Context ──────────────────────────────────────────────────────────
        "vox_set_context" => parse_obj(
            r#"{"type":"object","properties":{"agent_id":{"type":"integer","minimum":0},"key":{"type":"string"},"value":{"type":"string"},"ttl_seconds":{"type":"integer","minimum":0}},"required":["agent_id","key","value"],"additionalProperties":false}"#,
        ),
        "vox_get_context" => parse_obj(
            r#"{"type":"object","properties":{"key":{"type":"string","minLength":1}},"required":["key"],"additionalProperties":false}"#,
        ),
        "vox_list_context" => parse_obj(
            r#"{"type":"object","properties":{"prefix":{"type":"string"}},"required":["prefix"],"additionalProperties":false}"#,
        ),
        "vox_context_budget" => parse_obj(
            r#"{"type":"object","properties":{"agent_id":{"type":"integer","minimum":0}},"required":["agent_id"],"additionalProperties":false}"#,
        ),
        "vox_set_agent_budget" => {
            derived_tool_schema!(crate::mcp_context::SetAgentBudgetParams)
        }
        "vox_emergency_stop" => {
            derived_tool_schema!(crate::mcp_context::EmergencyStopParams)
        }
        "vox_handoff_context" => parse_obj(
            r#"{"type":"object","properties":{"from_agent":{"type":"integer","minimum":0},"to_agent":{"type":"integer","minimum":0}},"required":["from_agent","to_agent"],"additionalProperties":false}"#,
        ),

        // ── Gamify ───────────────────────────────────────────────────────────
        "vox_check_mood" | "vox_agent_status" | "vox_agent_continue" | "vox_agent_assess" => {
            parse_obj(
                r#"{"type":"object","additionalProperties":true,"description":"Pass agent_id and other fields per orchestrator tool docs."}"#,
            )
        }
        "vox_agent_handoff" => parse_obj(
            r#"{"type":"object","properties":{"from_agent_id":{"type":"integer","minimum":0},"to_agent_id":{"type":"integer","minimum":0},"plan_summary":{"type":"string","minLength":1},"unresolved_objectives":{"type":"array","items":{"type":"string"}},"verification_criteria":{"type":"array","items":{"type":"string"}},"context_envelope_json":{"type":"string","minLength":1},"harness_spec_json":{"type":"string","minLength":1}},"required":["from_agent_id","to_agent_id","plan_summary"],"additionalProperties":false}"#,
        ),
        "vox_gamify_notifications_list" => parse_obj(
            r#"{"type":"object","properties":{"limit":{"type":"integer","minimum":1,"maximum":100}},"additionalProperties":false}"#,
        ),
        "vox_gamify_progress_snapshot" => parse_obj(
            r#"{"type":"object","properties":{"notification_limit":{"type":"integer","minimum":1,"maximum":100},"policy_limit":{"type":"integer","minimum":1,"maximum":500},"policy_days":{"type":"integer","minimum":1,"maximum":3660}},"additionalProperties":false}"#,
        ),
        "vox_gamify_notification_ack" => parse_obj(
            r#"{"type":"object","properties":{"notification_id":{"type":"string","minLength":1}},"required":["notification_id"],"additionalProperties":false}"#,
        ),
        "vox_gamify_notifications_ack_all" => {
            parse_obj(r#"{"type":"object","additionalProperties":false}"#)
        }
        "vox_gamify_quest_list" => parse_obj(
            r#"{"type":"object","properties":{"limit":{"type":"integer","minimum":1,"maximum":200}},"additionalProperties":false}"#,
        ),
        "vox_gamify_shop_catalog" => parse_obj(r#"{"type":"object","additionalProperties":false}"#),
        "vox_gamify_shop_buy" => parse_obj(
            r#"{"type":"object","properties":{"item_index":{"type":"integer","minimum":1},"idempotency_key":{"type":"string"}},"required":["item_index"],"additionalProperties":false}"#,
        ),
        "vox_gamify_collegium_join" => parse_obj(
            r#"{"type":"object","properties":{"collegium_id":{"type":"string","minLength":1}},"required":["collegium_id"],"additionalProperties":false}"#,
        ),
        "vox_gamify_battle_start" => parse_obj(
            r#"{"type":"object","properties":{"companion_name":{"type":"string"},"rule_id":{"type":"string"},"message":{"type":"string"},"file_path":{"type":"string"},"line":{"type":"integer","minimum":1},"context":{"type":"string"}},"required":["companion_name","rule_id","message"],"additionalProperties":false}"#,
        ),
        "vox_gamify_battle_submit" => parse_obj(
            r#"{"type":"object","properties":{"companion_name":{"type":"string"},"code":{"type":"string"},"success":{"type":"boolean"}},"required":["companion_name","code"],"additionalProperties":false}"#,
        ),

        "vox_queue_status" | "vox_budget_status" | "vox_agent_events" | "vox_cost_history"
        | "vox_poll_events" => parse_obj(r#"{"type":"object","additionalProperties":true}"#),

        // ── Memory (MEMORY.md / search) ─────────────────────────────────────
        "vox_memory_store" => parse_obj(
            r#"{"type":"object","properties":{"agent_id":{"type":"integer","minimum":0},"key":{"type":"string"},"value":{"type":"string"},"relations":{"type":"array","items":{"type":"string"}},"media_url":{"type":"string"},"media_type":{"type":"string"}},"required":["agent_id","key","value"],"additionalProperties":false}"#,
        ),
        "vox_memory_recall" => parse_obj(
            r#"{"type":"object","properties":{"key":{"type":"string"}},"required":["key"],"additionalProperties":false}"#,
        ),
        "vox_memory_search" => parse_obj(
            r#"{"type":"object","properties":{"query":{"type":"string"}},"required":["query"],"additionalProperties":false}"#,
        ),
        "vox_semantic_fs_discover" => parse_obj(
            r#"{"type":"object","properties":{"intent":{"type":"string","minLength":1},"limit":{"type":"integer","minimum":1,"maximum":256}},"required":["intent"],"additionalProperties":false}"#,
        ),
        "vox_memory_log" => parse_obj(
            r#"{"type":"object","properties":{"entry":{"type":"string"}},"required":["entry"],"additionalProperties":false}"#,
        ),
        "vox_knowledge_query" => parse_obj(
            r#"{"type":"object","properties":{"query":{"type":"string"},"limit":{"type":"integer"}},"required":["query"],"additionalProperties":false}"#,
        ),
        "vox_research_run" => parse_obj(
            r#"{"type":"object","properties":{"query":{"type":"string","minLength":1},"scope":{"type":"string"},"max_sources":{"type":"integer","minimum":1,"maximum":50},"verify_claims":{"type":"boolean"},"site_scope":{"type":"string"},"json":{"type":"boolean"}},"required":["query"],"additionalProperties":false}"#,
        ),
        "vox_research_start" => parse_obj(
            r#"{"type":"object","properties":{"query":{"type":"string","minLength":1},"scope":{"type":"string"},"max_sources":{"type":"integer","minimum":1,"maximum":50},"verify_claims":{"type":"boolean"},"site_scope":{"type":"string"}},"required":["query"],"additionalProperties":false}"#,
        ),
        "vox_research_status" | "vox_research_get" => parse_obj(
            r#"{"type":"object","properties":{"session_id":{"type":"integer","minimum":1}},"required":["session_id"],"additionalProperties":false}"#,
        ),

        // ── Sessions & compaction ─────────────────────────────────────────────
        "vox_session_create" => parse_obj(
            r#"{"type":"object","properties":{"agent_id":{"type":"integer","minimum":0}},"required":["agent_id"],"additionalProperties":false}"#,
        ),
        "vox_session_reset" | "vox_session_info" | "vox_compaction_status" => parse_obj(
            r#"{"type":"object","properties":{"session_id":{"type":"string"}},"required":["session_id"],"additionalProperties":false}"#,
        ),
        "vox_session_compact" => parse_obj(
            r#"{"type":"object","properties":{"session_id":{"type":"string"},"summary":{"type":"string"}},"required":["session_id","summary"],"additionalProperties":false}"#,
        ),

        // ── Preferences & behavior ──────────────────────────────────────────
        "vox_preference_get" => parse_obj(
            r#"{"type":"object","properties":{"key":{"type":"string"}},"required":["key"],"additionalProperties":false}"#,
        ),
        "vox_preference_set" => parse_obj(
            r#"{"type":"object","properties":{"key":{"type":"string"},"value":{"type":"string"}},"required":["key","value"],"additionalProperties":false}"#,
        ),
        "vox_preference_list" => parse_obj(
            r#"{"type":"object","properties":{"prefix":{"type":"string"}},"additionalProperties":false}"#,
        ),
        "vox_learn_pattern" | "vox_behavior_record" | "vox_behavior_summary" => {
            parse_obj(r#"{"type":"object","additionalProperties":true}"#)
        }

        // ── Codex memory DB ─────────────────────────────────────────────────
        "vox_memory_save_db" | "vox_memory_recall_db" => parse_obj(
            r#"{"type":"object","additionalProperties":true,"description":"Typed agent_memory payloads; see memory module handlers."}"#,
        ),

        // ── Models ──────────────────────────────────────────────────────────
        "vox_list_models" => parse_obj(r#"{"type":"object","additionalProperties":false}"#),
        "vox_suggest_model" => parse_obj(
            r#"{"type":"object","properties":{"task_category":{"type":"string","description":"e.g. codegen, review, testing"}},"required":["task_category"],"additionalProperties":false}"#,
        ),
        "vox_set_model" => parse_obj(
            r#"{"type":"object","properties":{"agent_id":{"type":"integer","minimum":0},"model_id":{"type":"string"}},"required":["agent_id","model_id"],"additionalProperties":false}"#,
        ),
        "vox_set_active_model" => parse_obj(
            r#"{"type":"object","properties":{"model_id":{"type":"string","description":"Empty string clears sticky override"}},"required":["model_id"],"additionalProperties":false}"#,
        ),

        // ── Git & repo index ─────────────────────────────────────────────────
        "vox_git_log" => parse_obj(
            r#"{"type":"object","properties":{"max_commits":{"type":"integer"}},"additionalProperties":false}"#,
        ),
        "vox_git_diff" => parse_obj(
            r#"{"type":"object","properties":{"path":{"type":"string"}},"additionalProperties":false}"#,
        ),
        "vox_git_status" => parse_obj(r#"{"type":"object","additionalProperties":false}"#),
        "vox_git_blame" => parse_obj(
            r#"{"type":"object","properties":{"path":{"type":"string"}},"required":["path"],"additionalProperties":false}"#,
        ),
        "vox_repo_index_status" | "vox_repo_index_refresh" => {
            parse_obj(r#"{"type":"object","additionalProperties":false}"#)
        }
        "vox_visus_audit" => parse_obj(
            r#"{"type":"object","properties":{"url":{"type":"string","description":"Target URL for visual audit"}},"required":["url"],"additionalProperties":false}"#,
        ),
        "vox_visus_baseline" => parse_obj(
            r#"{"type":"object","properties":{"url":{"type":"string"}},"required":["url"],"additionalProperties":false}"#,
        ),
        "vox_visual_rag_query" => {
            derived_tool_schema!(crate::params::VoxVisualRagQueryParams)
        }
        "vox_repo_status" => parse_obj(r#"{"type":"object","additionalProperties":false}"#),
        "vox_project_init" => parse_obj(
            r#"{"type":"object","properties":{"project_name":{"type":"string","minLength":1,"description":"Project / package name"},"package_kind":{"type":"string","description":"e.g. application, skill, agent, workflow, chatbot, library"},"template":{"type":"string","description":"Optional application template: chatbot, dashboard, api"},"target_subdir":{"type":"string","description":"Repo-relative directory for the scaffold (no `..`); default is workspace root"}},"required":["project_name"],"additionalProperties":false}"#,
        ),
        "vox_repo_catalog_list" | "vox_repo_catalog_refresh" => {
            parse_obj(r#"{"type":"object","additionalProperties":false}"#)
        }
        "vox_repo_query_text" => derived_tool_schema!(vox_repository::QueryTextParams),
        "vox_repo_query_file" => derived_tool_schema!(vox_repository::QueryFileParams),
        "vox_repo_query_history" => derived_tool_schema!(vox_repository::QueryHistoryParams),
        "vox_conflict_diff" => parse_obj(
            r#"{"type":"object","properties":{"conflict_id":{"description":"Conflict id as number or C-XXXXXX string","oneOf":[{"type":"integer","minimum":0},{"type":"string","pattern":"^C-[0-9]{6}$"}]}},"required":["conflict_id"],"additionalProperties":false}"#,
        ),
        "vox_snapshot_diff" => parse_obj(
            r#"{"type":"object","properties":{"before":{"oneOf":[{"type":"integer","minimum":0},{"type":"string","pattern":"^S-[0-9]{6}$"}]},"after":{"oneOf":[{"type":"integer","minimum":0},{"type":"string","pattern":"^S-[0-9]{6}$"}]}},"additionalProperties":false}"#,
        ),
        "vox_snapshot_restore" => parse_obj(
            r#"{"type":"object","properties":{"snapshot_id":{"type":"string","pattern":"^S-[0-9]{6}$"}},"required":["snapshot_id"],"additionalProperties":false}"#,
        ),
        "vox_undo" => parse_obj(
            r#"{"type":"object","properties":{"operation_id":{"oneOf":[{"type":"integer","minimum":0},{"type":"string","pattern":"^OP-[0-9]{6}$"}]}},"required":["operation_id"],"additionalProperties":false}"#,
        ),
        "vox_redo" => parse_obj(
            r#"{"type":"object","properties":{"operation_id":{"oneOf":[{"type":"integer","minimum":0},{"type":"string","pattern":"^OP-[0-9]{6}$"}]}},"required":["operation_id"],"additionalProperties":false}"#,
        ),
        "vox_resolve_conflict" => parse_obj(
            r#"{"type":"object","properties":{"conflict_id":{"oneOf":[{"type":"integer","minimum":0},{"type":"string","pattern":"^C-[0-9]{6}$"}]},"strategy":{"type":"string","enum":["take_left","take_right","defer"]},"defer_to_agent":{"type":"integer","minimum":0}},"required":["conflict_id"],"additionalProperties":false}"#,
        ),

        // ── VCS (pass-through args; shape varies by tool) ───────────────────
        "vox_snapshot_list"
        | "vox_oplog"
        | "vox_conflicts"
        | "vox_workspace_create"
        | "vox_workspace_merge"
        | "vox_workspace_status"
        | "vox_change_create"
        | "vox_change_log"
        | "vox_vcs_status" => parse_obj(
            r#"{"type":"object","additionalProperties":true,"description":"Tool-specific JSON; see vcs_tools handlers."}"#,
        ),

        // ── DB digest tools ──────────────────────────────────────────────────
        "vox_db_schema" | "vox_db_relationships" | "vox_db_data_flow" => {
            parse_obj(r#"{"type":"object","additionalProperties":true}"#)
        }
        "vox_db_sample_data" | "vox_db_explain_query" | "vox_db_suggest_query" => {
            parse_obj(r#"{"type":"object","additionalProperties":true}"#)
        }
        "vox_journey_canonical_steps" => parse_obj(
            r#"{"type":"object","properties":{"journey_id":{"type":"string","maxLength":512,"description":"Canonical journey id; defaults to canonical_journey.v1.greenfield_vox_mens_devloop"}},"additionalProperties":false}"#,
        ),

        "vox_db_research_session_upsert" => parse_obj(
            r#"{"type":"object","properties":{"session_key":{"type":"string"},"title":{"type":"string"},"status":{"type":"string"},"repository_id":{"type":"string"},"config_json":{"type":"object"},"summary_json":{"type":"object"}},"required":["session_key"],"additionalProperties":false}"#,
        ),
        "vox_db_conversation_version_append" => parse_obj(
            r#"{"type":"object","properties":{"conversation_id":{"type":"integer"},"version_index":{"type":"integer"},"label":{"type":"string"},"snapshot_json":{"type":"object"}},"required":["conversation_id","version_index"],"additionalProperties":false}"#,
        ),
        "vox_db_conversation_edge_insert" => parse_obj(
            r#"{"type":"object","properties":{"from_conversation_id":{"type":"integer"},"to_conversation_id":{"type":"integer"},"edge_kind":{"type":"string"},"weight":{"type":"number"},"metadata_json":{"type":"object"}},"required":["from_conversation_id","to_conversation_id"],"additionalProperties":false}"#,
        ),
        "vox_db_topic_evolution_append" => parse_obj(
            r#"{"type":"object","properties":{"topic_id":{"type":"integer"},"event_kind":{"type":"string"},"prior_label":{"type":"string"},"new_label":{"type":"string"},"detail_json":{"type":"object"}},"required":["topic_id","event_kind"],"additionalProperties":false}"#,
        ),
        "vox_db_research_metric_linked" => parse_obj(
            r#"{"type":"object","properties":{"session_key":{"type":"string"},"metric_type":{"type":"string"},"metric_value":{"type":"number"},"metadata_json":{"type":["string","object","null"]},"repository_id":{"type":"string"}},"required":["session_key","metric_type"],"additionalProperties":false}"#,
        ),
        "vox_db_trust_rollups" => parse_obj(
            r#"{"type":"object","properties":{"entity_type":{"type":"string","maxLength":128},"dimension":{"type":"string","maxLength":128},"domain":{"type":"string","maxLength":256},"repository_id":{"type":"string","maxLength":512},"repository_id_default_workspace":{"type":"boolean","description":"When true, filter rollups to the MCP workspace repository_id"},"limit":{"type":"integer","minimum":1,"maximum":10000}},"additionalProperties":false}"#,
        ),
        "vox_db_trust_summary" => parse_obj(
            r#"{"type":"object","properties":{"entity_type":{"type":"string","maxLength":128},"dimension":{"type":"string","maxLength":128},"domain":{"type":"string","maxLength":256},"repository_id":{"type":"string","maxLength":512},"repository_id_default_workspace":{"type":"boolean","description":"When true, filter trust rollups to the MCP workspace repository_id"},"group_by":{"type":"string","enum":["dimension","domain","entity_type","dimension_domain","entity_dimension"]},"limit_groups":{"type":"integer","minimum":1,"maximum":500}},"additionalProperties":false}"#,
        ),
        "vox_db_trust_drift" => parse_obj(
            r#"{"type":"object","properties":{"entity_type":{"type":"string","maxLength":128},"dimension":{"type":"string","maxLength":128},"window_ms":{"type":"integer","minimum":60000,"maximum":2592000000,"description":"Recent vs prior window length (ms); default 86400000"}},"additionalProperties":false}"#,
        ),
        "vox_db_trust_propagate" => parse_obj(
            r#"{"type":"object","properties":{"dimension":{"type":"string","minLength":1,"maxLength":128},"repository_id":{"type":"string","maxLength":512},"repository_id_default_workspace":{"type":"boolean","description":"When true, scope model rollups to workspace repository_id"},"damping":{"type":"number","minimum":0.0,"maximum":1.0},"iterations":{"type":"integer","minimum":1,"maximum":256},"persist":{"type":"boolean","description":"Write {dimension}_propagated trust observations"}},"required":["dimension"],"additionalProperties":false}"#,
        ),

        // ── Codegen ──────────────────────────────────────────────────────────
        "vox_generate_code" => parse_obj(
            r#"{"type":"object","properties":{"prompt":{"type":"string","minLength":1,"description":"Natural-language description of the `.vox` code to generate"},"validate":{"type":"boolean","description":"When true (default), run `validate_document_with_hir` and bounded repair retries"},"max_retries":{"type":"integer","minimum":0,"maximum":5,"description":"Max HIR repair attempts (clamped to speech policy cap)"},"session_id":{"type":"string","description":"Optional session id for model routing affinity"},"journey_id":{"type":"string","description":"Optional stable id for this generate request (joins routing + telemetry; server-generated if omitted)"},"output_surface_mode":{"type":"string","enum":["raw_code_only","fenced_transport"],"description":"How the model wraps `.vox` surface text"},"output_path":{"type":"string","description":"Optional workspace-relative path to write validated `.vox` text; `meta.file_outcomes` reports bytes and relative path"},"vcs_agent_id":{"type":"integer","minimum":0,"description":"When set with `output_path`, capture a filesystem snapshot for this agent after the write; `meta.file_outcomes.post_write_snapshot_id` is set"}},"required":["prompt"],"additionalProperties":false}"#,
        ),

        // ── Q&A & A2A ───────────────────────────────────────────────────────
        "vox_ask_agent" | "vox_answer_question" | "vox_pending_questions" | "vox_broadcast" => {
            parse_obj(
                r#"{"type":"object","additionalProperties":true,"description":"See qa module for expected fields."}"#,
            )
        }
        "vox_a2a_send" | "vox_a2a_inbox" | "vox_a2a_ack" | "vox_a2a_broadcast"
        | "vox_a2a_history" => parse_obj(
            r#"{"type":"object","additionalProperties":true,"description":"See a2a module for expected fields."}"#,
        ),

        // ── Skills ───────────────────────────────────────────────────────────
        "vox_skill_uninstall" | "vox_skill_info" | "vox_skill_parse" => {
            parse_obj(r#"{"type":"object","additionalProperties":true}"#)
        }
        "vox_skill_search" => parse_obj(
            r#"{"type":"object","properties":{"query":{"type":"string"}},"required":["query"],"additionalProperties":true}"#,
        ),
        "vox_skill_install" => parse_obj(
            r#"{"type":"object","additionalProperties":true,"description":"VoxSkillBundle JSON payload fields."}"#,
        ),

        // ── Config / session map / heartbeat / cost ────────────────────────
        "vox_config_set" => parse_obj(
            r#"{"type":"object","additionalProperties":true,"description":"Partial orchestrator config updates."}"#,
        ),
        "vox_map_agent_session" => {
            derived_tool_schema!(crate::params::MapAgentSessionParams)
        }
        "vox_heartbeat" | "vox_record_cost" => {
            parse_obj(r#"{"type":"object","additionalProperties":true}"#)
        }

        // ── Chat & plan ──────────────────────────────────────────────────────
        "vox_chat_message" => parse_obj(
            r#"{"type":"object","anyOf":[{"required":["prompt"]},{"required":["message"]}],"properties":{"prompt":{"type":"string","minLength":1,"maxLength":262144},"message":{"type":"string","minLength":1,"maxLength":262144,"description":"Alias for prompt (serde maps to prompt)"},"context_files":{"type":"array","items":{"type":"string","maxLength":4096}},"open_files":{"type":"array","items":{"type":"string","maxLength":4096}},"active_file":{"type":"string","maxLength":4096},"active_line":{"type":"integer"},"selected_text":{"type":"string","maxLength":1048576},"diagnostics":{"type":"array"},"session_id":{"type":"string","maxLength":2048,"description":"Opaque session isolation key. Independent sessions maintain separate history transcripts. Omit or pass null to use the shared default session."},"thread_id":{"type":"string","maxLength":2048,"description":"Editor thread id; included in structured transcript journey envelope"},"journey_id":{"type":"string","maxLength":2048,"description":"Stable request id across routing and storage (generated if omitted)"},"cognitive_profile":{"type":"string","enum":["fast","reasoning","creative"],"description":"Optional routing hint: fast=lowest latency model, reasoning=high-tier model, creative=high temperature. Omit for standard automatic resolution."},"temperature":{"type":"number","minimum":0.0,"maximum":2.0},"top_p":{"type":"number","minimum":0.0,"maximum":1.0}},"additionalProperties":true}"#,
        ),
        "vox_chat_history" => parse_obj(
            r#"{"type":"object","properties":{"session_id":{"type":"string","maxLength":2048,"description":"Session isolation key. Omit to retrieve the shared default session history."},"trace_id":{"type":"string","maxLength":256,"description":"Optional trace id; logged server-side for correlation with chat_message"}},"additionalProperties":false}"#,
        ),
        "vox_questioning_submit_answer" => parse_obj(
            r#"{"type":"object","properties":{"session_id":{"type":"string","minLength":1,"maxLength":2048},"answer_text":{"type":"string","minLength":1,"maxLength":131072},"answer_type":{"type":"string","maxLength":64},"question_id":{"type":"string","maxLength":512},"selected_option_id":{"type":"string","maxLength":256},"information_contribution_bits":{"type":"number"}},"required":["session_id","answer_text"],"additionalProperties":false}"#,
        ),
        "vox_questioning_pending" => parse_obj(
            r#"{"type":"object","properties":{"session_id":{"type":"string","minLength":1,"maxLength":2048,"description":"Same MCP session_id as chat / plan"}},"required":["session_id"],"additionalProperties":false}"#,
        ),
        "vox_questioning_sync_ssot" => parse_obj(
            r#"{"type":"object","properties":{"relative_path":{"type":"string","maxLength":4096,"description":"Workspace-relative path to questioning markdown; default is docs/src/reference/information-theoretic-questioning.md"}},"additionalProperties":false}"#,
        ),
        "vox_inline_edit" => parse_obj(
            r#"{"type":"object","properties":{"prompt":{"type":"string"},"instruction":{"type":"string"},"file":{"type":"string"},"file_path":{"type":"string"},"start_line":{"type":"integer"},"end_line":{"type":"integer"},"current_text":{"type":"string"},"selection":{"type":"string"},"language":{"type":"string"},"context_before":{"type":"string"},"context_after":{"type":"string"},"temperature":{"type":"number","minimum":0.0,"maximum":2.0},"top_p":{"type":"number","minimum":0.0,"maximum":1.0}},"required":["start_line","end_line","current_text"],"additionalProperties":true}"#,
        ),
        "vox_apply_structured_edit" => parse_obj(
            r#"{"type":"object","properties":{"file_path":{"type":"string"},"start_line":{"type":"integer","minimum":1},"end_line":{"type":"integer","minimum":1},"target_content":{"type":"string"},"replacement_code":{"type":"string"}},"required":["file_path","target_content","replacement_code"],"additionalProperties":false}"#,
        ),
        "vox_plan" => parse_obj(
            r#"{"type":"object","properties":{"goal":{"type":"string","minLength":1,"maxLength":65536},"scope_files":{"type":"array","items":{"type":"string","maxLength":4096}},"write_to_disk":{"type":"boolean"},"max_tasks":{"type":"integer","minimum":1,"maximum":2000},"session_id":{"type":"string","maxLength":2048},"plan_depth":{"type":"string","enum":["minimal","standard","deep"]},"auto_expand_thin_plan":{"type":"boolean"},"loop_mode":{"type":"string","enum":["off","auto","force"]},"max_refine_rounds":{"type":"integer","minimum":0,"maximum":8},"refine_budget_tokens":{"type":"integer","minimum":0,"maximum":200000},"gap_risk_threshold":{"type":"number","minimum":0.05,"maximum":0.95},"plan_page_offset":{"type":"integer","minimum":0,"maximum":500000},"plan_page_limit":{"type":"integer","minimum":1,"maximum":2000},"plan_telemetry_session_id":{"type":"string","maxLength":2048},"question_link_session_id":{"type":"string","maxLength":2048},"questioning_hints_enabled":{"type":"boolean"},"answerer_profile":{"type":"string","enum":["local_first","cloud_first","balanced"]},"temperature":{"type":"number","minimum":0.0,"maximum":2.0},"top_p":{"type":"number","minimum":0.0,"maximum":1.0}},"required":["goal"],"additionalProperties":false}"#,
        ),
        "vox_ghost_text" => parse_obj(
            r#"{"type":"object","properties":{"prefix":{"type":"string"},"suffix":{"type":"string"},"language":{"type":"string"},"file_path":{"type":"string"},"max_tokens":{"type":"integer"},"session_id":{"type":"string"},"temperature":{"type":"number","minimum":0.0,"maximum":2.0},"top_p":{"type":"number","minimum":0.0,"maximum":1.0}},"required":["prefix","suffix"],"additionalProperties":true}"#,
        ),
        "vox_replan" => parse_obj(
            r#"{"type":"object","properties":{"session_id":{"type":"string","minLength":1,"maxLength":2048},"delta_hint":{"type":"string","minLength":1,"maxLength":65536},"write_to_disk":{"type":"boolean"},"mode":{"type":"string","maxLength":64}},"required":["session_id","delta_hint"],"additionalProperties":false}"#,
        ),
        "vox_plan_status" => parse_obj(
            r#"{"type":"object","properties":{"session_id":{"type":"string","minLength":1,"maxLength":2048}},"required":["session_id"],"additionalProperties":false}"#,
        ),
        "vox_plan_list_sessions" => {
            derived_tool_schema!(crate::chat_tools::params::PlanListSessionsParams)
        }
        "vox_plan_resume" => {
            derived_tool_schema!(crate::chat_tools::params::PlanResumeParams)
        }
        "vox_attention_summary" => {
            derived_tool_schema!(crate::dei_tools::params::AttentionSummaryParams)
        }
        "vox_attention_history" | "vox_attention_reset" | "vox_trust_override" => {
            parse_obj(r#"{"type":"object","additionalProperties":false}"#)
        }
        "vox_handoff_lineage" => {
            derived_tool_schema!(crate::dei_tools::params::HandoffLineageParams)
        }
        "vox_benchmark_list" => parse_obj(
            r#"{"type":"object","properties":{"limit":{"type":"integer","minimum":1,"maximum":500},"metric_type":{"type":"string","enum":["benchmark_event","syntax_k_event"]},"source":{"type":"string","enum":["research_metrics","build_health","build_regressions","build_warnings","dependency_shape"]},"run_id":{"type":"integer","minimum":1}},"additionalProperties":false}"#,
        ),
        "vox_benchmark_record" => parse_obj(
            r#"{"type":"object","properties":{"name":{"type":"string","minLength":1,"maxLength":512,"description":"Benchmark name (e.g. build_time, eval_p95)"},"fixture_id":{"type":"string","maxLength":512},"metric_type":{"type":"string","enum":["benchmark_event","syntax_k_event"]},"value":{"type":"number","description":"Optional metric value"},"details":{"description":"Optional structured JSON details"}},"required":["name"],"additionalProperties":false}"#,
        ),
        "vox_code_audit_findings_upsert" => parse_obj(
            r#"{"type":"object","properties":{"findings":{"type":"array","minItems":1,"items":{"type":"object","properties":{"rule_id":{"type":"string","minLength":1},"rule_name":{"type":"string","minLength":1},"severity":{"type":"string","enum":["Info","Warning","Error","Critical"]},"file":{"type":"string","minLength":1},"line":{"type":"integer","minimum":1},"column":{"type":"integer","minimum":0},"message":{"type":"string","minLength":1},"suggestion":{"type":"string"},"context":{"type":"string"}},"required":["rule_id","rule_name","severity","file","line","column","message"],"additionalProperties":false}},"session_id":{"type":"string","maxLength":2048}},"required":["findings"],"additionalProperties":false}"#,
        ),
        "vox_schola_submit" => parse_obj(
            r#"{"type":"object","properties":{"description":{"type":"string","minLength":1,"maxLength":65536},"require_cuda":{"type":"boolean"},"require_metal":{"type":"boolean"},"min_vram_mb":{"type":"integer","minimum":0},"pool_label":{"type":"string","maxLength":256},"trajectory_capture":{"type":"boolean"},"min_quality_score":{"type":"integer","minimum":1,"maximum":5}},"required":["description"],"additionalProperties":false}"#,
        ),
        "vox_populi_local_status" => parse_obj(
            r#"{"type":"object","properties":{"registry_path":{"type":"string","description":"Optional override for the mens registry JSON path"}},"additionalProperties":false}"#,
        ),

        // ── Unified news (syndication safety + templates) ───────────────────
        "vox_news_test_syndicate" => parse_obj(
            r#"{"type":"object","properties":{"content":{"type":"string","minLength":1,"description":"Markdown with YAML frontmatter"}},"required":["content"],"additionalProperties":false}"#,
        ),
        "vox_news_draft_research" => parse_obj(
            r#"{"type":"object","properties":{"news_id":{"type":"string","minLength":1,"maxLength":256,"description":"Filename stem for docs/news/drafts/{news_id}.md"},"title":{"type":"string","minLength":1},"author":{"type":"string","minLength":1},"abstract_text":{"type":"string"}},"required":["news_id","title","author","abstract_text"],"additionalProperties":false}"#,
        ),
        "vox_news_approve" => parse_obj(
            r#"{"type":"object","properties":{"news_id":{"type":"string","minLength":1,"maxLength":256},"approver":{"type":"string","minLength":1,"maxLength":256}},"required":["news_id","approver"],"additionalProperties":false}"#,
        ),
        "vox_news_approval_status" => parse_obj(
            r#"{"type":"object","properties":{"news_id":{"type":"string","minLength":1,"maxLength":256}},"required":["news_id"],"additionalProperties":false}"#,
        ),
        "vox_news_simulate_publish_gate" => parse_obj(
            r#"{"type":"object","properties":{"news_id":{"type":"string","minLength":1,"maxLength":256},"content":{"type":"string","minLength":1}},"required":["news_id","content"],"additionalProperties":false}"#,
        ),
        "vox_scientia_publication_prepare" => parse_obj(
            r#"{"type":"object","properties":{"publication_id":{"type":"string","minLength":1,"maxLength":256},"title":{"type":"string","minLength":1},"author":{"type":"string","minLength":1},"content":{"type":"string","minLength":1},"abstract_text":{"type":"string"},"citations_json":{"type":"object"},"scholarly_metadata":{"type":"object","description":"ScientificPublicationMetadata (authors, license_spdx, funding_statement, ...)","additionalProperties":true},"preflight":{"type":"boolean","description":"If true, run publication_preflight before upsert; fail on error-level findings."},"preflight_profile":{"type":"string","enum":["default","double_blind","metadata_complete","arxiv_assist"]},"scientia_evidence":{"type":"object","description":"Optional ScientiaEvidenceContext for metadata_json.scientia_evidence","additionalProperties":true},"discovery_intake_gate":{"type":"string","enum":["none","strong_signals_only","allow_review_suggested"],"description":"Optional scientia intake gate evaluated before upsert"}},"required":["publication_id","title","author","content"],"additionalProperties":false}"#,
        ),
        "vox_scientia_publication_preflight" => parse_obj(
            r#"{"type":"object","properties":{"publication_id":{"type":"string","minLength":1,"maxLength":256},"profile":{"type":"string","enum":["default","double_blind","metadata_complete","arxiv_assist"]},"with_worthiness":{"type":"boolean","description":"If true, attach conservative worthiness rubric output (repo default YAML)."}},"required":["publication_id"],"additionalProperties":false}"#,
        ),
        "vox_scientia_worthiness_evaluate" => parse_obj(
            r#"{"type":"object","properties":{"contract_yaml_relative":{"type":"string","minLength":1,"maxLength":512,"description":"Repo-relative path to worthiness YAML"},"with_live_trust":{"type":"boolean","description":"When true and VoxDb is attached, attach summarized trust_rollups for this repository to the result"},"metrics":{"type":"object","description":"WorthinessInputs","properties":{"red_line_violation_ids":{"type":"array","items":{"type":"string","minLength":1}},"repeated_unresolved_contradiction":{"type":"boolean"},"claim_evidence_coverage":{"type":"number"},"artifact_replayability":{"type":"number"},"before_after_pair_integrity":{"type":"number"},"metadata_completeness":{"type":"number"},"ai_disclosure_compliance":{"type":"number"},"epistemic":{"type":"number"},"reproducibility":{"type":"number"},"novelty":{"type":"number"},"reliability":{"type":"number"},"metadata_policy":{"type":"number"},"meaningful_advance":{"type":"boolean"}},"required":["claim_evidence_coverage","artifact_replayability","before_after_pair_integrity","metadata_completeness","ai_disclosure_compliance","epistemic","reproducibility","novelty","reliability","metadata_policy"],"additionalProperties":false}},"required":["metrics"],"additionalProperties":false}"#,
        ),
        "vox_scientia_publication_approve" => parse_obj(
            r#"{"type":"object","properties":{"publication_id":{"type":"string","minLength":1,"maxLength":256},"approver":{"type":"string","minLength":1,"maxLength":256}},"required":["publication_id","approver"],"additionalProperties":false}"#,
        ),
        "vox_scientia_publication_submit_local" => parse_obj(
            r#"{"type":"object","properties":{"publication_id":{"type":"string","minLength":1,"maxLength":256},"adapter":{"type":"string","minLength":1,"maxLength":64,"description":"Override scholarly adapter (zenodo, openreview, local_ledger, …) instead of VOX_SCHOLARLY_ADAPTER"}},"required":["publication_id"],"additionalProperties":false}"#,
        ),
        "vox_scientia_publication_status" => parse_obj(
            r#"{"type":"object","properties":{"publication_id":{"type":"string","minLength":1,"maxLength":256},"with_worthiness":{"type":"boolean","description":"If true, enrich the embedded preflight report with worthiness scoring and hydrated scientia_evidence sidecars."}},"required":["publication_id"],"additionalProperties":false}"#,
        ),
        "vox_scientia_publication_scholarly_remote_status" => parse_obj(
            r#"{"type":"object","properties":{"publication_id":{"type":"string","minLength":1,"maxLength":256},"external_submission_id":{"type":"string","minLength":1,"maxLength":512}},"required":["publication_id"],"additionalProperties":false}"#,
        ),
        "vox_scientia_publication_scholarly_remote_status_sync_all" => parse_obj(
            r#"{"type":"object","properties":{"publication_id":{"type":"string","minLength":1,"maxLength":256}},"required":["publication_id"],"additionalProperties":false}"#,
        ),
        "vox_scientia_publication_scholarly_remote_status_sync_batch" => parse_obj(
            r#"{"type":"object","properties":{"limit":{"type":"integer","minimum":1,"maximum":500,"default":25},"iterations":{"type":"integer","minimum":1,"maximum":10000,"default":1},"interval_secs":{"type":"integer","minimum":0,"maximum":3600,"default":0},"max_runtime_secs":{"type":"integer","minimum":1,"maximum":86400,"description":"Wall-clock cap for loop mode"},"jitter_secs":{"type":"integer","minimum":0,"maximum":3600,"default":0,"description":"Optional extra sleep jitter bound (capped at interval)"}},"additionalProperties":false}"#,
        ),
        "vox_scientia_publication_arxiv_handoff_record" => parse_obj(
            r#"{"type":"object","properties":{"publication_id":{"type":"string","minLength":1,"maxLength":256},"stage":{"type":"string","enum":["staging_exported","operator_ack","bundle_validated","submitted","published"]},"operator":{"type":"string","maxLength":256},"note":{"type":"string","maxLength":4096},"arxiv_id":{"type":"string","maxLength":256}},"required":["publication_id","stage"],"additionalProperties":false}"#,
        ),
        "vox_scientia_publication_scholarly_staging_export" => parse_obj(
            r#"{"type":"object","properties":{"publication_id":{"type":"string","minLength":1,"maxLength":256},"output_dir":{"type":"string","minLength":1,"maxLength":4096,"description":"Directory to write staging files (created if needed)"},"venue":{"type":"string","minLength":1,"maxLength":64,"description":"zenodo | openreview | arxiv-assist"}},"required":["publication_id","output_dir","venue"],"additionalProperties":false}"#,
        ),
        "vox_scientia_publication_external_jobs_due" => parse_obj(
            r#"{"type":"object","properties":{"limit":{"type":"integer","minimum":1,"maximum":500,"default":50}},"additionalProperties":false}"#,
        ),
        "vox_scientia_publication_external_jobs_dead_letter" => parse_obj(
            r#"{"type":"object","properties":{"limit":{"type":"integer","minimum":1,"maximum":500,"default":50}},"additionalProperties":false}"#,
        ),
        "vox_scientia_publication_external_jobs_replay" => parse_obj(
            r#"{"type":"object","properties":{"job_id":{"type":"integer","minimum":1}},"required":["job_id"],"additionalProperties":false}"#,
        ),
        "vox_scientia_publication_external_jobs_tick" => parse_obj(
            r#"{"type":"object","properties":{"limit":{"type":"integer","minimum":1,"maximum":500,"default":10},"lock_ttl_ms":{"type":"integer","minimum":5000,"maximum":3600000,"default":120000},"lock_owner":{"type":"string","minLength":1,"maxLength":256},"iterations":{"type":"integer","minimum":1,"maximum":10000,"default":1},"interval_secs":{"type":"integer","minimum":0,"maximum":3600,"default":0},"max_runtime_secs":{"type":"integer","minimum":1,"maximum":86400},"jitter_secs":{"type":"integer","minimum":0,"maximum":3600,"default":0}},"additionalProperties":false}"#,
        ),
        "vox_scientia_publication_scholarly_pipeline_run" => parse_obj(
            r#"{"type":"object","properties":{"publication_id":{"type":"string","minLength":1,"maxLength":256},"preflight_profile":{"type":"string","enum":["default","double_blind","metadata_complete"]},"dry_run":{"type":"boolean","default":false},"staging_output_dir":{"type":"string","maxLength":4096},"venue":{"type":"string","maxLength":64,"description":"zenodo | openreview | arxiv-assist when staging_output_dir set"},"adapter":{"type":"string","maxLength":64},"json_compact":{"type":"boolean","default":false,"description":"Single-line JSON in tool result"}},"required":["publication_id"],"additionalProperties":false}"#,
        ),
        "vox_scientia_publication_external_pipeline_metrics" => parse_obj(
            r#"{"type":"object","properties":{"since_hours":{"type":"integer","minimum":0,"maximum":8760,"default":168,"description":"Window for attempts/snapshots/latencies/publication_attempts; 0 = all time"}},"additionalProperties":false}"#,
        ),
        "vox_scientia_publication_media_list" => parse_obj(
            r#"{"type":"object","properties":{"publication_id":{"type":"string","minLength":1,"maxLength":256}},"required":["publication_id"],"additionalProperties":false}"#,
        ),
        "vox_scientia_publication_media_delete" => parse_obj(
            r#"{"type":"object","properties":{"publication_id":{"type":"string","minLength":1,"maxLength":256},"asset_ref":{"type":"string","minLength":1,"maxLength":2048}},"required":["publication_id","asset_ref"],"additionalProperties":false}"#,
        ),
        "vox_scientia_publication_media_upsert" => parse_obj(
            r#"{"type":"object","properties":{"publication_id":{"type":"string","minLength":1,"maxLength":256},"asset_ref":{"type":"string","minLength":1,"maxLength":2048},"media_type":{"type":"string","minLength":1,"maxLength":64},"storage_uri":{"type":"string","maxLength":4096},"status":{"type":"string","minLength":1,"maxLength":64},"metadata_json":{"type":"object","additionalProperties":true}},"required":["publication_id","asset_ref","media_type","status"],"additionalProperties":false}"#,
        ),
        "vox_scientia_publication_route_simulate" => parse_obj(
            r#"{"type":"object","properties":{"publication_id":{"type":"string","minLength":1,"maxLength":256}},"required":["publication_id"],"additionalProperties":false}"#,
        ),
        "vox_scientia_publication_publish" => parse_obj(
            r#"{"type":"object","properties":{"publication_id":{"type":"string","minLength":1,"maxLength":256},"channels":{"type":"array","items":{"type":"string","minLength":1,"maxLength":64}},"dry_run":{"type":"boolean"},"json":{"type":"boolean"}},"required":["publication_id"],"additionalProperties":false}"#,
        ),
        "vox_scientia_publication_retry_failed" => parse_obj(
            r#"{"type":"object","properties":{"publication_id":{"type":"string","minLength":1,"maxLength":256},"channel":{"type":"string","minLength":1,"maxLength":64},"dry_run":{"type":"boolean"},"json":{"type":"boolean"}},"required":["publication_id"],"additionalProperties":false}"#,
        ),
        "vox_scientia_publication_discovery_scan" => parse_obj(
            r#"{"type":"object","properties":{"content_type":{"type":"string","maxLength":64,"description":"Filter manifests (e.g. scientia)"},"state":{"type":"string","maxLength":64,"description":"Filter by manifest state"},"limit":{"type":"integer","minimum":1,"maximum":500,"default":50}},"additionalProperties":false}"#,
        ),
        "vox_scientia_publication_discovery_explain" => parse_obj(
            r#"{"type":"object","properties":{"publication_id":{"type":"string","minLength":1,"maxLength":256}},"required":["publication_id"],"additionalProperties":false}"#,
        ),
        "vox_scientia_publication_discovery_refresh_evidence" => parse_obj(
            r#"{"type":"object","properties":{"publication_id":{"type":"string","minLength":1,"maxLength":256}},"required":["publication_id"],"additionalProperties":false}"#,
        ),
        "vox_scientia_publication_novelty_fetch" => parse_obj(
            r#"{"type":"object","properties":{"publication_id":{"type":"string","minLength":1,"maxLength":256},"offline":{"type":"boolean"},"persist_metadata":{"type":"boolean"}},"required":["publication_id"],"additionalProperties":false}"#,
        ),
        "vox_scientia_publication_decision_explain" => parse_obj(
            r#"{"type":"object","properties":{"publication_id":{"type":"string","minLength":1,"maxLength":256},"live_prior_art":{"type":"boolean"},"offline":{"type":"boolean"}},"required":["publication_id"],"additionalProperties":false}"#,
        ),
        "vox_scientia_publication_novelty_happy_path" => parse_obj(
            r#"{"type":"object","properties":{"publication_id":{"type":"string","minLength":1,"maxLength":256},"offline":{"type":"boolean"}},"required":["publication_id"],"additionalProperties":false}"#,
        ),
        "vox_scientia_assist_suggestions" => parse_obj(
            r#"{"type":"object","properties":{"publication_id":{"type":"string","minLength":1,"maxLength":256},"use_llm":{"type":"boolean","description":"When false, return heuristic JSON only"}},"required":["publication_id"],"additionalProperties":false}"#,
        ),

        _ => Map::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::tool_input_schema;
    use crate::TOOL_REGISTRY;
    use serde_json::Value;

    fn resolve_local_ref<'a>(root: &'a Value, reference: &str) -> Option<&'a Value> {
        let ptr = reference.strip_prefix('#')?;
        root.pointer(ptr)
    }

    fn schema_has_concrete_shape(schema: &Value, root: &Value) -> bool {
        let Some(obj) = schema.as_object() else {
            return false;
        };
        if let Some(reference) = obj.get("$ref").and_then(Value::as_str) {
            return resolve_local_ref(root, reference).is_some();
        }
        obj.contains_key("type")
            || obj.contains_key("anyOf")
            || obj.contains_key("oneOf")
            || obj.contains_key("allOf")
    }

    fn schema_min_length_at_least_one_when_present(schema: &Value, root: &Value) -> bool {
        let Some(obj) = schema.as_object() else {
            return true;
        };
        if let Some(reference) = obj.get("$ref").and_then(Value::as_str) {
            return resolve_local_ref(root, reference)
                .map(|resolved| schema_min_length_at_least_one_when_present(resolved, root))
                .unwrap_or(true);
        }
        if let Some(min_len) = obj.get("minLength").and_then(Value::as_u64) {
            return min_len >= 1;
        }
        ["anyOf", "oneOf", "allOf"].iter().all(|k| {
            obj.get(*k)
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .all(|v| schema_min_length_at_least_one_when_present(v, root))
                })
                .unwrap_or(true)
        })
    }

    #[test]
    fn registry_tools_have_input_schema_coverage() {
        let mut missing = Vec::new();
        for e in TOOL_REGISTRY {
            let name = e.name;
            if tool_input_schema(name).is_empty() {
                missing.push(name);
            }
        }
        assert!(
            missing.is_empty(),
            "TOOL_REGISTRY tools missing non-empty input_schema: {missing:?}"
        );
    }

    #[test]
    fn submit_task_schema_has_files_array() {
        let m = tool_input_schema("vox_submit_task");
        let props = m.get("properties").and_then(|p| p.as_object()).unwrap();
        assert!(props.contains_key("files"));
        assert!(props.contains_key("description"));
    }

    #[test]
    fn submit_task_schema_exposes_context_envelope_json() {
        let m = tool_input_schema("vox_submit_task");
        let root = Value::Object(m.clone());
        let props = m.get("properties").and_then(|p| p.as_object()).unwrap();
        assert!(
            props.contains_key("context_envelope_json"),
            "missing context_envelope_json field"
        );
        let context_prop = props
            .get("context_envelope_json")
            .expect("context_envelope_json");
        assert!(
            schema_has_concrete_shape(context_prop, &root),
            "context_envelope_json must expose a concrete schema shape or resolvable $ref"
        );
        assert!(
            schema_min_length_at_least_one_when_present(context_prop, &root),
            "if minLength is present for context_envelope_json, it must be >= 1"
        );
        let required = m
            .get("required")
            .and_then(|r| r.as_array())
            .expect("required array");
        assert!(
            !required
                .iter()
                .any(|v| v.as_str() == Some("context_envelope_json")),
            "context_envelope_json should be optional"
        );
        assert_eq!(
            m.get("additionalProperties").and_then(|v| v.as_bool()),
            Some(false)
        );
    }

    #[test]
    fn submit_task_schema_exposes_harness_spec_json() {
        let m = tool_input_schema("vox_submit_task");
        let root = Value::Object(m.clone());
        let props = m.get("properties").and_then(|p| p.as_object()).unwrap();
        assert!(
            props.contains_key("harness_spec_json"),
            "missing harness_spec_json field"
        );
        let harness_prop = props.get("harness_spec_json").expect("harness_spec_json");
        assert!(
            schema_has_concrete_shape(harness_prop, &root),
            "harness_spec_json must expose a concrete schema shape or resolvable $ref"
        );
        assert!(
            schema_min_length_at_least_one_when_present(harness_prop, &root),
            "if minLength is present for harness_spec_json, it must be >= 1"
        );
    }

    #[test]
    fn map_session_deprecated_aliases_match_canonical_schema() {
        let canonical = tool_input_schema("vox_map_agent_session");
        assert!(!canonical.is_empty());
        assert_eq!(tool_input_schema("vox_map_opencode_session"), canonical);
        assert_eq!(tool_input_schema("vox_map_vscode_session"), canonical);
    }

    #[test]
    fn config_deprecated_aliases_match_canonical_schema() {
        let canonical = tool_input_schema("vox_config_get");
        assert!(!canonical.is_empty());
        assert_eq!(tool_input_schema("vox_get_config"), canonical);
        let set_canon = tool_input_schema("vox_config_set");
        assert!(!set_canon.is_empty());
        assert_eq!(tool_input_schema("vox_set_config"), set_canon);
    }
    #[test]
    fn chat_message_schema_requires_prompt_or_message() {
        let m = tool_input_schema("vox_chat_message");
        assert!(
            m.get("anyOf").is_some(),
            "expected anyOf for prompt|message"
        );
        let props = m.get("properties").and_then(|p| p.as_object()).unwrap();
        let p = props.get("prompt").and_then(|v| v.as_object()).unwrap();
        assert_eq!(p.get("minLength").and_then(|x| x.as_u64()), Some(1));
    }

    #[test]
    fn submit_task_description_has_length_bounds() {
        let m = tool_input_schema("vox_submit_task");
        let props = m.get("properties").and_then(|p| p.as_object()).unwrap();
        let d = props
            .get("description")
            .and_then(|v| v.as_object())
            .unwrap();
        assert_eq!(d.get("minLength").and_then(|x| x.as_u64()), Some(1));
        assert_eq!(d.get("maxLength").and_then(|x| x.as_u64()), Some(131072));
    }

    #[test]
    fn submit_task_capabilities_schema_covers_extended_hints() {
        let m = tool_input_schema("vox_submit_task");
        let props = m.get("properties").and_then(|p| p.as_object()).unwrap();
        let caps = props
            .get("capabilities")
            .and_then(|v| v.as_object())
            .unwrap();
        let cap_props = caps.get("properties").and_then(|p| p.as_object()).unwrap();
        for key in [
            "gpu_cuda",
            "gpu_metal",
            "min_vram_mb",
            "cpu_cores",
            "arch",
            "hostname",
            "labels",
            "min_cpu_cores",
            "prefer_gpu_compute",
        ] {
            assert!(
                cap_props.contains_key(key),
                "capabilities.properties missing {key:?}"
            );
        }
    }

    #[test]
    fn map_session_session_id_has_bounds() {
        let m = tool_input_schema("vox_map_agent_session");
        let props = m.get("properties").and_then(|p| p.as_object()).unwrap();
        let s = props.get("session_id").and_then(|v| v.as_object()).unwrap();
        assert_eq!(s.get("minLength").and_then(|x| x.as_u64()), Some(1));
        assert_eq!(s.get("maxLength").and_then(|x| x.as_u64()), Some(2048));
    }

    #[test]
    fn generate_code_schema_matches_implemented_args() {
        let m = tool_input_schema("vox_generate_code");
        let props = m.get("properties").and_then(|p| p.as_object()).unwrap();
        for key in [
            "prompt",
            "validate",
            "max_retries",
            "session_id",
            "output_surface_mode",
        ] {
            assert!(props.contains_key(key), "missing property {key}");
        }
        assert_eq!(
            m.get("additionalProperties").and_then(|x| x.as_bool()),
            Some(false)
        );
        let req = m
            .get("required")
            .and_then(|r| r.as_array())
            .expect("required array");
        assert!(
            req.iter().any(|x| x.as_str() == Some("prompt")),
            "prompt must be required"
        );
    }

    #[test]
    fn agent_handoff_schema_exposes_context_envelope_json() {
        let m = tool_input_schema("vox_agent_handoff");
        let props = m.get("properties").and_then(|p| p.as_object()).unwrap();
        for key in [
            "from_agent_id",
            "to_agent_id",
            "plan_summary",
            "unresolved_objectives",
            "verification_criteria",
            "context_envelope_json",
        ] {
            assert!(props.contains_key(key), "missing property {key}");
        }
        let context_prop = props
            .get("context_envelope_json")
            .and_then(|v| v.as_object())
            .expect("context_envelope_json object");
        assert_eq!(
            context_prop.get("type").and_then(|v| v.as_str()),
            Some("string")
        );
        assert_eq!(
            context_prop.get("minLength").and_then(|v| v.as_u64()),
            Some(1)
        );
        let required = m
            .get("required")
            .and_then(|r| r.as_array())
            .expect("required array");
        assert!(required.iter().any(|v| v.as_str() == Some("from_agent_id")));
        assert!(required.iter().any(|v| v.as_str() == Some("to_agent_id")));
        assert!(required.iter().any(|v| v.as_str() == Some("plan_summary")));
        assert!(
            !required
                .iter()
                .any(|v| v.as_str() == Some("context_envelope_json"))
        );
        assert_eq!(
            m.get("additionalProperties").and_then(|v| v.as_bool()),
            Some(false)
        );
    }
    #[test]
    fn all_parse_obj_schemas_are_valid_jsonschema() {
        // Draft-07 meta-schema
        let meta: serde_json::Value = serde_json::from_str(include_str!(
            "../../../contracts/capability/json-schema-draft-07-meta.json"
        ))
        .expect("meta schema");
        let compiled = jsonschema::validator_for(&meta).expect("compile meta-schema");
        for entry in TOOL_REGISTRY {
            // Check only parse_obj generated tools (these are the ones missing structure derivations).
            // (Testing all tools ensures full schema validity)
            let schema = serde_json::Value::Object(tool_input_schema(entry.name));
            if schema.as_object().map(|m| m.is_empty()).unwrap_or(true) {
                continue;
            }
            if let Err(error) = compiled.validate(&schema) {
                panic!("Tool '{}' has invalid JSON Schema: {}", entry.name, error);
            }
        }
    }
}
