//! Chat, inline edit, planning, ghost text, and ambient editor tools for the Vox MCP server.
//!
//! These back the VS Code extension thin-client layer. All context gathering,
//! @mention resolution, LLM routing, and history persistence happen here in Rust.

pub mod params;

mod ambient;
pub mod chat;
mod ghost_text;
mod inline_edit;
mod plan;
mod plan_gap;
mod plan_loop;
mod skill_catalog;

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
use sha2::{Digest, Sha256};
use vox_telemetry::{SkillActivationEvent, TelemetryEvent};

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

/// Map a raw model key to the taxonomy `canonical_model_id` enum bucket.
///
/// The taxonomy uses underscored family names; the registry key may use hyphens
/// or provider-specific suffixes.  This is a best-effort prefix match; unknown
/// models map to "unknown" so they are dropped by the server-side allowlist.
fn telemetry_model_bucket(key: &str) -> &'static str {
    if key.contains("fable") {
        "claude_fable_5"
    } else if key.contains("opus") {
        "claude_opus_4"
    } else if key.contains("haiku") {
        "claude_haiku_4"
    } else if key.contains("sonnet") {
        "claude_sonnet_4"
    } else if key.contains("gemini") && key.contains("flash") {
        "gemini_flash_2"
    } else if key.contains("gemini") {
        "gemini_pro_2"
    } else if key.contains("gpt-4o-mini") || key.contains("gpt4o_mini") {
        "gpt4o_mini"
    } else if key.contains("gpt-4o") || key.contains("gpt4o") {
        "gpt4o"
    } else if key.contains("deepseek") {
        "deepseek_v3"
    } else if key.contains("ollama") || key.contains("local") {
        "local_ollama"
    } else {
        "unknown"
    }
}

/// Build the full system prompt for the Vox chat assistant.
pub(crate) async fn build_system_prompt(state: &ServerState, session_id: Option<&str>) -> String {
    build_system_prompt_with_skill(state, session_id, None, None, &[]).await
}

/// Drop excluded skills before either consumer (the tier-1 catalog and the
/// pinned-skill lookup) ever sees them. A single filter point upstream of
/// both — rather than filtering each consumer separately — is what makes
/// "cannot be pinned" a structural guarantee instead of a second place this
/// could be (and, per the harness-unification plan's Task E2 note, first
/// was) forgotten.
fn filter_excluded_skills(
    manifests: Vec<vox_skills::SkillManifest>,
    excluded: &[String],
) -> Vec<vox_skills::SkillManifest> {
    if excluded.is_empty() {
        return manifests;
    }
    manifests
        .into_iter()
        .filter(|m| !excluded.iter().any(|e| e == &m.id || e == &m.name))
        .collect()
}

#[cfg(test)]
mod skill_exclusion_tests {
    use super::filter_excluded_skills;
    use vox_skills::SkillManifest;

    fn manifest(id: &str) -> SkillManifest {
        SkillManifest {
            id: id.to_string(),
            name: id.to_string(),
            version: "1.0.0".to_string(),
            description: "d".to_string(),
            ..Default::default()
        }
    }

    /// Task E2 Step 1: an excluded id must be absent from the rendered
    /// catalog and cannot be pinned. `filter_excluded_skills` is the single
    /// upstream filter `build_system_prompt_with_skill` applies before
    /// either consumer runs, so "absent from the catalog" and "cannot be
    /// pinned" are both proven by this one assertion: the excluded skill is
    /// gone from the list both the catalog renderer and the pinned-skill
    /// `.find()` read from.
    #[test]
    fn excluded_skill_is_absent_and_therefore_unpinnable() {
        let manifests = vec![manifest("ponytail"), manifest("brainstorming")];
        let filtered = filter_excluded_skills(manifests, &["ponytail".to_string()]);
        assert!(filtered.iter().all(|m| m.id != "ponytail"));
        assert!(filtered.iter().any(|m| m.id == "brainstorming"));
        // The pinned-skill lookup in `build_system_prompt_with_skill` is
        // `manifests.iter().find(|m| m.id == pinned || m.name == pinned)` —
        // proving it against the SAME filtered list is what makes "cannot be
        // pinned" a real assertion, not just a catalog-rendering one.
        assert!(filtered.iter().find(|m| m.id == "ponytail").is_none());
    }

    #[test]
    fn no_exclusions_is_a_no_op() {
        let manifests = vec![manifest("a"), manifest("b")];
        let filtered = filter_excluded_skills(manifests, &[]);
        assert_eq!(filtered.len(), 2);
    }
}

