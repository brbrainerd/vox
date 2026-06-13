# Semantic Behavior Map — `vox-orchestrator`

Deterministically synthesized from 685 distinct proven-behavior claims (of 686 extracted) across 398 symbols. 34 symbols have an explicit error-path proof; **277 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `OrchestratorConfig`  (happy, invariant; EXTRACTED)
- [happy] OrchestratorConfig::load_from_toml successfully merges TOML section keys (max_agents, populi URLs, GPU labels) into config  (crates/vox-orchestrator/src/config/tests.rs)
- [happy] Environment variable VOX_ORCHESTRATOR_POPULI_INFERENCE_BASE_URL overrides TOML-configured populi_inference_base_url when merge_env_overrides() is called  (crates/vox-orchestrator/src/config/tests.rs)
- [happy] Environment variable VOX_ORCHESTRATOR_MESH_CONTROL_URL overrides TOML-configured populi_control_url when merge_env_overrides() is called  (crates/vox-orchestrator/src/config/tests.rs)
- [happy] Environment variables for repo_shard (specialization_weight, validation_failure_penalty, reduce_conflict_cooldown_penalty, reduce_conflict_cooldown_ms) are parsed and applied consistently via merge_env_overrides()  (crates/vox-orchestrator/src/config/tests.rs)
- [happy] Environment variable VOX_ORCHESTRATOR_MESH_REMOTE_RESULT_MAX_MESSAGES_PER_POLL overrides populi_remote_result_max_messages_per_poll when merge_env_overrides() is called  (crates/vox-orchestrator/src/config/tests.rs)
- [happy] Environment variable VOX_ORCHESTRATOR_COMPLETION_MARKDOWN_LINK_AUDIT_ENABLED overrides completion_markdown_link_audit_enabled, with default=true for default() and false for for_testing()  (crates/vox-orchestrator/src/config/tests.rs)
- [happy] news.reddit_client_id is populated from environment in lenient profile and None in strict hard_cut profile after merge_env_overrides()  (crates/vox-orchestrator/src/config/tests.rs)
- [invariant] OrchestratorConfig survives JSON serialization and deserialization with values preserved  (crates/vox-orchestrator/src/config/tests.rs)

### `RiskGrade`  (edge, happy; EXTRACTED)
- [happy] When all risk dimensions are zero, m.evaluate() returns Low grade  (crates/vox-orchestrator/src/risk_matrix.rs)
- [edge] High irreversibility (0.95) causes m.grade() to return Critical  (crates/vox-orchestrator/src/risk_matrix.rs)
- [edge] High blast_radius (0.91) causes m.grade() to return Critical  (crates/vox-orchestrator/src/risk_matrix.rs)
- [edge] High confidence_deficit (0.95) causes m.grade() to return Critical  (crates/vox-orchestrator/src/risk_matrix.rs)
- [edge] High compliance_exposure (0.90) causes m.grade() to return Critical  (crates/vox-orchestrator/src/risk_matrix.rs)
- [happy] Composite medium-risk dimensions (score in 0.25-0.50 band) result in Medium grade  (crates/vox-orchestrator/src/risk_matrix.rs)
- [happy] Composite high-risk dimensions (irreversibility=0.7, blast_radius=0.7, etc.) result in High grade  (crates/vox-orchestrator/src/risk_matrix.rs)
- [happy] When irreversibility risk is 0.95 (critical threshold), classify() produces RiskGrade::Critical  (crates/vox-orchestrator/src/orchestrator_policy.rs)

### `execute_handoff function`  (error, happy; EXTRACTED)
- [happy] publishes PlanHandoff event to EventBus with correct from/to agent IDs and metadata presence flags  (crates/vox-orchestrator/src/handoff.rs)
- [error] rejects handoff with pending tasks when verification criteria are missing, returning MissingVerificationCriteria error  (crates/vox-orchestrator/src/handoff.rs)
- [error] rejects handoff with pending tasks when execution_role metadata is missing, returning MissingExecutionRoleMetadata error  (crates/vox-orchestrator/src/handoff.rs)
- [happy] accepts handoff payload with valid JSON-serialized ContextEnvelope in metadata  (crates/vox-orchestrator/src/handoff.rs)
- [error] rejects handoff with invalid JSON in harness spec metadata, returning InvalidHarnessSpec error  (crates/vox-orchestrator/src/handoff.rs)
- [happy] accepts handoff payload with valid serialized AgentHarnessSpec in metadata  (crates/vox-orchestrator/src/handoff.rs)
- [happy] extracts and emits context envelope metadata fields (session_id, has_context_envelope) into PlanHandoff event  (crates/vox-orchestrator/src/handoff.rs)

### `run_dispatched_bundle`  (error, happy; EXTRACTED)
- [happy] Returns None when exec policy is 'no-exec'  (crates/vox-orchestrator/src/a2a/remote_worker.rs)
- [error] Returns failure with error containing 'source-only' when policy is 'source-only'  (crates/vox-orchestrator/src/a2a/remote_worker.rs)
- [error] Returns failure with error containing 'blake3' when integrity hash is missing  (crates/vox-orchestrator/src/a2a/remote_worker.rs)
- [error] Returns failure with error containing 'hash mismatch' when provided hash does not match  (crates/vox-orchestrator/src/a2a/remote_worker.rs)
- [error] Returns failure with error containing 'native' when attempting to execute native binary under 'strict' policy  (crates/vox-orchestrator/src/a2a/remote_worker.rs)
- [happy] Forwards only low-value secrets into WASM sandbox, filtering out credentials marked in auth_registry  (crates/vox-orchestrator/src/a2a/remote_worker.rs)
- [happy] Successfully executes valid WASI module via 'vox wasm run' with success=true  (crates/vox-orchestrator/src/a2a/remote_worker.rs)

### `run_dispatched_source`  (error, happy; EXTRACTED)
- [happy] run_dispatched_source returns None when exec_policy is 'no-exec' regardless of other parameters  (crates/vox-orchestrator/src/a2a/remote_worker.rs)
- [error] run_dispatched_source returns Some with success=false when integrity_hash is None, without spawning execution  (crates/vox-orchestrator/src/a2a/remote_worker.rs)
- [error] run_dispatched_source returns Some with success=false when provided hash doesn't match computed hash, without spawning execution  (crates/vox-orchestrator/src/a2a/remote_worker.rs)
- [error] run_dispatched_source returns Some with success=false when source parameter is not valid base64  (crates/vox-orchestrator/src/a2a/remote_worker.rs)
- [happy] run_dispatched_source successfully executes valid Vox source and returns actual stdout from execution in result field  (crates/vox-orchestrator/src/a2a/remote_worker.rs)
- [happy] run_dispatched_source accepts base64 and hash fields constructed by build_exec_source_fields and successfully executes the source  (crates/vox-orchestrator/src/a2a/remote_worker.rs)
- [error] run_dispatched_source returns success=false when the executed script exits with non-zero status code  (crates/vox-orchestrator/src/a2a/remote_worker.rs)

### `CompactionTrigger::select`  (edge, happy; EXTRACTED)
- [happy] select() returns Conservative strategy for 0.30 (30%) utilization  (crates/vox-orchestrator/src/compaction_trigger.rs)
- [edge] select() returns Balanced strategy at 0.60 (60%) utilization, the Conservative-Balanced boundary  (crates/vox-orchestrator/src/compaction_trigger.rs)
- [happy] select() returns Balanced strategy for 0.70 (70%) utilization  (crates/vox-orchestrator/src/compaction_trigger.rs)
- [edge] select() returns Aggressive strategy at 0.85 (85%) utilization, the Balanced-Aggressive boundary  (crates/vox-orchestrator/src/compaction_trigger.rs)
- [happy] select() returns Aggressive strategy for 1.0 (100%) utilization  (crates/vox-orchestrator/src/compaction_trigger.rs)
- [edge] select() clamps negative utilization to Conservative and utilization > 1.0 to Aggressive  (crates/vox-orchestrator/src/compaction_trigger.rs)

### `multi_agent_enforcement()`  (edge, happy, invariant; EXTRACTED)
- [edge] multi_agent_enforcement with 2 agents and Warn enforcement escalates to Strict  (crates/vox-orchestrator/src/scope.rs)
- [invariant] multi_agent_enforcement with 5 agents and Disabled enforcement remains Disabled  (crates/vox-orchestrator/src/scope.rs)
- [happy] multi_agent_enforcement with 1 agent returns enforcement level unchanged  (crates/vox-orchestrator/src/scope.rs)
- [happy] multi_agent_enforcement with 0 agents returns enforcement level unchanged  (crates/vox-orchestrator/src/scope.rs)
- [invariant] multi_agent_enforcement with 3 agents and Strict enforcement remains Strict  (crates/vox-orchestrator/src/scope.rs)
- [happy] multi_agent_enforcement with 3 agents and Strict enforcement returns Strict  (crates/vox-orchestrator/src/scope.rs)

### `resolve_policy`  (edge, error, happy, invariant; EXTRACTED)
- [happy] PinModel selection step causes resolve_policy to select and return the pinned model by ID  (crates/vox-orchestrator/src/models/policy.rs)
- [error] resolve_policy returns None when the PinModel step references a model that does not exist in the registry  (crates/vox-orchestrator/src/models/policy.rs)
- [happy] PreferFree selection step causes resolve_policy to select the free/cheaper model over premium models  (crates/vox-orchestrator/src/models/policy.rs)
- [happy] EmphasizeAxis step with Intelligence weight 90 selects premium-paid model while Efficiency weight 90 selects free-cheap model  (crates/vox-orchestrator/src/models/policy.rs)
- [invariant] FallbackWhen with OutOfTokens condition applies fallback step only when budget_exhausted=true; otherwise falls through to primary step  (crates/vox-orchestrator/src/models/policy.rs)
- [edge] resolve_policy returns None for an empty/default SelectionPolicy, allowing caller to use existing cascade logic  (crates/vox-orchestrator/src/models/policy.rs)

### `split_summary_into_claim_segments`  (happy; EXTRACTED)
- [happy] split_summary_into_claim_segments preserves dotted version numbers like v1.2.3 without treating periods as sentence boundaries  (crates/vox-orchestrator/src/grounding.rs)
- [happy] split_summary_into_claim_segments respects common abbreviations like Mr. as sentence-continuation rather than sentence-end  (crates/vox-orchestrator/src/grounding.rs)
- [happy] split_summary_into_claim_segments preserves street suffix abbreviations like St. without treating them as sentence boundaries  (crates/vox-orchestrator/src/grounding.rs)
- [happy] split_summary_into_claim_segments respects French title abbreviations like Mme. without splitting at the period  (crates/vox-orchestrator/src/grounding.rs)
- [happy] split_summary_into_claim_segments correctly handles Nr. and Tel. abbreviations as clause continuations, not sentence boundaries  (crates/vox-orchestrator/src/grounding.rs)
- [happy] split_summary_into_claim_segments splits sentences after truncated abbreviations ending in -istr or -estr (Ministr, Illustr, Orchestr)  (crates/vox-orchestrator/src/grounding.rs)

### `BudgetGate::evaluate_fraction`  (edge, happy; EXTRACTED)
- [happy] evaluate_fraction(0.50) returns BudgetStatus::Ok when budget consumption is 50%  (crates/vox-orchestrator/src/budget_gate.rs)
- [edge] evaluate_fraction(0.80) returns BudgetStatus::Downgrade at downgrade threshold  (crates/vox-orchestrator/src/budget_gate.rs)
- [happy] evaluate_fraction(0.90) returns BudgetStatus::Downgrade when between downgrade (80%) and halt (95%) thresholds  (crates/vox-orchestrator/src/budget_gate.rs)
- [edge] evaluate_fraction(0.95) returns BudgetStatus::Halt at halt threshold  (crates/vox-orchestrator/src/budget_gate.rs)
- [happy] evaluate_fraction(1.0) returns BudgetStatus::Halt when budget consumption is 100%  (crates/vox-orchestrator/src/budget_gate.rs)

### `CircuitBreaker::should_trip()`  (happy; EXTRACTED)
- [happy] should_trip returns Some(TripReason::NoProgress) when no_progress_loops equals 3  (crates/vox-orchestrator/src/circuit_breaker.rs)
- [happy] should_trip returns Some(TripReason::SameError) when same_error_loops equals 5  (crates/vox-orchestrator/src/circuit_breaker.rs)
- [happy] should_trip returns Some(TripReason::ToolThrash) when tool_thrash_count equals 15  (crates/vox-orchestrator/src/circuit_breaker.rs)
- [happy] should_trip returns Some(TripReason::NgramOverlap) when ngram_overlap equals 0.90  (crates/vox-orchestrator/src/circuit_breaker.rs)
- [happy] should_trip returns Some(TripReason::SemanticDrift) when semantic_drift_sigma equals 2.5  (crates/vox-orchestrator/src/circuit_breaker.rs)

### `DispatchRouter::route()`  (happy, invariant; EXTRACTED)
- [happy] Returns DispatchDecision::Reject when chain_depth equals the configured limit (5) regardless of high complexity  (crates/vox-orchestrator/src/subagent_dispatch.rs)
- [happy] Returns DispatchDecision::Reject when chain_depth exceeds the configured limit  (crates/vox-orchestrator/src/subagent_dispatch.rs)
- [happy] Returns DispatchDecision::Inline when budget_exhausted signal is true, overriding high complexity  (crates/vox-orchestrator/src/subagent_dispatch.rs)
- [happy] Returns DispatchDecision::Inline when parent_lock_held signal is true, overriding high complexity  (crates/vox-orchestrator/src/subagent_dispatch.rs)
- [invariant] Prioritizes Reject decision over Inline constraints; returns Reject when chain_depth at limit even with budget_exhausted and parent_lock_held both true  (crates/vox-orchestrator/src/subagent_dispatch.rs)

### `HitlAction`  (happy, invariant; EXTRACTED)
- [happy] When all risk dimensions are zero, m.evaluate() returns Proceed action  (crates/vox-orchestrator/src/risk_matrix.rs)
- [invariant] HitlAction for Critical grade is BlockAndEscalate  (crates/vox-orchestrator/src/risk_matrix.rs)
- [happy] Composite medium-risk dimensions result in WarnContext action  (crates/vox-orchestrator/src/risk_matrix.rs)
- [happy] Composite high-risk dimensions result in Escalate action  (crates/vox-orchestrator/src/risk_matrix.rs)
- [happy] When risk grade reaches Critical, hitl_action is set to HitlAction::BlockAndEscalate  (crates/vox-orchestrator/src/orchestrator_policy.rs)

### `Orchestrator.status()`  (happy; EXTRACTED)
- [happy] Orchestrator.status().agent_count increments to 1 after accepting handoff  (crates/vox-orchestrator/tests/handoff_test.rs)
- [happy] status() reports total_completed equal to submitted task count after all tasks are processed  (crates/vox-orchestrator/tests/stress_test.rs)
- [happy] status() reports total_queued as 0 when all tasks have been completed  (crates/vox-orchestrator/tests/stress_test.rs)
- [happy] status() reports total_in_progress as 0 when no tasks are actively running  (crates/vox-orchestrator/tests/stress_test.rs)
- [happy] status() reports total_completed equal to 1000 when 1000 tasks are submitted and drained  (crates/vox-orchestrator/tests/stress_test.rs)

### `PreregGate`  (error, happy; EXTRACTED)
- [happy] PreregGate::new() constructs a gate instance that refuses check_campaign(None, None)  (crates/vox-orchestrator/tests/research_gate.rs)
- [happy] PreregGate.check_campaign() returns GateResult::Approved when given valid preregistration with correct signature  (crates/vox-orchestrator/src/preregistration/gate.rs)
- [error] PreregGate.check_campaign() returns GateResult::Refused with reason mentioning preregistration when prereg is None  (crates/vox-orchestrator/src/preregistration/gate.rs)
- [error] PreregGate.check_campaign() returns GateResult::Refused with reason mentioning signature when signature is None despite valid prereg  (crates/vox-orchestrator/src/preregistration/gate.rs)
- [error] PreregGate.check_campaign() returns GateResult::Refused with reason mentioning signature when provided signature is invalid  (crates/vox-orchestrator/src/preregistration/gate.rs)

### `StopDecision`  (edge, happy; EXTRACTED)
- [happy] BayesianStoppingRule.should_stop returns StopAccept when posterior probability (0.97) exceeds threshold (0.95)  (crates/vox-orchestrator/src/preregistration/symbolic.rs)
- [happy] BayesianStoppingRule.should_stop returns StopReject when posterior probability (0.02) falls below inverse threshold (0.05 = 1-0.95)  (crates/vox-orchestrator/src/preregistration/symbolic.rs)
- [happy] BayesianStoppingRule.should_stop returns Continue when posterior probability (0.50) is within acceptable region between thresholds  (crates/vox-orchestrator/src/preregistration/symbolic.rs)
- [edge] BayesianStoppingRule.should_stop returns StopAccept at exact threshold boundary (posterior == threshold == 0.95)  (crates/vox-orchestrator/src/preregistration/symbolic.rs)
- [edge] BayesianStoppingRule.should_stop uses default threshold of 0.95 when StopRule.threshold is None  (crates/vox-orchestrator/src/preregistration/symbolic.rs)

### `TierCascadeRouter::select()`  (edge, happy; EXTRACTED)
- [happy] Returns RoutingTier::Economy for low complexity signals (complexity=2) with sufficient confidence  (crates/vox-orchestrator/src/tier_cascade.rs)
- [happy] Returns RoutingTier::Standard for mid-range complexity signals (complexity=5) with sufficient confidence  (crates/vox-orchestrator/src/tier_cascade.rs)
- [happy] Returns RoutingTier::Strong for high complexity signals (complexity=9) with sufficient confidence  (crates/vox-orchestrator/src/tier_cascade.rs)
- [edge] Returns RoutingTier::Economy when budget_exhausted is true, overriding high complexity and warning alarm level  (crates/vox-orchestrator/src/tier_cascade.rs)
- [edge] Upgrades from RoutingTier::Economy to RoutingTier::Standard when confidence is low (0.40) for low complexity signals  (crates/vox-orchestrator/src/tier_cascade.rs)

### `analyze_plan_refinement_report`  (error, happy; EXTRACTED)
- [error] Sets is_too_thin flag when plan has too few tasks for complex goal via assert assertion  (crates/vox-orchestrator/src/planning/plan_adequacy.rs)
- [error] Includes 'too_few_tasks' in reason_codes when tasks are insufficient via iter.any assertion  (crates/vox-orchestrator/src/planning/plan_adequacy.rs)
- [error] Flags complex mutating tasks without preconditions with 'precondition_missing' via iter.any assertion  (crates/vox-orchestrator/src/planning/plan_adequacy.rs)
- [happy] Does not flag complex mutating tasks when preconditions are present via negated iter.any assertion  (crates/vox-orchestrator/src/planning/plan_adequacy.rs)
- [error] Flags repeated identical task descriptions with 'repeated_task_phrasing' in reason_codes via iter.any assertion  (crates/vox-orchestrator/src/planning/plan_adequacy.rs)

### `infer_strengths`  (happy; EXTRACTED)
- [happy] presence of 'tools' parameter causes StrengthTag::Codegen and StrengthTag::Logic to be inferred, suppressing StrengthTag::Generalist  (crates/vox-orchestrator/src/catalog.rs)
- [happy] deepseek provider family infers StrengthTag::Codegen even with no parameters or name signals, and suppresses Generalist  (crates/vox-orchestrator/src/catalog.rs)
- [happy] name heuristic 'code' in model name yields StrengthTag::Codegen when provider is unknown  (crates/vox-orchestrator/src/catalog.rs)
- [happy] infer_strengths returns exactly [StrengthTag::Generalist] for totally unknown model with no signals  (crates/vox-orchestrator/src/catalog.rs)
- [happy] presence of 'reasoning' parameter causes StrengthTag::Logic and StrengthTag::Debugging to be inferred  (crates/vox-orchestrator/src/catalog.rs)

### `markdown_to_content_blocks()`  (edge, happy; EXTRACTED)
- [happy] markdown_to_content_blocks() correctly extracts code fences from markdown  (crates/vox-orchestrator/src/planning/content_blocks.rs)
- [happy] markdown_to_content_blocks() parses numbered lists into task items  (crates/vox-orchestrator/src/planning/content_blocks.rs)
- [edge] markdown_to_content_blocks() handles unclosed code fences without panicking  (crates/vox-orchestrator/src/planning/content_blocks.rs)
- [edge] markdown_to_content_blocks() returns empty vector for empty string input  (crates/vox-orchestrator/src/planning/content_blocks.rs)
- [edge] markdown_to_content_blocks() returns empty vector for whitespace-only input  (crates/vox-orchestrator/src/planning/content_blocks.rs)

### `route_for_level`  (happy; EXTRACTED)
- [happy] route_for_level routes PrivacyLevel::Regulated to PrivacyRoutingDecision::LocalOnly  (crates/vox-orchestrator/src/privacy_classifier.rs)
- [happy] route_for_level with force_local=true routes PrivacyLevel::Private to PrivacyRoutingDecision::LocalOnly  (crates/vox-orchestrator/src/privacy_classifier.rs)
- [happy] route_for_level with force_local=false routes PrivacyLevel::Private to PrivacyRoutingDecision::Redact  (crates/vox-orchestrator/src/privacy_classifier.rs)
- [happy] route_for_level routes PrivacyLevel::Internal to PrivacyRoutingDecision::Redact  (crates/vox-orchestrator/src/privacy_classifier.rs)
- [happy] route_for_level routes PrivacyLevel::Public to PrivacyRoutingDecision::Allowed  (crates/vox-orchestrator/src/privacy_classifier.rs)

### `submission_approval_block_reason()`  (happy, invariant; EXTRACTED)
- [happy] submission_approval_block_reason() returns Some reason when approval_tier is Blocked  (crates/vox-orchestrator/src/orchestrator/task_dispatch/submit/attention_fields.rs)
- [invariant] reason returned for Blocked tier contains the word 'Blocked'  (crates/vox-orchestrator/src/orchestrator/task_dispatch/submit/attention_fields.rs)
- [happy] submission_approval_block_reason() returns Some reason when status is BlockedOnApproval  (crates/vox-orchestrator/src/orchestrator/task_dispatch/submit/attention_fields.rs)
- [invariant] reason for BlockedOnApproval status contains phrase 'requires explicit approval'  (crates/vox-orchestrator/src/orchestrator/task_dispatch/submit/attention_fields.rs)
- [happy] submission_approval_block_reason() returns None when approval_tier is not Blocked  (crates/vox-orchestrator/src/orchestrator/task_dispatch/submit/attention_fields.rs)

