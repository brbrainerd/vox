use crate::planning::{PlanningMode, PlanningStrategy};
use crate::types::{AccessKind, FileAffinity, TaskId, TaskPriority};
use std::collections::HashSet;
use std::path::PathBuf;

use super::super::super::{Orchestrator, OrchestratorError};

fn merge_file_affinities(into: &mut Vec<FileAffinity>, extra: &[FileAffinity]) {
    let mut have: HashSet<(std::path::PathBuf, AccessKind)> =
        into.iter().map(|f| (f.path.clone(), f.access)).collect();
    for f in extra {
        let key = (f.path.clone(), f.access);
        if have.insert(key) {
            into.push(f.clone());
        }
    }
}

fn infer_repo_root_from_manifest(manifest: &[FileAffinity]) -> Option<PathBuf> {
    let start = manifest.first().map(|fa| {
        let mut p = fa.path.clone();
        if p.is_file() {
            p.pop();
        }
        p
    })?;
    let mut cur = start.clone();
    for _ in 0..24 {
        if cur.join("Cargo.toml").is_file() {
            return Some(cur);
        }
        if !cur.pop() {
            break;
        }
    }
    Some(start)
}

fn socrates_task_from_search_pass(
    execution: &vox_search::SearchExecution,
    diagnostics: &vox_db::SearchDiagnostics,
    plan: &vox_db::SearchPlan,
    policy: &vox_search::SearchPolicy,
) -> crate::socrates::SocratesTaskContext {
    let memory_hits = execution.memory_lines.len();
    let knowledge_hits = execution.knowledge_lines.len();
    let chunk_hits = execution.chunk_lines.len()
        + execution.tantivy_doc_lines.len()
        + execution.qdrant_lines.len();
    let repo_hits = execution.repo_lines.len();
    let doc_graph_hits = knowledge_hits + chunk_hits + repo_hits;
    let required_citations = if memory_hits == 0 && doc_graph_hits == 0 {
        1_u8
    } else {
        0_u8
    };
    let evidence_total = (memory_hits + doc_graph_hits).min(u8::MAX as usize) as u8;
    let retrieval_tier = if execution.used_vector && execution.used_bm25 {
        "hybrid"
    } else if execution.used_bm25 {
        "bm25"
    } else if execution.lexical_fallback_used {
        "lexical_fallback"
    } else {
        "none"
    };
    let mut with_hits = Vec::new();
    let mut empty = Vec::new();
    for c in &plan.corpora {
        let has = match c {
            vox_db::SearchCorpus::Memory => memory_hits > 0,
            vox_db::SearchCorpus::KnowledgeGraph => knowledge_hits > 0,
            vox_db::SearchCorpus::DocumentChunks => chunk_hits > 0,
            vox_db::SearchCorpus::RepoInventory => repo_hits > 0,
            vox_db::SearchCorpus::WebResearch => false,
            vox_db::SearchCorpus::SymbolProximity => false,
        };
        let label = format!("{c:?}").to_ascii_lowercase();
        if has {
            with_hits.push(label);
        } else {
            empty.push(label);
        }
    }
    let evidence_shape = if execution.contradiction_count > 0 {
        "contradictory"
    } else if evidence_total == 0 {
        "empty"
    } else if execution.source_diversity <= 1 {
        "narrow"
    } else {
        "ok"
    };
    let recommended_next_action = diagnostics
        .recommended_action
        .or(execution.recommended_next_action)
        .map(|a| format!("{a:?}").to_ascii_lowercase());
    crate::socrates::SocratesTaskContext {
        risk_budget: "normal".to_string(),
        factual_mode: true,
        required_citations,
        evidence_count: evidence_total,
        contradiction_hints: execution.contradiction_count.min(u8::MAX as usize) as u8,
        retrieval_tier: Some(retrieval_tier.to_string()),
        retrieval_used_vector: execution.used_vector,
        retrieval_used_lexical_fallback: execution.lexical_fallback_used,
        source_diversity: execution.source_diversity.min(u8::MAX as usize) as u8,
        evidence_quality: execution.evidence_quality.clamp(0.0, 1.0),
        citation_coverage: execution.citation_coverage.clamp(0.0, 1.0),
        verification_performed: diagnostics.verification_performed,
        verification_reason: diagnostics.verification_reason.clone(),
        recommended_next_action,
        retrieval_diagnosis: Some(crate::socrates::RetrievalDiagnosis {
            corpora_with_hits: with_hits,
            corpora_empty: empty,
            policy_version: policy.version,
            planner_intent: format!("{:?}", plan.intent).to_ascii_lowercase(),
            evidence_shape: evidence_shape.to_string(),
        }),
        fatigue_active: false,
        orient_report: None,
        answered_questions: vec![],
        research_model_enabled: false,
        fabricated_tool_claims: None,
    }
}

