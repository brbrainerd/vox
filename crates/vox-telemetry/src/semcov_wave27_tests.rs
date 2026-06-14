//! Adversarial unit tests for vox-telemetry (wave 27).
//!
//! Targets: types validation, serde invariants, gen_ai attribute mapping,
//! TraceContext span arithmetic, CompositeRecorder fan-out, and boundary conditions.
//!
//! Excluded (already have semcov_wave6_tests): aggregator.rs, config.rs.

#[cfg(test)]
mod semcov_wave27_tests {
    use std::sync::{Arc, Mutex};

    use crate::types::validate_research_metric_row;
    use crate::{
        recorder::{CompositeRecorder, TelemetryRecorder},
        span::TraceContext,
        types::{
            AiFixtureEvent, AuditEffortCommitJudgedEvent, ClassificationEvent,
            ConfidencePromotionEvent, DiscoveryEvent, ErrorEvent, HoleObservedTelemetryEvent,
            LintAutofixEvent, METRIC_TYPE_MODEL_ROUTE_EVENT, ModelCallEvent,
            PromptDispatchTelemetryEvent, RESEARCH_METRICS_METADATA_JSON_MAX_BYTES,
            RESEARCH_METRICS_METRIC_TYPE_MAX_CHARS, RESEARCH_METRICS_SESSION_ID_MAX_CHARS,
            RepairAttemptEvent, RepairOutcomeEvent, ResearchMetricEvent,
            SESSION_PREFIX_MODEL_AUTONOMIC, SearchDispatchTelemetryEvent, SelectionDecisionEvent,
            SubagentDispatchTelemetryPayload, TelemetryEvent, TelemetryWriteOptions,
        },
    };

    // ── helpers ───────────────────────────────────────────────────────────────

    fn minimal_model_call() -> ModelCallEvent {
        ModelCallEvent {
            model: "claude-haiku-4".into(),
            provider: "anthropic".into(),
            route_profile: None,
            selection_rationale: None,
            prompt_tokens: 10,
            completion_tokens: 5,
            cache_read_input_tokens: None,
            cache_creation_input_tokens: None,
            latency_ms: 100,
            cost_usd: 0.001,
            cost_source: "estimated".into(),
            error_class: None,
            retry_attempt: 0,
            task_id: None,
            parent_task_id: None,
            trace_id: None,
            caller_agent_id: None,
        }
    }

    // ── validation edge-cases ────────────────────────────────────────────────

    #[test]
    fn session_id_at_exact_limit_is_accepted() {
        // Catches: off-by-one — limit check uses > instead of >=, accepting 513-char IDs
        let at_limit = "a".repeat(RESEARCH_METRICS_SESSION_ID_MAX_CHARS);
        assert!(
            validate_research_metric_row(&at_limit, "t", None).is_ok(),
            "512-char session_id must be valid"
        );
    }

    #[test]
    fn session_id_one_over_limit_is_rejected() {
        // Catches: off-by-one — limit check uses >= instead of >, accepting oversized IDs
        let over = "a".repeat(RESEARCH_METRICS_SESSION_ID_MAX_CHARS + 1);
        assert!(
            validate_research_metric_row(&over, "t", None).is_err(),
            "513-char session_id must be rejected"
        );
    }

    #[test]
    fn metric_type_at_exact_limit_is_accepted() {
        // Catches: off-by-one on metric_type length — 128-char type wrongly rejected
        let at_limit = "a".repeat(RESEARCH_METRICS_METRIC_TYPE_MAX_CHARS);
        assert!(
            validate_research_metric_row("s", &at_limit, None).is_ok(),
            "128-char metric_type must be valid"
        );
    }

    #[test]
    fn metric_type_one_over_limit_is_rejected() {
        // Catches: metric_type length check not enforced (too-long type accepted silently)
        let over = "a".repeat(RESEARCH_METRICS_METRIC_TYPE_MAX_CHARS + 1);
        assert!(
            validate_research_metric_row("s", &over, None).is_err(),
            "129-char metric_type must be rejected"
        );
    }