### `task_matches_populi_remote_lease_gate()`  (error, happy; EXTRACTED)
- [happy] Function returns false when task has no execution_role  (crates/vox-orchestrator/src/populi_remote.rs)
- [happy] Function returns true when task execution_role matches a configured lease-gated role  (crates/vox-orchestrator/src/populi_remote.rs)
- [happy] Function returns false when task execution_role does not match configured lease-gated roles  (crates/vox-orchestrator/src/populi_remote.rs)
- [error] Function always returns false when lease gating is disabled, regardless of execution_role  (crates/vox-orchestrator/src/populi_remote.rs)
- [happy] When task execution_role is set from TaskEnqueueHints and matches gate role, function returns true  (crates/vox-orchestrator/src/populi_remote.rs)

### `ConfidenceFusion::decide()`  (edge, happy; EXTRACTED)
- [happy] Returns FusionDecision::Ship when score is exactly 0.85  (crates/vox-orchestrator/src/confidence_fusion.rs)
- [edge] Returns FusionDecision::Resample when score is 0.84 (below 0.85 threshold)  (crates/vox-orchestrator/src/confidence_fusion.rs)
- [happy] Returns FusionDecision::SpawnSocrates when score is exactly 0.30  (crates/vox-orchestrator/src/confidence_fusion.rs)
- [edge] Returns FusionDecision::Abstain when score is 0.29 (below 0.30 threshold)  (crates/vox-orchestrator/src/confidence_fusion.rs)

### `ContextEnvelope::with_agentos_intent_hints()`  (happy; EXTRACTED)
- [happy] Populates suggested_tools vector with tool name 'vox_run_tests' when intent is 'run cargo tests'  (crates/vox-orchestrator/src/context_envelope.rs)
- [happy] Sets sparse_checkpoint_recommended to Some(false) for non-mutating intent 'run cargo tests'  (crates/vox-orchestrator/src/context_envelope.rs)
- [happy] Sets sparse_checkpoint_recommended to Some(true) when intent contains mutation keyword 'patch'  (crates/vox-orchestrator/src/context_envelope.rs)
- [happy] Includes 'vox_write_file' in suggested_tools when intent indicates file mutation  (crates/vox-orchestrator/src/context_envelope.rs)

### `DeviationDetector`  (error, happy; EXTRACTED)
- [happy] DeviationDetector.check() returns Report with is_clean=true and empty deviations when metric and test match preregistration  (crates/vox-orchestrator/src/preregistration/deviation.rs)
- [error] DeviationDetector.check() sets metric_matches=false and includes metric deviation in report when metric name differs from preregistration  (crates/vox-orchestrator/src/preregistration/deviation.rs)
- [error] DeviationDetector.check() sets test_matches=false and includes test deviation in report when test kind differs from preregistration  (crates/vox-orchestrator/src/preregistration/deviation.rs)
- [error] DeviationDetector.check() reports both metric and test mismatches when both differ from preregistration (deviations.len()==2)  (crates/vox-orchestrator/src/preregistration/deviation.rs)

### `OrchestratorPolicy::evaluate()`  (error, happy; EXTRACTED)
- [happy] Default PolicyContext produces: no circuit trip, AlarmTier::None, valid fusion_score [0-1], Standard routing tier, React plan mode, Proceed HITL action, Redact privacy routing, and Inline dispatch  (crates/vox-orchestrator/src/orchestrator_policy.rs)
- [error] Circuit breaker with 3 no_progress_loops trips with TripReason::NoProgress  (crates/vox-orchestrator/src/orchestrator_policy.rs)
- [happy] When budget token fraction is 0.97 (exhausted), routing tier downgrades to Economy and budget_decision marks exhausted status  (crates/vox-orchestrator/src/orchestrator_policy.rs)
- [happy] When regulated_marker_detected is set, privacy level classifies as Regulated and routing decision becomes LocalOnly  (crates/vox-orchestrator/src/orchestrator_policy.rs)

### `RemoteTaskEnvelope`  (happy; EXTRACTED)
- [happy] RemoteTaskEnvelope deserializes from JSON lacking trace fields (parent_task_id, trace_id, span_depth), with those fields as None.  (crates/vox-orchestrator/src/a2a/envelope.rs)
- [happy] RemoteTaskEnvelope with trace fields (parent_task_id, span_depth, trace_id) round-trips through JSON serialization preserving values.  (crates/vox-orchestrator/src/a2a/envelope.rs)
- [happy] Legacy RemoteTaskEnvelope JSON without exec_source fields deserializes with exec_source_b64 and exec_source_blake3_hex as None.  (crates/vox-orchestrator/src/a2a/envelope.rs)
- [happy] RemoteTaskEnvelope with exec_source_b64 and exec_source_blake3_hex set emits those fields in JSON and round-trips preserving values.  (crates/vox-orchestrator/src/a2a/envelope.rs)

### `RouteResult`  (happy; EXTRACTED)
- [happy] When experimental mesh routing is enabled and task capability labels match local agent labels, RouteResult::Existing is returned for the matching agent  (crates/vox-orchestrator/src/services/routing.rs)
- [happy] Training routing selects the agent with higher VRAM when multiple agents have GPU capability  (crates/vox-orchestrator/src/services/routing.rs)
- [happy] When attention trust routing is enabled with configured weight, the agent with higher trust score is selected  (crates/vox-orchestrator/src/services/routing.rs)
- [happy] Agents below task_completion_trust_floor score are disqualified and only agents above the floor are routed to  (crates/vox-orchestrator/src/services/routing.rs)

### `ScopeGuard.check_write()`  (error, happy; EXTRACTED)
- [error] check_write denies writes to files outside agent's assigned scope when enforcement is Strict  (crates/vox-orchestrator/src/scope.rs)
- [happy] check_write allows out-of-scope writes when enforcement is Warn but returns warned result  (crates/vox-orchestrator/src/scope.rs)
- [happy] check_write allows writes anywhere when enforcement is Disabled  (crates/vox-orchestrator/src/scope.rs)
- [happy] check_write allows writes to remaining assigned file but denies writes to revoked file  (crates/vox-orchestrator/src/scope.rs)

### `TierRouter::select()`  (edge, happy, invariant; EXTRACTED)
- [happy] When complexity is 5, alarm_level is None, and confidence is 0.40 (below threshold), select() returns RoutingTier::Strong  (crates/vox-orchestrator/src/tier_cascade.rs)
- [happy] When complexity is 2, alarm_level is Warning, and confidence is 0.80, select() returns RoutingTier::Standard  (crates/vox-orchestrator/src/tier_cascade.rs)
- [edge] When complexity is 2, alarm_level is Caution, and confidence is 0.80, select() returns RoutingTier::Economy (Caution does not upgrade tier)  (crates/vox-orchestrator/src/tier_cascade.rs)
- [invariant] When complexity is 5, alarm_level is Warning, and confidence is 0.40, select() returns RoutingTier::Strong (tier selection caps at Strong maximum)  (crates/vox-orchestrator/src/tier_cascade.rs)

### `classify_tier`  (error, happy, invariant; EXTRACTED)
- [happy] classify_tier returns ApprovalTier::AutoApprove for read-only actions (0 writes) regardless of trust tier.  (crates/vox-orchestrator/src/attention/mod.rs)
- [error] classify_tier returns ApprovalTier::Blocked when an Untrusted agent attempts 4 file writes.  (crates/vox-orchestrator/src/attention/mod.rs)
- [error] classify_tier returns ApprovalTier::Blocked for actions marked external=true, regardless of trust tier or entropy.  (crates/vox-orchestrator/src/attention/mod.rs)
- [invariant] classify_tier returns AutoApprove when entropy < 0.15 AND trust >= 0.85 AND action has 10+ repeated approvals, but denies when entropy > 0.15.  (crates/vox-orchestrator/src/attention/mod.rs)

### `evaluate_interruption`  (happy; EXTRACTED)
- [happy] evaluate_interruption() returns InterruptionDecision::InterruptNow when in shadow mode  (crates/vox-orchestrator/src/attention/interruption_policy.rs)
- [happy] evaluate_interruption() returns RequireHumanBeforeContinue when risk is high and confidence is low  (crates/vox-orchestrator/src/attention/interruption_policy.rs)
- [happy] evaluate_interruption() returns DeferUntilCheckpoint when utility is low and budget is mostly spent  (crates/vox-orchestrator/src/attention/interruption_policy.rs)
- [happy] evaluate_interruption() returns ProceedAutonomously when max clarification turns reached  (crates/vox-orchestrator/src/attention/interruption_policy.rs)

### `parse()`  (error, happy; EXTRACTED)
- [happy] parse() successfully decodes a valid traceparent header and preserves trace_id and parent_id  (crates/vox-orchestrator/tests/traceparent_roundtrip.rs)
- [error] parse() returns None for empty string  (crates/vox-orchestrator/tests/traceparent_roundtrip.rs)
- [error] parse() returns None for invalid format (non-traceparent string)  (crates/vox-orchestrator/tests/traceparent_roundtrip.rs)
- [error] parse() returns None for truncated/malformed traceparent with wrong segment lengths  (crates/vox-orchestrator/tests/traceparent_roundtrip.rs)

### `AffinityGroupRegistry::defaults()::resolve()`  (happy; EXTRACTED)
- [happy] AffinityGroupRegistry::defaults() resolves parser/mod.rs to a group named 'lexer-parser-group'  (crates/vox-orchestrator/src/groups.rs)
- [happy] AffinityGroupRegistry::defaults() resolves typeck/infer.rs to a group named 'typeck-group'  (crates/vox-orchestrator/src/groups.rs)
- [happy] AffinityGroupRegistry::defaults() resolves codegen-rust/src/emit.rs to a group named 'codegen-rust-group'  (crates/vox-orchestrator/src/groups.rs)

### `BudgetManager::doom_loop_cost_check`  (happy; EXTRACTED)
- [happy] doom_loop_cost_check returns None when accumulated cost is at or below threshold, but returns Some with reason when cost exceeds threshold.  (crates/vox-orchestrator/src/budget/mod.rs)
- [happy] doom_loop_cost_check returns None for an agent that has never been tracked.  (crates/vox-orchestrator/src/budget/mod.rs)
- [happy] doom_loop_cost_check returns None when cost equals threshold (strict > contract)  (crates/vox-orchestrator/src/budget/mod.rs)

### `BudgetManager::would_exceed_token_budget`  (happy; EXTRACTED)
- [happy] would_exceed_token_budget returns true when remaining tokens (100) plus requested (200) exceeds budget (1000)  (crates/vox-orchestrator/src/budget/mod.rs)
- [happy] would_exceed_token_budget returns false when remaining tokens are sufficient for requested amount  (crates/vox-orchestrator/src/budget/mod.rs)
- [happy] would_exceed_token_budget returns false when no budget has been set for the agent  (crates/vox-orchestrator/src/budget/mod.rs)

### `CalibrationLoop`  (happy, invariant; EXTRACTED)
- [invariant] z_score is 0.0 for all observations before min_observations threshold is reached  (crates/vox-orchestrator/src/calibration.rs)
- [happy] drift_detected is true and z_score exceeds 2.0 when an outlier (50.0) is observed against normal variance baseline  (crates/vox-orchestrator/src/calibration.rs)
- [happy] drift_detected is false when observations remain within established statistical range  (crates/vox-orchestrator/src/calibration.rs)

### `ContextEnvelope.verify()`  (error, happy; EXTRACTED)
- [happy] ContextEnvelope.verify() returns false before signing  (crates/vox-orchestrator/tests/context_envelope_contract.rs)
- [happy] ContextEnvelope.verify() returns true after signing with correct key  (crates/vox-orchestrator/tests/context_envelope_contract.rs)
- [error] ContextEnvelope.verify() returns false when verifying with wrong key  (crates/vox-orchestrator/tests/context_envelope_contract.rs)

### `GateResult`  (happy; EXTRACTED)
- [happy] check_doom_loop returns GateResult::DoomLoop variant when cost progress exceeds doom_loop_cost_threshold  (crates/vox-orchestrator/src/gate.rs)
- [happy] check_doom_loop returns GateResult::Allowed after recording task completion, even with prior cost overage  (crates/vox-orchestrator/src/gate.rs)
- [happy] check_attention_snapshot returns GateResult::Allowed when attention_enabled is false, regardless of debit amount  (crates/vox-orchestrator/src/gate.rs)

### `InMemoryHopper`  (happy; EXTRACTED)
- [happy] submit() creates a new item with state=ItemState::Inbox that appears in inbox() and not in history()  (crates/vox-orchestrator/src/hopper/store.rs)
- [happy] reprioritize() updates item classified_priority to Urgent and records the DeveloperOverride capability in override_history with audit_id  (crates/vox-orchestrator/src/hopper/store.rs)
- [happy] Submitted item has priority_source=Orchestrator initially; after reprioritize() with DeveloperOverride cap, priority_source changes to Developer  (crates/vox-orchestrator/src/hopper/store.rs)

### `InProcessSkillRuntime.run_with_secrets()`  (error, happy; EXTRACTED)
- [happy] run_with_secrets() returns outcome with exit_code 0 when secrets are provided  (crates/vox-orchestrator/tests/skill_runtime_inproc.rs)
- [error] run_with_secrets() does not leak secret values to stdout  (crates/vox-orchestrator/tests/skill_runtime_inproc.rs)
- [error] run_with_secrets() does not leak secret values to stderr  (crates/vox-orchestrator/tests/skill_runtime_inproc.rs)

### `IsolationStrategy`  (happy, invariant; EXTRACTED)
- [happy] When long_running signal is true, choose_strategy returns SeparateBranches even with SharedBranch default  (crates/vox-orchestrator/src/isolation.rs)
- [happy] When no signal overrides present, choose_strategy returns the configured default strategy  (crates/vox-orchestrator/src/isolation.rs)
- [invariant] IsolationStrategy::SeparateBranches survives JSON serialization and deserialization roundtrip  (crates/vox-orchestrator/src/config/orchestrator_fields.rs)

### `NewsService::tick`  (happy; EXTRACTED)
- [happy] NewsService::tick does not publish news when publish_armed is false  (crates/vox-orchestrator/tests/news_service_test.rs)
- [happy] NewsService::tick publishes news when armed and content has dual approval from both alice and bob  (crates/vox-orchestrator/tests/news_service_test.rs)
- [happy] NewsService::tick blocks publication when worthiness enforcement is enabled and score is below the minimum floor  (crates/vox-orchestrator/tests/news_service_test.rs)

### `Observer::compute_action_raw`  (error, happy; EXTRACTED)
- [happy] returns Continue action when called with clean source (0 errors, high parse and coverage scores)  (crates/vox-orchestrator/src/observer.rs)
- [error] returns TriggerReplan action when lsp_error_count is 6 (above threshold) with high quality scores  (crates/vox-orchestrator/src/observer.rs)
- [error] returns EscalateToHuman action when lsp_error_count is 20 (severe) even with decent quality scores  (crates/vox-orchestrator/src/observer.rs)

### `Orchestrator`  (happy; EXTRACTED)
- [happy] After task completion and idle timeout, agents retire back to 1 from multiple agents  (crates/vox-orchestrator/tests/scaling_test.rs)
- [happy] total_weighted_load decreases after tasks are drained and time advances  (crates/vox-orchestrator/tests/scaling_test.rs)
- [happy] orchestrator can handle 1000 submitted tasks across 10 agents without exceeding drain round limit  (crates/vox-orchestrator/tests/stress_test.rs)

### `OrientPhase::classify_task_category`  (happy; EXTRACTED)
- [happy] Classifies 'document the API' as General task category via assert_eq assertion  (crates/vox-orchestrator/src/planning/orient.rs)
- [happy] Classifies 'run unit tests' as Testing task category via assert_eq assertion  (crates/vox-orchestrator/src/planning/orient.rs)
- [happy] Classifies 'implement standard backend' as CodeGen task category via assert_eq assertion  (crates/vox-orchestrator/src/planning/orient.rs)

### `PlacementReasonCode`  (invariant; EXTRACTED)
- [invariant] LocalQueueDefault variant has stable string representation 'local_queue_default'  (crates/vox-orchestrator/src/populi_remote.rs)
- [invariant] PopuliRemoteLeaseHold variant has stable string representation 'populi_remote_lease_hold'  (crates/vox-orchestrator/src/populi_remote.rs)
- [invariant] LocalQueueFallbackAfterRemoteRelayError variant has stable string representation 'local_queue_fallback_after_remote_relay_error'  (crates/vox-orchestrator/src/populi_remote.rs)

### `PlanModeTrigger::decide`  (edge, happy; EXTRACTED)
- [happy] Returns React mode for low-complexity signal (complexity=3, deps=1) via assert_eq assertion  (crates/vox-orchestrator/src/planning/plan_mode_trigger.rs)
- [happy] Returns PlanAndExecute mode when complexity is 8 via assert_eq assertion  (crates/vox-orchestrator/src/planning/plan_mode_trigger.rs)
- [edge] Returns PlanAndExecute mode when dependency_count is 3 despite low complexity via assert_eq assertion  (crates/vox-orchestrator/src/planning/plan_mode_trigger.rs)

### `PrivacyClassifier::classify`  (happy; EXTRACTED)
- [happy] Classifier with internal_marker set to true returns PrivacyLevel::Internal  (crates/vox-orchestrator/src/privacy_classifier.rs)
- [happy] Classifier with public_source set to true returns PrivacyLevel::Public  (crates/vox-orchestrator/src/privacy_classifier.rs)
- [happy] Classifier with default (empty) signals returns PrivacyLevel::Internal  (crates/vox-orchestrator/src/privacy_classifier.rs)

### `PrivacyLevel`  (happy, invariant; EXTRACTED)
- [happy] When regulated_marker_detected signal is true, classify() returns PrivacyLevel::Regulated  (crates/vox-orchestrator/src/privacy_classifier.rs)
- [happy] When only pii_detected signal is true (no regulated marker), classify() returns PrivacyLevel::Private  (crates/vox-orchestrator/src/privacy_classifier.rs)
- [invariant] When both pii_detected and regulated_marker_detected are true, regulated_marker takes precedence and returns PrivacyLevel::Regulated  (crates/vox-orchestrator/src/privacy_classifier.rs)

### `RoutingProfile`  (happy; EXTRACTED)
- [happy] Only RoutingProfile::Free variant returns true from is_free_only(); other variants return false  (crates/vox-orchestrator/src/types/routing_profile.rs)
- [happy] prefers_free() returns true for Free and Mixed variants, false for Performance and Local variants  (crates/vox-orchestrator/src/types/routing_profile.rs)
- [happy] Only RoutingProfile::Local variant returns true from is_local_only(); other variants return false  (crates/vox-orchestrator/src/types/routing_profile.rs)

### `RoutingProfile::from_str()`  (error, happy; EXTRACTED)
- [happy] from_str() correctly parses all canonical string representations (Free, Mixed, Performance, Local) to their corresponding RoutingProfile variants  (crates/vox-orchestrator/src/types/routing_profile.rs)
- [happy] from_str() accepts 'perf' as an alias for Performance variant and is case-insensitive (handles uppercase and whitespace)  (crates/vox-orchestrator/src/types/routing_profile.rs)
- [error] from_str() returns Err(()) when given unknown input strings  (crates/vox-orchestrator/src/types/routing_profile.rs)

### `RoutingService::route`  (happy, invariant; EXTRACTED)
- [invariant] With epsilon=1.0 (100% exploration) and 3 agents routing 1000 tasks, no single agent receives more than 70% share  (crates/vox-orchestrator/tests/routing_tests.rs)
- [happy] route returns higher-reliability agent when socrates_reputation_routing is enabled  (crates/vox-orchestrator/src/services/routing.rs)
- [happy] route selects GPU-capable agent when TaskCapabilityHints.prefer_gpu_compute is true  (crates/vox-orchestrator/src/services/routing.rs)

### `ScopeCheckResult`  (happy; EXTRACTED)
- [happy] ScopeCheckResult::Denied variant contains reason string mentioning 'outside its assigned scope'  (crates/vox-orchestrator/src/scope.rs)
- [happy] ScopeCheckResult matches Warned variant when enforcement is Warn and file is out of scope  (crates/vox-orchestrator/src/scope.rs)
- [happy] ScopeCheckResult matches Allowed variant when enforcement is Disabled  (crates/vox-orchestrator/src/scope.rs)

### `ScopeGuard.check_write`  (happy; EXTRACTED)
- [happy] check_write on assigned paths returns Allowed result  (crates/vox-orchestrator/tests/scope_test.rs)
- [happy] check_write in Strict mode denies access to out-of-scope paths for assigned agents  (crates/vox-orchestrator/tests/scope_test.rs)
- [happy] Unassigned agents can write anywhere regardless of scope enforcement level  (crates/vox-orchestrator/tests/scope_test.rs)

### `SelectionIntent::repair_loop()`  (happy; EXTRACTED)
- [happy] SelectionIntent::repair_loop() sets cacheable_workload to true  (crates/vox-orchestrator/src/models/select.rs)
- [happy] SelectionIntent::repair_loop() sets caller_hint to 'repair-loop'  (crates/vox-orchestrator/src/models/select.rs)
- [happy] SelectionIntent::repair_loop() sets task category to TaskCategory::CodeGen  (crates/vox-orchestrator/src/models/select.rs)

### `SubAgentRouter::route`  (happy; EXTRACTED)
- [happy] SubAgentRouter::route returns DispatchDecision::Inline for low complexity scores below the spawn threshold  (crates/vox-orchestrator/src/subagent_dispatch.rs)
- [happy] SubAgentRouter::route returns DispatchDecision::Spawn for high complexity scores above the spawn threshold  (crates/vox-orchestrator/src/subagent_dispatch.rs)
- [happy] SubAgentRouter::route returns DispatchDecision::Spawn when complexity equals the spawn threshold (6)  (crates/vox-orchestrator/src/subagent_dispatch.rs)