impl Orchestrator {
    /// If the context store holds a session-scoped context envelope, attach projected retrieval context.
    pub(crate) fn attach_session_retrieval_envelope_if_present(
        &self,
        task_id: TaskId,
        session_id: &Option<String>,
    ) -> bool {
        let Some(sid) = session_id.as_ref() else {
            return false;
        };
        let context_key = crate::socrates::session_context_envelope_key(sid);
        let store = crate::sync_lock::rw_read(&*self.context_store);
        let context_raw = store.get(&context_key);
        drop(store);

        let parsed = context_raw
            .as_ref()
            .and_then(|raw| serde_json::from_str::<crate::ContextEnvelope>(raw).ok());

        let Some(context_envelope) = parsed else {
            return false;
        };

        let cfg = crate::sync_lock::rw_read(&*self.config).clone();
        let repo = crate::lineage::repository_id();
        if let Err(e) = crate::context_lifecycle::apply_context_lifecycle_policy(
            &cfg,
            &context_envelope,
            crate::context_lifecycle::ContextIngestExpectations {
                repository_id: repo.as_str(),
                session_id: Some(sid.as_str()),
            },
            crate::context_lifecycle::ContextIngestSource::SessionAttach,
        ) {
            tracing::warn!(
                task_id = task_id.0,
                error = %e,
                "session context envelope rejected by lifecycle policy"
            );
            if cfg.context_lifecycle_enforce {
                return false;
            }
        }

        let Some(env) =
            crate::socrates::SessionRetrievalEnvelope::from_context_envelope(&context_envelope)
        else {
            return false;
        };
        if let Err(e) = self.attach_socrates_context(task_id, env.to_task_context()) {
            tracing::debug!(
                task_id = task_id.0,
                error = %e,
                "session retrieval context parse OK but Socrates attach failed"
            );
            return false;
        }
        true
    }

    fn generate_goal_search_heuristic_only(
        &self,
        description: &str,
    ) -> crate::socrates::SocratesTaskContext {
        let plan = vox_db::heuristic_search_plan(description, false, None);
        let recommended_next_action = match plan.intent {
            vox_db::SearchIntent::CodeNavigation => Some("focus_repo".to_string()),
            vox_db::SearchIntent::RepoStructure => Some("broaden_scope".to_string()),
            vox_db::SearchIntent::BroadResearch => Some("focus_codex".to_string()),
            vox_db::SearchIntent::FactualLookup => Some("retry_hybrid".to_string()),
            vox_db::SearchIntent::Verification => Some("retry_hybrid".to_string()),
        };

        crate::socrates::SocratesTaskContext {
            risk_budget: "normal".to_string(),
            factual_mode: true,
            required_citations: 1,
            evidence_count: 0,
            contradiction_hints: 0,
            retrieval_tier: Some("none".to_string()),
            retrieval_used_vector: false,
            retrieval_used_lexical_fallback: false,
            source_diversity: 0,
            evidence_quality: 0.0,
            citation_coverage: 0.0,
            verification_performed: false,
            verification_reason: None,
            recommended_next_action,
            retrieval_diagnosis: None,
            fatigue_active: false,
            orient_report: None,
            answered_questions: vec![],
            research_model_enabled: false,
            fabricated_tool_claims: None,
        }
    }