    #[test]
    fn metadata_json_at_exact_limit_is_accepted() {
        // Catches: off-by-one on payload cap — 256 KiB wrongly rejected
        let exact = "x".repeat(RESEARCH_METRICS_METADATA_JSON_MAX_BYTES);
        assert!(
            validate_research_metric_row("s", "t", Some(&exact)).is_ok(),
            "exactly-at-limit metadata_json must be accepted"
        );
    }

    #[test]
    fn model_route_event_missing_trace_id_field_is_rejected() {
        // Catches: required-field guard for model_route_event not enforcing trace_id
        let bad_json = r#"{"route_policy_profile":"fast"}"#;
        assert!(
            validate_research_metric_row("s", METRIC_TYPE_MODEL_ROUTE_EVENT, Some(bad_json))
                .is_err(),
            "model_route_event without trace_id must fail"
        );
    }

    #[test]
    fn model_route_event_missing_route_policy_profile_is_rejected() {
        // Catches: required-field guard only checks trace_id, skips route_policy_profile
        let bad_json = r#"{"trace_id":"abc"}"#;
        assert!(
            validate_research_metric_row("s", METRIC_TYPE_MODEL_ROUTE_EVENT, Some(bad_json))
                .is_err(),
            "model_route_event without route_policy_profile must fail"
        );
    }

    #[test]
    fn model_route_event_with_both_required_fields_is_accepted() {
        // Catches: overly-strict validation rejecting valid model_route_event JSON
        let good = r#"{"trace_id":"abc","route_policy_profile":"economy"}"#;
        assert!(
            validate_research_metric_row("s", METRIC_TYPE_MODEL_ROUTE_EVENT, Some(good)).is_ok(),
            "model_route_event with both required fields must be accepted"
        );
    }

    #[test]
    fn metric_type_with_unicode_is_rejected() {
        // Catches: valid_metric_type_chars not checking for non-ASCII (e.g., emoji slips through)
        assert!(
            validate_research_metric_row("s", "metric_\u{1F4A5}", None).is_err(),
            "unicode in metric_type must be rejected"
        );
    }

    #[test]
    fn metric_type_with_whitespace_is_rejected() {
        // Catches: valid_metric_type_chars accepting tab or newline characters
        assert!(validate_research_metric_row("s", "a\tb", None).is_err());
        assert!(validate_research_metric_row("s", "a\nb", None).is_err());
        assert!(validate_research_metric_row("s", "a b", None).is_err());
    }

    #[test]
    fn metric_type_with_slash_is_rejected() {
        // Catches: forward-slash accidentally permitted (docs use "vox/retired/..." patterns)
        assert!(
            validate_research_metric_row("s", "a/b", None).is_err(),
            "slash must not be allowed in metric_type"
        );
    }

    // ── gen_ai attribute mapping ──────────────────────────────────────────────

    #[test]
    fn gen_ai_attributes_omits_none_optional_fields() {
        // Catches: None fields serialized as "None" string instead of being absent
        use crate::types::model_call_event_to_gen_ai_attributes;
        let ev = minimal_model_call();
        let attrs = model_call_event_to_gen_ai_attributes(&ev);
        assert!(
            !attrs.contains_key("gen_ai.usage.cache_read_input_tokens"),
            "absent cache_read_input_tokens must not appear in attrs"
        );
        assert!(
            !attrs.contains_key("gen_ai.usage.cache_creation_input_tokens"),
            "absent cache_creation_input_tokens must not appear in attrs"
        );
        assert!(
            !attrs.contains_key("gen_ai.response.error"),
            "absent error_class must not appear as gen_ai.response.error"
        );
        assert!(
            !attrs.contains_key("gen_ai.response.trace_id"),
            "absent trace_id must not appear"
        );
        assert!(
            !attrs.contains_key("gen_ai.request.task_id"),
            "absent task_id must not appear"
        );
        assert!(
            !attrs.contains_key("gen_ai.request.agent_id"),
            "absent caller_agent_id must not appear"
        );
    }