### `SymbolicVerdict`  (edge, happy; EXTRACTED)
- [happy] NumericComparatorVerifier returns Refuted when claim text specifies 'decreased' but measured value (4.0) is higher than baseline (3.0)  (crates/vox-orchestrator/src/preregistration/symbolic.rs)
- [edge] NumericComparatorVerifier returns Inconclusive when claim text lacks directional keywords  (crates/vox-orchestrator/src/preregistration/symbolic.rs)
- [edge] NumericComparatorVerifier returns Inconclusive when measured value equals baseline even if direction keyword is present  (crates/vox-orchestrator/src/preregistration/symbolic.rs)

### `bigram_jaccard()`  (edge, happy; EXTRACTED)
- [happy] bigram_jaccard returns value approximately equal to 1.0 for identical sequences  (crates/vox-orchestrator/src/circuit_breaker.rs)
- [edge] bigram_jaccard returns value approximately equal to 0.0 for completely disjoint sequences  (crates/vox-orchestrator/src/circuit_breaker.rs)
- [edge] bigram_jaccard returns value approximately equal to 0.0 when both input slices are empty  (crates/vox-orchestrator/src/circuit_breaker.rs)

### `dispatch_request()`  (happy; EXTRACTED)
- [happy] dispatch_request() with VCS_ISOLATION_SET_STRATEGY persists strategy_default value in shared orchestrator state  (crates/vox-orchestrator/src/orch_daemon/mod.rs)
- [happy] dispatch_request() with agent_id and strategy parameter sets per_agent override in response  (crates/vox-orchestrator/src/orch_daemon/mod.rs)
- [happy] dispatch_request() with null strategy value clears per_agent override, leaving empty object  (crates/vox-orchestrator/src/orch_daemon/mod.rs)

### `gate_secrets`  (happy, invariant; EXTRACTED)
- [invariant] Returns empty vector when execution tier is BareMetal, regardless of declared or bag contents  (crates/vox-orchestrator/src/a2a/secret_gate.rs)
- [happy] gate_secrets() filters out credentials and injects only low-value secrets when ExecTier is Sandboxed  (crates/vox-orchestrator/src/a2a/secret_gate.rs)
- [happy] gate_secrets() returns correct secret values for injected low-value secrets  (crates/vox-orchestrator/src/a2a/secret_gate.rs)

### `parse_remote_payload_context`  (edge, happy; EXTRACTED)
- [happy] parse_remote_payload_context correctly extracts and trims session_id, thread_id, context_envelope_json, and harness_spec_json from JSON payload  (crates/vox-orchestrator/src/a2a/remote_worker.rs)
- [edge] parse_remote_payload_context returns None for all optional fields when they are missing from the JSON payload  (crates/vox-orchestrator/src/a2a/remote_worker.rs)
- [happy] parse_remote_payload_context handles context_envelope_json as both string and object form, serializing objects to JSON string  (crates/vox-orchestrator/src/a2a/remote_worker.rs)

### `sensitivity_of`  (happy; EXTRACTED)
- [happy] sensitivity_of() classifies OpenRouterApiKey as Credential sensitivity level  (crates/vox-orchestrator/src/a2a/secret_gate.rs)
- [happy] sensitivity_of() classifies VoxOpenRouterChatModel as LowValue sensitivity level  (crates/vox-orchestrator/src/a2a/secret_gate.rs)
- [happy] sensitivity_of() defaults unknown secrets to Credential sensitivity level  (crates/vox-orchestrator/src/a2a/secret_gate.rs)

### `should_promote() on ModelConfidence::Shadowed`  (edge, error, happy; EXTRACTED)
- [edge] should_promote() stays None when successful call count is below threshold (10 calls insufficient)  (crates/vox-orchestrator/src/models/autonomic.rs)
- [happy] should_promote() promotes to Confirmed when call count is sufficient (100+) and latency is acceptable relative to catalog median  (crates/vox-orchestrator/src/models/autonomic.rs)
- [error] should_promote() blocks promotion to Confirmed when latency exceeds 3x the catalog median, even with sufficient call count  (crates/vox-orchestrator/src/models/autonomic.rs)

### `snapshot_list_json`  (happy, invariant; EXTRACTED)
- [invariant] returns snapshot list with id, agent_id, description, file_count round-tripping from workspace_create_json  (crates/vox-orchestrator/src/json_vcs_facade.rs)
- [invariant] when scoped to specific agent, returns only that agent's snapshots and not other agents' snapshots  (crates/vox-orchestrator/src/json_vcs_facade.rs)
- [happy] when called without agent scope, returns snapshots for all agents  (crates/vox-orchestrator/src/json_vcs_facade.rs)

### `validate_handoff_invariants()`  (error, happy; EXTRACTED)
- [error] Detects and rejects payload when context contains raw transcript markers like '<|im_start|>'  (crates/vox-orchestrator/src/handoff.rs)
- [error] Detects and rejects payload when metadata contains raw transcript (e.g., 'raw_history' field)  (crates/vox-orchestrator/src/handoff.rs)
- [happy] Accepts payloads with clean, summarized context without raw transcript markers  (crates/vox-orchestrator/src/handoff.rs)

### `AffinityGroupRegistry::detect_from_repository_layout`  (happy; EXTRACTED)
- [happy] creates groups with names matching detected Node.js package layout members  (crates/vox-orchestrator/src/groups.rs)
- [happy] Auto-detects workspace members as affinity groups named <member>-group  (crates/vox-orchestrator/tests/affinity_group_tests.rs)

### `AgentQueue`  (happy, invariant; EXTRACTED)
- [happy] AgentQueue.dequeue returns tasks in priority order (Urgent before Normal before Background)  (crates/vox-orchestrator/src/queue/mod.rs)
- [invariant] AgentQueue maintains FIFO order when dequeuing tasks with identical priority  (crates/vox-orchestrator/src/queue/mod.rs)

### `AgentTask::start()`  (happy, invariant; EXTRACTED)
- [happy] start() sets started_at_ms field to a recent timestamp (within 5000ms of current time)  (crates/vox-orchestrator/src/types/tasks.rs)
- [invariant] start() is idempotent such that calling it twice does not go backward in time (second timestamp >= first timestamp)  (crates/vox-orchestrator/src/types/tasks.rs)

### `AgentTrustScore::record_outcome`  (error, happy; EXTRACTED)
- [happy] After 5 successful outcomes with 0.1 threshold, trust score exceeds 0.45 or tier becomes Provisional, proving EWMA promotion logic.  (crates/vox-orchestrator/src/attention/mod.rs)
- [error] After 3 consecutive failures (false outcomes), a Trusted tier agent demotes to Provisional or drops below 0.70 trust score.  (crates/vox-orchestrator/src/attention/mod.rs)

### `AgentWorkspace::set_bound_branch`  (happy; EXTRACTED)
- [happy] Sets and retrieves bound_branch, returning the value via bound_branch() method  (crates/vox-orchestrator/src/workspace.rs)
- [happy] Returns the previous branch name when rebinding, confirming old value is displaced by new  (crates/vox-orchestrator/src/workspace.rs)

### `AttentionBudget::focus_depth`  (happy; EXTRACTED)
- [happy] focus_depth returns FocusDepth::Deep when interrupt_freq_per_hour is 8.0.  (crates/vox-orchestrator/src/attention/mod.rs)
- [happy] focus_depth returns FocusDepth::Focused when interrupt_freq_per_hour is 5.0.  (crates/vox-orchestrator/src/attention/mod.rs)

### `BuildStageKind serialization`  (invariant; EXTRACTED)
- [invariant] BuildStageKind serializes to snake_case JSON (codegen not CamelCase)  (crates/vox-orchestrator/tests/event_bus_new_variants_test.rs)
- [invariant] All BuildStageKind variants serialize to their snake_case representations  (crates/vox-orchestrator/tests/event_bus_new_variants_test.rs)

### `BulletinBoard`  (happy; EXTRACTED)
- [happy] publish() delivers message via subscribe() receiver with TaskId and AgentId fields intact  (crates/vox-orchestrator/src/bulletin.rs)
- [happy] subscribe() returns independent receivers and publish() delivers same message to all subscribers  (crates/vox-orchestrator/src/bulletin.rs)

### `CachePrediction`  (edge, happy; EXTRACTED)
- [happy] CachePrediction::Miss is returned when prefix_overlap_tokens (500) is below threshold relative to total_context_tokens (1000)  (crates/vox-orchestrator/src/cache_predictor.rs)
- [edge] CachePrediction::Miss is returned when total_context_tokens is zero, regardless of prefix_overlap_tokens  (crates/vox-orchestrator/src/cache_predictor.rs)

### `CachePredictor::predict`  (edge, happy; EXTRACTED)
- [happy] predict() returns CachePrediction::Hit when prefix_overlap_tokens/total_tokens ratio (0.70) is at or above cache threshold  (crates/vox-orchestrator/src/cache_predictor.rs)
- [edge] predict() returns CachePrediction::Hit exactly at cache overlap threshold (70%)  (crates/vox-orchestrator/src/cache_predictor.rs)

### `CircuitBreaker::check_tier()`  (happy; EXTRACTED)
- [happy] check_tier returns AlarmTier::Caution when no_progress_loops equals 1  (crates/vox-orchestrator/src/circuit_breaker.rs)
- [happy] check_tier returns AlarmTier::Warning when no_progress_loops equals 2  (crates/vox-orchestrator/src/circuit_breaker.rs)

### `CircuitBreaker::should_escalate()`  (happy; EXTRACTED)
- [happy] should_escalate returns false when replan_attempts equals 2  (crates/vox-orchestrator/src/circuit_breaker.rs)
- [happy] should_escalate returns true when replan_attempts equals 3  (crates/vox-orchestrator/src/circuit_breaker.rs)

### `ConfidenceFusion::evaluate()`  (happy; EXTRACTED)
- [happy] Returns FusionDecision::Abstain when input quality metrics are all at 0.1 or lower  (crates/vox-orchestrator/src/confidence_fusion.rs)
- [happy] Returns FusionDecision::SpawnSocrates when all FusionInputs metrics equal 0.4  (crates/vox-orchestrator/src/confidence_fusion.rs)

### `ConflictManager::active_conflicts`  (edge, happy; EXTRACTED)
- [happy] active_conflicts count reflects conflicts recorded by record_overlap_conflicts  (crates/vox-orchestrator/src/merge_conflicts.rs)
- [edge] is_empty when record_overlap_conflicts receives no overlaps  (crates/vox-orchestrator/src/merge_conflicts.rs)

### `ConflictManager::resolve()`  (happy; EXTRACTED)
- [happy] Marks conflict as resolved, decrements active_count() to zero, and prevent double-resolution  (crates/vox-orchestrator/src/conflicts.rs)
- [happy] Accepts ConflictResolution::DeferToAgent variant and stores the delegated AgentId  (crates/vox-orchestrator/src/conflicts.rs)

### `ContextStore`  (happy; EXTRACTED)
- [happy] ContextStore.get() returns None for expired entries and expire_stale() removes them, returning the count of expired entries  (crates/vox-orchestrator/src/context/mod.rs)
- [happy] ContextStore.set_with_vcs() stores VcsContext metadata which is retrievable via get_entry() with preserved snapshot_before, snapshot_after, and touched_paths  (crates/vox-orchestrator/src/context/mod.rs)

### `EventBus`  (happy; EXTRACTED)
- [happy] EventBus.emit() sends LockAcquired event that is received by subscriber with correct agent_id and path  (crates/vox-orchestrator/tests/events_test.rs)
- [happy] EventBus correctly transmits BuildStage events with run_id preservation  (crates/vox-orchestrator/tests/event_bus_new_variants_test.rs)

### `EventBus::emit()`  (happy, invariant; EXTRACTED)
- [happy] Emit returns EventId(1) for first event, and emitted event can be received by subscriber with matching id, timestamp > 0, and kind matching original  (crates/vox-orchestrator/src/events.rs)
- [invariant] EventBus.emit() returns sequential EventIds starting from EventId(1), incrementing by 1 for each emission  (crates/vox-orchestrator/src/events.rs)

### `FileLockManager.try_acquire_persisted()`  (error, happy; EXTRACTED)
- [happy] first agent to acquire exclusive lock on a path succeeds  (crates/vox-orchestrator/tests/two_daemon_lock_contention.rs)
- [error] subsequent agents fail to acquire exclusive lock on already-locked path  (crates/vox-orchestrator/tests/two_daemon_lock_contention.rs)

### `HeartbeatMonitor`  (happy; EXTRACTED)
- [happy] Registers agents with initial Idle activity and updates activity on heartbeat calls  (crates/vox-orchestrator/src/heartbeat.rs)
- [happy] Calling heartbeat() clears stale status and resets staleness_level to Healthy  (crates/vox-orchestrator/src/heartbeat.rs)

### `IsolationPlan::strategy_for`  (happy; EXTRACTED)
- [happy] strategy_for returns agent-specific override when set_override has been called for that agent  (crates/vox-orchestrator/src/isolation.rs)
- [happy] strategy_for returns default strategy for agents without a set_override  (crates/vox-orchestrator/src/isolation.rs)

### `LivingReviewManifest`  (happy; EXTRACTED)
- [happy] LivingReviewManifest.version_count() returns 0 for newly created manifest  (crates/vox-orchestrator/src/preregistration/living_review.rs)
- [happy] LivingReviewManifest.latest_version() returns None when manifest has no versions  (crates/vox-orchestrator/src/preregistration/living_review.rs)

### `ModelRegistry::inject_scoreboard_latency()`  (edge, happy; EXTRACTED)
- [happy] inject_scoreboard_latency() returns count of updated models matching scoreboard rows  (crates/vox-orchestrator/src/models/tests.rs)
- [edge] inject_scoreboard_latency() returns 0 when all rows have None or non-positive p50 values  (crates/vox-orchestrator/src/models/tests.rs)

### `ModelSpec::latency_p50_ms`  (edge, happy; EXTRACTED)
- [happy] ModelSpec latency_p50_ms capability field is updated by inject_scoreboard_latency()  (crates/vox-orchestrator/src/models/tests.rs)
- [edge] None and non-positive p50 values in scoreboard rows do not update model latency_p50_ms  (crates/vox-orchestrator/src/models/tests.rs)

### `Observer::compute_action_raw()`  (happy; EXTRACTED)
- [happy] When parse confidence is 0.50 (below threshold) and coverage is 0.85, returns ObserverAction::RequestMoreEvidence  (crates/vox-orchestrator/src/observer.rs)
- [happy] When parse confidence is 0.95 and coverage is 0.40 (below threshold), returns ObserverAction::EmitNegativeExample  (crates/vox-orchestrator/src/observer.rs)

### `Observer::observe_rust_file()`  (error, happy; EXTRACTED)
- [happy] Observing clean, compilable Rust source code records correct task_id, reports zero LSP errors, and recommends Continue action  (crates/vox-orchestrator/src/observer.rs)
- [error] Observing Rust file with multiple todo!() macros detects >= 5 LSP errors and recommends either TriggerReplan or EscalateToHuman  (crates/vox-orchestrator/src/observer.rs)

### `OpLog`  (happy; EXTRACTED)
- [happy] OpLog.list() returns operations filtered by agent_id, and contains TaskSubmit and TaskComplete entries for task submissions and completions.  (crates/vox-orchestrator/tests/vcs_test.rs)
- [happy] When rebalance() moves tasks (moved > 0), OpLog contains a Rebalance operation kind entry.  (crates/vox-orchestrator/tests/vcs_test.rs)

### `Orchestrator task admission for VRAM requirements`  (edge, happy; EXTRACTED)
- [edge] When task min_vram_mb exceeds all available remote nodes, task does not enter in_progress state and falls back to local queue  (crates/vox-orchestrator/src/orchestrator/tests/populi_single_owner.rs)
- [happy] When task min_vram_mb is satisfied by available remote node, task enters in_progress state for remote hold  (crates/vox-orchestrator/src/orchestrator/tests/populi_single_owner.rs)

### `Orchestrator.check_scaling`  (happy, invariant; EXTRACTED)
- [happy] check_scaling scales up from 1 agent to more than 1 agent when 10 tasks are submitted with scaling_threshold=2  (crates/vox-orchestrator/tests/scaling_test.rs)
- [invariant] Scaled agents never exceed max_agents (4) during scaling operation  (crates/vox-orchestrator/tests/scaling_test.rs)

### `Orchestrator.rebalance()`  (happy, invariant; EXTRACTED)
- [happy] Orchestrator.rebalance() distributes tasks from expensive agent to cheaper agent with lower cost per token  (crates/vox-orchestrator/tests/economy_test.rs)
- [invariant] rebalance() maintains total_queued count without losing or duplicating tasks  (crates/vox-orchestrator/tests/stress_test.rs)

### `Orchestrator.status`  (happy; EXTRACTED)
- [happy] status() returns non-zero total_weighted_load when tasks are submitted with non-normal priority  (crates/vox-orchestrator/tests/scaling_test.rs)
- [happy] status() returns non-negative predicted_load value  (crates/vox-orchestrator/tests/scaling_test.rs)

### `Orchestrator.tick`  (happy, invariant; EXTRACTED)
- [invariant] After tick when agent exceeds urgent_rebalance_threshold, task count is preserved (no data loss)  (crates/vox-orchestrator/tests/scaling_test.rs)
- [happy] When agent has more urgent tasks than rebalance threshold, tick() does not increase agent's load  (crates/vox-orchestrator/tests/scaling_test.rs)

### `Orchestrator::replay_queued_routes_after_populi_schedulable_drop`  (edge, happy; EXTRACTED)
- [happy] replay_queued_routes_after_populi_schedulable_drop() moves at least 1 task from initial agent to affinity group's default agent  (crates/vox-orchestrator/src/orchestrator/tests/populi_single_owner.rs)
- [edge] replay_queued_routes_after_populi_schedulable_drop() returns 0 when all tasks in queue have populi_remote_delegate set  (crates/vox-orchestrator/src/orchestrator/tests/populi_single_owner.rs)

### `OrientPhase::request_missing_evidence`  (happy; EXTRACTED)
- [happy] Returns None when called with evidence score 0.3 via is_none() assertion  (crates/vox-orchestrator/src/planning/orient.rs)
- [happy] Returns Some when called with evidence score 0.5 via is_some() assertion  (crates/vox-orchestrator/src/planning/orient.rs)

### `PiiFilter::redact()`  (happy; EXTRACTED)
- [happy] Email addresses in input are replaced with [REDACTED_EMAIL] token in output  (crates/vox-orchestrator/src/pii_filter.rs)
- [happy] IP addresses in input are replaced with [REDACTED_IP] token in output  (crates/vox-orchestrator/src/pii_filter.rs)

### `PolicyEngine::check_before_queue`  (error; EXTRACTED)
- [error] check_before_queue denies queueing when another agent holds exclusive lock  (crates/vox-orchestrator/src/services/policy.rs)
- [error] check_before_queue denies queueing when agent reliability below minimum despite scope relaxation enabled  (crates/vox-orchestrator/src/services/policy.rs)

### `ReconstructionBenchmarkTier::next`  (edge, happy; EXTRACTED)
- [happy] ReconstructionBenchmarkTier::IssueRepair.next() returns Some(SubsystemRegen)  (crates/vox-orchestrator/src/reconstruction.rs)
- [edge] ReconstructionBenchmarkTier::RepoRegen.next() returns None  (crates/vox-orchestrator/src/reconstruction.rs)

### `RoutingService::remote_hint_matches_task`  (error, happy; EXTRACTED)
- [error] remote_hint_matches_task returns false when remote node is quarantined, in maintenance mode, or has stale heartbeat  (crates/vox-orchestrator/src/services/routing.rs)
- [happy] GPU task routing requires at least one allocatable or healthy GPU; returns false if gpu_readiness_ok is false  (crates/vox-orchestrator/src/services/routing.rs)

### `RubricScores::weighted_score`  (edge, happy; EXTRACTED)
- [happy] Returns 1.0 when all rubric scores are 10 via abs difference assertion < 0.01  (crates/vox-orchestrator/src/planning/plan_adequacy.rs)
- [edge] Returns 0.0 when all rubric scores are 0 via abs difference assertion < 0.01  (crates/vox-orchestrator/src/planning/plan_adequacy.rs)

### `ScalingAction`  (happy; EXTRACTED)
- [happy] ScalingService returns ScaleUp action when local pressure exceeds the configured scaling threshold  (crates/vox-orchestrator/src/services/scaling.rs)
- [happy] When remote GPU capacity is available, scaling pressure is reduced and NoOp action is returned instead of ScaleUp  (crates/vox-orchestrator/src/services/scaling.rs)

### `SelectionAxes::from_env()`  (happy; EXTRACTED)
- [happy] SelectionAxes::from_env() returns SelectionAxes::BALANCED when VOX_MODEL_AXES env var is unset  (crates/vox-orchestrator/src/models/select.rs)
- [happy] SelectionAxes::from_env() correctly parses custom axes from VOX_MODEL_AXES environment variable  (crates/vox-orchestrator/src/models/select.rs)

### `SelectionIntent::ide_autocomplete()`  (happy; EXTRACTED)
- [happy] SelectionIntent::ide_autocomplete() sets prefer_local to true  (crates/vox-orchestrator/src/models/select.rs)
- [happy] SelectionIntent::ide_autocomplete() sets axes to SelectionAxes::FAST  (crates/vox-orchestrator/src/models/select.rs)

### `SelectionIntent::nli_classifier()`  (happy; EXTRACTED)
- [happy] SelectionIntent::nli_classifier() sets axes to SelectionAxes::COST_FIRST  (crates/vox-orchestrator/src/models/select.rs)
- [happy] SelectionIntent::nli_classifier() has a max_cost_usd_per_call value set  (crates/vox-orchestrator/src/models/select.rs)

### `SelectionIntent::research()`  (happy; EXTRACTED)
- [happy] SelectionIntent::research() sets axes to SelectionAxes::QUALITY_FIRST  (crates/vox-orchestrator/src/models/select.rs)
- [happy] SelectionIntent::research() sets task to TaskCategory::Research  (crates/vox-orchestrator/src/models/select.rs)

### `SnapshotStore`  (invariant; EXTRACTED)
- [invariant] SnapshotStore maintains a maximum capacity constraint, evicting old snapshots to keep count at or below the configured limit  (crates/vox-orchestrator/src/snapshot_tests.rs)
- [invariant] SnapshotStore deduplicates identical file content across multiple snapshots, storing each unique blob only once  (crates/vox-orchestrator/src/snapshot_tests.rs)

