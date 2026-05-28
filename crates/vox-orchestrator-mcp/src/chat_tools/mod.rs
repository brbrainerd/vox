//! Chat, inline edit, planning, ghost text, and ambient editor tools for the Vox MCP server.
//!
//! These back the VS Code extension thin-client layer. All context gathering,
//! @mention resolution, LLM routing, and history persistence happen here in Rust.

pub mod params;

mod ambient;
mod chat;
mod ghost_text;
mod inline_edit;
mod plan;
mod plan_gap;
mod plan_loop;

pub use ambient::ambient_state;
pub use chat::{chat_history, chat_message};
pub use ghost_text::ghost_text;
pub use inline_edit::inline_edit;
pub use params::*;
pub use plan::{plan_goal, plan_list_sessions, plan_replan, plan_resume, plan_status};
pub use plan_gap::analyze_plan_gaps;

use std::time::{SystemTime, UNIX_EPOCH};

use super::chat_socrates_meta::socrates_system_rider;
use crate::server_state::ServerState;

pub(crate) fn now_ts() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Simple ISO date formatter (YYYY-MM-DD) without external chrono/time deps.
pub(crate) fn ts_to_date_str(secs: u64) -> String {
    let days = secs / 86400;
    // Base 1970-01-01 was a Thursday
    // Simple proleptic Gregorian algorithm (good until 2100)
    let z = (days as i64) + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{:04}-{:02}-{:02}", y + if m <= 2 { 1 } else { 0 }, m, d)
}

/// Build the full system prompt for the Vox chat assistant.
pub(crate) async fn build_system_prompt(state: &ServerState, session_id: Option<&str>) -> String {
    let ws_root = state
        .workspace_root
        .as_deref()
        .unwrap_or(std::path::Path::new("."));

    let mut prompt = String::from(
        "You are assisting with the **Vox** programming language and its ecosystem. \
         Vox is AI-native, full-stack, and compiles to Rust/TypeScript/WASM. \
         Prefer `Option[T]` and explicit errors over null.\n\n",
    );

    let vox_md = ws_root.join("VOX.md");
    if let Ok(content) = vox_bounded_fs::read_utf8_path_capped(&vox_md) {
        prompt.push_str("## VOX.md\n\n");
        prompt.push_str(&content);
        prompt.push_str("\n\n");
    }

    let memory_path = state.orchestrator_config.memory.memory_md_path.clone();
    if let Ok(content) = vox_bounded_fs::read_utf8_path_capped(&memory_path) {
        prompt.push_str("## Repository memory (MEMORY.md)\n\n");
        prompt.push_str(&content);
        prompt.push_str("\n\n");
    } else {
        // Legacy layout (pre–`.vox/memory/`): single file at repo `.vox/MEMORY.md`
        let legacy = ws_root.join(vox_config::paths::REPO_MEMORY_INDEX_FILE);
        if legacy != memory_path {
            if let Ok(content) = vox_bounded_fs::read_utf8_path_capped(&legacy) {
                prompt.push_str("## Repository memory (.vox/MEMORY.md legacy)\n\n");
                prompt.push_str(&content);
                prompt.push_str("\n\n");
            }
        }
    }

    prompt.push_str(&format!(
        "## Environment\nWorkspace Root: {}\n\nYou are Vox, an elite AI coding assistant. You have access to the Vox MCP toolbelt. You can read and modify files, run tests, inspect VCS history, manage agents, and query the knowledge graph.\n\nRules:\n- Be concise and precise. Prefer code over prose.\n- Always cite which files you modified or plan to modify.\n- When generating code, produce valid, complete implementations — no stubs or placeholders.\n- Use Markdown code blocks with language tags.\n- For multi-file changes, use a structured diff or list each file separately.\n- When asked to plan, produce a numbered task list in Markdown.\n",
        ws_root.display()
    ));

    prompt.push_str(params::ANTI_LAZINESS_RIDER);

    prompt.push_str(
        "\n\n## Premature completion / anti-skeleton (Vox SSOT)\n\
         Do not treat plans or code as finished without **verifiable** evidence (tests passing, CI gates, or an explicit per-file audit). \
         Plans must name concrete paths, impacted callers, and verification steps — avoid thin task lists. \
         Repository policy: `contracts/operations/completion-policy.v1.yaml`; CI guard: `vox ci completion-audit` (TOESTUB victory-claim merge when built with `completion-toestub`).\n",
    );

    let ts = now_ts();
    let date_str = ts_to_date_str(ts);

    let bm = state.orchestrator.budget_manager_handle();
    let attention_budget = vox_orchestrator::sync_lock::rw_read(&*bm).attention_signal(0.7);

    // NOTE: Keep this section day-stable to preserve DeepSeek/Anthropic prompt-prefix caching.
    // Do NOT embed per-second Unix timestamps or server-idle counters here — they bust the
    // prefix cache on every call. Per-request volatile data belongs in the user prompt.
    prompt.push_str(&format!(
        "\n\n## Temporal Context\nCurrent date: {date_str}.\n\
         **Enforcement**: Before triggering any compilation, re-reindexing, or full file walk, \
         check if things are fresh (< 30s since last run).\n"
    ));

    prompt.push_str(&format!(
        "\n\n## Budget Status\nAttention Budget Signal: {:?}\nIf the budget is 'HighLoad' or 'Critical', you MUST summarize and abort your workflow immediately to defer to the operator.\n",
        attention_budget
    ));

    let pol = state.orchestrator_config.effective_socrates_policy();
    prompt.push_str(&socrates_system_rider(&pol));

    // Attempt to pull Operating Mode from the session's active ContextEnvelope
    if let Some(session_id) = session_id {
        let env_key = vox_orchestrator::socrates::session_context_envelope_key(session_id);
        let store = state.orchestrator.context_store();
        if let Some(env_raw) = crate::sync_poison::poison_rw_read(store.read(), "context_store")
            .ok()
            .and_then(|g| g.get(&env_key).clone())
        {
            if let Ok(env) = serde_json::from_str::<vox_orchestrator::ContextEnvelope>(&env_raw) {
                if let Some(mode) = env.operating_mode {
                    prompt.push_str(&mode.system_rider());
                }
            }
        }
    }

    prompt
}

