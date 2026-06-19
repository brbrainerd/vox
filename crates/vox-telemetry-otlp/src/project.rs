use serde_json::Value;
use vox_telemetry::TelemetryEvent;

/// Maps a `TelemetryEvent` to its collection `(category, flat_map)`, applying privacy
/// transforms. Returns `None` for variants with no product-relevant mapping (not uploaded).
///
/// Privacy transforms applied here (layer 1 of the two-layer egress guard):
/// - `session_id` → prefix kept as enum under `session_prefix`; suffix is salt-hashed
///   under `session_suffix_hash` (salt comes from `vox_telemetry::config::install_salt`).
/// - `metadata_json` and any free-form `String` → dropped entirely.
/// - Numeric/enum/bool fields → passed through under their taxonomy field name.
pub fn project_event(event: &TelemetryEvent) -> Option<(String, serde_json::Map<String, Value>)> {
    match event {
        TelemetryEvent::ResearchMetric(e) => {
            let mut map = serde_json::Map::new();
            let prefix = e.session_id.split(':').next().unwrap_or("unknown");
            map.insert("session_prefix".into(), Value::String(prefix.to_string()));
            map.insert("metric_type".into(), Value::String(e.metric_type.clone()));
            if let Some(v) = e.metric_value {
                map.insert(
                    "metric_value_bucket".into(),
                    Value::Number((v as i64).into()),
                );
            }
            // metadata_json DROPPED — free-form, prohibited by spec §3.2.
            Some(("research_metrics".into(), map))
        }

        TelemetryEvent::ModelCall(e) => {
            let mut map = serde_json::Map::new();
            map.insert("model_id".into(), Value::String(e.model.clone()));
            map.insert("provider".into(), Value::String(e.provider.clone()));
            map.insert(
                "duration_bucket".into(),
                Value::String(duration_bucket(e.latency_ms)),
            );
            map.insert(
                "prompt_tokens_bucket".into(),
                Value::String(token_bucket(e.prompt_tokens as u64)),
            );
            map.insert(
                "completion_tokens_bucket".into(),
                Value::String(token_bucket(e.completion_tokens as u64)),
            );
            if let Some(ec) = &e.error_class {
                map.insert("error_class".into(), Value::String(ec.clone()));
            }
            // selection_rationale / trace_id / caller_agent_id dropped (free-form).
            Some(("model_calls".into(), map))
        }

        TelemetryEvent::Error(e) => {
            let mut map = serde_json::Map::new();
            map.insert("subsystem".into(), Value::String(e.subsystem.clone()));
            map.insert("error_class".into(), Value::String(e.error_class.clone()));
            map.insert("recoverable".into(), Value::Bool(e.retried));
            // trace_id / model / provider dropped.
            Some(("errors".into(), map))
        }

        TelemetryEvent::BuildSummary(e) => {
            let mut map = serde_json::Map::new();
            map.insert("exit_class".into(), Value::String(e.outcome.clone()));
            map.insert(
                "duration_bucket".into(),
                Value::String(duration_bucket(e.wall_time_ms)),
            );
            // build_id dropped (could be CI job id / path).
            Some(("build".into(), map))
        }

        TelemetryEvent::TaskRootSummary(e) => {
            let mut map = serde_json::Map::new();
            map.insert("outcome".into(), Value::String(e.outcome.clone()));
            map.insert(
                "child_call_bucket".into(),
                Value::String(count_bucket(e.child_call_count as u64)),
            );
            map.insert(
                "subagent_bucket".into(),
                Value::String(count_bucket(e.subagent_fanout as u64)),
            );
            // task_id, trace_id, repository_id dropped.
            Some(("agent_orchestration".into(), map))
        }

        TelemetryEvent::AiFixture(e) => {
            // Nested enum: project the inner fixture kind as a coarse category tag.
            use vox_telemetry::AiFixtureEvent;
            let mut map = serde_json::Map::new();
            let kind = match e {
                AiFixtureEvent::ModelIntent(_) => "model_intent",
                AiFixtureEvent::SubagentDispatch(_) => "subagent_dispatch",
                AiFixtureEvent::PromptDispatch(_) => "prompt_dispatch",
                AiFixtureEvent::SearchDispatch(_) => "search_dispatch",
                AiFixtureEvent::HoleObserved(_) => "hole_observed",
                _ => return None,
            };
            map.insert("fixture_kind".into(), Value::String(kind.to_string()));
            Some(("agent_orchestration".into(), map))
        }

        TelemetryEvent::LintFinding(e) => {
            let mut map = serde_json::Map::new();
            map.insert("rule_id".into(), Value::String(e.rule_id.clone()));
            map.insert("severity".into(), Value::String(e.severity.clone()));
            map.insert("autofix_available".into(), Value::Bool(e.autofix_available));
            // relative_path / repository_id dropped.
            Some(("build".into(), map))
        }

        TelemetryEvent::RepairAttempt(e) => {
            let mut map = serde_json::Map::new();
            map.insert("attempt".into(), Value::Number(e.attempt_number.into()));
            map.insert(
                "diagnostics_delta".into(),
                Value::Number((e.diagnostics_in.saturating_sub(e.diagnostics_out) as i64).into()),
            );
            // panel_member_id / repository_id dropped.
            Some(("agent_orchestration".into(), map))
        }

        TelemetryEvent::RepairOutcome(e) => {
            let mut map = serde_json::Map::new();
            map.insert("success".into(), Value::Bool(e.final_state == "success"));
            map.insert(
                "attempts_used".into(),
                Value::Number(e.attempts_used.into()),
            );
            // note / repository_id dropped.
            Some(("agent_orchestration".into(), map))
        }

        TelemetryEvent::AuditRun(e) => {
            let mut map = serde_json::Map::new();
            map.insert("thing".into(), Value::String(e.thing.clone()));
            map.insert("outcome".into(), Value::String(e.outcome.clone()));
            map.insert("passed".into(), Value::Bool(e.outcome == "ok"));
            // corpus_hash / repository_id dropped.
            Some(("agent_orchestration".into(), map))
        }

        TelemetryEvent::SelectionDecision(e) => {
            let mut map = serde_json::Map::new();
            map.insert("task".into(), Value::String(e.task.clone()));
            map.insert("chosen_model".into(), Value::String(e.chosen_model.clone()));
            map.insert("reason".into(), Value::String(e.reason.clone()));
            // repository_id dropped.
            Some(("model_calls".into(), map))
        }

        TelemetryEvent::ModelDiscovery(e) => {
            let mut map = serde_json::Map::new();
            map.insert("source".into(), Value::String(e.source.clone()));
            // model_id / description dropped (could be long free-form text).
            Some(("model_calls".into(), map))
        }

        TelemetryEvent::ModelClassification(e) => {
            let mut map = serde_json::Map::new();
            map.insert("tier".into(), Value::String(e.tier.clone()));
            // model_id / classifier_model / strengths dropped.
            Some(("model_calls".into(), map))
        }

        TelemetryEvent::ConfidencePromotion(e) => {
            let mut map = serde_json::Map::new();
            map.insert("from_confidence".into(), Value::String(e.from.clone()));
            map.insert("to_confidence".into(), Value::String(e.to.clone()));
            map.insert("evidence".into(), Value::String(e.evidence.clone()));
            // model_id dropped.
            Some(("model_calls".into(), map))
        }

        TelemetryEvent::DoctorProjectCheck(e) => {
            let mut map = serde_json::Map::new();
            map.insert("outcome".into(), Value::String(e.outcome.clone()));
            map.insert("passed".into(), Value::Bool(e.outcome == "green"));
            // project_root / repository_id dropped.
            Some(("agent_orchestration".into(), map))
        }

        // LintAutofix: no product-relevant aggregate signal yet.
        TelemetryEvent::LintAutofix(_) => None,

        // ── Track E: new product-category emit sites ──────────────────────
        TelemetryEvent::CommandUsage(e) => {
            let mut map = serde_json::Map::new();
            map.insert("verb".into(), Value::String(e.verb.clone()));
            map.insert("exit_class".into(), Value::String(e.exit_class.clone()));
            map.insert(
                "duration_bucket".into(),
                Value::String(e.duration_bucket.clone()),
            );
            Some(("command_usage".into(), map))
        }

        TelemetryEvent::SkillActivation(e) => {
            let mut map = serde_json::Map::new();
            // skill_id_hash is already salted-hash — safe to include.
            map.insert(
                "skill_id_hash".into(),
                Value::String(e.skill_id_hash.clone()),
            );
            map.insert(
                "trigger_source".into(),
                Value::String(e.trigger_source.clone()),
            );
            map.insert("accepted".into(), Value::Bool(e.accepted));
            map.insert("surface".into(), Value::String(e.surface.clone()));
            Some(("skill_activation".into(), map))
        }

        TelemetryEvent::EditPattern(e) => {
            let mut map = serde_json::Map::new();
            map.insert("op_type".into(), Value::String(e.op_type.clone()));
            map.insert("file_kind".into(), Value::String(e.file_kind.clone()));
            map.insert("size_bucket".into(), Value::String(e.size_bucket.clone()));
            Some(("edit_pattern".into(), map))
        }

        TelemetryEvent::HarnessUsage(e) => {
            let mut map = serde_json::Map::new();
            map.insert(
                "tool_call_kind".into(),
                Value::String(e.tool_call_kind.clone()),
            );
            map.insert("mode".into(), Value::String(e.mode.clone()));
            Some(("agent_orchestration".into(), map))
        }

        TelemetryEvent::ErrorSurface(e) => {
            let mut map = serde_json::Map::new();
            map.insert("error_class".into(), Value::String(e.error_class.clone()));
            map.insert("subsystem".into(), Value::String(e.subsystem.clone()));
            map.insert("recoverable".into(), Value::Bool(e.recoverable));
            Some(("errors".into(), map))
        }

        TelemetryEvent::DefaultDecision(e) => {
            let mut map = serde_json::Map::new();
            map.insert("decision_id".into(), Value::String(e.decision_id.clone()));
            map.insert("chosen".into(), Value::String(e.chosen.clone()));
            map.insert("outcome".into(), Value::String(e.outcome.clone()));
            if let Some(mag) = e.magnitude_bucket {
                map.insert("magnitude_bucket".into(), Value::Number(mag.into()));
            }
            Some(("default_decision".into(), map))
        }

        TelemetryEvent::ModelPrompt(e) => {
            let mut map = serde_json::Map::new();
            map.insert(
                "canonical_model_id".into(),
                Value::String(e.canonical_model_id.clone()),
            );
            map.insert(
                "profile_variant_id".into(),
                Value::String(e.profile_variant_id.clone()),
            );
            map.insert(
                "task_category".into(),
                Value::String(e.task_category.clone()),
            );
            map.insert(
                "quality_bucket".into(),
                Value::String(e.quality_bucket.clone()),
            );
            Some(("model_prompt".into(), map))
        }

        // Unhandled variants that don't yet have a product-category mapping.
        _ => None,
    }
}

// ─── helper bucketing fns ────────────────────────────────────────────────────

pub fn duration_bucket(ms: u64) -> String {
    match ms {
        0..=999 => "lt1s".into(),
        1_000..=4_999 => "1_to_5s".into(),
        5_000..=29_999 => "5_to_30s".into(),
        30_000..=299_999 => "30s_to_5m".into(),
        _ => "gt5m".into(),
    }
}

pub fn token_bucket(n: u64) -> String {
    match n {
        0..=511 => "lt512".into(),
        512..=2047 => "512_to_2k".into(),
        2048..=8191 => "2k_to_8k".into(),
        8192..=32767 => "8k_to_32k".into(),
        _ => "gt32k".into(),
    }
}

pub fn count_bucket(n: u64) -> String {
    match n {
        0 => "0".into(),
        1..=3 => "1_to_3".into(),
        4..=9 => "4_to_9".into(),
        10..=24 => "10_to_24".into(),
        _ => "gte25".into(),
    }
}