### `SpotCheckSampler.should_check()`  (happy; EXTRACTED)
- [happy] should_check() returns false for all task_ids when probability is 0.0  (crates/vox-orchestrator/tests/spot_check.rs)
- [happy] should_check() returns true for all task_ids when probability is 1.0  (crates/vox-orchestrator/tests/spot_check.rs)

### `Task.description`  (happy; EXTRACTED)
- [happy] Generated shard tasks contain [PHASE:SHARD_GEN] marker in description  (crates/vox-orchestrator/tests/repo_scale_simulation_test.rs)
- [happy] Validation shard tasks contain [PHASE:SHARD_VALIDATE] marker in description  (crates/vox-orchestrator/tests/repo_scale_simulation_test.rs)

### `TelemetryEvent`  (happy; EXTRACTED)
- [happy] orch.task.cancelled event includes task_id in session_id field  (crates/vox-orchestrator/tests/telemetry_task_cancelled.rs)
- [happy] orch.task.cancelled event metadata includes task_id and path fields  (crates/vox-orchestrator/tests/telemetry_task_cancelled.rs)

### `TestDecision`  (happy; EXTRACTED)
- [happy] TestDecision::Required variant is produced when evaluating a task with vox file writes  (crates/vox-orchestrator/src/planning/test_decision.rs)
- [happy] TestDecision::Skip variant is produced when evaluating a task with General category  (crates/vox-orchestrator/src/planning/test_decision.rs)

### `TestDecisionPolicy`  (happy; EXTRACTED)
- [happy] TestDecisionPolicy.evaluate() returns TestDecision::Required when task contains files with .vox extension  (crates/vox-orchestrator/src/planning/test_decision.rs)
- [happy] TestDecisionPolicy.evaluate() returns TestDecision::Skip when task has General category in OrientReport  (crates/vox-orchestrator/src/planning/test_decision.rs)

### `base_routing_weights()`  (happy; EXTRACTED)
- [happy] base_routing_weights() returns the installed base routing priority when install_base_routing_priority() is called with Some value  (crates/vox-orchestrator/src/models/scoring.rs)
- [happy] base_routing_weights() falls back to AutoRoutingPriority::from_env() when install_base_routing_priority(None) clears the override  (crates/vox-orchestrator/src/models/scoring.rs)

### `build_exec_source_fields()`  (happy; EXTRACTED)
- [happy] build_exec_source_fields() returns base64 string that decodes back to the original source bytes.  (crates/vox-orchestrator/src/a2a/exec_source.rs)
- [happy] build_exec_source_fields() returns blake3 hex hash that matches the hash of the decoded base64 bytes.  (crates/vox-orchestrator/src/a2a/exec_source.rs)

### `build_repo_scoped_orchestrator`  (invariant; EXTRACTED)
- [invariant] Two calls to build_repo_scoped_orchestrator with same config and path produce identical repository_id  (crates/vox-orchestrator/tests/bootstrap_build_parity.rs)
- [invariant] Repeated builds produce identical memory.log_dir and memory.memory_md_path paths  (crates/vox-orchestrator/tests/bootstrap_build_parity.rs)

### `build_repo_shard_descriptors()`  (invariant; EXTRACTED)
- [invariant] build_repo_shard_descriptors() creates 5 descriptors for 2 shards (2 gen + 2 validate + 1 reducer)  (crates/vox-orchestrator/src/orchestrator/task_dispatch/submit/batch.rs)
- [invariant] Last descriptor description contains '[PHASE:REDUCE]' tag identifying it as the reducer task  (crates/vox-orchestrator/src/orchestrator/task_dispatch/submit/batch.rs)

### `check_campaign_prereg`  (happy; EXTRACTED)
- [happy] check_campaign_prereg(None, None) returns GateResult::Refused with preregistration-mentioning reason  (crates/vox-orchestrator/tests/research_gate.rs)
- [happy] check_campaign_prereg with valid prereg but no signature returns GateResult::Refused with signature-mentioning reason  (crates/vox-orchestrator/tests/research_gate.rs)

### `choose_strategy()`  (happy; EXTRACTED)
- [happy] Returns SharedBranch isolation strategy when predicted_overlap is 0 and not long_running  (crates/vox-orchestrator/src/isolation.rs)
- [happy] Returns SplitChanges isolation strategy when predicted_overlap exceeds zero  (crates/vox-orchestrator/src/isolation.rs)

### `compute_trusty_uri`  (happy, invariant; EXTRACTED)
- [happy] compute_trusty_uri produces URIs starting with 'RA' prefix  (crates/vox-orchestrator/src/preregistration/trusty_uri.rs)
- [invariant] compute_trusty_uri produces different output when preregistration hypothesis field is modified  (crates/vox-orchestrator/src/preregistration/trusty_uri.rs)

### `confidence_state_for_model()`  (happy; EXTRACTED)
- [happy] confidence_state_for_model() returns ModelConfidence::Provisional when model.pricing_source is PricingSource::Unknown  (crates/vox-orchestrator/src/models/select.rs)
- [happy] confidence_state_for_model() returns ModelConfidence::Confirmed when model.pricing_source is PricingSource::Telemetry  (crates/vox-orchestrator/src/models/select.rs)

### `diff_and_emit_discovery()`  (happy, invariant; EXTRACTED)
- [invariant] diff_and_emit_discovery() excludes retired model IDs (anthropic/claude-3.5-sonnet) from the newly discovered set  (crates/vox-orchestrator/src/models/autonomic.rs)
- [happy] diff_and_emit_discovery() includes non-retired newly discovered model IDs in the result set  (crates/vox-orchestrator/src/models/autonomic.rs)

### `encrypt_jwe_compact()`  (happy; EXTRACTED)
- [happy] encrypt_jwe_compact() produces a JWE compact string with 5 dot-separated parts.  (crates/vox-orchestrator/src/a2a/jwe.rs)
- [happy] encrypt_jwe_compact() produces JWE with empty encrypted key (dir algorithm) indicated by '..' substring.  (crates/vox-orchestrator/src/a2a/jwe.rs)

### `evaluate_goal()`  (happy; EXTRACTED)
- [happy] evaluate_goal() with Direct PlanningMode returns ImmediateAct strategy  (crates/vox-orchestrator/src/planning/intake_router.rs)
- [happy] evaluate_goal() recognizes 'workflow' keyword in goal string and returns WorkflowHandoff strategy  (crates/vox-orchestrator/src/planning/intake_router.rs)

### `evaluate_socrates_gate`  (happy; EXTRACTED)
- [happy] When factual mode is enabled and evidence count is below required citations threshold, evaluate_socrates_gate returns Abstain decision  (crates/vox-orchestrator/src/socrates.rs)
- [happy] When retrieval diagnosis indicates contradictory evidence shape, evaluate_socrates_gate reduces confidence score compared to non-contradictory baseline  (crates/vox-orchestrator/src/socrates.rs)

### `gpu_compute_ms_from_attestation`  (happy, invariant; EXTRACTED)
- [invariant] Sum of gpu_compute_ms_from_attestation across multiple attestations equals sum of (gpu_seconds * 1000) for each attestation  (crates/vox-orchestrator/tests/kudos_reconciliation.rs)
- [happy] gpu_compute_ms_from_attestation converts gpu_seconds field to milliseconds: 2.5s->2500ms, 0.001s->1ms, 0.0s->0ms  (crates/vox-orchestrator/tests/kudos_reconciliation.rs)

### `harness_completion_issues()`  (error, happy; EXTRACTED)
- [happy] harness_completion_issues() returns empty issues list when harness has no required artifacts and no completion gates  (crates/vox-orchestrator/src/orchestrator/task_dispatch/complete/harness.rs)
- [error] harness_completion_issues() detects missing required harness artifacts and reports them in issues list  (crates/vox-orchestrator/src/orchestrator/task_dispatch/complete/harness.rs)

### `latency_score`  (happy; EXTRACTED)
- [happy] latency_score returns 1.0 when p50_ms <= 500, intermediate score when p50_ms in middle range, and 0.0 when p50_ms >= 8000  (crates/vox-orchestrator/src/models/scoring.rs)
- [happy] latency_score uses provider-type-specific fallback values: Ollama=0.95, OpenRouter=0.7, Groq=0.95, Anthropic=0.75 when p50 unavailable  (crates/vox-orchestrator/src/models/scoring.rs)

### `load_from_config()`  (happy; EXTRACTED)
- [happy] load_from_config() parses affinity_groups from Vox.toml with both array and single-string pattern syntax  (crates/vox-orchestrator/src/groups.rs)
- [happy] load_from_config() returns None when affinity_groups table is missing or contains an empty array  (crates/vox-orchestrator/src/groups.rs)

### `locks::acquire_distributed_lock_with_breaker`  (error, happy; EXTRACTED)
- [happy] locks::acquire_distributed_lock_with_breaker returns fence value 1 when lock is first acquired  (crates/vox-orchestrator/tests/populi_coordination.rs)
- [error] locks::acquire_distributed_lock_with_breaker returns error with holder node name when lock is already held  (crates/vox-orchestrator/tests/populi_coordination.rs)

### `model_supports_privacy_local_inference`  (happy; EXTRACTED)
- [happy] model_supports_privacy_local_inference returns true for Ollama provider type  (crates/vox-orchestrator/src/privacy_router.rs)
- [happy] model_supports_privacy_local_inference returns false for OpenRouter provider type  (crates/vox-orchestrator/src/privacy_router.rs)

### `occ_guarded_write()`  (happy; EXTRACTED)
- [happy] When remote is newer and ConflictResolution is TakeRight, skips write and returns WriteOutcome::Skipped without executing callback  (crates/vox-orchestrator/src/occ.rs)
- [happy] When remote row does not exist (no conflict), executes write callback and returns WriteOutcome::Written  (crates/vox-orchestrator/src/occ.rs)

### `ok_page function`  (happy; EXTRACTED)
- [happy] ok_page creates envelope with v=1, data containing array elements, and cursor field set to provided value  (crates/vox-orchestrator/tests/api_v2_health_test.rs)
- [happy] ok_page sets cursor to null when None is passed  (crates/vox-orchestrator/tests/api_v2_health_test.rs)

### `read_only_fast_forward_eligible`  (happy; EXTRACTED)
- [happy] read_only_fast_forward_eligible() returns true for read-only tools like vox_validate_file  (crates/vox-orchestrator/src/agentos/replay_fast_forward.rs)
- [happy] read_only_fast_forward_eligible() returns false for non-read-only tools like vox_run_shell  (crates/vox-orchestrator/src/agentos/replay_fast_forward.rs)

### `record_overlap_conflicts`  (edge, happy; EXTRACTED)
- [happy] returns one conflict id per overlapping file path in the input  (crates/vox-orchestrator/src/merge_conflicts.rs)
- [edge] returns empty vec when given empty overlap list  (crates/vox-orchestrator/src/merge_conflicts.rs)

### `render_council_report()`  (happy; EXTRACTED)
- [happy] render_council_report() successfully renders with an empty ModelRegistry and includes the report header  (crates/vox-orchestrator/src/models/autonomic.rs)
- [happy] render_council_report() output includes 'Catalog snapshot' section  (crates/vox-orchestrator/src/models/autonomic.rs)

### `resolve_eligibility()`  (edge, happy; EXTRACTED)
- [happy] resolve_eligibility() promotes OpenRouter models from Shadowed to Confirmed when scoreboard shows 1000+ successful calls and acceptable latency (10ms vs 10s catalog median)  (crates/vox-orchestrator/src/models/discovery_pipeline.rs)
- [edge] resolve_eligibility() keeps OpenRouter models in Shadowed state when scoreboard shows zero successful calls, even with a scoreboard row present  (crates/vox-orchestrator/src/models/discovery_pipeline.rs)

### `resolve_model_with_registry_fallbacks`  (happy; EXTRACTED)
- [happy] resolve_model_with_registry_fallbacks returns a registered model (cloud-a or cloud-b) when registry arm stats are injected  (crates/vox-orchestrator/tests/p12_policy_program_wiring.rs)
- [happy] resolve_model_with_registry_fallbacks filters to local-a when privacy_requires_local is true and ollama is available  (crates/vox-orchestrator/tests/p12_policy_program_wiring.rs)

### `ship_decision function`  (edge, happy; EXTRACTED)
- [happy] ship_decision returns Some(inline_b64) for bundles under INLINE_BUNDLE_BYTE_LIMIT  (crates/vox-orchestrator/tests/bundle_fetch.rs)
- [edge] ship_decision returns None for inline when bundle exceeds INLINE_BUNDLE_BYTE_LIMIT  (crates/vox-orchestrator/tests/bundle_fetch.rs)

### `should_promote() on ModelConfidence::Provisional`  (edge, happy; EXTRACTED)
- [edge] should_promote() stays None when classifier confidence is below 0.50  (crates/vox-orchestrator/src/models/autonomic.rs)
- [happy] should_promote() promotes to Shadowed when classifier confidence is at or above 0.80  (crates/vox-orchestrator/src/models/autonomic.rs)

### `should_sparse_checkpoint`  (happy; EXTRACTED)
- [happy] should_sparse_checkpoint() returns false for read-only tools like vox_git_status  (crates/vox-orchestrator/src/agentos/checkpoint_engine.rs)
- [happy] should_sparse_checkpoint() returns true for mutation tools like vox_write_file  (crates/vox-orchestrator/src/agentos/checkpoint_engine.rs)

### `takeover_handoff_json`  (happy, invariant; EXTRACTED)
- [happy] returns JSON with schema=vox_takeover_handoff_v1 and required keys: repository, workspace, snapshots, oplog  (crates/vox-orchestrator/src/json_vcs_facade.rs)
- [invariant] embeds snapshot agent_id in snapshots array matching top-level agent_id field  (crates/vox-orchestrator/src/json_vcs_facade.rs)

### `validate_context_envelope_ingest()`  (error; EXTRACTED)
- [error] Rejects context envelope ingest when repository_id does not match expectations, returning error containing 'repository_id'  (crates/vox-orchestrator/src/context_lifecycle.rs)
- [error] Rejects context envelope ingest when session_id does not match expectations, returning error containing 'session_id'  (crates/vox-orchestrator/src/context_lifecycle.rs)

### `/api/v2/health endpoint`  (happy; EXTRACTED)
- [happy] GET /api/v2/health returns StatusCode::OK with envelope containing v=1 and data.status='ok'  (crates/vox-orchestrator/tests/api_v2_health_test.rs)

### `AXES_OVERRIDE`  (happy; EXTRACTED)
- [happy] AXES_OVERRIDE is cleared (None) when AxesOverrideGuard drops  (crates/vox-orchestrator/src/models/scoring.rs)

### `AffinityGroupRegistry::defaults`  (happy; EXTRACTED)
- [happy] Default registry resolves crates/vox-package to pm-group and crates/vox-compiler lexer paths to lexer-parser-group  (crates/vox-orchestrator/tests/affinity_group_tests.rs)

### `AffinityGroupRegistry::detect_from_repository_layout()`  (happy; EXTRACTED)
- [happy] AffinityGroupRegistry::detect_from_repository_layout() automatically creates groups for workspace member crates with correct path resolution  (crates/vox-orchestrator/src/groups.rs)

### `AffinityGroupRegistry::find_by_name()`  (happy; EXTRACTED)
- [happy] AffinityGroupRegistry::find_by_name() returns Some for existing group names and None for nonexistent names  (crates/vox-orchestrator/src/groups.rs)

### `AffinityGroupRegistry::resolve()`  (happy; EXTRACTED)
- [happy] AffinityGroupRegistry::resolve() returns None for paths that do not match any affinity group pattern  (crates/vox-orchestrator/src/groups.rs)

### `AgentEvent serialization`  (happy; EXTRACTED)
- [happy] AgentEvent serializes to JSON and deserializes back with EventId preserved  (crates/vox-orchestrator/src/events.rs)

### `AgentEventKind.FileDiagChanged`  (happy; EXTRACTED)
- [happy] FileDiagChanged event round-trips through EventBus with correct path, error_count, and warn_count  (crates/vox-orchestrator/tests/event_bus_new_variants_test.rs)

### `AgentEventKind.MeshTopologyChanged`  (happy; EXTRACTED)
- [happy] MeshTopologyChanged event round-trips through EventBus with correct added/removed nodes and changed_edges  (crates/vox-orchestrator/tests/event_bus_new_variants_test.rs)

### `AgentHandoffAccepted event`  (happy; EXTRACTED)
- [happy] AgentHandoffAccepted event is emitted with has_context_envelope=true and correct session_id when context envelope is provided  (crates/vox-orchestrator/tests/handoff_test.rs)

### `AgentHandoffAccepted event metadata`  (happy; EXTRACTED)
- [happy] AgentHandoffAccepted event emits has_harness_spec=true, session_id, and thread_id when harness spec metadata is provided  (crates/vox-orchestrator/tests/handoff_test.rs)

### `AgentHarnessSpec::minimal_contract_first`  (happy; EXTRACTED)
- [happy] Data created by minimal_contract_first validates against the agent-harness.schema.json schema  (crates/vox-orchestrator/tests/agent_harness_contract.rs)

### `AgentMessage`  (happy; EXTRACTED)
- [happy] AgentMessage FileChanged variant survives JSON serialization roundtrip with all fields (path, agent, summary) preserved  (crates/vox-orchestrator/src/types/messages.rs)

### `AgentQueue::attach_socrates_context`  (happy; EXTRACTED)
- [happy] attach_socrates_context succeeds and updates task context fields  (crates/vox-orchestrator/src/queue/mod.rs)

### `AgentQueue::dequeue`  (happy; EXTRACTED)
- [happy] dequeue returns previously-blocked task after dependency completes  (crates/vox-orchestrator/src/queue/mod.rs)

### `AgentQueue::enqueue`  (error; EXTRACTED)
- [error] enqueue marks task Failed when handoff_count exceeds MAX_A2A_BOUNCE  (crates/vox-orchestrator/src/queue/priority.rs)

### `AgentQueue::enqueue_dedup`  (happy; EXTRACTED)
- [happy] enqueue_dedup rejects duplicate task descriptions  (crates/vox-orchestrator/src/queue/mod.rs)

### `AgentQueue::len`  (invariant; EXTRACTED)
- [invariant] queue length remains 1 after duplicate-description task is rejected  (crates/vox-orchestrator/src/queue/mod.rs)

### `AgentQueue::mark_complete`  (happy; EXTRACTED)
- [happy] mark_complete unblocks dependent tasks  (crates/vox-orchestrator/src/queue/mod.rs)

### `AgentQueue::reorder`  (happy; EXTRACTED)
- [happy] reorder changes task dequeue order by priority  (crates/vox-orchestrator/src/queue/mod.rs)

### `AgentQueue::to_markdown`  (happy; EXTRACTED)
- [happy] to_markdown emits agent ID, name, and task IDs  (crates/vox-orchestrator/src/queue/mod.rs)

### `AgentTask`  (happy; EXTRACTED)
- [happy] AgentTask serialization and deserialization preserves id, priority, and description fields  (crates/vox-orchestrator/src/types/tasks.rs)

### `AgentTask::elapsed_since_last_expensive_op_ms()`  (happy; EXTRACTED)
- [happy] elapsed_since_last_expensive_op_ms() returns None before record_expensive_op() is called and returns Some(elapsed_ms) after it is called, with elapsed time less than 1000ms in test context  (crates/vox-orchestrator/src/types/tasks.rs)

### `AgentTask::is_ready()`  (happy; EXTRACTED)
- [happy] is_ready() returns false when dependencies are not fully satisfied and true only when all dependencies are in the provided list  (crates/vox-orchestrator/src/types/tasks.rs)

### `AgentTask::socrates`  (happy; EXTRACTED)
- [happy] socrates context fields are persisted after attachment  (crates/vox-orchestrator/src/queue/mod.rs)

### `AlarmLevel::from(AlarmTier)`  (happy; EXTRACTED)
- [happy] AlarmLevel can be converted from AlarmTier with AlarmLevel::None from AlarmTier::None, AlarmLevel::Caution from AlarmTier::Caution, and AlarmLevel::Warning from AlarmTier::Warning  (crates/vox-orchestrator/src/tier_cascade.rs)

### `AuditLog`  (invariant; EXTRACTED)
- [invariant] AuditLog with capacity 3 drops oldest entries when more than 3 entries are recorded, maintaining size of 3  (crates/vox-orchestrator/src/security.rs)

### `BudgetDecision::is_exhausted`  (invariant; EXTRACTED)
- [invariant] is_exhausted() returns false when status is not Halt and true only when status is Halt  (crates/vox-orchestrator/src/budget_gate.rs)

### `BudgetDecisionEvent`  (happy; EXTRACTED)
- [happy] BudgetDecisionEvent metric_type field is set to 'orch.budget.decision'  (crates/vox-orchestrator/src/budget_gate.rs)

### `BudgetGate::check_attention_snapshot()`  (happy; EXTRACTED)
- [happy] When attention is enabled and fully debited (exhausted), returns GateResult::AttentionExhausted variant  (crates/vox-orchestrator/src/gate.rs)

### `BudgetGate::evaluate`  (invariant; EXTRACTED)
- [invariant] evaluate(0.50, 0.96) returns decision with Halt status when cost fraction (0.96) is in halt zone despite token fraction (0.50) being ok  (crates/vox-orchestrator/src/budget_gate.rs)

### `BudgetManager::record_task_completion`  (happy; EXTRACTED)
- [happy] record_task_completion resets the doom_loop_cost_check counter so it no longer fires after reset  (crates/vox-orchestrator/src/budget/mod.rs)

### `BuildStageKind`  (happy; EXTRACTED)
- [happy] BuildStageKind.Hir round-trips through EventBus without modification  (crates/vox-orchestrator/tests/event_bus_new_variants_test.rs)

### `Catalog.best_for()`  (happy; EXTRACTED)
- [happy] Catalog.best_for() with Performance preference selects cheapest paid model for task category  (crates/vox-orchestrator/tests/economy_test.rs)

### `ChangeId`  (happy; EXTRACTED)
- [happy] ChangeId(42) formats as 'CH-000042' with zero-padded 6-digit display  (crates/vox-orchestrator/src/workspace.rs)

