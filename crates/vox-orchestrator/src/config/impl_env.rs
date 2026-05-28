use super::enums::{CostPreference, ScalingProfile};
use super::orchestrator_fields::OrchestratorConfig;

impl OrchestratorConfig {
    pub fn merge_env_overrides(&mut self) {
        fn secrets_opt(id: vox_secrets::SecretId) -> Option<String> {
            vox_secrets::resolve_secret(id)
                .expose()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        }

        fn parse_or_warn<T: std::str::FromStr>(key: &str, val: &str, default: T) -> T {
            val.parse().unwrap_or_else(|_| {
                tracing::warn!("{}: invalid value '{}', using default", key, val);
                default
            })
        }

        if let Some(val) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorEnabled) {
            self.enabled = parse_or_warn("VOX_ORCHESTRATOR_ENABLED", &val, self.enabled);
        }
        if let Some(val) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorMaxAgents) {
            self.max_agents = parse_or_warn("VOX_ORCHESTRATOR_MAX_AGENTS", &val, self.max_agents);
        }
        if let Some(val) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorLockTimeoutMs) {
            self.lock_timeout_ms = parse_or_warn(
                "VOX_ORCHESTRATOR_LOCK_TIMEOUT_MS",
                &val,
                self.lock_timeout_ms,
            );
        }
        if let Some(val) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorToestubGate) {
            self.toestub_gate =
                parse_or_warn("VOX_ORCHESTRATOR_TOESTUB_GATE", &val, self.toestub_gate);
        }
        if let Some(val) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorMaxDebugIterations) {
            self.max_debug_iterations = parse_or_warn(
                "VOX_ORCHESTRATOR_MAX_DEBUG_ITERATIONS",
                &val,
                self.max_debug_iterations,
            );
        }
        if let Some(val) =
            secrets_opt(vox_secrets::SecretId::VoxOrchestratorMaxToestubDebugIterations)
        {
            self.max_toestub_debug_iterations = parse_or_warn(
                "VOX_ORCHESTRATOR_MAX_TOESTUB_DEBUG_ITERATIONS",
                &val,
                self.max_toestub_debug_iterations,
            );
        }
        if let Some(val) =
            secrets_opt(vox_secrets::SecretId::VoxOrchestratorMaxSocratesDebugIterations)
        {
            self.max_socrates_debug_iterations = parse_or_warn(
                "VOX_ORCHESTRATOR_MAX_SOCRATES_DEBUG_ITERATIONS",
                &val,
                self.max_socrates_debug_iterations,
            );
        }
        if let Some(val) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorSocratesGateShadow) {
            self.socrates_gate_shadow = parse_or_warn(
                "VOX_ORCHESTRATOR_SOCRATES_GATE_SHADOW",
                &val,
                self.socrates_gate_shadow,
            );
        }
        if let Some(val) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorSocratesGateEnforce) {
            self.socrates_gate_enforce = parse_or_warn(
                "VOX_ORCHESTRATOR_SOCRATES_GATE_ENFORCE",
                &val,
                self.socrates_gate_enforce,
            );
        }
        if let Some(val) =
            secrets_opt(vox_secrets::SecretId::VoxOrchestratorSocratesReputationRouting)
        {
            self.socrates_reputation_routing = parse_or_warn(
                "VOX_ORCHESTRATOR_SOCRATES_REPUTATION_ROUTING",
                &val,
                self.socrates_reputation_routing,
            );
        }
        if let Some(val) =
            secrets_opt(vox_secrets::SecretId::VoxOrchestratorSocratesReputationWeight)
        {
            self.socrates_reputation_weight = parse_or_warn(
                "VOX_ORCHESTRATOR_SOCRATES_REPUTATION_WEIGHT",
                &val,
                self.socrates_reputation_weight,
            );
        }
        if let Some(val) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorTrustGateRelaxEnabled)
        {
            self.trust_gate_relax_enabled = parse_or_warn(
                "VOX_ORCHESTRATOR_TRUST_GATE_RELAX_ENABLED",
                &val,
                self.trust_gate_relax_enabled,
            );
        }
        if let Some(val) =
            secrets_opt(vox_secrets::SecretId::VoxOrchestratorTrustGateRelaxMinReliability)
        {
            self.trust_gate_relax_min_reliability = parse_or_warn(
                "VOX_ORCHESTRATOR_TRUST_GATE_RELAX_MIN_RELIABILITY",
                &val,
                self.trust_gate_relax_min_reliability,
            );
        }
        if let Some(val) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorLogLevel) {
            self.log_level = val;
        }
        if let Some(val) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorFallbackSingle) {
            self.fallback_to_single_agent = parse_or_warn(
                "VOX_ORCHESTRATOR_FALLBACK_SINGLE",
                &val,
                self.fallback_to_single_agent,
            );
        }
        if let Some(val) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorMinAgents) {
            self.min_agents = parse_or_warn("VOX_ORCHESTRATOR_MIN_AGENTS", &val, self.min_agents);
        }
        if let Some(val) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorScalingThreshold) {
            self.scaling_threshold = parse_or_warn(
                "VOX_ORCHESTRATOR_SCALING_THRESHOLD",
                &val,
                self.scaling_threshold,
            );
        }
        if let Some(val) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorIdleRetirementMs) {
            self.idle_retirement_ms = parse_or_warn(
                "VOX_ORCHESTRATOR_IDLE_RETIREMENT_MS",
                &val,
                self.idle_retirement_ms,
            );
        }
        if let Some(val) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorScalingEnabled) {
            self.scaling_enabled = parse_or_warn(
                "VOX_ORCHESTRATOR_SCALING_ENABLED",
                &val,
                self.scaling_enabled,
            );
        }
        if let Some(val) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorCostPreference) {
            match val.to_lowercase().as_str() {
                "performance" => self.cost_preference = CostPreference::Performance,
                "economy" => self.cost_preference = CostPreference::Economy,
                _ => tracing::warn!(
                    "VOX_ORCHESTRATOR_COST_PREFERENCE: invalid value '{}', expected 'performance' or 'economy'",
                    val
                ),
            }
        }
        if let Some(val) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorScalingLookback) {
            self.scaling_lookback_ticks = parse_or_warn(
                "VOX_ORCHESTRATOR_SCALING_LOOKBACK",
                &val,
                self.scaling_lookback_ticks,
            );
        }
        if let Some(val) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorResourceWeight) {
            self.resource_weight = parse_or_warn(
                "VOX_ORCHESTRATOR_RESOURCE_WEIGHT",
                &val,
                self.resource_weight,
            );
        }
        if let Some(val) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorResourceCpuMult) {
            self.resource_cpu_multiplier = parse_or_warn(
                "VOX_ORCHESTRATOR_RESOURCE_CPU_MULT",
                &val,
                self.resource_cpu_multiplier,
            );
        }
        if let Some(val) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorResourceMemMult) {
            self.resource_mem_multiplier = parse_or_warn(
                "VOX_ORCHESTRATOR_RESOURCE_MEM_MULT",
                &val,
                self.resource_mem_multiplier,
            );
        }
        if let Some(val) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorResourceExponent) {
            self.resource_exponent = parse_or_warn(
                "VOX_ORCHESTRATOR_RESOURCE_EXPONENT",
                &val,
                self.resource_exponent,
            );
        }
        if let Some(val) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorScalingProfile) {
            match val.to_lowercase().as_str() {
                "conservative" => self.scaling_profile = ScalingProfile::Conservative,
                "balanced" => self.scaling_profile = ScalingProfile::Balanced,
                "aggressive" => self.scaling_profile = ScalingProfile::Aggressive,
                _ => tracing::warn!(
                    "VOX_ORCHESTRATOR_SCALING_PROFILE: invalid value '{}', expected conservative|balanced|aggressive",
                    val
                ),
            }
        }
        if let Some(val) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorMaxSpawnPerTick) {
            self.max_spawn_per_tick = parse_or_warn(
                "VOX_ORCHESTRATOR_MAX_SPAWN_PER_TICK",
                &val,
                self.max_spawn_per_tick,
            );
        }
        if let Some(val) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorScalingCooldownMs) {
            self.scaling_cooldown_ms = parse_or_warn(
                "VOX_ORCHESTRATOR_SCALING_COOLDOWN_MS",
                &val,
                self.scaling_cooldown_ms,
            );
        }
        if let Some(val) =
            secrets_opt(vox_secrets::SecretId::VoxOrchestratorUrgentRebalanceThreshold)
        {
            self.urgent_rebalance_threshold = parse_or_warn(
                "VOX_ORCHESTRATOR_URGENT_REBALANCE_THRESHOLD",
                &val,
                self.urgent_rebalance_threshold,
            );
        }
        if let Some(val) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorMigrationV2Enabled) {
            self.orchestration_migration.orchestration_v2_enabled = parse_or_warn(
                "VOX_ORCHESTRATOR_MIGRATION_V2_ENABLED",
                &val,
                self.orchestration_migration.orchestration_v2_enabled,
            );
        }
        if let Some(val) =
            secrets_opt(vox_secrets::SecretId::VoxOrchestratorMigrationLegacyFallback)
        {
            self.orchestration_migration.legacy_orchestration_fallback = parse_or_warn(
                "VOX_ORCHESTRATOR_MIGRATION_LEGACY_FALLBACK",
                &val,
                self.orchestration_migration.legacy_orchestration_fallback,
            );
        }
        if let Some(val) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorMeshControlUrl) {
            let v = val.trim();
            if v.is_empty() {
                self.populi_control_url = None;
            } else {
                self.populi_control_url = Some(v.to_string());
            }
        }
        if let Some(val) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorPopuliInferenceBaseUrl)
        {
            let v = val.trim();
            if v.is_empty() {
                self.populi_inference_base_url = None;
            } else {
                self.populi_inference_base_url = Some(v.to_string());
            }
        }
        if let Some(val) = secrets_opt(vox_secrets::SecretId::VoxMeshScopeId) {
            let v = val.trim();
            if v.is_empty() {
                self.populi_scope_id = None;
            } else {
                self.populi_scope_id = Some(v.to_string());
            }
        }
        if let Some(val) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorMeshPollIntervalSecs) {
            self.populi_poll_interval_secs = parse_or_warn(
                "VOX_ORCHESTRATOR_MESH_POLL_INTERVAL_SECS",
                &val,
                self.populi_poll_interval_secs,
            );
        }
        if let Some(val) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorMeshHttpTimeoutMs) {
            self.populi_http_timeout_ms = parse_or_warn(
                "VOX_ORCHESTRATOR_MESH_HTTP_TIMEOUT_MS",
                &val,
                self.populi_http_timeout_ms,
            );
        }
        if let Some(val) =
            secrets_opt(vox_secrets::SecretId::VoxOrchestratorMeshRoutingExperimental)
        {
            self.populi_routing_experimental = parse_or_warn(
                "VOX_ORCHESTRATOR_MESH_ROUTING_EXPERIMENTAL",
                &val,
                self.populi_routing_experimental,
            );
        }
        if let Some(val) =
            secrets_opt(vox_secrets::SecretId::VoxOrchestratorMeshRebalanceOnRemoteSchedulableDrop)
        {
            self.populi_rebalance_on_remote_schedulable_drop = parse_or_warn(
                "VOX_ORCHESTRATOR_MESH_REBALANCE_ON_REMOTE_SCHEDULABLE_DROP",
                &val,
                self.populi_rebalance_on_remote_schedulable_drop,
            );
        }
        if let Some(val) = secrets_opt(
            vox_secrets::SecretId::VoxOrchestratorMeshReplayQueuedRoutesOnRemoteSchedulableDrop,
        ) {
            self.populi_replay_queued_routes_on_remote_schedulable_drop = parse_or_warn(
                "VOX_ORCHESTRATOR_MESH_REPLAY_QUEUED_ROUTES_ON_REMOTE_SCHEDULABLE_DROP",
                &val,
                self.populi_replay_queued_routes_on_remote_schedulable_drop,
            );
        }
        if let Some(val) =
            secrets_opt(vox_secrets::SecretId::VoxOrchestratorMeshTrainingRoutingExperimental)
        {
            self.populi_training_routing_experimental = parse_or_warn(
                "VOX_ORCHESTRATOR_MESH_TRAINING_ROUTING_EXPERIMENTAL",
                &val,
                self.populi_training_routing_experimental,
            );
        }
        if let Some(val) =
            secrets_opt(vox_secrets::SecretId::VoxOrchestratorMeshTrainingBudgetPressure)
        {
            self.populi_training_budget_pressure = parse_or_warn(
                "VOX_ORCHESTRATOR_MESH_TRAINING_BUDGET_PRESSURE",
                &val,
                self.populi_training_budget_pressure,
            );
        }
        if let Some(val) =
            secrets_opt(vox_secrets::SecretId::VoxOrchestratorMeshRemoteExecuteExperimental)
        {
            self.populi_remote_execute_experimental = parse_or_warn(
                "VOX_ORCHESTRATOR_MESH_REMOTE_EXECUTE_EXPERIMENTAL",
                &val,
                self.populi_remote_execute_experimental,
            );
        }
        if let Some(val) =
            secrets_opt(vox_secrets::SecretId::VoxOrchestratorMeshRemoteExecuteReceiverAgent)
        {
            let t = val.trim();
            if t.is_empty() {
                self.populi_remote_execute_receiver_agent = None;
            } else {
                self.populi_remote_execute_receiver_agent = Some(t.to_string());
            }
        }
        if let Some(val) =
            secrets_opt(vox_secrets::SecretId::VoxOrchestratorMeshRemoteExecuteSenderAgent)
        {
            let t = val.trim();
            if t.is_empty() {
                self.populi_remote_execute_sender_agent = None;
            } else {
                self.populi_remote_execute_sender_agent = Some(t.to_string());
            }
        }
        if let Some(val) =
            secrets_opt(vox_secrets::SecretId::VoxOrchestratorMeshRemoteResultPollIntervalSecs)
        {
            self.populi_remote_result_poll_interval_secs = parse_or_warn(
                "VOX_ORCHESTRATOR_MESH_REMOTE_RESULT_POLL_INTERVAL_SECS",
                &val,
                self.populi_remote_result_poll_interval_secs,
            );
        }
        if let Some(val) =
            secrets_opt(vox_secrets::SecretId::VoxOrchestratorMeshRemoteResultMaxMessagesPerPoll)
        {
            self.populi_remote_result_max_messages_per_poll = parse_or_warn(
                "VOX_ORCHESTRATOR_MESH_REMOTE_RESULT_MAX_MESSAGES_PER_POLL",
                &val,
                self.populi_remote_result_max_messages_per_poll,
            );
        }
        if let Some(val) =
            secrets_opt(vox_secrets::SecretId::VoxOrchestratorMeshRemoteWorkerPollIntervalSecs)
        {
            self.populi_remote_worker_poll_interval_secs = parse_or_warn(
                "VOX_ORCHESTRATOR_MESH_REMOTE_WORKER_POLL_INTERVAL_SECS",
                &val,
                self.populi_remote_worker_poll_interval_secs,
            );
        }
        if let Some(val) =
            secrets_opt(vox_secrets::SecretId::VoxOrchestratorMeshRemoteLeaseGatingEnabled)
        {
            self.populi_remote_lease_gating_enabled = parse_or_warn(
                "VOX_ORCHESTRATOR_MESH_REMOTE_LEASE_GATING_ENABLED",
                &val,
                self.populi_remote_lease_gating_enabled,
            );
        }
        if let Some(val) =
            secrets_opt(vox_secrets::SecretId::VoxOrchestratorMeshRemoteLeaseGatedRoles)
        {
            let mut roles = Vec::new();
            for part in val.split(',') {
                let p = part.trim();
                if p.is_empty() {
                    continue;
                }
                let r = match p.to_ascii_lowercase().as_str() {
                    "planner" => Some(crate::reconstruction::AgentExecutionRole::Planner),
                    "builder" => Some(crate::reconstruction::AgentExecutionRole::Builder),
                    "verifier" => Some(crate::reconstruction::AgentExecutionRole::Verifier),
                    "reproducer" => Some(crate::reconstruction::AgentExecutionRole::Reproducer),
                    "researcher" => Some(crate::reconstruction::AgentExecutionRole::Researcher),
                    _ => None,
                };
                if let Some(role) = r {
                    if !roles.contains(&role) {
                        roles.push(role);
                    }
                } else {
                    tracing::warn!(
                        "VOX_ORCHESTRATOR_MESH_REMOTE_LEASE_GATED_ROLES: unknown role token {:?} (ignored)",
                        p
                    );
                }
            }
            self.populi_remote_lease_gated_roles = roles;
        }
        if let Some(val) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorChatmlStrict) {
            self.chatml_strict =
                parse_or_warn("VOX_ORCHESTRATOR_CHATML_STRICT", &val, self.chatml_strict);
        }
        if let Some(val) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorPlanningEnabled) {
            self.planning_enabled = parse_or_warn(
                "VOX_ORCHESTRATOR_PLANNING_ENABLED",
                &val,
                self.planning_enabled,
            );
        }
        if let Some(val) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorPlanningRouterEnabled)
        {
            self.planning_router_enabled = parse_or_warn(
                "VOX_ORCHESTRATOR_PLANNING_ROUTER_ENABLED",
                &val,
                self.planning_router_enabled,
            );
        }
        if let Some(val) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorPlanningReplanEnabled)
        {
            self.planning_replan_enabled = parse_or_warn(
                "VOX_ORCHESTRATOR_PLANNING_REPLAN_ENABLED",
                &val,
                self.planning_replan_enabled,
            );
        }
        if let Some(val) =
            secrets_opt(vox_secrets::SecretId::VoxOrchestratorPlanningWorkflowHandoffEnabled)
        {
            self.planning_workflow_handoff_enabled = parse_or_warn(
                "VOX_ORCHESTRATOR_PLANNING_WORKFLOW_HANDOFF_ENABLED",
                &val,
                self.planning_workflow_handoff_enabled,
            );
        }
        if let Some(val) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorPlanningShadowMode) {
            self.planning_shadow_mode = parse_or_warn(
                "VOX_ORCHESTRATOR_PLANNING_SHADOW_MODE",
                &val,
                self.planning_shadow_mode,
            );
        }
        if let Some(val) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorResearchModelEnabled) {
            self.research_model_enabled = parse_or_warn(
                "VOX_ORCHESTRATOR_RESEARCH_MODEL_ENABLED",
                &val,
                self.research_model_enabled,
            );
        }
        if let Some(val) =
            secrets_opt(vox_secrets::SecretId::VoxOrchestratorPlanningAutoModeEnabled)
        {
            self.planning_auto_mode_enabled = parse_or_warn(
                "VOX_ORCHESTRATOR_PLANNING_AUTO_MODE_ENABLED",
                &val,
                self.planning_auto_mode_enabled,
            );
        }
        if let Some(val) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorPlanningRolloutPercent)
        {
            self.planning_rollout_percent = parse_or_warn(
                "VOX_ORCHESTRATOR_PLANNING_ROLLOUT_PERCENT",
                &val,
                self.planning_rollout_percent,
            );
        }
        if let Some(val) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorPlanAdequacyShadow) {
            self.plan_adequacy_shadow = parse_or_warn(
                "VOX_ORCHESTRATOR_PLAN_ADEQUACY_SHADOW",
                &val,
                self.plan_adequacy_shadow,
            );
        }
        if let Some(val) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorPlanAdequacyEnforce) {
            self.plan_adequacy_enforce = parse_or_warn(
                "VOX_ORCHESTRATOR_PLAN_ADEQUACY_ENFORCE",
                &val,
                self.plan_adequacy_enforce,
            );
        }
        if let Some(val) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorContextLifecycleShadow)
        {
            self.context_lifecycle_shadow = parse_or_warn(
                "VOX_ORCHESTRATOR_CONTEXT_LIFECYCLE_SHADOW",
                &val,
                self.context_lifecycle_shadow,
            );
        }
        if let Some(val) =
            secrets_opt(vox_secrets::SecretId::VoxOrchestratorContextLifecycleEnforce)
        {
            self.context_lifecycle_enforce = parse_or_warn(
                "VOX_ORCHESTRATOR_CONTEXT_LIFECYCLE_ENFORCE",
                &val,
                self.context_lifecycle_enforce,
            );
        }
        if let Some(val) =
            secrets_opt(vox_secrets::SecretId::VoxOrchestratorCompletionGroundingShadow)
        {
            self.completion_grounding_shadow = parse_or_warn(
                "VOX_ORCHESTRATOR_COMPLETION_GROUNDING_SHADOW",
                &val,
                self.completion_grounding_shadow,
            );
        }
        if let Some(val) =
            secrets_opt(vox_secrets::SecretId::VoxOrchestratorCompletionGroundingEnforce)
        {
            self.completion_grounding_enforce = parse_or_warn(
                "VOX_ORCHESTRATOR_COMPLETION_GROUNDING_ENFORCE",
                &val,
                self.completion_grounding_enforce,
            );
        }
        if let Some(val) =
            secrets_opt(vox_secrets::SecretId::VoxOrchestratorCompletionMarkdownLinkAuditEnabled)
        {
            self.completion_markdown_link_audit_enabled = parse_or_warn(
                "VOX_ORCHESTRATOR_COMPLETION_MARKDOWN_LINK_AUDIT_ENABLED",
                &val,
                self.completion_markdown_link_audit_enabled,
            );
        }
        // Phase 15: Attention Budget env overrides
        if let Some(v) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorAttentionEnabled) {
            self.attention_enabled = parse_or_warn(
                "VOX_ORCHESTRATOR_ATTENTION_ENABLED",
                &v,
                self.attention_enabled,
            );
        }
        if let Some(v) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorAttentionBudgetMs) {
            self.attention_budget_ms = parse_or_warn(
                "VOX_ORCHESTRATOR_ATTENTION_BUDGET_MS",
                &v,
                self.attention_budget_ms,
            );
        }
        if let Some(v) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorAttentionAlertThreshold)
        {
            self.attention_alert_threshold = parse_or_warn(
                "VOX_ORCHESTRATOR_ATTENTION_ALERT_THRESHOLD",
                &v,
                self.attention_alert_threshold,
            );
        }
        if let Some(v) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorAttentionInterruptCostMs)
        {
            self.attention_interrupt_cost_ms = parse_or_warn(
                "VOX_ORCHESTRATOR_ATTENTION_INTERRUPT_COST_MS",
                &v,
                self.attention_interrupt_cost_ms,
            );
        }
        if let Some(v) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorTrustEwmaAlpha) {
            self.trust_ewma_alpha = parse_or_warn(
                "VOX_ORCHESTRATOR_TRUST_EWMA_ALPHA",
                &v,
                self.trust_ewma_alpha,
            );
        }
        if let Some(v) =
            secrets_opt(vox_secrets::SecretId::VoxOrchestratorTrustProvisionalThreshold)
        {
            self.trust_provisional_threshold = parse_or_warn(
                "VOX_ORCHESTRATOR_TRUST_PROVISIONAL_THRESHOLD",
                &v,
                self.trust_provisional_threshold,
            );
        }
        if let Some(v) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorTrustTrustedThreshold) {
            self.trust_trusted_threshold = parse_or_warn(
                "VOX_ORCHESTRATOR_TRUST_TRUSTED_THRESHOLD",
                &v,
                self.trust_trusted_threshold,
            );
        }
        if let Some(v) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorTrustAutoApproveMin) {
            self.trust_auto_approve_min = parse_or_warn(
                "VOX_ORCHESTRATOR_TRUST_AUTO_APPROVE_MIN",
                &v,
                self.trust_auto_approve_min,
            );
        }
        if let Some(v) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorTlxMental) {
            self.attention_tlx_weights.mental = parse_or_warn(
                "VOX_ORCHESTRATOR_TLX_MENTAL",
                &v,
                self.attention_tlx_weights.mental,
            );
        }
        if let Some(v) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorTlxTemporal) {
            self.attention_tlx_weights.temporal = parse_or_warn(
                "VOX_ORCHESTRATOR_TLX_TEMPORAL",
                &v,
                self.attention_tlx_weights.temporal,
            );
        }
        if let Some(v) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorTlxFrustration) {
            self.attention_tlx_weights.frustration = parse_or_warn(
                "VOX_ORCHESTRATOR_TLX_FRUSTRATION",
                &v,
                self.attention_tlx_weights.frustration,
            );
        }
        if let Some(v) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorTlxTrustDiscount) {
            self.attention_tlx_weights.trust_discount = parse_or_warn(
                "VOX_ORCHESTRATOR_TLX_TRUST_DISCOUNT",
                &v,
                self.attention_tlx_weights.trust_discount,
            );
        }
        if let Some(v) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorTierGateEntropyThreshold)
        {
            self.tier_gate.entropy_auto_approve_threshold = parse_or_warn(
                "VOX_ORCHESTRATOR_TIER_GATE_ENTROPY_THRESHOLD",
                &v,
                self.tier_gate.entropy_auto_approve_threshold,
            );
        }
        if let Some(v) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorTierGateMinObservations)
        {
            self.tier_gate.auto_approve_min_observations = parse_or_warn(
                "VOX_ORCHESTRATOR_TIER_GATE_MIN_OBSERVATIONS",
                &v,
                self.tier_gate.auto_approve_min_observations,
            );
        }
        if let Some(v) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorInterruptionCalPlanGain)
        {
            self.interruption_calibration.plan_review_gain_offset_bits = parse_or_warn(
                "VOX_ORCHESTRATOR_INTERRUPTION_CAL_PLAN_GAIN",
                &v,
                self.interruption_calibration.plan_review_gain_offset_bits,
            );
        }
        if let Some(v) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorInterruptionCalA2AGain) {
            self.interruption_calibration
                .a2a_escalation_gain_offset_bits = parse_or_warn(
                "VOX_ORCHESTRATOR_INTERRUPTION_CAL_A2A_GAIN",
                &v,
                self.interruption_calibration
                    .a2a_escalation_gain_offset_bits,
            );
        }
        if let Some(v) =
            secrets_opt(vox_secrets::SecretId::VoxOrchestratorInterruptionCalBacklogPenalty)
        {
            self.interruption_calibration.backlog_cost_penalty_per_item = parse_or_warn(
                "VOX_ORCHESTRATOR_INTERRUPTION_CAL_BACKLOG_PENALTY",
                &v,
                self.interruption_calibration.backlog_cost_penalty_per_item,
            );
        }
        if let Some(v) =
            secrets_opt(vox_secrets::SecretId::VoxOrchestratorAttentionTrustRoutingWeight)
        {
            self.attention_trust_routing_weight = parse_or_warn(
                "VOX_ORCHESTRATOR_ATTENTION_TRUST_ROUTING_WEIGHT",
                &v,
                self.attention_trust_routing_weight,
            );
        }
        if let Some(v) =
            secrets_opt(vox_secrets::SecretId::VoxOrchestratorRepoShardSpecializationWeight)
        {
            self.repo_shard_specialization_weight = parse_or_warn(
                "VOX_ORCHESTRATOR_REPO_SHARD_SPECIALIZATION_WEIGHT",
                &v,
                self.repo_shard_specialization_weight,
            );
        }
        if let Some(v) =
            secrets_opt(vox_secrets::SecretId::VoxOrchestratorRepoShardValidationFailurePenalty)
        {
            self.repo_shard_validation_failure_penalty = parse_or_warn(
                "VOX_ORCHESTRATOR_REPO_SHARD_VALIDATION_FAILURE_PENALTY",
                &v,
                self.repo_shard_validation_failure_penalty,
            );
        }
        if let Some(v) =
            secrets_opt(vox_secrets::SecretId::VoxOrchestratorRepoReduceConflictCooldownPenalty)
        {
            self.repo_reduce_conflict_cooldown_penalty = parse_or_warn(
                "VOX_ORCHESTRATOR_REPO_REDUCE_CONFLICT_COOLDOWN_PENALTY",
                &v,
                self.repo_reduce_conflict_cooldown_penalty,
            );
        }
        if let Some(v) =
            secrets_opt(vox_secrets::SecretId::VoxOrchestratorRepoReduceConflictCooldownMs)
        {
            self.repo_reduce_conflict_cooldown_ms = parse_or_warn(
                "VOX_ORCHESTRATOR_REPO_REDUCE_CONFLICT_COOLDOWN_MS",
                &v,
                self.repo_reduce_conflict_cooldown_ms,
            );
        }
        // News syndication (see docs/architecture/news_syndication_security.md)
        if let Some(v) = secrets_opt(vox_secrets::SecretId::VoxNewsPublishArmed) {
            self.news.publish_armed =
                parse_or_warn("VOX_NEWS_PUBLISH_ARMED", &v, self.news.publish_armed);
        }
        if let Some(v) = secrets_opt(vox_secrets::SecretId::VoxNewsSiteBaseUrl) {
            let t = v.trim();
            if t.is_empty() {
                self.news.site_base_url = None;
            } else {
                self.news.site_base_url = Some(t.to_string());
            }
        }
        if let Some(v) = secrets_opt(vox_secrets::SecretId::VoxNewsRssFeedPath) {
            let t = v.trim();
            if t.is_empty() {
                self.news.rss_feed_path = None;
            } else {
                self.news.rss_feed_path = Some(t.to_string());
            }
        }
        if let Some(v) = secrets_opt(vox_secrets::SecretId::VoxNewsScanRecursive) {
            self.news.scan_recursive =
                parse_or_warn("VOX_NEWS_SCAN_RECURSIVE", &v, self.news.scan_recursive);
        }
        if let Some(v) = secrets_opt(vox_secrets::SecretId::VoxNewsTwitterTextChunkMax) {
            self.news.twitter_text_chunk_max = Some(parse_or_warn(
                "VOX_NEWS_TWITTER_TEXT_CHUNK_MAX",
                &v,
                self.news.twitter_text_chunk_max.unwrap_or(280),
            ));
        }
        if let Some(v) = secrets_opt(vox_secrets::SecretId::VoxNewsTwitterTruncationSuffix) {
            let t = v.trim();
            if t.is_empty() {
                self.news.twitter_truncation_suffix = None;
            } else {
                self.news.twitter_truncation_suffix = Some(t.to_string());
            }
        }
        self.news.reddit_client_id =
            vox_secrets::resolve_secret(vox_secrets::SecretId::VoxSocialRedditClientId)
                .expose()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string);
        self.news.reddit_client_secret =
            vox_secrets::resolve_secret(vox_secrets::SecretId::VoxSocialRedditClientSecret)
                .expose()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string);
        self.news.reddit_refresh_token =
            vox_secrets::resolve_secret(vox_secrets::SecretId::VoxSocialRedditRefreshToken)
                .expose()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string);
        self.news.reddit_user_agent =
            vox_secrets::resolve_secret(vox_secrets::SecretId::VoxSocialRedditUserAgent)
                .expose()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string);
        self.news.youtube_client_id =
            vox_secrets::resolve_secret(vox_secrets::SecretId::VoxSocialYoutubeClientId)
                .expose()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string);
        self.news.youtube_client_secret =
            vox_secrets::resolve_secret(vox_secrets::SecretId::VoxSocialYoutubeClientSecret)
                .expose()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string);
        self.news.youtube_refresh_token =
            vox_secrets::resolve_secret(vox_secrets::SecretId::VoxSocialYoutubeRefreshToken)
                .expose()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string);
        if let Some(v) = secrets_opt(vox_secrets::SecretId::VoxSocialHnMode) {
            let t = v.trim();
            self.news.hacker_news_mode = if t.is_empty() {
                None
            } else {
                Some(t.to_string())
            };
        }
        if let Some(v) = secrets_opt(vox_secrets::SecretId::VoxSocialWorthinessEnforce) {
            self.news.worthiness_enforce = parse_or_warn(
                "VOX_SOCIAL_WORTHINESS_ENFORCE",
                &v,
                self.news.worthiness_enforce,
            );
        }
        if let Some(v) = secrets_opt(vox_secrets::SecretId::VoxSocialWorthinessScoreMin) {
            self.news.worthiness_score_min = Some(parse_or_warn(
                "VOX_SOCIAL_WORTHINESS_SCORE_MIN",
                &v,
                self.news.worthiness_score_min.unwrap_or(0.85),
            ));
        }
        if let Some(v) = secrets_opt(vox_secrets::SecretId::VoxSocialChannelWorthinessFloors) {
            for pair in v.split(',') {
                let trimmed = pair.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let mut parts = trimmed.splitn(2, '=');
                let key = parts.next().unwrap_or("").trim().to_lowercase();
                let val = parts.next().unwrap_or("").trim();
                if key.is_empty() || val.is_empty() {
                    continue;
                }
                let floor = parse_or_warn("VOX_SOCIAL_CHANNEL_WORTHINESS_FLOORS", val, 0.85_f64);
                self.news.channel_worthiness_floors.insert(key, floor);
            }
        }
        self.news.bluesky_handle = secrets_opt(vox_secrets::SecretId::VoxSocialBlueskyHandle);
        self.news.bluesky_password = secrets_opt(vox_secrets::SecretId::VoxSocialBlueskyPassword);
        self.news.mastodon_token = secrets_opt(vox_secrets::SecretId::VoxSocialMastodonToken);
        self.news.mastodon_domain = secrets_opt(vox_secrets::SecretId::VoxSocialMastodonDomain);
        self.news.linkedin_token = secrets_opt(vox_secrets::SecretId::VoxSocialLinkedinAccessToken);
        self.news.discord_webhook = secrets_opt(vox_secrets::SecretId::VoxSocialDiscordWebhook);

        if let Some(v) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorExecTimeBudgetEnabled) {
            self.exec_time_budget_enabled = parse_or_warn(
                "VOX_ORCHESTRATOR_EXEC_TIME_BUDGET_ENABLED",
                &v,
                self.exec_time_budget_enabled,
            );
        }
        if let Some(v) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorExecTimeSafetyMultiplier)
        {
            self.exec_time_safety_multiplier = parse_or_warn(
                "VOX_ORCHESTRATOR_EXEC_TIME_SAFETY_MULTIPLIER",
                &v,
                self.exec_time_safety_multiplier,
            );
        }
        if let Some(v) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorExecTimeTimeoutRateAlert)
        {
            self.exec_time_timeout_rate_alert = parse_or_warn(
                "VOX_ORCHESTRATOR_EXEC_TIME_TIMEOUT_RATE_ALERT",
                &v,
                self.exec_time_timeout_rate_alert,
            );
        }
        if let Some(v) = secrets_opt(vox_secrets::SecretId::VoxOrchestratorExecTimeDefaultBudgetMs)
        {
            self.exec_time_default_budget_ms = parse_or_warn(
                "VOX_ORCHESTRATOR_EXEC_TIME_DEFAULT_BUDGET_MS",
                &v,
                self.exec_time_default_budget_ms,
            );
        }
        if let Some(v) =
            secrets_opt(vox_secrets::SecretId::VoxOrchestratorExecTimeHistoryWindowDays)
        {
            self.exec_time_history_window_days = parse_or_warn(
                "VOX_ORCHESTRATOR_EXEC_TIME_HISTORY_WINDOW_DAYS",
                &v,
                self.exec_time_history_window_days,
            );
        }

        if let Some(v) =
            secrets_opt(vox_secrets::SecretId::VoxOrchestratorAgentosAciEnvelopeEnabled)
        {
            self.agentos_aci_envelope_enabled = parse_or_warn(
                "VOX_ORCHESTRATOR_AGENTOS_ACI_ENVELOPE_ENABLED",
                &v,
                self.agentos_aci_envelope_enabled,
            );
        }
        if let Some(v) =
            secrets_opt(vox_secrets::SecretId::VoxOrchestratorAgentosGuardrailKernelEnabled)
        {
            self.agentos_guardrail_kernel_enabled = parse_or_warn(
                "VOX_ORCHESTRATOR_AGENTOS_GUARDRAIL_KERNEL_ENABLED",
                &v,
                self.agentos_guardrail_kernel_enabled,
            );
        }
        if let Some(v) =
            secrets_opt(vox_secrets::SecretId::VoxOrchestratorAgentosCheckpointHintsEnabled)
        {
            self.agentos_checkpoint_hints_enabled = parse_or_warn(
                "VOX_ORCHESTRATOR_AGENTOS_CHECKPOINT_HINTS_ENABLED",
                &v,
                self.agentos_checkpoint_hints_enabled,
            );
        }

        // SCIENTIA research mesh intake (see docs/architecture/research-scientia-telemetry-channels.md)
        if let Some(v) =
            secrets_opt(vox_secrets::SecretId::VoxScientiaResearchMeshIntakeWriterEnabled)
        {
            self.scientia_research_mesh.intake_writer_enabled = parse_or_warn(
                "VOX_SCIENTIA_RESEARCH_MESH_INTAKE_WRITER",
                &v,
                self.scientia_research_mesh.intake_writer_enabled,
            );
        }
        if let Some(v) =
            secrets_opt(vox_secrets::SecretId::VoxScientiaResearchMeshConsumerPollEnabled)
        {
            self.scientia_research_mesh.intake_consumer_poll_enabled = parse_or_warn(
                "VOX_SCIENTIA_RESEARCH_MESH_CONSUMER_POLL",
                &v,
                self.scientia_research_mesh.intake_consumer_poll_enabled,
            );
        }
        if let Some(v) =
            secrets_opt(vox_secrets::SecretId::VoxScientiaResearchMeshConsumerPollIntervalMs)
        {
            self.scientia_research_mesh.intake_consumer_poll_interval_ms = parse_or_warn(
                "VOX_SCIENTIA_RESEARCH_MESH_CONSUMER_POLL_INTERVAL_MS",
                &v,
                self.scientia_research_mesh.intake_consumer_poll_interval_ms,
            );
        }
    }

    /// Create a config suitable for testing (small limits, fast timeouts).
    pub fn for_testing() -> Self {
        Self {
            max_agents: 4,
            lock_timeout_ms: 1000,
            bulletin_capacity: 16,
            toestub_gate: false,
            behavioral_gate_on_complete: false,
            completion_markdown_link_audit_enabled: false,
            ..Default::default()
        }
    }
}