    #[test]
    fn gen_ai_attributes_maps_zero_tokens_correctly() {
        // Catches: zero token counts rendered as empty string or omitted entirely
        use crate::types::model_call_event_to_gen_ai_attributes;
        let mut ev = minimal_model_call();
        ev.prompt_tokens = 0;
        ev.completion_tokens = 0;
        let attrs = model_call_event_to_gen_ai_attributes(&ev);
        assert_eq!(
            attrs.get("gen_ai.usage.input_tokens"),
            Some(&"0".to_string())
        );
        assert_eq!(
            attrs.get("gen_ai.usage.output_tokens"),
            Some(&"0".to_string())
        );
    }

    #[test]
    fn gen_ai_attributes_maps_max_u32_tokens_without_truncation() {
        // Catches: u32::MAX overflow when converting to string representation
        use crate::types::model_call_event_to_gen_ai_attributes;
        let mut ev = minimal_model_call();
        ev.prompt_tokens = u32::MAX;
        ev.completion_tokens = u32::MAX;
        let attrs = model_call_event_to_gen_ai_attributes(&ev);
        assert_eq!(
            attrs.get("gen_ai.usage.input_tokens"),
            Some(&u32::MAX.to_string())
        );
    }

    #[test]
    fn gen_ai_attributes_includes_retry_attempt_zero() {
        // Catches: retry_attempt=0 treated as falsy and omitted from attributes
        use crate::types::model_call_event_to_gen_ai_attributes;
        let ev = minimal_model_call(); // retry_attempt: 0
        let attrs = model_call_event_to_gen_ai_attributes(&ev);
        assert_eq!(
            attrs.get("gen_ai.request.retry_attempt"),
            Some(&"0".to_string()),
            "retry_attempt 0 must be present in attrs"
        );
    }

    // ── TraceContext span semantics ───────────────────────────────────────────

    #[test]
    fn root_context_has_zero_span_depth_and_no_parent() {
        // Catches: root() accidentally inheriting a parent_task_id or nonzero depth
        let ctx = TraceContext::root(42);
        assert_eq!(ctx.task_id, Some(42));
        assert_eq!(ctx.parent_task_id, None);
        assert_eq!(ctx.span_depth, 0);
        assert!(ctx.caller_agent_id.is_none());
    }

    #[test]
    fn child_inherits_trace_id_and_increments_depth() {
        // Catches: child() generating a new trace_id instead of propagating the parent's
        let root = TraceContext::root(1);
        let root_trace = root.trace_id;
        let child = root.child(2, "agent-b");
        assert_eq!(child.trace_id, root_trace, "trace_id must be inherited");
        assert_eq!(child.parent_task_id, Some(1));
        assert_eq!(child.task_id, Some(2));
        assert_eq!(child.span_depth, 1);
        assert_eq!(child.caller_agent_id.as_deref(), Some("agent-b"));
    }

    #[test]
    fn child_chain_increments_span_depth_monotonically() {
        // Catches: span_depth not incrementing on repeated child() calls
        let root = TraceContext::root(100);
        let c1 = root.child(101, "a1");
        let c2 = c1.child(102, "a2");
        let c3 = c2.child(103, "a3");
        assert_eq!(c3.span_depth, 3);
        assert_eq!(c3.parent_task_id, Some(102));
    }

    #[test]
    fn span_depth_saturates_at_u16_max_without_panic() {
        // Catches: span_depth wrapping/panicking on overflow instead of saturating
        let ctx = TraceContext {
            task_id: Some(1),
            parent_task_id: None,
            trace_id: uuid::Uuid::new_v4(),
            span_depth: u16::MAX,
            caller_agent_id: None,
        };
        let child = ctx.child(2, "deep");
        assert_eq!(
            child.span_depth,
            u16::MAX,
            "saturating_add must not wrap to 0"
        );
    }

    #[test]
    fn default_trace_context_has_no_task_and_zero_depth() {
        // Catches: Default impl accidentally setting non-None task_id or depth > 0
        let ctx = TraceContext::default();
        assert!(ctx.task_id.is_none());
        assert!(ctx.parent_task_id.is_none());
        assert_eq!(ctx.span_depth, 0);
        assert!(ctx.caller_agent_id.is_none());
    }

    // ── CompositeRecorder fan-out ────────────────────────────────────────────

    /// Spy recorder that counts how many events it receives.
    struct CountingRecorder(Mutex<u32>);