### `ChangeStatus`  (happy; EXTRACTED)
- [happy] Change lifecycle transitions from InProgress to Merged via update_change_status(), and status persists when queried.  (crates/vox-orchestrator/tests/vcs_test.rs)

### `CircuitBreaker`  (invariant; EXTRACTED)
- [invariant] should_trip() returns None when CircuitBreakerState has all signal counters at zero  (crates/vox-orchestrator/src/circuit_breaker.rs)

### `ClaimKind`  (happy; EXTRACTED)
- [happy] classify_line_claim_kind returns Speculative for hedged claims, Procedural for action-oriented statements, and Factual for declarative statements  (crates/vox-orchestrator/src/grounding.rs)

### `CompactionEngine::compact`  (happy; EXTRACTED)
- [happy] compact() returns compacted=false and dropped_turns=0 when token count is well under the threshold  (crates/vox-orchestrator/src/compaction.rs)

### `CompactionEngine::compact with CompactionStrategy::Aggressive`  (happy; EXTRACTED)
- [happy] compact() with Aggressive strategy triggers compaction (compacted=true) even at low threshold  (crates/vox-orchestrator/src/compaction.rs)

### `CompactionEngine::compact with CompactionStrategy::Balanced`  (happy; EXTRACTED)
- [happy] compact() with Balanced strategy sets compacted=true, drops some turns (dropped_turns > 0), and reduces total tokens  (crates/vox-orchestrator/src/compaction.rs)

### `CompactionEngine::estimate_tokens`  (happy; EXTRACTED)
- [happy] estimate_tokens() produces a token estimate in range [1, 10] for typical input strings like 'Hello, world!'  (crates/vox-orchestrator/src/compaction.rs)

### `CompactionStrategy Display trait`  (invariant; EXTRACTED)
- [invariant] CompactionStrategy Aggressive, Balanced, and Conservative display as their lowercase names respectively  (crates/vox-orchestrator/src/compaction.rs)

### `ConfidenceFusion::evaluate`  (happy; EXTRACTED)
- [happy] evaluate() returns FusionDecision::Ship for high-quality inputs  (crates/vox-orchestrator/src/confidence_fusion.rs)

### `ConflictId::to_string()`  (happy; EXTRACTED)
- [happy] Formats ConflictId as 'C-' prefix followed by zero-padded 6-digit decimal number  (crates/vox-orchestrator/src/conflicts.rs)

### `ConflictManager::record_conflict()`  (happy; EXTRACTED)
- [happy] Creates a conflict that can be retrieved via get(), found via has_conflict(), and its active_count() increments  (crates/vox-orchestrator/src/conflicts.rs)

### `ConflictResolution`  (happy; EXTRACTED)
- [happy] ConflictResolution.TakeLeft resolves active conflicts, reducing active conflict count from 1 to 0.  (crates/vox-orchestrator/tests/vcs_test.rs)

### `ContentBlock`  (happy; EXTRACTED)
- [happy] markdown_to_content_blocks() returns both prose and code ContentBlock types  (crates/vox-orchestrator/src/planning/content_blocks.rs)

### `ContentBlock::TaskItem`  (happy; EXTRACTED)
- [happy] TaskItem blocks preserve id and complexity fields from parsed markdown  (crates/vox-orchestrator/src/planning/content_blocks.rs)

### `ContextEnvelope persistence by session_id`  (happy; EXTRACTED)
- [happy] ContextEnvelope is persisted in orchestrator context store when handoff includes valid context envelope JSON metadata  (crates/vox-orchestrator/tests/handoff_test.rs)

### `ContextEnvelope persistence gate`  (edge; EXTRACTED)
- [edge] ContextEnvelope is NOT persisted when context.subject.session_id is None even if metadata contains context envelope JSON  (crates/vox-orchestrator/tests/handoff_test.rs)

### `ContextEnvelope.obo_token`  (happy; EXTRACTED)
- [happy] ContextEnvelope.obo_token is None before signing  (crates/vox-orchestrator/tests/context_envelope_contract.rs)

### `ContextEnvelope.sign()`  (happy; EXTRACTED)
- [happy] ContextEnvelope.sign() returns a new envelope with obo_token populated  (crates/vox-orchestrator/tests/context_envelope_contract.rs)

### `ContextEnvelope::from_session_retrieval`  (happy; EXTRACTED)
- [happy] ContextEnvelope created from SessionRetrievalEnvelope validates against context-envelope.schema.json  (crates/vox-orchestrator/tests/context_envelope_contract.rs)

### `ContextualBandit`  (happy; EXTRACTED)
- [happy] select() returns the arm with highest expected_reward (strong 9/10 over weak 1/10)  (crates/vox-orchestrator/src/calibration.rs)

### `ContinuationEngine::generate_continuation()`  (happy; EXTRACTED)
- [happy] Generates continuation prompt with correct agent_id, strategy, and pending count in prompt text  (crates/vox-orchestrator/src/continuation.rs)

### `ContinuationEngine::is_exhausted()`  (happy; EXTRACTED)
- [happy] Returns true when agent reaches max continuations limit (configured as 2 in test)  (crates/vox-orchestrator/src/continuation.rs)

### `ContinuationEngine::reset_cooldown()`  (happy; EXTRACTED)
- [happy] After calling reset_cooldown(), exhausted agent becomes non-exhausted and can_continue() returns true  (crates/vox-orchestrator/src/continuation.rs)

### `DiscoverySource::as_str()`  (invariant; EXTRACTED)
- [invariant] DiscoverySource variants have stable wire-protocol string representations (openrouter, litellm, anthropic_direct, populi_mesh)  (crates/vox-orchestrator/src/models/autonomic.rs)

### `DispatchDecision`  (happy; EXTRACTED)
- [happy] When task complexity metric is 8 or higher, evaluate() returns DispatchDecision::Spawn  (crates/vox-orchestrator/src/orchestrator_policy.rs)

### `FileAffinity`  (happy; EXTRACTED)
- [happy] FileAffinity::read() constructor creates FileAffinity with AccessKind::Read and FileAffinity::write() creates FileAffinity with AccessKind::Write  (crates/vox-orchestrator/src/types/tasks.rs)

### `FileLockManager.holder()`  (happy; EXTRACTED)
- [happy] holder() returns the correct AgentId and LockKind for a locked path  (crates/vox-orchestrator/tests/two_daemon_lock_contention.rs)

### `FileLockManager.hydrate_from_db()`  (happy; EXTRACTED)
- [happy] hydrate_from_db() restores persisted lock state after rehydration  (crates/vox-orchestrator/tests/two_daemon_lock_contention.rs)

### `FusionInputs::from_task_context()`  (happy; EXTRACTED)
- [happy] Maps SocratesTaskContext fields to FusionInputs with matching evidence_quality and self_consistency, derives sep_estimate and citation_coverage from source_diversity/contradiction_hints  (crates/vox-orchestrator/src/confidence_fusion.rs)

### `HandoffPayload builder`  (happy; EXTRACTED)
- [happy] builder pattern correctly sets from_agent, to_agent, completed_tasks, pending_tasks, owned_files, and metadata fields  (crates/vox-orchestrator/src/handoff.rs)

### `HandoffPayload builder with_timeout and with_step`  (happy; EXTRACTED)
- [happy] builder methods correctly store timeout_ms and execution_history as sequences of ExecutionStep records  (crates/vox-orchestrator/src/handoff.rs)

### `HandoffPayload.from_json()`  (happy; EXTRACTED)
- [happy] HandoffPayload.from_json() deserializes and preserves from_agent, to_agent, pending_tasks, and metadata  (crates/vox-orchestrator/tests/handoff_test.rs)

### `HandoffPayload.to_json()`  (happy; EXTRACTED)
- [happy] HandoffPayload.to_json() produces JSON containing serialized from_agent, to_agent, description, and metadata  (crates/vox-orchestrator/tests/handoff_test.rs)

### `HandoffPayload::to_json and HandoffPayload::from_json`  (happy; EXTRACTED)
- [happy] JSON serialization and deserialization preserves from_agent, pending_tasks, and metadata across roundtrip  (crates/vox-orchestrator/src/handoff.rs)

### `HeartbeatMonitor::at_or_above()`  (happy; EXTRACTED)
- [happy] Filters and returns agents whose staleness level meets or exceeds the specified threshold  (crates/vox-orchestrator/src/heartbeat.rs)

### `HeartbeatMonitor::check_stale()`  (happy; EXTRACTED)
- [happy] Returns empty stale list immediately after registration, then returns stale agents after timeout threshold exceeded  (crates/vox-orchestrator/src/heartbeat.rs)

### `HeartbeatMonitor::unregister()`  (happy; EXTRACTED)
- [happy] Removes agent from monitor, decrementing agent_count and returning None for activity queries  (crates/vox-orchestrator/src/heartbeat.rs)

### `HeartbeatPolicy::level_for_misses()`  (invariant; EXTRACTED)
- [invariant] Maps missed heartbeat counts to discrete staleness levels: 0->Healthy, 1->Warn, 3->Alert, 5->Critical, 10->Dead  (crates/vox-orchestrator/src/heartbeat.rs)

### `InMemoryHopper lifecycle`  (happy; EXTRACTED)
- [happy] assigned() and inbox() queues transition correctly through assign() -> complete(): item moves from inbox to assigned, then to history  (crates/vox-orchestrator/src/hopper/store.rs)

### `InMemoryHopper::reprioritize()`  (error; EXTRACTED)
- [error] reprioritize() on a completed/terminal item returns HopperError::Terminal  (crates/vox-orchestrator/src/hopper/store.rs)

### `InProcessSkillRuntime.run`  (happy; EXTRACTED)
- [happy] run() with default RunOpts succeeds and returns outcome with exit_code 0  (crates/vox-orchestrator/tests/skill_runtime_inproc.rs)

### `InterruptionDecision::scaled_cost_ms`  (happy; EXTRACTED)
- [happy] InterruptionDecision::scaled_cost_ms() returns positive cost when interruption is required  (crates/vox-orchestrator/src/attention/interruption_policy.rs)

### `IsolationPlan::set_override`  (happy; EXTRACTED)
- [happy] set_override with None clears a previously set override, reverting strategy_for to default  (crates/vox-orchestrator/src/isolation.rs)

### `IsolationStrategy::SharedBranch`  (happy; EXTRACTED)
- [happy] SharedBranch isolation strategy rejects second agent writing to same file with OrchestratorError::LockConflict  (crates/vox-orchestrator/tests/isolation_shared_branch_test.rs)

### `IsolationStrategy::SplitChanges`  (happy; EXTRACTED)
- [happy] SplitChanges isolation strategy tolerates concurrent writes to same file without hard LockConflict failure  (crates/vox-orchestrator/tests/isolation_shared_branch_test.rs)

### `LoadBalancer::evaluate_scaling`  (happy; EXTRACTED)
- [happy] LoadBalancer.evaluate_scaling returns LoadBalancerAction::ScaleUp when queue depth reaches 15 tasks  (crates/vox-orchestrator/src/rebalance.rs)

### `LoadBalancer::pick_agent`  (happy; EXTRACTED)
- [happy] LoadBalancer with ShortestQueue strategy selects agent with shortest queue (AgentId 2 when agent 1 has 1 task)  (crates/vox-orchestrator/src/rebalance.rs)

### `LockLeaderElection`  (happy; EXTRACTED)
- [happy] leader election with two nodes results in one Leader and one Follower role  (crates/vox-orchestrator/tests/two_daemon_lock_contention.rs)

### `LongTermMemory::list_keys()`  (happy; EXTRACTED)
- [happy] LongTermMemory::list_keys() returns all keys previously set  (crates/vox-orchestrator/src/memory/tests.rs)

### `LongTermMemory::set() and get()`  (happy; EXTRACTED)
- [happy] LongTermMemory::set(key, value) persists and get(key) retrieves the stored value  (crates/vox-orchestrator/src/memory/tests.rs)

### `LongTermMemory::set() upsert semantics`  (happy; EXTRACTED)
- [happy] LongTermMemory::set() on an existing key updates (upserts) the value to the latest one set  (crates/vox-orchestrator/src/memory/tests.rs)

### `MemoryManager with enabled=false`  (edge; EXTRACTED)
- [edge] MemoryManager with enabled=false returns an empty context string from bootstrap_context()  (crates/vox-orchestrator/src/memory/tests.rs)

### `MemoryManager::account_id()`  (happy; EXTRACTED)
- [happy] account_id() returns the account ID string that the manager was created with  (crates/vox-orchestrator/src/memory/tests.rs)

### `MemoryManager::bootstrap_context()`  (happy; EXTRACTED)
- [happy] bootstrap_context() returns a context string containing persisted fact keys and values  (crates/vox-orchestrator/src/memory/tests.rs)

### `MemoryManager::flush_before_compaction()`  (happy; EXTRACTED)
- [happy] flush_before_compaction() persists all facts in the input map and returns the count flushed, making them retrievable via lookup_fact_by_key()  (crates/vox-orchestrator/src/memory/tests.rs)

### `MemoryManager::persist_fact() and lookup_fact_by_key()`  (happy; EXTRACTED)
- [happy] MemoryManager::persist_fact() stores a fact that can be retrieved via lookup_fact_by_key()  (crates/vox-orchestrator/src/memory/tests.rs)

### `MemoryManager::search()`  (happy; EXTRACTED)
- [happy] search() finds matching text in both persisted facts and logged entries  (crates/vox-orchestrator/src/memory/tests.rs)

### `ModelConfidence::Confirmed`  (happy; EXTRACTED)
- [happy] Confirmed state is eligible for routing  (crates/vox-orchestrator/src/models/autonomic.rs)

### `ModelConfidence::eligible_for_routing()`  (invariant; EXTRACTED)
- [invariant] Provisional, Shadowed, and Deprecated states are not eligible for routing  (crates/vox-orchestrator/src/models/autonomic.rs)

### `ModelConfig`  (happy; EXTRACTED)
- [happy] Each premium_alias target value exists in the default models list via ids() lookup  (crates/vox-orchestrator/src/models/tests.rs)

### `ModelRegistry::arm_stats_snapshot`  (happy; EXTRACTED)
- [happy] ModelRegistry::arm_stats_snapshot returns non-empty snapshot after arm stats are injected  (crates/vox-orchestrator/tests/p12_policy_program_wiring.rs)

### `ModelRegistry::best_for_with_filter()`  (happy; EXTRACTED)
- [happy] best_for_with_filter() respects the filter predicate and excludes Ollama provider when filter rejects it  (crates/vox-orchestrator/src/models/tests.rs)

### `Observer::summarize()`  (happy; EXTRACTED)
- [happy] Task summary correctly filters observations by task_id and reports accurate observation_count per task  (crates/vox-orchestrator/src/observer.rs)

### `OpLog.count()`  (happy; EXTRACTED)
- [happy] OpLog.count() remains unchanged when rebalance() does not move any tasks (moved == 0).  (crates/vox-orchestrator/tests/vcs_test.rs)

### `OpLog.redo()`  (happy; EXTRACTED)
- [happy] OpLog.redo() returns Some snapshot_after for undone operations.  (crates/vox-orchestrator/tests/vcs_test.rs)

### `OpLog.undo()`  (happy; EXTRACTED)
- [happy] OpLog.undo() returns Some snapshot_before for recorded operations.  (crates/vox-orchestrator/tests/vcs_test.rs)

### `OrchDaemonClient::complete_task`  (happy; EXTRACTED)
- [happy] OrchDaemonClient::complete_task completes a dequeued task  (crates/vox-orchestrator/tests/orchestrator_daemon_tcp.rs)

### `OrchDaemonClient::fail_task`  (happy; EXTRACTED)
- [happy] OrchDaemonClient::fail_task marks a dequeued task as failed  (crates/vox-orchestrator/tests/orchestrator_daemon_tcp.rs)

### `OrchDaemonClient::spawn_agent_ext`  (happy; EXTRACTED)
- [happy] OrchDaemonClient::spawn_agent_ext creates a dynamic agent with assigned agent_id  (crates/vox-orchestrator/tests/orchestrator_daemon_tcp.rs)

### `OrchDaemonClient::submit_task`  (happy; EXTRACTED)
- [happy] OrchDaemonClient::submit_task creates a task with an assigned task_id  (crates/vox-orchestrator/tests/orchestrator_daemon_tcp.rs)

### `OrchDaemonClient::subscribe`  (happy; EXTRACTED)
- [happy] OrchDaemonClient::subscribe pushes initial orchestrator status snapshot with agent_count field  (crates/vox-orchestrator/tests/orchestrator_daemon_tcp.rs)

### `OrchDaemonClient::subscribe_events`  (happy; EXTRACTED)
- [happy] OrchDaemonClient::subscribe_events streams live agent events with type 'token_streamed' after subscription  (crates/vox-orchestrator/tests/orchestrator_daemon_tcp.rs)

### `OrchDaemonClient::task_status`  (happy; EXTRACTED)
- [happy] OrchDaemonClient::task_status returns task status as 'InProgress' for dequeued tasks  (crates/vox-orchestrator/tests/orchestrator_daemon_tcp.rs)

### `OrchestrationMigrationFlags`  (happy; EXTRACTED)
- [happy] OrchestrationMigrationFlags deserializes from TOML with orchestration_v2_enabled=true and legacy_orchestration_fallback=false  (crates/vox-orchestrator/src/config/tests.rs)

### `Orchestrator.accept_handoff()`  (happy; EXTRACTED)
- [happy] Orchestrator.accept_handoff() accepts handoff with valid ContextEnvelope metadata JSON and returns from_agent  (crates/vox-orchestrator/tests/handoff_test.rs)

### `Orchestrator.cancel_task()`  (happy; EXTRACTED)
- [happy] cancel_task() emits exactly one orch.task.cancelled telemetry event  (crates/vox-orchestrator/tests/telemetry_task_cancelled.rs)

### `Orchestrator.submit_task`  (happy; EXTRACTED)
- [happy] Two tasks writing to the same file route to the same agent via file affinity  (crates/vox-orchestrator/tests/scaling_test.rs)

### `Orchestrator::evaluate_orchestrator_policy_for_agent`  (happy; EXTRACTED)
- [happy] Risk score increases when shell mutation tools are recorded versus read-only tools  (crates/vox-orchestrator/tests/agentos_mcp_policy_wiring.rs)

### `Orchestrator::map_agent_session`  (happy; EXTRACTED)
- [happy] map_agent_session succeeds and returns Ok  (crates/vox-orchestrator/tests/agent_session_test.rs)

### `Orchestrator::message_bus.audit_trail`  (happy; EXTRACTED)
- [happy] All sent messages appear in the audit trail for traceability  (crates/vox-orchestrator/tests/a2a_integration_test.rs)

### `Orchestrator::message_bus.send`  (happy; EXTRACTED)
- [happy] Sends A2A messages with correct id, sender, msg_type, and payload to recipient agent's inbox  (crates/vox-orchestrator/tests/a2a_integration_test.rs)

### `Orchestrator::spawn_agent`  (happy; EXTRACTED)
- [happy] Spawned agents appear in the message bus and can receive messages sent via send()  (crates/vox-orchestrator/tests/a2a_integration_test.rs)

### `Orchestrator::status`  (happy; EXTRACTED)
- [happy] status() includes tracked session_id for agents that were mapped  (crates/vox-orchestrator/tests/agent_session_test.rs)

### `Orchestrator::submit_repo_shard_dag`  (happy; EXTRACTED)
- [happy] submit_repo_shard_dag with 100 shards creates 201 tasks total (100 generation + 100 validation + 1 reducer)  (crates/vox-orchestrator/tests/repo_scale_simulation_test.rs)

### `OrchestratorConfig::default`  (happy; EXTRACTED)
- [happy] OrchestratorConfig::default sets enabled=true, max_agents=8, default_priority=Normal, queue_overflow_strategy=SpawnNewAgent, lock_timeout_ms=30000  (crates/vox-orchestrator/src/config/tests.rs)

### `OrchestratorConfig::isolation_strategy_default`  (happy; EXTRACTED)
- [happy] isolation_strategy_default defaults to SharedBranch  (crates/vox-orchestrator/src/config/orchestrator_fields.rs)

### `OrchestratorConfig::load_from_toml`  (happy; EXTRACTED)
- [happy] load_from_toml returns default configuration when orchestrator section is missing from TOML file  (crates/vox-orchestrator/src/config/tests.rs)

### `OrchestratorConfig::validate`  (error; EXTRACTED)
- [error] validate returns 4 errors when max_agents=0, lock_timeout_ms=50, bulletin_capacity=0, min_agents=1  (crates/vox-orchestrator/src/config/tests.rs)

### `PolicyCheckResult::LockConflict`  (error; EXTRACTED)
- [error] lock conflict check returns LockConflict error variant  (crates/vox-orchestrator/src/services/policy.rs)

### `PrioritySource::dominates()`  (invariant; EXTRACTED)
- [invariant] PrioritySource::Developer dominates both Orchestrator and LearningPolicy, and Orchestrator dominates LearningPolicy, forming a partial order  (crates/vox-orchestrator/src/hopper/store.rs)

### `PrivacyRouter::filter_models`  (happy; EXTRACTED)
- [happy] PrivacyRouter::filter_models filters models to only Ollama (local) when PrivacyLevel is Private  (crates/vox-orchestrator/tests/p12_policy_program_wiring.rs)

### `ReconstructionArtifactKind`  (invariant; EXTRACTED)
- [invariant] ReconstructionArtifactKind::VerificationEvidence.as_str() produces stable string representation 'verification_evidence'  (crates/vox-orchestrator/src/reconstruction.rs)

### `RemoteTaskEnvelope deserialization`  (edge; EXTRACTED)
- [edge] RemoteTaskEnvelope deserializes from legacy JSON lacking bundle_ref and bundle_inline_b64 fields with None values  (crates/vox-orchestrator/tests/bundle_fetch.rs)

### `RemoteTaskEnvelope serialization`  (happy; EXTRACTED)
- [happy] RemoteTaskEnvelope round-trips through JSON serialization preserving bundle_ref.fn_hash and bundle_inline_b64  (crates/vox-orchestrator/tests/bundle_fetch.rs)

### `ReplyWindowGate`  (happy; EXTRACTED)
- [happy] ReplyWindowGate.status() returns WindowStatus::Open { days_remaining: 9 } when 5 days have elapsed from 14-day window opening  (crates/vox-orchestrator/src/preregistration/reply_window.rs)