    /// Goal intake retrieval: run shared `vox-search` when `VoxDb` is attached, else heuristic hints.
    pub(crate) async fn generate_goal_search_context(
        &self,
        description: &str,
        file_manifest: &[FileAffinity],
    ) -> crate::socrates::SocratesTaskContext {
        if self.db().is_none() {
            return self.generate_goal_search_heuristic_only(description);
        }

        let band =
            vox_orchestrator_types::socrates_policy::SocratesComplexityJudge::estimate_complexity(
                description,
                None,
            );
        let mut route = crate::retrieval::crag::CragRouter::evaluate_query(description);
        if let vox_orchestrator_types::socrates_policy::ComplexityBand::Moderate
        | vox_orchestrator_types::socrates_policy::ComplexityBand::Complex
        | vox_orchestrator_types::socrates_policy::ComplexityBand::MultiHop = band
        {
            route = crate::retrieval::crag::CragRoute::WebSearch;
            tracing::info!("Query complexity is Moderate+, forcing autonomous research");
        }

        match route {
            crate::retrieval::crag::CragRoute::InContext => {
                tracing::info!("CRAG routing decided InContext; bypassing vector retrieval.");
                return self.generate_goal_search_heuristic_only(description);
            }
            crate::retrieval::crag::CragRoute::WebSearch => {
                tracing::info!("CRAG routing decided WebSearch; triggering autonomous research.");
                let results = self
                    .perform_autonomous_research(
                        None,
                        None,
                        vec![description.to_string()],
                        "crag_proactive_web_search",
                    )
                    .await
                    .unwrap_or_default();
                if !results.is_empty() {
                    let mut s_ctx = self.generate_goal_search_heuristic_only(description);
                    self.inject_research_results(&mut s_ctx, results);
                    return s_ctx;
                }
            }
            crate::retrieval::crag::CragRoute::Retrieve => {
                tracing::info!("CRAG routing decided Retrieve; delegating to hybrid search.");
            }
        }

        let policy = vox_search::SearchPolicy::from_env();
        let repo_root =
            infer_repo_root_from_manifest(file_manifest).unwrap_or_else(|| PathBuf::from("."));
        let mem_cfg = crate::sync_lock::rw_read(&*self.config).memory.clone();
        let ctx = vox_search::SearchRuntimeContext::new(
            repo_root,
            self.db(),
            mem_cfg.log_dir.clone(),
            mem_cfg.memory_md_path.clone(),
        )
        .with_trace_id(Some(format!(
            "orchestrator-goal-task-{}",
            uuid::Uuid::new_v4()
        )));
        let fallback: Option<Box<dyn vox_search::LexicalMemoryFallback>> = if mem_cfg.enabled {
            Some(Box::new(crate::search_bridge::MemorySubstringFallback(
                mem_cfg.clone(),
            )))
        } else {
            None
        };
        let lex = fallback.as_deref();
        #[cfg_attr(not(feature = "runtime"), allow(unused_mut))]
        let Ok((mut execution, mut diagnostics, plan)) = vox_search::run_search_with_verification(
            &ctx,
            description,
            vox_search::RetrievalTriggerMode::AutoChatPreamble,
            8,
            &policy,
            lex,
            None,
        )
        .await
        else {
            return self.generate_goal_search_heuristic_only(description);
        };

        let mut fused_text = String::new();
        for chunk in &execution.chunk_lines {
            fused_text.push_str(chunk);
            fused_text.push('\n');
        }
        for chunk in &execution.repo_lines {
            fused_text.push_str(chunk);
            fused_text.push('\n');
        }

        if !fused_text.is_empty() {
            #[cfg(feature = "runtime")]
            let maybe_llm_cfg = crate::sync_lock::rw_read(&*self.models).get_llm_config(
                crate::types::TaskCategory::Research,
                2,
                crate::config::CostPreference::Economy,
            );

            #[cfg(feature = "runtime")]
            if let Some(mut llm_cfg) = maybe_llm_cfg {
                llm_cfg.temperature = Some(vox_config::gemini_tuning_temperature().unwrap_or(0.0));
                llm_cfg.top_p = vox_config::gemini_tuning_top_p();
                let rel = crate::retrieval::crag::CragRouter::evaluate_document_relevance(
                    description,
                    &fused_text,
                    |sys, user| {
                        let sys_msg = vox_actor_runtime::llm::LlmChatMessage {
                            role: "system".into(),
                            content: sys.into(),
                        };
                        let user_msg = vox_actor_runtime::llm::LlmChatMessage {
                            role: "user".into(),
                            content: user.into(),
                        };
                        let cfg = llm_cfg.clone();
                        async move {
                            let opts =
                                vox_actor_runtime::ActivityOptions::new().with_timeout_secs(10);
                            match vox_actor_runtime::llm::llm_chat(
                                &opts,
                                vec![sys_msg, user_msg],
                                cfg,
                            )
                            .await
                            {
                                vox_actor_runtime::ActivityResult::Ok(Ok(res)) => Ok(res.content),
                                vox_actor_runtime::ActivityResult::Ok(Err(e)) => Err(e),
                                _ => Err("activity_failed".to_string()),
                            }
                        }
                    },
                )
                .await;

                if rel == crate::retrieval::crag::DocumentRelevance::Irrelevant {
                    tracing::info!(
                        "CRAG Evaluator marked local context as IRRELEVANT. Clearing poor evidence."
                    );
                    execution.evidence_quality = 0.0;
                    execution.chunk_lines.clear();
                    execution.repo_lines.clear();
                    execution.memory_lines.clear();
                    diagnostics
                        .notes
                        .push("crag_eval=irrelevant (evidence stripped)".to_string());
                } else if rel == crate::retrieval::crag::DocumentRelevance::Ambiguous {
                    tracing::debug!("CRAG Evaluator marked local context as AMBIGUOUS.");
                    execution.evidence_quality *= 0.5;
                    diagnostics
                        .notes
                        .push("crag_eval=ambiguous (quality halved)".to_string());
                } else {
                    tracing::debug!("CRAG Evaluator marked local context as RELEVANT.");
                    diagnostics.notes.push("crag_eval=relevant".to_string());
                }
            }
        }

        socrates_task_from_search_pass(&execution, &diagnostics, &plan, &policy)
    }