#[cfg(test)]
mod routing_tests {
    use super::super::chat_socrates_meta::{SocratesJsonMeta, socrates_tool_meta};
    use super::chat::mentions::{chat_grounding_score, safe_truncate_for_prompt};
    use super::ghost_text::ghost_grounding_score;
    use super::params::{ChatMessageParams, GhostTextParams, PlanTask};
    use crate::llm_bridge::clamp_http_max_output_tokens;
    use vox_orchestrator_types::socrates_policy::ConfidencePolicy;

    #[test]
    fn clamp_http_max_output_respects_bounds() {
        assert_eq!(clamp_http_max_output_tokens(0), 1);
        assert_eq!(clamp_http_max_output_tokens(100), 100);
        assert_eq!(clamp_http_max_output_tokens(9000), 8192);
    }

    #[test]
    fn socrates_meta_contains_required_fields() {
        let p = ConfidencePolicy::workspace_default();
        let v = socrates_tool_meta(&p, 0.61, false, 0, 0, 0, None);
        assert!(v.get("risk_decision").is_some());
        assert!(v.get("confidence_estimate").is_some());
        assert!(v.get("contradiction_ratio").is_some());
    }

    #[test]
    fn socrates_tool_meta_matches_telemetry_deserializer() {
        let p = ConfidencePolicy::workspace_default();
        let v = socrates_tool_meta(&p, 0.71, true, 0, 0, 0, None);
        let m: SocratesJsonMeta = serde_json::from_value(v).expect("telemetry JSON must parse");
        assert!((m.confidence_estimate - 0.71).abs() < 1e-9);
        assert!((m.contradiction_ratio - 0.35).abs() < 1e-9);
    }

    #[test]
    fn socrates_tool_meta_includes_retrieval_refinement_hints() {
        let p = ConfidencePolicy::workspace_default();
        let retrieval = crate::memory::RetrievalEvidenceEnvelope {
            trigger: crate::memory::RetrievalTriggerMode::ExplicitToolQuery,
            retrieval_tier: "lexical_fallback".to_string(),
            memory_hit_count: 1,
            knowledge_hit_count: 0,
            chunk_hit_count: 0,
            repo_hit_count: 1,
            used_vector: false,
            used_bm25: false,
            used_lexical_fallback: true,
            contradiction_count: 0,
            top_score: Some(0.2),
            search_intent: "code_navigation".to_string(),
            selected_mode: "fulltext".to_string(),
            backend_mix: vec!["repo_path".to_string()],
            source_diversity: 1,
            evidence_quality: 0.2,
            citation_coverage: 0.25,
            verification_performed: true,
            verification_reason: Some("lexical_fallback_only".to_string()),
            verification_query: Some("memorysearchengine".to_string()),
            recommended_next_action: Some("focus_repo".to_string()),
            search_plan: serde_json::json!({ "intent": "code_navigation" }),
            search_diagnostics: serde_json::json!({ "verification_performed": true }),
            sqlite_journal_mode: None,
            sqlite_fts5_reported: None,
            sqlite_foreign_keys_on: None,
            rrf_fused_hit_count: 0,
        };
        let v = socrates_tool_meta(&p, 0.48, false, 0, 0, 0, Some(&retrieval));
        let refinement = v.get("search_refinement").expect("search_refinement field");
        assert_eq!(refinement["recommended_action"], "focus_repo");
        assert_eq!(refinement["verification_performed"], true);
    }