/// Like [`build_system_prompt`], but injects the full body of a user-pinned
/// skill (by id or name) so prompt-only models honor it without a tool call.
///
/// When `model_key` is `Some`, a cache-stable `## Model guidance` segment is
/// appended after the skill catalog if a `Confirmed` prompt profile exists for
/// that key (see `ModelPromptRegistry`).
pub(crate) async fn build_system_prompt_with_skill(
    state: &ServerState,
    session_id: Option<&str>,
    pinned_skill: Option<&str>,
    model_key: Option<&str>,
    excluded_skills: &[String],
) -> String {
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

    // Tier-1 skill disclosure (agentskills.io progressive disclosure): name +
    // description for every installed skill, so even prompt-only models (MENS)
    // know which skills exist. Alphabetical + capped → cache-prefix stable.
    // One registry read, reused below for the pinned-skill lookup.
    let reg = &state.orchestrator.skill_registry;
    // Filtered once here: both the tier-1 catalog below and the pinned-skill
    // lookup (`manifests.iter().find(...)`) read from this same, already-
    // excluded list — see `filter_excluded_skills`'s doc comment.
    let manifests = filter_excluded_skills(reg.list(None), excluded_skills);
    // Task 3.1: join against `reliability_scores` (`entity_type = 'skill'`) so
    // the catalog ranks by reliability rather than alphabet. No producer has
    // written skill rows yet, so this is typically empty — every skill then
    // falls back to alphabetical, which is exactly the graceful-degradation
    // behavior `render_skill_catalog` implements. A DB error is swallowed to
    // `None`-for-all rather than failing the whole system prompt.
    let reliability: std::collections::HashMap<String, f64> = match state.db.as_ref() {
        Some(db) => db.list_skill_reliability().await.unwrap_or_default(),
        None => std::collections::HashMap::new(),
    };
    let skill_entries: Vec<skill_catalog::CatalogEntry> = manifests
        .iter()
        .map(|m| skill_catalog::CatalogEntry {
            name: m.name.clone(),
            description: m.description.clone(),
            reliability: reliability.get(&m.name).copied(),
        })
        .collect();
    prompt.push_str(&skill_catalog::render_skill_catalog(&skill_entries, 64));

    // Pinned-skill ("tier-pinned") disclosure: inject the full SKILL.md body
    // for an explicitly selected skill. Matched by id or name; works on the
    // prompt-only MENS path because it needs no tool round-trip.
    if let Some(pinned) = pinned_skill.map(str::trim).filter(|p| !p.is_empty()) {
        if let Some(m) = manifests
            .iter()
            .find(|m| m.id == pinned || m.name == pinned)
        {
            let body = reg.lookup(&m.id).ok().map(|s| s.body).unwrap_or_default();
            if !body.is_empty() {
                tracing::info!(skill = %m.id, source = "pinned", "skill_activated");
                // Track E — skill_activation: hash the id with install-salt (never upload raw id).
                let salt = vox_telemetry::config::install_salt();
                let mut hasher = Sha256::new();
                hasher.update(salt);
                hasher.update(m.id.as_bytes());
                let hash = hasher
                    .finalize()
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>();
                vox_telemetry::record_event!(&TelemetryEvent::SkillActivation(
                    SkillActivationEvent {
                        skill_id_hash: hash,
                        trigger_source: "pinned".to_string(),
                        accepted: true,
                        surface: "mcp".to_string(),
                    }
                ));
                prompt.push_str(&skill_catalog::render_pinned_skill(&m.name, &body));
            }
        } else {
            tracing::warn!(pinned = %pinned, "pinned skill not found in registry");
        }
    }

    // F3/F6: inject cache-stable model guidance + emit telemetry for Confirmed profiles.
    if let Some(key) = model_key {
        let profile_variant_id = match state.model_prompt_registry.active_profile(key) {
            Some(p) => {
                prompt.push_str(&format!(
                    "\n\n## Model guidance ({})\n\n{}\n",
                    key, p.preamble_text
                ));
                format!("confirmed_{}", p.variant_id)
            }
            None => "none".to_string(),
        };
        // Normalize to taxonomy canonical_model_id enum (underscores, family-level bucketing).
        let canonical_model_id = telemetry_model_bucket(key);
        vox_telemetry::record_event!(&TelemetryEvent::ModelPrompt(
            vox_telemetry::ModelPromptEvent {
                canonical_model_id: canonical_model_id.to_string(),
                profile_variant_id,
                task_category: "unknown".to_string(),
                quality_bucket: "unknown".to_string(),
            }
        ));
    }

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
            skill: None,
            model_override: None,
            tier: None,
            clutch: None,
            risk: None,
            skill_exclusions: vec![],
            mode: None,
            priority: None,
            dry_run: None,
            force_research: None,
            research_scope: None,
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
            skill: None,
            model_override: None,
            tier: None,
            clutch: None,
            risk: None,
            skill_exclusions: vec![],
            mode: None,
            priority: None,
            dry_run: None,
            force_research: None,
            research_scope: None,
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