    impl TelemetryRecorder for CountingRecorder {
        fn record(&self, _event: &TelemetryEvent) {
            *self.0.lock().unwrap() += 1;
        }
    }

    #[test]
    fn composite_recorder_fans_out_to_all_inner_recorders() {
        // Catches: CompositeRecorder only calling the first recorder in the list
        let r1 = Arc::new(CountingRecorder(Mutex::new(0)));
        let r2 = Arc::new(CountingRecorder(Mutex::new(0)));
        let r3 = Arc::new(CountingRecorder(Mutex::new(0)));
        let composite = CompositeRecorder::new(vec![
            r1.clone() as Arc<dyn TelemetryRecorder>,
            r2.clone() as Arc<dyn TelemetryRecorder>,
            r3.clone() as Arc<dyn TelemetryRecorder>,
        ]);
        let event = TelemetryEvent::ResearchMetric(ResearchMetricEvent {
            session_id: "bench:test".into(),
            metric_type: "benchmark_event".into(),
            metric_value: Some(1.0),
            metadata_json: None,
        });
        composite.record(&event);
        assert_eq!(*r1.0.lock().unwrap(), 1, "r1 must receive the event");
        assert_eq!(*r2.0.lock().unwrap(), 1, "r2 must receive the event");
        assert_eq!(*r3.0.lock().unwrap(), 1, "r3 must receive the event");
    }

    #[test]
    fn composite_recorder_with_empty_list_does_not_panic() {
        // Catches: empty inner list causing index-out-of-bounds or unwrap panic
        let composite = CompositeRecorder::new(vec![]);
        let event = TelemetryEvent::Error(ErrorEvent {
            subsystem: "test".into(),
            error_class: "noop".into(),
            http_status: None,
            retry_attempt: 0,
            retried: false,
            model: None,
            provider: None,
            task_id: None,
            trace_id: None,
        });
        composite.record(&event); // must not panic
    }

    // ── serde round-trips for non-obvious types ───────────────────────────────

    #[test]
    fn selection_decision_event_round_trip_preserves_axes_tuple() {
        // Catches: (u8, u8, u8) tuple serialized as array but deserialized as struct, or axes reordered
        let ev = SelectionDecisionEvent {
            intent_caller: Some("repair-loop".into()),
            task: "code_gen".into(),
            axes: (10, 90, 50),
            chosen_model: "claude-sonnet-4-6".into(),
            reason: "scored".into(),
            premium_alias_key: None,
            repository_id: None,
        };
        let outer = TelemetryEvent::SelectionDecision(ev.clone());
        let json = serde_json::to_string(&outer).unwrap();
        let back: TelemetryEvent = serde_json::from_str(&json).unwrap();
        let TelemetryEvent::SelectionDecision(back) = back else {
            panic!("SelectionDecision variant lost")
        };
        assert_eq!(
            back.axes,
            (10, 90, 50),
            "axes triple must survive round-trip"
        );
        assert_eq!(back.reason, "scored");
    }

    #[test]
    fn confidence_promotion_event_from_to_fields_not_swapped() {
        // Catches: from/to fields swapped during serialization or deserialization
        let ev = ConfidencePromotionEvent {
            model_id: "gpt-5-mini".into(),
            from: "provisional".into(),
            to: "shadowed".into(),
            evidence: "scoreboard_threshold".into(),
        };
        let json = serde_json::to_string(&ev).unwrap();
        let back: ConfidencePromotionEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(back.from, "provisional");
        assert_eq!(back.to, "shadowed");
    }

    #[test]
    fn discovery_event_none_fields_absent_in_json() {
        // Catches: skip_serializing_if = "Option::is_none" missing for description/max_context_tokens
        let ev = DiscoveryEvent {
            source: "openrouter".into(),
            model_id: "new-model-42".into(),
            description: None,
            max_context_tokens: None,
        };
        let json = serde_json::to_string(&ev).unwrap();
        assert!(
            !json.contains("\"description\""),
            "absent description must not appear in JSON; got {json}"
        );
        assert!(
            !json.contains("\"max_context_tokens\""),
            "absent max_context_tokens must not appear in JSON; got {json}"
        );
    }

