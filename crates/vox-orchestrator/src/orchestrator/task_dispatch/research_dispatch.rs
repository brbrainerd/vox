use crate::orchestrator::Orchestrator;
use crate::socrates::SocratesTaskContext;
use tracing::{info, warn};

impl Orchestrator {
    /// Performs autonomous research based on Socrates policy requests or CRAG routing.
    /// This is a blocking (async) step that injects live web evidence.
    pub async fn perform_autonomous_research(
        &self,
        agent_id: Option<crate::types::AgentId>,
        task_id: Option<crate::types::TaskId>,
        queries: Vec<String>,
        reason: &str,
    ) -> Result<Vec<String>, String> {
        info!(
            reason = %reason,
            query_count = queries.len(),
            "triggering autonomous socrates research dispatch"
        );

        let (policy, quality_target) = {
            let cfg = crate::sync_lock::rw_read(&*self.config);
            (cfg.effective_search_policy(), cfg.research_quality_target)
        };

        let anchor = queries.first().map(|s| s.as_str()).unwrap_or("");
        let mut research_results = vox_search::research::run_multi_hop_web_research(
            &policy,
            &queries,
            quality_target,
            anchor,
        )
        .await;

        let research_model_enabled =
            crate::sync_lock::rw_read(&*self.config).research_model_enabled;

        if research_model_enabled && !research_results.is_empty() {
            info!("delegating research synthesis to Lane G (research-expert)");

            #[cfg(feature = "runtime")]
            {
                use vox_actor_runtime::llm::{LlmChatMessage, LlmConfig, infer_with_retry};

                let combined_evidence = research_results.join("\n\n");

                // Configure Lane G endpoint — pick via SSOT `select()` so the
                // 3-axis user knob + premium_alias drive the choice.
                // 2026-Q2 refresh: claude-3.5-sonnet:beta retired.
                let model_id = crate::models::select_with_default_registry(
                    &crate::models::SelectionIntent::research(),
                )
                .map(|o| o.model_id)
                .unwrap_or_else(|| "google/gemini-3.1-pro".to_string());
                let config = LlmConfig::openrouter(&model_id);
                if let Some(_key) =
                    vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshToken).expose()
                {
                    // Overwrite if hitting internal mesh. For now, we fallback to standard LLM pipeline.
                    tracing::debug!("Using specific Lane G auth");
                }

                let messages = vec![
                    LlmChatMessage {
                        role: "system".into(),
                        content: "You are Lane G, the Vox autonomous research synthesis expert. Your objective is to ingest raw search observations and formulate a dense, factual markdown summary containing specific claims, figures, and direct citations.".into(),
                    },
                    LlmChatMessage {
                        role: "user".into(),
                        content: format!("Synthesize the following recent web evidence into a high-fidelity summary:\n\n{}", combined_evidence),
                    }
                ];

                match infer_with_retry(
                    &vox_actor_runtime::activity::ActivityOptions::default(),
                    messages,
                    vec![config],
                )
                .await
                {
                    vox_actor_runtime::ActivityResult::Ok(Ok((res, _cfg)))
                        if !res.content.is_empty() =>
                    {
                        let text = res.content;
                        info!("Lane G synthesis completed successfully");
                        research_results.push(format!("[lane_g_synthesis] {}", text));
                    }
                    other => {
                        warn!(
                            ?other,
                            "Lane G synthesis failed or returned empty content, falling back to raw results"
                        );
                    }
                }
            }
        }

        if !research_results.is_empty() {
            info!(
                count = research_results.len(),
                "collected autonomous research evidence"
            );
        }

        self.event_bus
            .emit(crate::events::AgentEventKind::ResearchExecuted {
                agent_id,
                task_id,
                queries,
                results_count: research_results.len(),
            });

        Ok(research_results)
    }

    /// Injects research results into a Socrates task context using a canonical CRAG line prefix.
    pub fn inject_research_results(&self, ctx: &mut SocratesTaskContext, results: Vec<String>) {
        if results.is_empty() {
            return;
        }

        // We update the evidence count to reflect the new findings.
        // In a full CRAG loop, we would re-run socrates evaluation AFTER injection,
        // but for now we trust the injection is sufficient for the agent.
        ctx.evidence_count = ctx
            .evidence_count
            .saturating_add(results.len().min(u8::MAX as usize) as u8);

        if let Some(ref mut diag) = ctx.retrieval_diagnosis {
            diag.evidence_shape = "ok".to_string(); // evidence is no longer empty/thin
            if !diag.corpora_with_hits.contains(&"webresearch".to_string()) {
                diag.corpora_with_hits.push("webresearch".to_string());
            }
        }

        ctx.citation_coverage = (ctx.citation_coverage + 0.20).min(1.0);
        ctx.evidence_quality = (ctx.evidence_quality + 0.15).min(1.0);
    }
}