### `RoutingPolicy::exploration`  (happy; EXTRACTED)
- [happy] exploration.epsilon_ceiling is positive  (crates/vox-orchestrator/src/routing/policy.rs)

### `RoutingPolicy::fallback_graph`  (happy; EXTRACTED)
- [happy] fallback_graph is non-empty after load  (crates/vox-orchestrator/src/routing/policy.rs)

### `RoutingPolicy::load`  (happy; EXTRACTED)
- [happy] load returns policy with routing_objective.kind equal to 'quality_first'  (crates/vox-orchestrator/src/routing/policy.rs)

### `RoutingProfile::default()`  (happy; EXTRACTED)
- [happy] RoutingProfile::default() returns RoutingProfile::Free variant  (crates/vox-orchestrator/src/types/routing_profile.rs)

### `RoutingService::least_loaded_or_spawn`  (happy; EXTRACTED)
- [happy] least_loaded_or_spawn routes to agent with lower-priority queue load  (crates/vox-orchestrator/src/services/routing.rs)

### `ScopeGuard`  (happy; EXTRACTED)
- [happy] In Warn mode, check_write returns Warned result for out-of-scope paths  (crates/vox-orchestrator/tests/scope_test.rs)

### `ScopeGuard.agent_scope()`  (happy; EXTRACTED)
- [happy] agent_scope returns set of 2 files after assigning 2 files to an agent  (crates/vox-orchestrator/src/scope.rs)

### `ScopeGuard.revoke_file()`  (happy; EXTRACTED)
- [happy] revoke_file removes file from agent's scope, reducing count from 2 to 1  (crates/vox-orchestrator/src/scope.rs)

### `SecurityAction`  (happy; EXTRACTED)
- [happy] SecurityAction::DbRead returns Ok when allowed in policy, DbWrite returns Err when not allowed  (crates/vox-orchestrator/src/security.rs)

### `SecurityGuard.check()`  (happy; EXTRACTED)
- [happy] SecurityGuard check enforces policy for specific agent, allowing allowed actions and denying others  (crates/vox-orchestrator/src/security.rs)

### `SecurityGuard.rate_check()`  (edge; EXTRACTED)
- [edge] rate_check allows up to 5 requests and denies 6th request with same limit  (crates/vox-orchestrator/src/security.rs)

### `SecurityPolicy.check()`  (happy; EXTRACTED)
- [happy] SecurityPolicy allows specified actions and denies unspecified actions  (crates/vox-orchestrator/src/security.rs)

### `SecurityPolicy.deny()`  (happy; EXTRACTED)
- [happy] deny rule in permissive policy overrides the permissive default for specified action  (crates/vox-orchestrator/src/security.rs)

### `SelectionAxes preset constants`  (invariant; EXTRACTED)
- [invariant] All SelectionAxes presets (COST_FIRST, BALANCED, QUALITY_FIRST, FAST) sum their cost, responsiveness, and intelligence fields to 100  (crates/vox-orchestrator/src/models/select.rs)

### `SelectionAxes::BALANCED::to_cost_preference()`  (happy; EXTRACTED)
- [happy] SelectionAxes::BALANCED.to_cost_preference() returns CostPreference::Performance  (crates/vox-orchestrator/src/models/select.rs)

### `SelectionAxes::COST_FIRST::to_cost_preference()`  (happy; EXTRACTED)
- [happy] SelectionAxes::COST_FIRST.to_cost_preference() returns CostPreference::Economy  (crates/vox-orchestrator/src/models/select.rs)

### `SelectionAxes::FAST::to_cost_preference()`  (happy; EXTRACTED)
- [happy] SelectionAxes::FAST.to_cost_preference() returns CostPreference::Performance  (crates/vox-orchestrator/src/models/select.rs)

### `SelectionAxes::QUALITY_FIRST::to_cost_preference()`  (happy; EXTRACTED)
- [happy] SelectionAxes::QUALITY_FIRST.to_cost_preference() returns CostPreference::Performance  (crates/vox-orchestrator/src/models/select.rs)

### `SelectionPolicy`  (invariant; EXTRACTED)
- [invariant] SelectionPolicy with EmphasizeAxis, PinModel, PreferFree, and FallbackWhen steps survives JSON serialization and deserialization without data loss  (crates/vox-orchestrator/src/models/policy.rs)

### `Session initialization`  (invariant; EXTRACTED)
- [invariant] Newly created session has state Active and zero turns  (crates/vox-orchestrator/src/session/manager/tests.rs)

### `SessionManager::create() and SessionManager::get()`  (happy; EXTRACTED)
- [happy] create() creates a session with specified agent_id that can be retrieved via get()  (crates/vox-orchestrator/src/session/manager/tests.rs)

### `SessionRetrievalEnvelope::from_context_envelope`  (happy; EXTRACTED)
- [happy] SessionRetrievalEnvelope::from_context_envelope parses ContextEnvelope and correctly preserves retrieval_tier, memory_hit_count, and rrf_fused_hit_count fields  (crates/vox-orchestrator/src/socrates.rs)

### `SessionRetrievalEnvelope::to_task_context`  (happy; EXTRACTED)
- [happy] SessionRetrievalEnvelope::to_task_context maps rrf_fused_hit_count to evidence_count=1 and sets required_citations=0 when hybrid retrieval succeeds  (crates/vox-orchestrator/src/socrates.rs)

### `SkillRegistry.get`  (happy; EXTRACTED)
- [happy] After bridge installation, registry.get returns Some with correct id and version  (crates/vox-orchestrator/tests/skill_compiler_via_plugin.rs)

### `SnapshotId`  (happy; EXTRACTED)
- [happy] SnapshotId(42) displays as 'S-000042' with zero-padded 6-digit format  (crates/vox-orchestrator/src/snapshot_tests.rs)

### `SnapshotStore.count()`  (happy; EXTRACTED)
- [happy] SnapshotStore.count() returns 1 after taking one snapshot  (crates/vox-orchestrator/src/snapshot_tests.rs)

### `SnapshotStore.get()`  (happy; EXTRACTED)
- [happy] SnapshotStore.get() returns Some when querying a stored snapshot by id  (crates/vox-orchestrator/src/snapshot_tests.rs)

### `SnapshotStore::diff`  (happy; EXTRACTED)
- [happy] SnapshotStore::diff detects when a file is modified by comparing content hashes and reports it with FileDiffKind::Modified  (crates/vox-orchestrator/src/snapshot_tests.rs)

### `SnapshotStore::get_blob`  (happy; EXTRACTED)
- [happy] SnapshotStore::get_blob retrieves previously stored blobs by hash and returns None for non-existent hashes  (crates/vox-orchestrator/src/snapshot_tests.rs)

### `SnapshotStore::hash_file`  (happy; EXTRACTED)
- [happy] SnapshotStore::hash_file produces a non-empty hash and correctly reports file size in bytes for a real file on disk  (crates/vox-orchestrator/src/snapshot_tests.rs)

### `SocratesPlanJudge::parse_evaluation_scores`  (happy; EXTRACTED)
- [happy] Parses JSON evaluation text to tuple (8, 10, 10, 5, 0) via unwrap and assert_eq  (crates/vox-orchestrator/src/planning/orient.rs)

### `SummaryManager::get_summary()`  (happy; EXTRACTED)
- [happy] Returns summary text containing formatted question-answer pairs (Q[0] and A[0] formats) for recorded interactions  (crates/vox-orchestrator/src/summary.rs)

### `SummaryManager::handoff()`  (happy; EXTRACTED)
- [happy] Transfers interaction summary to target agent with previous summary header and interaction content preserved  (crates/vox-orchestrator/src/summary.rs)