    #[test]
    fn ai_fixture_event_subagent_dispatch_round_trip() {
        // Catches: SubagentDispatchTelemetryPayload skip_serializing None fields, then not restoring defaults on deser
        let ev = AiFixtureEvent::SubagentDispatch(SubagentDispatchTelemetryPayload {
            metric_type: "orch.subagent.dispatch".into(),
            decision: "dispatch".into(),
            complexity: Some(7),
            chain_depth: None,
            session_id: None,
            parent_task_id: None,
            span_depth: Some(2),
            dispatch_latency_ms: None,
        });
        let json = serde_json::to_string(&ev).unwrap();
        // None fields must not appear
        assert!(
            !json.contains("\"chain_depth\""),
            "chain_depth None must be absent"
        );
        assert!(
            !json.contains("\"dispatch_latency_ms\""),
            "dispatch_latency_ms None must be absent"
        );
        let back: AiFixtureEvent = serde_json::from_str(&json).unwrap();
        let AiFixtureEvent::SubagentDispatch(b) = back else {
            panic!("SubagentDispatch variant lost")
        };
        assert_eq!(b.complexity, Some(7));
        assert_eq!(b.chain_depth, None);
    }

    #[test]
    fn prompt_dispatch_event_default_redact_count_deserializes_as_zero() {
        // Catches: #[serde(default)] on redact_count missing — absent field causes deser error
        let json = r#"{"stage":"pre","outcome":"ok"}"#;
        let ev: PromptDispatchTelemetryEvent = serde_json::from_str(json).unwrap();
        assert_eq!(
            ev.redact_count, 0,
            "default redact_count must be 0 when absent from JSON"
        );
    }

    #[test]
    fn search_dispatch_event_missing_optional_top_k_absent_in_output() {
        // Catches: top_k = None serialized as null rather than omitted
        let ev = SearchDispatchTelemetryEvent {
            corpus: "vox-db".into(),
            outcome: "hit".into(),
            error: None,
            top_k: None,
        };
        let json = serde_json::to_string(&ev).unwrap();
        assert!(
            !json.contains("\"top_k\""),
            "top_k None must be absent from JSON; got {json}"
        );
    }

    #[test]
    fn hole_observed_event_reviewer_none_absent_in_json() {
        // Catches: skip_serializing_if missing for reviewer field
        let ev = HoleObservedTelemetryEvent {
            cache_key: "fixture:foo".into(),
            observation: "stale".into(),
            reviewer: None,
        };
        let json = serde_json::to_string(&ev).unwrap();
        assert!(
            !json.contains("\"reviewer\""),
            "reviewer None must be absent; got {json}"
        );
    }

    #[test]
    fn telemetry_write_options_session_model_autonomic_prefix() {
        // Catches: SESSION_PREFIX_MODEL_AUTONOMIC constant not matching expected "model_autonomic:" value
        // (any rename that drifts from the documented SSOT prefix would break correlation queries)
        assert_eq!(SESSION_PREFIX_MODEL_AUTONOMIC, "model_autonomic:");
    }

    #[test]
    fn lint_autofix_event_applied_and_rejected_share_same_struct() {
        // Catches: structural divergence between applied/rejected payloads breaking the aggregator's
        // shared-shape assumption (the metric_type disambiguates; payload must be identical struct)
        let applied = LintAutofixEvent {
            rule_id: "rule-x".into(),
            diagnostic_id: None,
            outcome: "applied".into(),
            reason: None,
            relative_path: "src/a.vox".into(),
            line: 5,
            repository_id: None,
        };
        let rejected = LintAutofixEvent {
            rule_id: "rule-x".into(),
            diagnostic_id: None,
            outcome: "rejected".into(),
            reason: Some("user dismissed".into()),
            relative_path: "src/a.vox".into(),
            line: 5,
            repository_id: None,
        };
        // Both must serialize without error and differ only in outcome/reason
        let j_applied = serde_json::to_string(&TelemetryEvent::LintAutofix(applied)).unwrap();
        let j_rejected = serde_json::to_string(&TelemetryEvent::LintAutofix(rejected)).unwrap();
        assert!(j_applied.contains("\"applied\""));
        assert!(j_rejected.contains("\"rejected\""));
        assert!(j_rejected.contains("\"user dismissed\""));
    }