    pub(crate) async fn attach_goal_search_context_with_retrieval(
        &self,
        task_id: TaskId,
        description: &str,
        file_manifest: &[FileAffinity],
    ) {
        let ctx = self
            .generate_goal_search_context(description, file_manifest)
            .await;
        if let Err(e) = self.attach_socrates_context(task_id, ctx) {
            tracing::debug!(
                task_id = task_id.0,
                error = %e,
                "goal search context attach failed"
            );
        }
    }

    /// Submit a higher-level goal that may be routed through planning.
    pub async fn submit_goal(
        &self,
        goal: impl Into<String>,
        file_manifest: Vec<FileAffinity>,
        priority: Option<TaskPriority>,
        planning_mode: Option<PlanningMode>,
        session_id: Option<String>,
        enqueue_hints: Option<crate::types::TaskEnqueueHints>,
        tenant_id: Option<String>,
    ) -> Result<TaskId, OrchestratorError> {
        let goal = goal.into();
        let cfg = crate::sync_lock::rw_read(&*self.config).clone();
        if planning_mode.is_none()
            && (!cfg.planning_auto_mode_enabled || cfg.planning_rollout_percent == 0)
        {
            return self
                .submit_task_with_agent(
                    goal,
                    file_manifest,
                    priority,
                    None,
                    None,
                    enqueue_hints.clone(),
                    session_id,
                    tenant_id,
                )
                .await;
        }
        if planning_mode.is_none() {
            let selector = xxhash_rust::xxh3::xxh3_64(goal.as_bytes()) % 100;
            if selector >= u64::from(cfg.planning_rollout_percent) {
                return self
                    .submit_task_with_agent(
                        goal,
                        file_manifest,
                        priority,
                        None,
                        None,
                        enqueue_hints.clone(),
                        session_id,
                        tenant_id,
                    )
                    .await;
            }
        }
        let eval = crate::planning::intake_router::evaluate_goal(&cfg, &goal, planning_mode);
        self.event_bus
            .emit(crate::events::AgentEventKind::PlanningRouted {
                strategy: format!("{:?}", eval.strategy),
                complexity: eval.complexity,
                confidence: eval.confidence,
                rationale: eval.rationale.clone(),
            });

        if cfg.planning_shadow_mode || eval.strategy == PlanningStrategy::ImmediateAct {
            return self
                .submit_task_with_agent(
                    goal,
                    file_manifest,
                    priority,
                    None,
                    None,
                    enqueue_hints.clone(),
                    session_id,
                    tenant_id,
                )
                .await;
        }

        if eval.strategy == PlanningStrategy::WorkflowHandoff
            && cfg.planning_workflow_handoff_enabled
        {
            return self
                .submit_workflow_handoff_goal(
                    goal,
                    file_manifest,
                    priority,
                    session_id,
                    enqueue_hints,
                    tenant_id,
                )
                .await;
        }

        let socrates_ctx = self
            .generate_goal_search_context(&goal, &file_manifest)
            .await;

        let _cost_ms = {
            let bm = crate::sync_lock::rw_read(&*self.budget_manager);
            if bm.attention_snapshot().spent_ratio() > 1.0 {
                return Err(OrchestratorError::ApprovalBlocked(
                    "Planning rejected: Attention budget is completely exhausted. Please review notifications or reset budget.".to_string()
                ));
            }
            let action = crate::attention::ActionDescriptor {
                estimated_complexity: eval.complexity as u8,
                tokens_output: 0,
                priority: priority.unwrap_or(crate::types::TaskPriority::Normal),
                write_file_count: file_manifest.len().max(1),
                external: false,
                repeated_approve_count: 0,
                concurrent_tasks: 0,
            };
            let base = cfg.attention_interrupt_cost_ms.max(1);
            let cost_ms = crate::attention::compute_attention_cost_ms(
                &action,
                0.5,
                base,
                &cfg.attention_tlx_weights,
            );
            bm.add_questioning_attention_debit_ms(cost_ms);
            cost_ms
        };

        let plan_session_id = format!("plan-{}", uuid::Uuid::new_v4());
        let plan_version = 1_u32;
        let mut nodes = if cfg.planning_llm_synthesis_enabled {
            #[cfg(feature = "runtime")]
            {
                let maybe_llm_cfg = crate::sync_lock::rw_read(&*self.models).get_llm_config(
                    crate::types::TaskCategory::Planning,
                    2,
                    crate::config::CostPreference::Performance,
                );
                if let Some(mut llm_cfg) = maybe_llm_cfg {
                    llm_cfg.temperature =
                        Some(vox_config::gemini_tuning_temperature().unwrap_or(0.2));
                    llm_cfg.top_p = vox_config::gemini_tuning_top_p();
                    let depth_str = format!("{:?}", cfg.planning_depth);
                    crate::planning::synthesizer::synthesize_plan_nodes_with_llm(
                        &goal,
                        &depth_str,
                        |sys, user| {
                            let sys_msg = vox_actor_runtime::llm::LlmChatMessage {
                                role: "system".into(),
                                content: sys.into(),
                            };
                            let user_msg = vox_actor_runtime::llm::LlmChatMessage {
                                role: "user".into(),
                                content: user.into(),
                            };
                            let cfg_clone = llm_cfg.clone();
                            async move {
                                let opts =
                                    vox_actor_runtime::ActivityOptions::new().with_timeout_secs(45);
                                match vox_actor_runtime::llm::llm_chat(
                                    &opts,
                                    vec![sys_msg, user_msg],
                                    cfg_clone,
                                )
                                .await
                                {
                                    vox_actor_runtime::ActivityResult::Ok(Ok(res)) => {
                                        Ok(res.content)
                                    }
                                    vox_actor_runtime::ActivityResult::Ok(Err(e)) => Err(e),
                                    _ => Err("activity_failed".to_string()),
                                }
                            }
                        },
                    )
                    .await
                } else {
                    crate::planning::synthesizer::synthesize_plan_nodes(&goal)
                }
            }
            #[cfg(not(feature = "runtime"))]
            {
                crate::planning::synthesizer::synthesize_plan_nodes(&goal)
            }
        } else {
            crate::planning::synthesizer::synthesize_plan_nodes(&goal)
        };
        let cfg_research = cfg.research_model_enabled;
        let socrates_ctx_clone = socrates_ctx.clone();
        for n in &mut nodes {
            let mut h = enqueue_hints.clone().unwrap_or_default();
            let mut soc = socrates_ctx_clone.clone();
            if cfg_research {
                soc.research_model_enabled = true;
            }
            h.socrates_context = Some(soc);
            n.execution_policy.enqueue_hints = Some(h);
            if !file_manifest.is_empty() {
                merge_file_affinities(&mut n.execution_policy.file_manifest, &file_manifest);
            }
        }
        crate::planning::quality_gate::validate_plan_nodes(&nodes)?;
        let adeq_tasks = crate::planning::plan_nodes_to_adequacy_tasks(&nodes);
        let adeq_report = crate::planning::analyze_plan_refinement_report(
            goal.as_str(),
            file_manifest.len(),
            Some(eval.complexity),
            0,
            &adeq_tasks,
            socrates_ctx.fatigue_active,
        );
        if adeq_report.adequacy.is_too_thin {
            if cfg.plan_adequacy_enforce {
                return Err(OrchestratorError::ScopeDenied(format!(
                    "Plan adequacy gate: synthesized plan is too thin for this goal (score {:.2}, reasons {:?}). \
                     Broaden the goal, add scoped steps, or disable VOX_ORCHESTRATOR_PLAN_ADEQUACY_ENFORCE.",
                    adeq_report.adequacy.score, adeq_report.adequacy.reason_codes
                )));
            }
            if cfg.plan_adequacy_shadow {
                tracing::info!(
                    target = "vox_orchestrator::plan_adequacy",
                    plan_session_id = %plan_session_id,
                    score = adeq_report.adequacy.score,
                    reasons = ?adeq_report.adequacy.reason_codes,
                    "orchestrator-native plan adequacy: thin plan detected (shadow telemetry)"
                );
            } else {
                tracing::warn!(
                    target = "vox_orchestrator::plan_adequacy",
                    plan_session_id = %plan_session_id,
                    score = adeq_report.adequacy.score,
                    reasons = ?adeq_report.adequacy.reason_codes,
                    "orchestrator-native plan adequacy: thin plan detected (elevated signal; enqueue not blocked)"
                );
            }
        }
        let db_opt = self.db();
        if let Some(db) = db_opt.as_ref() {
            let strategy = format!("{:?}", eval.strategy);
            let _ = db
                .create_plan_session(&plan_session_id, session_id.as_deref(), &goal, &strategy)
                .await;
            let _ = db
                .append_plan_version(&plan_session_id, plan_version as i64, None, None, None)
                .await;
            for n in &nodes {
                let deps_json =
                    serde_json::to_string(&n.depends_on).unwrap_or_else(|_| "[]".to_string());
                let pol_json =
                    serde_json::to_string(&n.execution_policy).unwrap_or_else(|_| "{}".to_string());
                let _ = db
                    .upsert_plan_node(
                        &plan_session_id,
                        plan_version as i64,
                        &n.node_id,
                        &n.description,
                        &deps_json,
                        &pol_json,
                        "pending",
                        n.workflow_invocation.as_deref(),
                    )
                    .await;
            }
        }
        self.event_bus
            .emit(crate::events::AgentEventKind::PlanSessionCreated {
                plan_session_id: plan_session_id.clone(),
                strategy: format!("{:?}", eval.strategy),
                version: plan_version as i64,
            });

        if crate::lineage::orchestration_lineage_persist_enabled() {
            if let Some(db) = self.db() {
                let repo = crate::lineage::repository_id();
                let mut payload = serde_json::json!({
                    "strategy": format!("{:?}", eval.strategy),
                    "plan_version": plan_version,
                    "node_count": nodes.len(),
                    "goal_preview": goal.chars().take(240).collect::<String>(),
                    "plan_adequacy": {
                        "score": adeq_report.adequacy.score,
                        "is_too_thin": adeq_report.adequacy.is_too_thin,
                        "reason_codes": adeq_report.adequacy.reason_codes,
                        "detail_target_min_tasks": adeq_report.adequacy.detail_target_min_tasks,
                        "estimated_goal_complexity": adeq_report.adequacy.estimated_goal_complexity,
                        "aggregate_unresolved_risk": adeq_report.aggregate_unresolved_risk,
                        "plan_adequacy_shadow": cfg.plan_adequacy_shadow,
                        "plan_adequacy_enforce": cfg.plan_adequacy_enforce,
                    },
                });
                if let Some(cid) = crate::lineage::orchestration_campaign_id() {
                    payload["campaign_id"] = serde_json::Value::String(cid);
                }
                let payload_str = payload.to_string();
                let _ = db
                    .append_orchestration_lineage_event(
                        &repo,
                        "plan_session_created",
                        0_i64,
                        None,
                        session_id.as_deref(),
                        None,
                        Some(plan_session_id.as_str()),
                        None,
                        Some(payload_str.as_str()),
                    )
                    .await;
            }
        }

        if db_opt.is_some() {
            let enqueued = crate::planning::schedule::enqueue_runnable_plan_nodes(
                self,
                &plan_session_id,
                plan_version,
                session_id.clone(),
                tenant_id.clone(),
            )
            .await?;
            return enqueued.into_iter().next().ok_or_else(|| {
                OrchestratorError::DatabaseError(
                    "planning produced no initial runnable nodes".into(),
                )
            });
        }

        let first = nodes
            .first()
            .cloned()
            .unwrap_or_else(|| crate::planning::PlanNode {
                node_id: "n1".to_string(),
                description: goal.clone(),
                depends_on: vec![],
                status: crate::planning::PlanStatus::Pending,
                execution_policy: crate::planning::ExecutionPolicy::default(),
                workflow_invocation: None,
            });
        crate::planning::executor_bridge::enqueue_plan_node(
            self,
            &first,
            &plan_session_id,
            plan_version,
            session_id,
            tenant_id,
        )
        .await
    }
}