### `Task assignment routing`  (invariant; EXTRACTED)
- [invariant] When task is reassigned after replay, its queue membership follows the new assignment (task leaves original agent queue and lands on new agent's queue)  (crates/vox-orchestrator/src/orchestrator/tests/populi_single_owner.rs)

### `Task populi_remote_delegate assignment`  (error; EXTRACTED)
- [error] When task fails VRAM admission check, populi_remote_delegate is not assigned  (crates/vox-orchestrator/src/orchestrator/tests/populi_single_owner.rs)

### `Task queue behavior for remote hold`  (invariant; EXTRACTED)
- [invariant] When task is held remotely (has_in_progress true), no local copy is queued (queue length is 0)  (crates/vox-orchestrator/src/orchestrator/tests/populi_single_owner.rs)

### `Task.depends_on`  (happy; EXTRACTED)
- [happy] Reducer task depends on all 100 validator tasks  (crates/vox-orchestrator/tests/repo_scale_simulation_test.rs)

### `TaskPriority`  (invariant; EXTRACTED)
- [invariant] TaskPriority ordering is correct: Urgent > Normal > Background  (crates/vox-orchestrator/src/types/tasks.rs)

### `TaskStatus::Failed`  (error; EXTRACTED)
- [error] Failed status message contains 'Infinite A2A handoff loop detected'  (crates/vox-orchestrator/src/queue/priority.rs)

### `TraceContext encode/parse`  (happy; EXTRACTED)
- [happy] encoded traceparent has 4 dash-separated parts with version 00, 32-char trace_id, 16-char parent_id, 2-char flags  (crates/vox-orchestrator/tests/traceparent_roundtrip.rs)

### `TraceContext.from_current_span()`  (happy; EXTRACTED)
- [happy] from_current_span() generates a non-zero trace_id from the active span  (crates/vox-orchestrator/tests/traceparent_roundtrip.rs)

### `TripReason`  (edge; EXTRACTED)
- [edge] When completion text entropy drastically exceeds baseline, circuit_trip returns Some(TripReason::SemanticDrift)  (crates/vox-orchestrator/src/orchestrator_policy.rs)

### `UsageRecord`  (happy; EXTRACTED)
- [happy] UsageRecord can be serialized to JSON containing the model name and call count fields  (crates/vox-orchestrator/src/usage.rs)

### `WorkflowDrainState.is_draining()`  (happy; EXTRACTED)
- [happy] is_draining() returns true for function hashes recorded via record_drain(), and false for unrecorded hashes.  (crates/vox-orchestrator/tests/workflow_drain.rs)

### `WorkflowDrainState.may_start_new_run()`  (happy; EXTRACTED)
- [happy] may_start_new_run() returns false for drained function hashes and true for non-drained hashes.  (crates/vox-orchestrator/tests/workflow_drain.rs)

### `WorkflowDrainState.snapshot()`  (happy; EXTRACTED)
- [happy] snapshot() returns a collection with length equal to the number of record_drain() calls made.  (crates/vox-orchestrator/tests/workflow_drain.rs)

### `WorkspaceManager and Workspace modification tracking`  (happy; EXTRACTED)
- [happy] WorkspaceManager can create workspaces, has_workspace() correctly identifies them, and workspace modifications (record_modification, record_creation, record_deletion) accumulate with modified_count() returning the total and has_modification() finding specific files  (crates/vox-orchestrator/src/workspace.rs)

### `WorkspaceManager.overlapping_paths()`  (happy; EXTRACTED)
- [happy] overlapping_paths() detects when two agents modify the same file and returns the overlapping path.  (crates/vox-orchestrator/tests/vcs_test.rs)

### `WorkspaceManager::destroy_workspace`  (happy; EXTRACTED)
- [happy] Removes workspace from manager such that has_workspace returns false after destruction  (crates/vox-orchestrator/src/workspace.rs)

### `WorkspaceManager::get_change`  (happy; EXTRACTED)
- [happy] Retrieved Change object tracks snapshots collection and status transitions from InProgress to Merged  (crates/vox-orchestrator/src/workspace.rs)

### `WorkspaceManager::list_changes`  (happy; EXTRACTED)
- [happy] Filters changes by AgentId; agent 1 has 2, agent 2 has 1, and without filter returns all 3  (crates/vox-orchestrator/src/workspace.rs)

### `WorkspaceManager::overlapping_paths`  (happy; EXTRACTED)
- [happy] Returns detected overlapping file paths between two agents' modifications (both modified 'shared.rs')  (crates/vox-orchestrator/src/workspace.rs)

### `WorkspaceManager::setup_isolation() with SeparateBranches`  (happy; EXTRACTED)
- [happy] When setup_isolation() is called with IsolationStrategy::SeparateBranches for AgentId(4), it returns Some branch named 'agent/4' and the workspace bound_branch() returns Some('agent/4')  (crates/vox-orchestrator/src/workspace.rs)

### `WorkspaceManager::setup_isolation() with SharedBranch`  (happy; EXTRACTED)
- [happy] When setup_isolation() is called with IsolationStrategy::SharedBranch, it returns None and the workspace bound_branch() returns None  (crates/vox-orchestrator/src/workspace.rs)

### `WorkspaceManager::setup_isolation() with SplitChanges`  (happy; EXTRACTED)
- [happy] When setup_isolation() is called with IsolationStrategy::SplitChanges, list_changes() increments by 1 (SplitChanges starts a new per-agent change)  (crates/vox-orchestrator/src/workspace.rs)

### `WorkspaceManager::update_change_status`  (happy; EXTRACTED)
- [happy] When change status is updated to Merged, the ChangeStatus field reflects the transition correctly  (crates/vox-orchestrator/src/workspace.rs)

### `a2a::acknowledge_db_message`  (happy; EXTRACTED)
- [happy] a2a::acknowledge_db_message removes acknowledged message from inbox  (crates/vox-orchestrator/tests/populi_coordination.rs)

### `a2a::poll_inbox_from_db`  (happy; EXTRACTED)
- [happy] a2a::poll_inbox_from_db retrieves sent messages before acknowledgment  (crates/vox-orchestrator/tests/populi_coordination.rs)

### `a2a::send_to_db_with_breaker`  (happy; EXTRACTED)
- [happy] a2a::send_to_db_with_breaker persists a2a message and returns a UUID  (crates/vox-orchestrator/tests/populi_coordination.rs)

### `agentos_suggested_tools_from_intent`  (happy; EXTRACTED)
- [happy] agentos_suggested_tools_from_intent maps 'run cargo tests' intent to vox_run_tests tool  (crates/vox-orchestrator/src/grounding.rs)

### `analyze_plan_refinement_report_with_prior`  (edge; EXTRACTED)
- [edge] Detects possible_rewrite_compression in reason_codes when plan shrinks from 4 to 2 tasks via iter.any assertion  (crates/vox-orchestrator/src/planning/plan_adequacy.rs)

### `apply_harness_subject_defaults()`  (happy; EXTRACTED)
- [happy] apply_harness_subject_defaults() fills empty repository_id, session_id, and thread_id fields from HarnessIngestExpectations  (crates/vox-orchestrator/src/legacy/harness_hand.rs)

### `auto_score_model()`  (happy; EXTRACTED)
- [happy] auto_score_model() produces different scores when called with different AxesOverrideGuard values  (crates/vox-orchestrator/src/models/scoring.rs)

### `autonomous research gate behavior`  (invariant; INFERRED)
- [invariant] Disabled research gates prevent blocking on autonomous research during task completion  (crates/vox-orchestrator/src/orchestrator/task_dispatch/complete/success/socrates.rs)

### `build_classifier_prompt()`  (happy; EXTRACTED)
- [happy] build_classifier_prompt() includes the target model ID, description, capabilities, and formatted pricing in the output  (crates/vox-orchestrator/src/models/autonomic.rs)

### `build_repo_scoped_orchestrator_for_repository`  (invariant; EXTRACTED)
- [invariant] build_repo_scoped_orchestrator_for_repository with same repository produces matching repository_id and memory paths  (crates/vox-orchestrator/tests/bootstrap_build_parity.rs)

### `build_repo_shard_descriptors() dependency structure`  (invariant; EXTRACTED)
- [invariant] validate tasks have dependencies on corresponding gen tasks; reducer depends on all validates  (crates/vox-orchestrator/src/orchestrator/task_dispatch/submit/batch.rs)

### `check_before_local_fallback`  (happy; EXTRACTED)
- [happy] check_before_local_fallback returns Ok when no lease exists for the task  (crates/vox-orchestrator/tests/lease_gate.rs)

### `check_before_local_fallback with expired lease`  (edge; EXTRACTED)
- [edge] check_before_local_fallback returns Ok when task lease has expired even if held by remote node  (crates/vox-orchestrator/tests/lease_gate.rs)

### `check_before_local_fallback with self-held lease`  (happy; EXTRACTED)
- [happy] check_before_local_fallback returns Ok when the lease is held by the same local node requesting the fallback  (crates/vox-orchestrator/tests/lease_gate.rs)

### `check_before_local_fallback with unexpired remote lease`  (error; EXTRACTED)
- [error] check_before_local_fallback returns LeaseGateError::HeldByRemote with holder_node_id when task lease is held by remote node and not expired  (crates/vox-orchestrator/tests/lease_gate.rs)

### `clamp_context_envelope_injection_budget()`  (happy; EXTRACTED)
- [happy] Truncates summary text to fit within max_tokens_for_injection budget, inserting 'truncated' marker and setting token_estimate to max limit  (crates/vox-orchestrator/src/context_lifecycle.rs)

### `clarification_stop_rule`  (happy; EXTRACTED)
- [happy] clarification_stop_rule returns ClarificationLoopStop::MarginalGainTooLow when called with low gain threshold (0.01) and high cost (1_000).  (crates/vox-orchestrator/src/attention/mod.rs)

### `classify_bundle`  (happy; EXTRACTED)
- [happy] classify_bundle returns BundleKind::Wasm for WASM_HEADER magic bytes and BundleKind::Native for ELF headers or empty input  (crates/vox-orchestrator/src/a2a/remote_worker.rs)

### `classify_line_claim_kind`  (happy; EXTRACTED)
- [happy] classify_line_claim_kind correctly counts emoji by scalar count rather than byte length, classifying emoji-only strings as Procedural  (crates/vox-orchestrator/src/grounding.rs)

### `classify_line_claim_kind()`  (happy; EXTRACTED)
- [happy] classify_line_claim_kind() returns ClaimKind::Factual for strings containing whole-word procedural terms like 'emerge' and 'spread'  (crates/vox-orchestrator/src/grounding.rs)

### `complete_task_with_attestation() autonomous research gating`  (happy; EXTRACTED)
- [happy] complete_task_with_attestation() completes within 8 seconds when socrates_gate_enforce and completion_grounding_enforce are both false  (crates/vox-orchestrator/src/orchestrator/task_dispatch/complete/success/socrates.rs)

### `compute_attention_cost_ms`  (happy; EXTRACTED)
- [happy] Attention cost increases when token output increases from 50 to 5000, indicating token output weight in cost calculation.  (crates/vox-orchestrator/src/attention/mod.rs)

### `config_to_routing_profile()`  (happy; EXTRACTED)
- [happy] config_to_routing_profile() maps CostPreference::Economy to RoutingProfile::Free and CostPreference::Performance to RoutingProfile::Performance  (crates/vox-orchestrator/src/types/routing_profile.rs)

### `context-lifecycle-telemetry.fixtures.json`  (invariant; EXTRACTED)
- [invariant] Each fixture in context-lifecycle-telemetry.fixtures.json validates against the schema  (crates/vox-orchestrator/tests/context_lifecycle_telemetry_fixtures.rs)

### `decide()`  (happy; EXTRACTED)
- [happy] decide() respects CandidateScope::CloudOnly by excluding Ollama, VoxLocal, and PopuliMesh providers  (crates/vox-orchestrator/src/models/select.rs)

### `decode_inline function`  (happy; EXTRACTED)
- [happy] decode_inline recovers original bytes and fn_hash from ship_decision output  (crates/vox-orchestrator/tests/bundle_fetch.rs)

### `decrypt_jwe_compact`  (error; EXTRACTED)
- [error] decrypt_jwe_compact returns Err(JweError::InvalidFormat) when given a malformed JWE string with fewer than 5 dot-separated parts  (crates/vox-orchestrator/src/a2a/jwe.rs)

### `decrypt_jwe_compact()`  (happy; EXTRACTED)
- [happy] decrypt_jwe_compact() decrypts a JWE compact string back to the original payload.  (crates/vox-orchestrator/src/a2a/jwe.rs)

### `execute_handoff()`  (happy; EXTRACTED)
- [happy] Emits PlanHandoff event with has_harness_spec=true and correct session_id and thread_id from harness metadata  (crates/vox-orchestrator/src/handoff.rs)

### `gpu_compute_ms_from_attestation batch processing`  (happy; EXTRACTED)
- [happy] Sum of gpu_compute_ms_from_attestation over 10 attestations with 3.7s each equals 37000ms  (crates/vox-orchestrator/tests/kudos_reconciliation.rs)

### `grounding_violation_factual_mode_without_declarations()`  (happy; EXTRACTED)
- [happy] grounding_violation_factual_mode_without_declarations() returns None when completion_summary contains procedural directives separated by semicolons  (crates/vox-orchestrator/src/grounding.rs)

### `groups_from_workspace_members()`  (happy; EXTRACTED)
- [happy] groups_from_workspace_members() creates exactly one group per workspace member with matching name and pattern  (crates/vox-orchestrator/src/groups.rs)

### `health_score`  (happy; EXTRACTED)
- [happy] health_score returns uptime_score when available and defaults to 0.85 when uptime_score is None  (crates/vox-orchestrator/src/models/scoring.rs)

### `heartbeat::live_nodes_from_db`  (happy; EXTRACTED)
- [happy] heartbeat::live_nodes_from_db returns persisted heartbeat with node name 'node-1'  (crates/vox-orchestrator/tests/populi_coordination.rs)

### `latency_score()`  (happy; EXTRACTED)
- [happy] latency_score() scales from 0.0 to 1.0 based on measured p50 latency values  (crates/vox-orchestrator/src/models/tests.rs)

### `load_from_config`  (happy; EXTRACTED)
- [happy] Loads custom affinity groups from config file and patterns resolve correctly while others return None  (crates/vox-orchestrator/tests/affinity_group_tests.rs)

### `locks::release_distributed_lock_with_breaker`  (happy; EXTRACTED)
- [happy] locks::release_distributed_lock_with_breaker releases a held lock allowing reacquisition by another node  (crates/vox-orchestrator/tests/populi_coordination.rs)

### `merge_attestation_into_socrates_context()`  (happy; EXTRACTED)
- [happy] merge_attestation_into_socrates_context() increments evidence_count when citation substring matches evidence source  (crates/vox-orchestrator/src/grounding.rs)

### `merge_context_envelope_for_session_store()`  (happy; EXTRACTED)
- [happy] When merging context envelopes with AuthorityPrecedence strategy, incoming envelope with higher authority_rank overwrites existing envelope's authority rank  (crates/vox-orchestrator/src/context_lifecycle.rs)

### `oplog::append_to_db_with_breaker`  (happy; EXTRACTED)
- [happy] oplog::append_to_db_with_breaker appends operation with description to oplog  (crates/vox-orchestrator/tests/populi_coordination.rs)

### `oplog::list_from_db`  (happy; EXTRACTED)
- [happy] oplog::list_from_db retrieves appended operations with correct description field  (crates/vox-orchestrator/tests/populi_coordination.rs)

### `orch_daemon::serve_listener`  (happy; EXTRACTED)
- [happy] orch_daemon TCP server accepts ping requests and returns repository_id and protocol version  (crates/vox-orchestrator/tests/orchestrator_daemon_tcp.rs)

### `plan_intent`  (happy; EXTRACTED)
- [happy] plan_intent() detects write intent and includes vox_write_file in returned tool set  (crates/vox-orchestrator/src/agentos/intent_planner.rs)

### `plan_tool_daemon_alignment_valid()`  (invariant; EXTRACTED)
- [invariant] Returns true and MCP_PLAN_TOOL_NAMES has exactly 3 elements  (crates/vox-orchestrator/src/contract.rs)

### `populi_remote_delegate filtering`  (invariant; EXTRACTED)
- [invariant] Tasks with populi_remote_delegate remain on their original agent assignment and queue despite replay eligibility  (crates/vox-orchestrator/src/orchestrator/tests/populi_single_owner.rs)

### `prune_evidence_value`  (happy; EXTRACTED)
- [happy] prune_evidence_value() truncates items array to specified length limit  (crates/vox-orchestrator/src/agentos/context_budget_manager.rs)

### `quality_gate()`  (happy; EXTRACTED)
- [happy] quality_gate() returns true when ValidationResult.passed is true and false when ValidationResult.passed is false  (crates/vox-orchestrator/src/validation.rs)

### `relay_ai_fixture_distributed_subagent`  (happy; EXTRACTED)
- [happy] relay_ai_fixture_distributed_subagent returns 'inline|mesh=skipped_no_control_url' when control plane URL env vars are unset  (crates/vox-orchestrator/tests/mesh_ai_fixture_relay.rs)

### `remote_is_newer()`  (happy; EXTRACTED)
- [happy] Returns true when remote timestamp is newer than local timestamp; returns false when local is newer or remote row is missing  (crates/vox-orchestrator/src/occ.rs)

### `resolve_eligibility() with PricingSource::OpenRouter`  (happy; EXTRACTED)
- [happy] resolve_eligibility() returns Shadowed confidence for OpenRouter-sourced models when scoreboard data is absent  (crates/vox-orchestrator/src/models/discovery_pipeline.rs)

### `resolve_eligibility() with PricingSource::Telemetry`  (happy; EXTRACTED)
- [happy] resolve_eligibility() returns Confirmed confidence for Telemetry-sourced models when scoreboard data is absent  (crates/vox-orchestrator/src/models/discovery_pipeline.rs)

### `resolve_eligibility() with PricingSource::Unknown`  (happy; EXTRACTED)
- [happy] resolve_eligibility() returns Provisional confidence for Unknown-sourced models when scoreboard data is absent  (crates/vox-orchestrator/src/models/discovery_pipeline.rs)

### `resolve_local function`  (edge; EXTRACTED)
- [edge] resolve_local returns Ok(None) when bundle not found in local store  (crates/vox-orchestrator/tests/bundle_fetch.rs)

### `result_value()`  (happy; EXTRACTED)
- [happy] result_value() extracts JSON value from dispatch response  (crates/vox-orchestrator/src/orch_daemon/mod.rs)

### `secret_id_to_env_key`  (happy; EXTRACTED)
- [happy] Converts camelCase secret identifiers to SCREAMING_SNAKE_CASE environment variable names  (crates/vox-orchestrator/src/a2a/secret_bag.rs)

### `select_with_policy`  (invariant; EXTRACTED)
- [invariant] select_with_policy with empty policy returns same model selection as plain select() function  (crates/vox-orchestrator/src/models/policy.rs)

### `should_promote() on ModelConfidence::Confirmed`  (invariant; EXTRACTED)
- [invariant] should_promote() returns None for Confirmed state, making it a terminal state  (crates/vox-orchestrator/src/models/autonomic.rs)

### `should_promote() on ModelConfidence::Deprecated`  (invariant; EXTRACTED)
- [invariant] should_promote() returns None for Deprecated state, making it a terminal state  (crates/vox-orchestrator/src/models/autonomic.rs)

### `throughput_score`  (invariant; EXTRACTED)
- [invariant] throughput_score clamps result to [0.0, 1.0] interval, returning 1.0 for high RPM and computing proportional score for reference rates  (crates/vox-orchestrator/src/models/scoring.rs)

### `validate_file()`  (happy; EXTRACTED)
- [happy] validate_file() returns UNSUPPORTED_SYNTAX in response for syntactically invalid Vox files  (crates/vox-orchestrator/tests/validator_strictness_test.rs)

### `validate_handoff_invariants function`  (happy; EXTRACTED)
- [happy] accepts payload with pending tasks when execution_role metadata and verification_criteria are provided  (crates/vox-orchestrator/src/handoff.rs)

### `vox_check()`  (happy; EXTRACTED)
- [happy] vox_check() includes error code E091 in response for macro_rules definition  (crates/vox-orchestrator/tests/validator_strictness_test.rs)

### `vox_orchestrator::mcp_tools::plugin_skills_bridge::install_discovered_skills`  (happy; EXTRACTED)
- [happy] install_discovered_skills discovers and parses SKILL.md frontmatter to register skill with id from frontmatter  (crates/vox-orchestrator/tests/skill_compiler_via_plugin.rs)

### `workspace_merge_json()`  (happy; EXTRACTED)
- [happy] workspace_merge_json() returns merged=true and records at least one conflict when agents overlap on file modifications.  (crates/vox-orchestrator/tests/vcs_test.rs)

### `workspace_status_json`  (invariant; EXTRACTED)
- [invariant] returns base_snapshot id and modified_count matching state created by workspace_create_json  (crates/vox-orchestrator/src/json_vcs_facade.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`/api/v2/health endpoint`** — only: _GET /api/v2/health returns StatusCode::OK with envelope containing v=1 and data.status='ok'_
- **`AXES_OVERRIDE`** — only: _AXES_OVERRIDE is cleared (None) when AxesOverrideGuard drops_
- **`AffinityGroupRegistry::defaults`** — only: _Default registry resolves crates/vox-package to pm-group and crates/vox-compiler lexer paths to lexer-parser-group_
- **`AffinityGroupRegistry::defaults()::resolve()`** — only: _AffinityGroupRegistry::defaults() resolves parser/mod.rs to a group named 'lexer-parser-group'_
- **`AffinityGroupRegistry::detect_from_repository_layout`** — only: _creates groups with names matching detected Node.js package layout members_
- **`AffinityGroupRegistry::detect_from_repository_layout()`** — only: _AffinityGroupRegistry::detect_from_repository_layout() automatically creates groups for workspace member crates with correct path resolution_
- **`AffinityGroupRegistry::find_by_name()`** — only: _AffinityGroupRegistry::find_by_name() returns Some for existing group names and None for nonexistent names_
- **`AffinityGroupRegistry::resolve()`** — only: _AffinityGroupRegistry::resolve() returns None for paths that do not match any affinity group pattern_
- **`AgentEvent serialization`** — only: _AgentEvent serializes to JSON and deserializes back with EventId preserved_
- **`AgentEventKind.FileDiagChanged`** — only: _FileDiagChanged event round-trips through EventBus with correct path, error_count, and warn_count_
- **`AgentEventKind.MeshTopologyChanged`** — only: _MeshTopologyChanged event round-trips through EventBus with correct added/removed nodes and changed_edges_
- **`AgentHandoffAccepted event`** — only: _AgentHandoffAccepted event is emitted with has_context_envelope=true and correct session_id when context envelope is provided_
- **`AgentHandoffAccepted event metadata`** — only: _AgentHandoffAccepted event emits has_harness_spec=true, session_id, and thread_id when harness spec metadata is provided_
- **`AgentHarnessSpec::minimal_contract_first`** — only: _Data created by minimal_contract_first validates against the agent-harness.schema.json schema_
- **`AgentMessage`** — only: _AgentMessage FileChanged variant survives JSON serialization roundtrip with all fields (path, agent, summary) preserved_
- **`AgentQueue::attach_socrates_context`** — only: _attach_socrates_context succeeds and updates task context fields_
- **`AgentQueue::dequeue`** — only: _dequeue returns previously-blocked task after dependency completes_
- **`AgentQueue::enqueue_dedup`** — only: _enqueue_dedup rejects duplicate task descriptions_
- **`AgentQueue::mark_complete`** — only: _mark_complete unblocks dependent tasks_
- **`AgentQueue::reorder`** — only: _reorder changes task dequeue order by priority_
- **`AgentQueue::to_markdown`** — only: _to_markdown emits agent ID, name, and task IDs_
- **`AgentTask`** — only: _AgentTask serialization and deserialization preserves id, priority, and description fields_
- **`AgentTask::elapsed_since_last_expensive_op_ms()`** — only: _elapsed_since_last_expensive_op_ms() returns None before record_expensive_op() is called and returns Some(elapsed_ms) after it is called, with elapsed time less than 1000ms in test context_
- **`AgentTask::is_ready()`** — only: _is_ready() returns false when dependencies are not fully satisfied and true only when all dependencies are in the provided list_
- **`AgentTask::socrates`** — only: _socrates context fields are persisted after attachment_
- **`AgentWorkspace::set_bound_branch`** — only: _Sets and retrieves bound_branch, returning the value via bound_branch() method_
- **`AlarmLevel::from(AlarmTier)`** — only: _AlarmLevel can be converted from AlarmTier with AlarmLevel::None from AlarmTier::None, AlarmLevel::Caution from AlarmTier::Caution, and AlarmLevel::Warning from AlarmTier::Warning_
- **`AttentionBudget::focus_depth`** — only: _focus_depth returns FocusDepth::Deep when interrupt_freq_per_hour is 8.0._
- **`BudgetDecisionEvent`** — only: _BudgetDecisionEvent metric_type field is set to 'orch.budget.decision'_
- **`BudgetGate::check_attention_snapshot()`** — only: _When attention is enabled and fully debited (exhausted), returns GateResult::AttentionExhausted variant_
- **`BudgetManager::doom_loop_cost_check`** — only: _doom_loop_cost_check returns None when accumulated cost is at or below threshold, but returns Some with reason when cost exceeds threshold._
- **`BudgetManager::record_task_completion`** — only: _record_task_completion resets the doom_loop_cost_check counter so it no longer fires after reset_
- **`BudgetManager::would_exceed_token_budget`** — only: _would_exceed_token_budget returns true when remaining tokens (100) plus requested (200) exceeds budget (1000)_
- **`BuildStageKind`** — only: _BuildStageKind.Hir round-trips through EventBus without modification_
- **`BulletinBoard`** — only: _publish() delivers message via subscribe() receiver with TaskId and AgentId fields intact_
- **`Catalog.best_for()`** — only: _Catalog.best_for() with Performance preference selects cheapest paid model for task category_
- **`ChangeId`** — only: _ChangeId(42) formats as 'CH-000042' with zero-padded 6-digit display_
- **`ChangeStatus`** — only: _Change lifecycle transitions from InProgress to Merged via update_change_status(), and status persists when queried._
- **`CircuitBreaker::check_tier()`** — only: _check_tier returns AlarmTier::Caution when no_progress_loops equals 1_
- **`CircuitBreaker::should_escalate()`** — only: _should_escalate returns false when replan_attempts equals 2_
- **`CircuitBreaker::should_trip()`** — only: _should_trip returns Some(TripReason::NoProgress) when no_progress_loops equals 3_
- **`ClaimKind`** — only: _classify_line_claim_kind returns Speculative for hedged claims, Procedural for action-oriented statements, and Factual for declarative statements_
- **`CompactionEngine::compact`** — only: _compact() returns compacted=false and dropped_turns=0 when token count is well under the threshold_
- **`CompactionEngine::compact with CompactionStrategy::Aggressive`** — only: _compact() with Aggressive strategy triggers compaction (compacted=true) even at low threshold_
- **`CompactionEngine::compact with CompactionStrategy::Balanced`** — only: _compact() with Balanced strategy sets compacted=true, drops some turns (dropped_turns > 0), and reduces total tokens_
- **`CompactionEngine::estimate_tokens`** — only: _estimate_tokens() produces a token estimate in range [1, 10] for typical input strings like 'Hello, world!'_
- **`ConfidenceFusion::evaluate`** — only: _evaluate() returns FusionDecision::Ship for high-quality inputs_
- **`ConfidenceFusion::evaluate()`** — only: _Returns FusionDecision::Abstain when input quality metrics are all at 0.1 or lower_
- **`ConflictId::to_string()`** — only: _Formats ConflictId as 'C-' prefix followed by zero-padded 6-digit decimal number_
- **`ConflictManager::record_conflict()`** — only: _Creates a conflict that can be retrieved via get(), found via has_conflict(), and its active_count() increments_
- **`ConflictManager::resolve()`** — only: _Marks conflict as resolved, decrements active_count() to zero, and prevent double-resolution_
- **`ConflictResolution`** — only: _ConflictResolution.TakeLeft resolves active conflicts, reducing active conflict count from 1 to 0._
- **`ContentBlock`** — only: _markdown_to_content_blocks() returns both prose and code ContentBlock types_
- **`ContentBlock::TaskItem`** — only: _TaskItem blocks preserve id and complexity fields from parsed markdown_
- **`ContextEnvelope persistence by session_id`** — only: _ContextEnvelope is persisted in orchestrator context store when handoff includes valid context envelope JSON metadata_
- **`ContextEnvelope.obo_token`** — only: _ContextEnvelope.obo_token is None before signing_
- **`ContextEnvelope.sign()`** — only: _ContextEnvelope.sign() returns a new envelope with obo_token populated_
- **`ContextEnvelope::from_session_retrieval`** — only: _ContextEnvelope created from SessionRetrievalEnvelope validates against context-envelope.schema.json_
- **`ContextEnvelope::with_agentos_intent_hints()`** — only: _Populates suggested_tools vector with tool name 'vox_run_tests' when intent is 'run cargo tests'_
- **`ContextStore`** — only: _ContextStore.get() returns None for expired entries and expire_stale() removes them, returning the count of expired entries_
- **`ContextualBandit`** — only: _select() returns the arm with highest expected_reward (strong 9/10 over weak 1/10)_
- **`ContinuationEngine::generate_continuation()`** — only: _Generates continuation prompt with correct agent_id, strategy, and pending count in prompt text_
- **`ContinuationEngine::is_exhausted()`** — only: _Returns true when agent reaches max continuations limit (configured as 2 in test)_
- **`ContinuationEngine::reset_cooldown()`** — only: _After calling reset_cooldown(), exhausted agent becomes non-exhausted and can_continue() returns true_
- **`DispatchDecision`** — only: _When task complexity metric is 8 or higher, evaluate() returns DispatchDecision::Spawn_
- **`EventBus`** — only: _EventBus.emit() sends LockAcquired event that is received by subscriber with correct agent_id and path_
- **`FileAffinity`** — only: _FileAffinity::read() constructor creates FileAffinity with AccessKind::Read and FileAffinity::write() creates FileAffinity with AccessKind::Write_
- **`FileLockManager.holder()`** — only: _holder() returns the correct AgentId and LockKind for a locked path_
- **`FileLockManager.hydrate_from_db()`** — only: _hydrate_from_db() restores persisted lock state after rehydration_
- **`FusionInputs::from_task_context()`** — only: _Maps SocratesTaskContext fields to FusionInputs with matching evidence_quality and self_consistency, derives sep_estimate and citation_coverage from source_diversity/contradiction_hints_
- **`GateResult`** — only: _check_doom_loop returns GateResult::DoomLoop variant when cost progress exceeds doom_loop_cost_threshold_
- **`HandoffPayload builder`** — only: _builder pattern correctly sets from_agent, to_agent, completed_tasks, pending_tasks, owned_files, and metadata fields_
- **`HandoffPayload builder with_timeout and with_step`** — only: _builder methods correctly store timeout_ms and execution_history as sequences of ExecutionStep records_
- **`HandoffPayload.from_json()`** — only: _HandoffPayload.from_json() deserializes and preserves from_agent, to_agent, pending_tasks, and metadata_
- **`HandoffPayload.to_json()`** — only: _HandoffPayload.to_json() produces JSON containing serialized from_agent, to_agent, description, and metadata_
- **`HandoffPayload::to_json and HandoffPayload::from_json`** — only: _JSON serialization and deserialization preserves from_agent, pending_tasks, and metadata across roundtrip_
- **`HeartbeatMonitor`** — only: _Registers agents with initial Idle activity and updates activity on heartbeat calls_
- **`HeartbeatMonitor::at_or_above()`** — only: _Filters and returns agents whose staleness level meets or exceeds the specified threshold_
- **`HeartbeatMonitor::check_stale()`** — only: _Returns empty stale list immediately after registration, then returns stale agents after timeout threshold exceeded_
- **`HeartbeatMonitor::unregister()`** — only: _Removes agent from monitor, decrementing agent_count and returning None for activity queries_
- **`InMemoryHopper`** — only: _submit() creates a new item with state=ItemState::Inbox that appears in inbox() and not in history()_
- **`InMemoryHopper lifecycle`** — only: _assigned() and inbox() queues transition correctly through assign() -> complete(): item moves from inbox to assigned, then to history_
- **`InProcessSkillRuntime.run`** — only: _run() with default RunOpts succeeds and returns outcome with exit_code 0_
- **`InterruptionDecision::scaled_cost_ms`** — only: _InterruptionDecision::scaled_cost_ms() returns positive cost when interruption is required_
- **`IsolationPlan::set_override`** — only: _set_override with None clears a previously set override, reverting strategy_for to default_
- **`IsolationPlan::strategy_for`** — only: _strategy_for returns agent-specific override when set_override has been called for that agent_
- **`IsolationStrategy::SharedBranch`** — only: _SharedBranch isolation strategy rejects second agent writing to same file with OrchestratorError::LockConflict_
- **`IsolationStrategy::SplitChanges`** — only: _SplitChanges isolation strategy tolerates concurrent writes to same file without hard LockConflict failure_
- **`LivingReviewManifest`** — only: _LivingReviewManifest.version_count() returns 0 for newly created manifest_
- **`LoadBalancer::evaluate_scaling`** — only: _LoadBalancer.evaluate_scaling returns LoadBalancerAction::ScaleUp when queue depth reaches 15 tasks_
- **`LoadBalancer::pick_agent`** — only: _LoadBalancer with ShortestQueue strategy selects agent with shortest queue (AgentId 2 when agent 1 has 1 task)_
- **`LockLeaderElection`** — only: _leader election with two nodes results in one Leader and one Follower role_
- **`LongTermMemory::list_keys()`** — only: _LongTermMemory::list_keys() returns all keys previously set_
- **`LongTermMemory::set() and get()`** — only: _LongTermMemory::set(key, value) persists and get(key) retrieves the stored value_
- **`LongTermMemory::set() upsert semantics`** — only: _LongTermMemory::set() on an existing key updates (upserts) the value to the latest one set_
- **`MemoryManager::account_id()`** — only: _account_id() returns the account ID string that the manager was created with_
- **`MemoryManager::bootstrap_context()`** — only: _bootstrap_context() returns a context string containing persisted fact keys and values_
- **`MemoryManager::flush_before_compaction()`** — only: _flush_before_compaction() persists all facts in the input map and returns the count flushed, making them retrievable via lookup_fact_by_key()_
- **`MemoryManager::persist_fact() and lookup_fact_by_key()`** — only: _MemoryManager::persist_fact() stores a fact that can be retrieved via lookup_fact_by_key()_
- **`MemoryManager::search()`** — only: _search() finds matching text in both persisted facts and logged entries_
- **`ModelConfidence::Confirmed`** — only: _Confirmed state is eligible for routing_
- **`ModelConfig`** — only: _Each premium_alias target value exists in the default models list via ids() lookup_
- **`ModelRegistry::arm_stats_snapshot`** — only: _ModelRegistry::arm_stats_snapshot returns non-empty snapshot after arm stats are injected_
- **`ModelRegistry::best_for_with_filter()`** — only: _best_for_with_filter() respects the filter predicate and excludes Ollama provider when filter rejects it_
- **`NewsService::tick`** — only: _NewsService::tick does not publish news when publish_armed is false_
- **`Observer::compute_action_raw()`** — only: _When parse confidence is 0.50 (below threshold) and coverage is 0.85, returns ObserverAction::RequestMoreEvidence_
- **`Observer::summarize()`** — only: _Task summary correctly filters observations by task_id and reports accurate observation_count per task_
- **`OpLog`** — only: _OpLog.list() returns operations filtered by agent_id, and contains TaskSubmit and TaskComplete entries for task submissions and completions._
- **`OpLog.count()`** — only: _OpLog.count() remains unchanged when rebalance() does not move any tasks (moved == 0)._
- **`OpLog.redo()`** — only: _OpLog.redo() returns Some snapshot_after for undone operations._
- **`OpLog.undo()`** — only: _OpLog.undo() returns Some snapshot_before for recorded operations._
- **`OrchDaemonClient::complete_task`** — only: _OrchDaemonClient::complete_task completes a dequeued task_
- **`OrchDaemonClient::fail_task`** — only: _OrchDaemonClient::fail_task marks a dequeued task as failed_
- **`OrchDaemonClient::spawn_agent_ext`** — only: _OrchDaemonClient::spawn_agent_ext creates a dynamic agent with assigned agent_id_
- **`OrchDaemonClient::submit_task`** — only: _OrchDaemonClient::submit_task creates a task with an assigned task_id_
- **`OrchDaemonClient::subscribe`** — only: _OrchDaemonClient::subscribe pushes initial orchestrator status snapshot with agent_count field_
- **`OrchDaemonClient::subscribe_events`** — only: _OrchDaemonClient::subscribe_events streams live agent events with type 'token_streamed' after subscription_
- **`OrchDaemonClient::task_status`** — only: _OrchDaemonClient::task_status returns task status as 'InProgress' for dequeued tasks_
- **`OrchestrationMigrationFlags`** — only: _OrchestrationMigrationFlags deserializes from TOML with orchestration_v2_enabled=true and legacy_orchestration_fallback=false_
- **`Orchestrator`** — only: _After task completion and idle timeout, agents retire back to 1 from multiple agents_
- **`Orchestrator.accept_handoff()`** — only: _Orchestrator.accept_handoff() accepts handoff with valid ContextEnvelope metadata JSON and returns from_agent_
- **`Orchestrator.cancel_task()`** — only: _cancel_task() emits exactly one orch.task.cancelled telemetry event_
- **`Orchestrator.status`** — only: _status() returns non-zero total_weighted_load when tasks are submitted with non-normal priority_
- **`Orchestrator.status()`** — only: _Orchestrator.status().agent_count increments to 1 after accepting handoff_
- **`Orchestrator.submit_task`** — only: _Two tasks writing to the same file route to the same agent via file affinity_
- **`Orchestrator::evaluate_orchestrator_policy_for_agent`** — only: _Risk score increases when shell mutation tools are recorded versus read-only tools_
- **`Orchestrator::map_agent_session`** — only: _map_agent_session succeeds and returns Ok_
- **`Orchestrator::message_bus.audit_trail`** — only: _All sent messages appear in the audit trail for traceability_
- **`Orchestrator::message_bus.send`** — only: _Sends A2A messages with correct id, sender, msg_type, and payload to recipient agent's inbox_
- **`Orchestrator::spawn_agent`** — only: _Spawned agents appear in the message bus and can receive messages sent via send()_
- **`Orchestrator::status`** — only: _status() includes tracked session_id for agents that were mapped_
- **`Orchestrator::submit_repo_shard_dag`** — only: _submit_repo_shard_dag with 100 shards creates 201 tasks total (100 generation + 100 validation + 1 reducer)_
- **`OrchestratorConfig::default`** — only: _OrchestratorConfig::default sets enabled=true, max_agents=8, default_priority=Normal, queue_overflow_strategy=SpawnNewAgent, lock_timeout_ms=30000_
- **`OrchestratorConfig::isolation_strategy_default`** — only: _isolation_strategy_default defaults to SharedBranch_
- **`OrchestratorConfig::load_from_toml`** — only: _load_from_toml returns default configuration when orchestrator section is missing from TOML file_
- **`OrientPhase::classify_task_category`** — only: _Classifies 'document the API' as General task category via assert_eq assertion_
- **`OrientPhase::request_missing_evidence`** — only: _Returns None when called with evidence score 0.3 via is_none() assertion_
- **`PiiFilter::redact()`** — only: _Email addresses in input are replaced with [REDACTED_EMAIL] token in output_
- **`PrivacyClassifier::classify`** — only: _Classifier with internal_marker set to true returns PrivacyLevel::Internal_
- **`PrivacyRouter::filter_models`** — only: _PrivacyRouter::filter_models filters models to only Ollama (local) when PrivacyLevel is Private_
- **`RemoteTaskEnvelope`** — only: _RemoteTaskEnvelope deserializes from JSON lacking trace fields (parent_task_id, trace_id, span_depth), with those fields as None._
- **`RemoteTaskEnvelope serialization`** — only: _RemoteTaskEnvelope round-trips through JSON serialization preserving bundle_ref.fn_hash and bundle_inline_b64_
- **`ReplyWindowGate`** — only: _ReplyWindowGate.status() returns WindowStatus::Open { days_remaining: 9 } when 5 days have elapsed from 14-day window opening_
- **`RouteResult`** — only: _When experimental mesh routing is enabled and task capability labels match local agent labels, RouteResult::Existing is returned for the matching agent_
- **`RoutingPolicy::exploration`** — only: _exploration.epsilon_ceiling is positive_
- **`RoutingPolicy::fallback_graph`** — only: _fallback_graph is non-empty after load_
- **`RoutingPolicy::load`** — only: _load returns policy with routing_objective.kind equal to 'quality_first'_
- **`RoutingProfile`** — only: _Only RoutingProfile::Free variant returns true from is_free_only(); other variants return false_
- **`RoutingProfile::default()`** — only: _RoutingProfile::default() returns RoutingProfile::Free variant_
- **`RoutingService::least_loaded_or_spawn`** — only: _least_loaded_or_spawn routes to agent with lower-priority queue load_
- **`ScalingAction`** — only: _ScalingService returns ScaleUp action when local pressure exceeds the configured scaling threshold_
- **`ScopeCheckResult`** — only: _ScopeCheckResult::Denied variant contains reason string mentioning 'outside its assigned scope'_
- **`ScopeGuard`** — only: _In Warn mode, check_write returns Warned result for out-of-scope paths_
- **`ScopeGuard.agent_scope()`** — only: _agent_scope returns set of 2 files after assigning 2 files to an agent_
- **`ScopeGuard.check_write`** — only: _check_write on assigned paths returns Allowed result_
- **`ScopeGuard.revoke_file()`** — only: _revoke_file removes file from agent's scope, reducing count from 2 to 1_
- **`SecurityAction`** — only: _SecurityAction::DbRead returns Ok when allowed in policy, DbWrite returns Err when not allowed_
- **`SecurityGuard.check()`** — only: _SecurityGuard check enforces policy for specific agent, allowing allowed actions and denying others_
- **`SecurityPolicy.check()`** — only: _SecurityPolicy allows specified actions and denies unspecified actions_
- **`SecurityPolicy.deny()`** — only: _deny rule in permissive policy overrides the permissive default for specified action_
- **`SelectionAxes::BALANCED::to_cost_preference()`** — only: _SelectionAxes::BALANCED.to_cost_preference() returns CostPreference::Performance_
- **`SelectionAxes::COST_FIRST::to_cost_preference()`** — only: _SelectionAxes::COST_FIRST.to_cost_preference() returns CostPreference::Economy_
- **`SelectionAxes::FAST::to_cost_preference()`** — only: _SelectionAxes::FAST.to_cost_preference() returns CostPreference::Performance_
- **`SelectionAxes::QUALITY_FIRST::to_cost_preference()`** — only: _SelectionAxes::QUALITY_FIRST.to_cost_preference() returns CostPreference::Performance_
- **`SelectionAxes::from_env()`** — only: _SelectionAxes::from_env() returns SelectionAxes::BALANCED when VOX_MODEL_AXES env var is unset_
- **`SelectionIntent::ide_autocomplete()`** — only: _SelectionIntent::ide_autocomplete() sets prefer_local to true_
- **`SelectionIntent::nli_classifier()`** — only: _SelectionIntent::nli_classifier() sets axes to SelectionAxes::COST_FIRST_
- **`SelectionIntent::repair_loop()`** — only: _SelectionIntent::repair_loop() sets cacheable_workload to true_
- **`SelectionIntent::research()`** — only: _SelectionIntent::research() sets axes to SelectionAxes::QUALITY_FIRST_
- **`SessionManager::create() and SessionManager::get()`** — only: _create() creates a session with specified agent_id that can be retrieved via get()_
- **`SessionRetrievalEnvelope::from_context_envelope`** — only: _SessionRetrievalEnvelope::from_context_envelope parses ContextEnvelope and correctly preserves retrieval_tier, memory_hit_count, and rrf_fused_hit_count fields_
- **`SessionRetrievalEnvelope::to_task_context`** — only: _SessionRetrievalEnvelope::to_task_context maps rrf_fused_hit_count to evidence_count=1 and sets required_citations=0 when hybrid retrieval succeeds_
- **`SkillRegistry.get`** — only: _After bridge installation, registry.get returns Some with correct id and version_
- **`SnapshotId`** — only: _SnapshotId(42) displays as 'S-000042' with zero-padded 6-digit format_
- **`SnapshotStore.count()`** — only: _SnapshotStore.count() returns 1 after taking one snapshot_
- **`SnapshotStore.get()`** — only: _SnapshotStore.get() returns Some when querying a stored snapshot by id_
- **`SnapshotStore::diff`** — only: _SnapshotStore::diff detects when a file is modified by comparing content hashes and reports it with FileDiffKind::Modified_
- **`SnapshotStore::get_blob`** — only: _SnapshotStore::get_blob retrieves previously stored blobs by hash and returns None for non-existent hashes_
- **`SnapshotStore::hash_file`** — only: _SnapshotStore::hash_file produces a non-empty hash and correctly reports file size in bytes for a real file on disk_
- **`SocratesPlanJudge::parse_evaluation_scores`** — only: _Parses JSON evaluation text to tuple (8, 10, 10, 5, 0) via unwrap and assert_eq_
- **`SpotCheckSampler.should_check()`** — only: _should_check() returns false for all task_ids when probability is 0.0_
- **`SubAgentRouter::route`** — only: _SubAgentRouter::route returns DispatchDecision::Inline for low complexity scores below the spawn threshold_
- **`SummaryManager::get_summary()`** — only: _Returns summary text containing formatted question-answer pairs (Q[0] and A[0] formats) for recorded interactions_
- **`SummaryManager::handoff()`** — only: _Transfers interaction summary to target agent with previous summary header and interaction content preserved_
- **`Task.depends_on`** — only: _Reducer task depends on all 100 validator tasks_
- **`Task.description`** — only: _Generated shard tasks contain [PHASE:SHARD_GEN] marker in description_
- **`TelemetryEvent`** — only: _orch.task.cancelled event includes task_id in session_id field_
- **`TestDecision`** — only: _TestDecision::Required variant is produced when evaluating a task with vox file writes_
- **`TestDecisionPolicy`** — only: _TestDecisionPolicy.evaluate() returns TestDecision::Required when task contains files with .vox extension_
- **`TraceContext encode/parse`** — only: _encoded traceparent has 4 dash-separated parts with version 00, 32-char trace_id, 16-char parent_id, 2-char flags_
- **`TraceContext.from_current_span()`** — only: _from_current_span() generates a non-zero trace_id from the active span_
- **`UsageRecord`** — only: _UsageRecord can be serialized to JSON containing the model name and call count fields_
- **`WorkflowDrainState.is_draining()`** — only: _is_draining() returns true for function hashes recorded via record_drain(), and false for unrecorded hashes._
- **`WorkflowDrainState.may_start_new_run()`** — only: _may_start_new_run() returns false for drained function hashes and true for non-drained hashes._
- **`WorkflowDrainState.snapshot()`** — only: _snapshot() returns a collection with length equal to the number of record_drain() calls made._
- **`WorkspaceManager and Workspace modification tracking`** — only: _WorkspaceManager can create workspaces, has_workspace() correctly identifies them, and workspace modifications (record_modification, record_creation, record_deletion) accumulate with modified_count() returning the total and has_modification() finding specific files_
- **`WorkspaceManager.overlapping_paths()`** — only: _overlapping_paths() detects when two agents modify the same file and returns the overlapping path._
- **`WorkspaceManager::destroy_workspace`** — only: _Removes workspace from manager such that has_workspace returns false after destruction_
- **`WorkspaceManager::get_change`** — only: _Retrieved Change object tracks snapshots collection and status transitions from InProgress to Merged_
- **`WorkspaceManager::list_changes`** — only: _Filters changes by AgentId; agent 1 has 2, agent 2 has 1, and without filter returns all 3_
- **`WorkspaceManager::overlapping_paths`** — only: _Returns detected overlapping file paths between two agents' modifications (both modified 'shared.rs')_
- **`WorkspaceManager::setup_isolation() with SeparateBranches`** — only: _When setup_isolation() is called with IsolationStrategy::SeparateBranches for AgentId(4), it returns Some branch named 'agent/4' and the workspace bound_branch() returns Some('agent/4')_
- **`WorkspaceManager::setup_isolation() with SharedBranch`** — only: _When setup_isolation() is called with IsolationStrategy::SharedBranch, it returns None and the workspace bound_branch() returns None_
- **`WorkspaceManager::setup_isolation() with SplitChanges`** — only: _When setup_isolation() is called with IsolationStrategy::SplitChanges, list_changes() increments by 1 (SplitChanges starts a new per-agent change)_
- **`WorkspaceManager::update_change_status`** — only: _When change status is updated to Merged, the ChangeStatus field reflects the transition correctly_
- **`a2a::acknowledge_db_message`** — only: _a2a::acknowledge_db_message removes acknowledged message from inbox_
- **`a2a::poll_inbox_from_db`** — only: _a2a::poll_inbox_from_db retrieves sent messages before acknowledgment_
- **`a2a::send_to_db_with_breaker`** — only: _a2a::send_to_db_with_breaker persists a2a message and returns a UUID_
- **`agentos_suggested_tools_from_intent`** — only: _agentos_suggested_tools_from_intent maps 'run cargo tests' intent to vox_run_tests tool_
- **`apply_harness_subject_defaults()`** — only: _apply_harness_subject_defaults() fills empty repository_id, session_id, and thread_id fields from HarnessIngestExpectations_
- **`auto_score_model()`** — only: _auto_score_model() produces different scores when called with different AxesOverrideGuard values_
- **`base_routing_weights()`** — only: _base_routing_weights() returns the installed base routing priority when install_base_routing_priority() is called with Some value_
- **`build_classifier_prompt()`** — only: _build_classifier_prompt() includes the target model ID, description, capabilities, and formatted pricing in the output_
- **`build_exec_source_fields()`** — only: _build_exec_source_fields() returns base64 string that decodes back to the original source bytes._
- **`check_before_local_fallback`** — only: _check_before_local_fallback returns Ok when no lease exists for the task_
- **`check_before_local_fallback with self-held lease`** — only: _check_before_local_fallback returns Ok when the lease is held by the same local node requesting the fallback_
- **`check_campaign_prereg`** — only: _check_campaign_prereg(None, None) returns GateResult::Refused with preregistration-mentioning reason_
- **`choose_strategy()`** — only: _Returns SharedBranch isolation strategy when predicted_overlap is 0 and not long_running_
- **`clamp_context_envelope_injection_budget()`** — only: _Truncates summary text to fit within max_tokens_for_injection budget, inserting 'truncated' marker and setting token_estimate to max limit_
- **`clarification_stop_rule`** — only: _clarification_stop_rule returns ClarificationLoopStop::MarginalGainTooLow when called with low gain threshold (0.01) and high cost (1_000)._
- **`classify_bundle`** — only: _classify_bundle returns BundleKind::Wasm for WASM_HEADER magic bytes and BundleKind::Native for ELF headers or empty input_
- **`classify_line_claim_kind`** — only: _classify_line_claim_kind correctly counts emoji by scalar count rather than byte length, classifying emoji-only strings as Procedural_
- **`classify_line_claim_kind()`** — only: _classify_line_claim_kind() returns ClaimKind::Factual for strings containing whole-word procedural terms like 'emerge' and 'spread'_
- **`complete_task_with_attestation() autonomous research gating`** — only: _complete_task_with_attestation() completes within 8 seconds when socrates_gate_enforce and completion_grounding_enforce are both false_
- **`compute_attention_cost_ms`** — only: _Attention cost increases when token output increases from 50 to 5000, indicating token output weight in cost calculation._
- **`confidence_state_for_model()`** — only: _confidence_state_for_model() returns ModelConfidence::Provisional when model.pricing_source is PricingSource::Unknown_
- **`config_to_routing_profile()`** — only: _config_to_routing_profile() maps CostPreference::Economy to RoutingProfile::Free and CostPreference::Performance to RoutingProfile::Performance_
- **`decide()`** — only: _decide() respects CandidateScope::CloudOnly by excluding Ollama, VoxLocal, and PopuliMesh providers_
- **`decode_inline function`** — only: _decode_inline recovers original bytes and fn_hash from ship_decision output_
- **`decrypt_jwe_compact()`** — only: _decrypt_jwe_compact() decrypts a JWE compact string back to the original payload._
- **`dispatch_request()`** — only: _dispatch_request() with VCS_ISOLATION_SET_STRATEGY persists strategy_default value in shared orchestrator state_
- **`encrypt_jwe_compact()`** — only: _encrypt_jwe_compact() produces a JWE compact string with 5 dot-separated parts._
- **`evaluate_goal()`** — only: _evaluate_goal() with Direct PlanningMode returns ImmediateAct strategy_
- **`evaluate_interruption`** — only: _evaluate_interruption() returns InterruptionDecision::InterruptNow when in shadow mode_
- **`evaluate_socrates_gate`** — only: _When factual mode is enabled and evidence count is below required citations threshold, evaluate_socrates_gate returns Abstain decision_
- **`execute_handoff()`** — only: _Emits PlanHandoff event with has_harness_spec=true and correct session_id and thread_id from harness metadata_
- **`gpu_compute_ms_from_attestation batch processing`** — only: _Sum of gpu_compute_ms_from_attestation over 10 attestations with 3.7s each equals 37000ms_
- **`grounding_violation_factual_mode_without_declarations()`** — only: _grounding_violation_factual_mode_without_declarations() returns None when completion_summary contains procedural directives separated by semicolons_
- **`groups_from_workspace_members()`** — only: _groups_from_workspace_members() creates exactly one group per workspace member with matching name and pattern_
- **`health_score`** — only: _health_score returns uptime_score when available and defaults to 0.85 when uptime_score is None_
- **`heartbeat::live_nodes_from_db`** — only: _heartbeat::live_nodes_from_db returns persisted heartbeat with node name 'node-1'_
- **`infer_strengths`** — only: _presence of 'tools' parameter causes StrengthTag::Codegen and StrengthTag::Logic to be inferred, suppressing StrengthTag::Generalist_
- **`latency_score`** — only: _latency_score returns 1.0 when p50_ms <= 500, intermediate score when p50_ms in middle range, and 0.0 when p50_ms >= 8000_
- **`latency_score()`** — only: _latency_score() scales from 0.0 to 1.0 based on measured p50 latency values_
- **`load_from_config`** — only: _Loads custom affinity groups from config file and patterns resolve correctly while others return None_
- **`load_from_config()`** — only: _load_from_config() parses affinity_groups from Vox.toml with both array and single-string pattern syntax_
- **`locks::release_distributed_lock_with_breaker`** — only: _locks::release_distributed_lock_with_breaker releases a held lock allowing reacquisition by another node_
- **`merge_attestation_into_socrates_context()`** — only: _merge_attestation_into_socrates_context() increments evidence_count when citation substring matches evidence source_
- **`merge_context_envelope_for_session_store()`** — only: _When merging context envelopes with AuthorityPrecedence strategy, incoming envelope with higher authority_rank overwrites existing envelope's authority rank_
- **`model_supports_privacy_local_inference`** — only: _model_supports_privacy_local_inference returns true for Ollama provider type_
- **`occ_guarded_write()`** — only: _When remote is newer and ConflictResolution is TakeRight, skips write and returns WriteOutcome::Skipped without executing callback_
- **`ok_page function`** — only: _ok_page creates envelope with v=1, data containing array elements, and cursor field set to provided value_
- **`oplog::append_to_db_with_breaker`** — only: _oplog::append_to_db_with_breaker appends operation with description to oplog_
- **`oplog::list_from_db`** — only: _oplog::list_from_db retrieves appended operations with correct description field_
- **`orch_daemon::serve_listener`** — only: _orch_daemon TCP server accepts ping requests and returns repository_id and protocol version_
- **`plan_intent`** — only: _plan_intent() detects write intent and includes vox_write_file in returned tool set_
- **`prune_evidence_value`** — only: _prune_evidence_value() truncates items array to specified length limit_
- **`quality_gate()`** — only: _quality_gate() returns true when ValidationResult.passed is true and false when ValidationResult.passed is false_
- **`read_only_fast_forward_eligible`** — only: _read_only_fast_forward_eligible() returns true for read-only tools like vox_validate_file_
- **`relay_ai_fixture_distributed_subagent`** — only: _relay_ai_fixture_distributed_subagent returns 'inline|mesh=skipped_no_control_url' when control plane URL env vars are unset_
- **`remote_is_newer()`** — only: _Returns true when remote timestamp is newer than local timestamp; returns false when local is newer or remote row is missing_
- **`render_council_report()`** — only: _render_council_report() successfully renders with an empty ModelRegistry and includes the report header_
- **`resolve_eligibility() with PricingSource::OpenRouter`** — only: _resolve_eligibility() returns Shadowed confidence for OpenRouter-sourced models when scoreboard data is absent_
- **`resolve_eligibility() with PricingSource::Telemetry`** — only: _resolve_eligibility() returns Confirmed confidence for Telemetry-sourced models when scoreboard data is absent_
- **`resolve_eligibility() with PricingSource::Unknown`** — only: _resolve_eligibility() returns Provisional confidence for Unknown-sourced models when scoreboard data is absent_
- **`resolve_model_with_registry_fallbacks`** — only: _resolve_model_with_registry_fallbacks returns a registered model (cloud-a or cloud-b) when registry arm stats are injected_
- **`result_value()`** — only: _result_value() extracts JSON value from dispatch response_
- **`route_for_level`** — only: _route_for_level routes PrivacyLevel::Regulated to PrivacyRoutingDecision::LocalOnly_
- **`secret_id_to_env_key`** — only: _Converts camelCase secret identifiers to SCREAMING_SNAKE_CASE environment variable names_
- **`sensitivity_of`** — only: _sensitivity_of() classifies OpenRouterApiKey as Credential sensitivity level_
- **`should_sparse_checkpoint`** — only: _should_sparse_checkpoint() returns false for read-only tools like vox_git_status_
- **`split_summary_into_claim_segments`** — only: _split_summary_into_claim_segments preserves dotted version numbers like v1.2.3 without treating periods as sentence boundaries_
- **`validate_file()`** — only: _validate_file() returns UNSUPPORTED_SYNTAX in response for syntactically invalid Vox files_
- **`validate_handoff_invariants function`** — only: _accepts payload with pending tasks when execution_role metadata and verification_criteria are provided_
- **`vox_check()`** — only: _vox_check() includes error code E091 in response for macro_rules definition_
- **`vox_orchestrator::mcp_tools::plugin_skills_bridge::install_discovered_skills`** — only: _install_discovered_skills discovers and parses SKILL.md frontmatter to register skill with id from frontmatter_
- **`workspace_merge_json()`** — only: _workspace_merge_json() returns merged=true and records at least one conflict when agents overlap on file modifications._