    #[test]
    fn ghost_grounding_score_respects_file_and_fim_boundaries() {
        let thin = GhostTextParams {
            prefix: "a".into(),
            suffix: "".into(),
            language: None,
            file_path: None,
            max_tokens: None,
            session_id: None,
            temperature: None,
            top_p: None,
        };
        let rich = GhostTextParams {
            prefix: "fn main() {\n    let x = 1;\n".into(),
            suffix: "\n}\n".into(),
            language: Some("rust".into()),
            file_path: Some("src/main.rs".into()),
            max_tokens: None,
            session_id: None,
            temperature: None,
            top_p: None,
        };
        assert!(ghost_grounding_score(&rich) > ghost_grounding_score(&thin));
    }

    #[test]
    fn grounding_score_increases_with_context() {
        let empty = ChatMessageParams {
            prompt: "Hi".into(),
            context_files: vec![],
            open_files: vec![],
            active_file: None,
            active_line: None,
            selected_text: None,
            diagnostics: vec![],
            session_id: None,
            thread_id: None,
            journey_id: None,
            cognitive_profile: None,
            json_mode: false,
            trace_id: None,
            correlation_id: None,
            attachment_manifest: None,
            temperature: None,
            top_p: None,
        };
        let rich = ChatMessageParams {
            prompt: "Hi".into(),
            context_files: vec!["foo.rs".into()],
            open_files: vec!["bar.rs".into()],
            active_file: Some("src/main.rs".into()),
            active_line: Some(42),
            selected_text: Some("let x = 1;".into()),
            diagnostics: vec![],
            session_id: None,
            thread_id: None,
            journey_id: None,
            cognitive_profile: None,
            json_mode: false,
            trace_id: None,
            correlation_id: None,
            attachment_manifest: None,
            temperature: None,
            top_p: None,
        };
        let a = chat_grounding_score(&empty, 0);
        let b = chat_grounding_score(&rich, 3);
        assert!(b > a);
    }

    #[test]
    fn test_plan_response_schema_extraction() {
        let json = r#"{
            "summary": "Fixing the bug",
            "tasks": [
                { "id": 1, "description": "Identify root cause", "files": ["src/main.rs"], "estimated_complexity": 2, "depends_on": [] },
                { "id": 2, "description": "Write fix", "files": ["src/main.rs"], "estimated_complexity": 3, "depends_on": [1] }
            ]
        }"#;
        let parsed: serde_json::Value = serde_json::from_str(json).expect("valid JSON");
        assert_eq!(parsed["summary"], "Fixing the bug");
        let tasks = parsed["tasks"].as_array().expect("tasks array");
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0]["id"], 1);
        let deps: Vec<usize> = serde_json::from_value(tasks[1]["depends_on"].clone()).unwrap();
        assert_eq!(deps, vec![1]);
    }

    #[test]
    fn test_plan_schema_empty_tasks_is_valid() {
        let json = r#"{"summary": "Empty plan", "tasks": []}"#;
        let parsed: serde_json::Value = serde_json::from_str(json).expect("valid JSON");
        assert_eq!(parsed["summary"], "Empty plan");
        assert_eq!(parsed["tasks"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_plan_schema_raw_json_no_fence() {
        let json = r#"{
            "summary": "Raw JSON",
            "tasks": [
                { "id": 1, "description": "Do thing", "files": [], "estimated_complexity": 1, "depends_on": [] }
            ]
        }"#;
        let tasks: Vec<PlanTask> = serde_json::from_value(
            serde_json::from_str::<serde_json::Value>(json).unwrap()["tasks"].clone(),
        )
        .expect("PlanTask deserialization");
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].description, "Do thing");
        assert_eq!(tasks[0].estimated_complexity, 1);
        assert!(tasks[0].depends_on.is_empty());
    }

    #[test]
    fn truncate_for_prompt_keeps_utf8_boundaries() {
        let s = "abc🙂def🙂ghi";
        let t = safe_truncate_for_prompt(s, 7);
        assert!(t.contains("...[truncated]..."));
        let prefix = t.split("\n...[truncated]...").next().unwrap_or("");
        assert!(s.starts_with(prefix));
        assert!(!prefix.contains('\u{FFFD}'));
    }
}