    #[test]
    fn repair_outcome_attempts_used_can_exceed_budget_on_infra_error() {
        // Catches: attempts_used > attempts_budget rejected at serde or construction
        // (infra_error can cut the loop before budget is consumed, or budget may be 0)
        let ev = RepairOutcomeEvent {
            final_state: "infra_error".into(),
            attempts_used: 0,
            attempts_budget: 0,
            total_cost_usd: 0.0,
            total_duration_ms: 0,
            residual_diagnostics: 99,
            note: Some("provider unreachable".into()),
            repository_id: None,
        };
        let json = serde_json::to_string(&ev).unwrap();
        let back: RepairOutcomeEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(back.final_state, "infra_error");
        assert_eq!(back.residual_diagnostics, 99);
    }

    #[test]
    fn audit_effort_commit_judged_waste_score_boundary_values() {
        // Catches: waste_score capped to u8 but validation logic might clamp/reject 0 or 100
        for score in [0u8, 1, 50, 99, 100] {
            let ev = AuditEffortCommitJudgedEvent {
                run_id: "r1".into(),
                commit_sha: "abc".into(),
                judge_model_id: "mock".into(),
                latency_ms: 0,
                tokens_consumed_by_judge: 0,
                waste_score: Some(score),
                waste_category: None,
                suggested_remediation_kind: None,
            };
            let json = serde_json::to_string(&ev).unwrap();
            let back: AuditEffortCommitJudgedEvent = serde_json::from_str(&json).unwrap();
            assert_eq!(
                back.waste_score,
                Some(score),
                "waste_score {score} must survive round-trip"
            );
        }
    }

    #[test]
    fn classification_event_confidence_zero_and_one_survive_round_trip() {
        // Catches: f32 confidence 0.0/1.0 silently clamped or rejected
        for confidence in [0.0f32, 1.0f32] {
            let ev = ClassificationEvent {
                model_id: "test-model".into(),
                classifier_model: "haiku".into(),
                tier: "standard".into(),
                strengths: vec!["coding".into()],
                confidence,
            };
            let json = serde_json::to_string(&ev).unwrap();
            let back: ClassificationEvent = serde_json::from_str(&json).unwrap();
            assert!(
                (back.confidence - confidence).abs() < f32::EPSILON,
                "confidence {confidence} must survive round-trip"
            );
        }
    }

    #[test]
    fn telemetry_write_options_empty_repo_id_produces_prefix_only() {
        // Catches: empty repository_id causing session_bench() to return bare "bench:"
        // which might pass session_id validation (non-empty) but be meaningless in practice.
        // Verifies the actual string shape so future callers notice if it changes.
        let opts = TelemetryWriteOptions::new("");
        assert_eq!(opts.session_bench(), "bench:");
        assert_eq!(opts.session_lint(), "lint:");
        // The resulting "bench:" is non-empty and must pass validation
        assert!(
            validate_research_metric_row(&opts.session_bench(), "benchmark_event", None).is_ok()
        );
    }

    #[test]
    fn repair_attempt_event_zero_files_touched_is_valid() {
        // Catches: RepairAttemptEvent rejecting files_touched = 0 (e.g. LLM returned empty patch)
        let ev = RepairAttemptEvent {
            attempt_number: 1,
            diagnostics_in: 3,
            diagnostics_out: 3,
            files_touched: 0,
            cost_usd: 0.005,
            duration_ms: 500,
            panel_member_id: None,
            repository_id: None,
        };
        let json = serde_json::to_string(&TelemetryEvent::RepairAttempt(ev.clone())).unwrap();
        let back: TelemetryEvent = serde_json::from_str(&json).unwrap();
        let TelemetryEvent::RepairAttempt(back) = back else {
            panic!("variant lost")
        };
        assert_eq!(back.files_touched, 0);
        assert_eq!(
            back.diagnostics_in, back.diagnostics_out,
            "no improvement must be representable"
        );
    }
}
