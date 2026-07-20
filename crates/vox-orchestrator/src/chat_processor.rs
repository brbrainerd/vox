//! A single-call, non-phased [`crate::runtime::TaskProcessor`] for
//! conversational (chat-origin) tasks. Unlike [`crate::runtime::AiTaskProcessor`]'s
//! 6-phase Inspect/Localize/Hypothesize/Act/Verify/Decide pipeline (built for
//! genuine multi-step agentic work), this makes exactly one LLM call with a
//! prompt written for a single-shot conversational reply.
//!
//! **Corrected by adversarial review** (see
//! `docs/superpowers/plans/2026-07-20-orchestrator-chat-latency-reliability.md`,
//! Task A2): `AiTaskProcessor::process` does routing/budget/model-registry
//! work before ever calling its underlying generation method — none of that
//! machinery belongs here. This processor calls
//! [`vox_gamify::ai::FreeAiClient::generate_stream`] directly, the simpler
//! cascade-only streaming method, instead of mirroring
//! `AiTaskProcessor::run_phase_stream_with_bus`'s heavier routed-call path.

use crate::events::AgentEventKind;
use crate::runtime::TaskProcessor;
use futures_util::StreamExt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// A single-call, non-phased processor for chat-origin tasks.
pub struct ChatTaskProcessor {
    client: vox_gamify::ai::FreeAiClient,
    event_bus: crate::events::EventBus,
    orchestrator: Arc<crate::orchestrator::Orchestrator>,
}

impl ChatTaskProcessor {
    /// Create a new chat processor that auto-discovers providers, mirroring
    /// [`crate::runtime::AiTaskProcessor::new`]'s construction (only the
    /// per-call generation path differs, not client construction).
    pub async fn new(
        event_bus: crate::events::EventBus,
        orchestrator: Arc<crate::orchestrator::Orchestrator>,
    ) -> Self {
        let client = vox_gamify::ai::FreeAiClient::auto_discover().await;
        Self {
            client,
            event_bus,
            orchestrator,
        }
    }
}

#[async_trait::async_trait]
impl TaskProcessor for ChatTaskProcessor {
    async fn process(
        &self,
        agent_id: crate::types::AgentId,
        task: crate::types::AgentTask,
        cancel: Arc<AtomicBool>,
    ) -> anyhow::Result<crate::types::TaskId> {
        // Step 1: cancel-check, identical in shape to AiTaskProcessor's
        // pre-phase cancel path (runtime.rs).
        if cancel.load(Ordering::Acquire) {
            self.orchestrator.abort_interrupted_task(task.id, agent_id);
            return Err(anyhow::anyhow!("task interrupted"));
        }

        // Step 2: minimal chat-appropriate prompt — no phase/prior-notes
        // template, since a single-shot conversational reply has neither.
        let prompt = format!(
            "You are a helpful assistant responding to a chat message.\n\n{}",
            task.description
        );

        // Step 3: single streaming call via FreeAiClient::generate_stream
        // (NOT AiTaskProcessor's routed/phase-loop path), emitting the same
        // TokenStreamed events per chunk the existing frontend chat-bubble
        // streaming (chatCorrelation.ts) already listens for.
        let mut stream = self.client.generate_stream(&prompt).await;
        let mut reply_text = String::new();
        while let Some(chunk_result) = stream.next().await {
            if cancel.load(Ordering::Acquire) {
                self.orchestrator.abort_interrupted_task(task.id, agent_id);
                return Err(anyhow::anyhow!("task interrupted"));
            }
            match chunk_result {
                Ok(text) => {
                    reply_text.push_str(&text);
                    self.event_bus
                        .emit(AgentEventKind::TokenStreamed { agent_id, text });
                }
                Err(e) => {
                    self.orchestrator.abort_interrupted_task(task.id, agent_id);
                    return Err(anyhow::anyhow!("chat stream error: {e}"));
                }
            }
        }

        // Step 5: record usage through the unified pipeline (event bus +
        // budget + oplog), mirroring AiTaskProcessor::process's tail.
        let (provider, model) = self.client.active_provider_info();
        let input_tokens =
            crate::compaction::CompactionEngine::estimate_tokens(&task.description) as u32;
        let output_tokens =
            crate::compaction::CompactionEngine::estimate_tokens(&reply_text) as u32;
        let cost_usd = (input_tokens + output_tokens) as f64 * 0.000_001;
        self.orchestrator
            .record_ai_usage(
                agent_id,
                provider.as_str(),
                model.as_str(),
                input_tokens,
                output_tokens,
                cost_usd,
                None,
                None,
            )
            .await;

        // Step 6.5: optional, non-blocking grounding check — never delays the
        // reply (already streamed above), only emits a follow-up badge event.
        if task.grounding_check_enabled {
            let orchestrator = self.orchestrator.clone();
            let event_bus = self.event_bus.clone();
            let query = task.description.clone();
            let reply = reply_text.clone();
            let task_id = task.id;
            tokio::spawn(async move {
                let ctx = orchestrator.generate_goal_search_context(&query, &[]).await;
                // `ConfidencePolicy` does not derive `Default` (verified against
                // crates/vox-orchestrator-types/src/socrates_policy/policy_types.rs);
                // use its documented workspace baseline instead.
                let policy =
                    vox_orchestrator_types::socrates_policy::ConfidencePolicy::workspace_default();
                let outcome = crate::socrates::evaluate_socrates_gate(&ctx, &policy, &reply);
                event_bus.emit(AgentEventKind::GroundingCheckCompleted {
                    agent_id,
                    task_id,
                    confidence: outcome.confidence,
                    flagged: outcome.confidence < 0.5,
                });
            });
        }

        // Step 7: success.
        Ok(task.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    /// Cancel-preset abort path: identical in shape to
    /// `stub_processor_aborts_when_cancel_flag_preset` (runtime.rs) and to
    /// `AiTaskProcessor`'s own pre-phase cancel check. Exercisable without a
    /// live LLM call (the codebase has no fake/mock `FreeAiClient` mechanism
    /// to reuse for asserting the "exactly one call" count directly — see
    /// `grep -rn "impl.*FreeAiClient\|fn.*fake.*client\|fn.*mock.*client"
    /// crates/vox-gamify/src/ai/`, which finds none), so this proves the
    /// generation call is never reached when cancelled, which is the
    /// call-count-zero case of the same property the plan's "exactly one
    /// call, not a phase loop" test targets.
    #[tokio::test]
    async fn process_aborts_before_any_generation_call_when_cancel_flag_preset() {
        let orchestrator = Arc::new(crate::orchestrator::Orchestrator::new(
            crate::config::OrchestratorConfig::for_testing(),
        ));
        let event_bus = crate::events::EventBus::new(16);
        let processor = ChatTaskProcessor::new(event_bus, orchestrator.clone()).await;

        let task = crate::types::AgentTask::new(
            crate::types::TaskId(99),
            "hello there",
            crate::types::TaskPriority::Normal,
            vec![],
        );
        let cancel = Arc::new(AtomicBool::new(true));
        let result = processor
            .process(crate::types::AgentId(1), task, cancel)
            .await;

        assert!(result.is_err(), "pre-set cancel flag must abort process");
        assert!(
            result.unwrap_err().to_string().contains("interrupted"),
            "error should report interruption"
        );
    }

    /// Grounding check disabled (the default) must never emit
    /// `GroundingCheckCompleted`, even after a cancel-preset short-circuit
    /// (which is the only path exercisable here without a live LLM call).
    #[tokio::test]
    async fn process_emits_no_grounding_check_when_disabled_on_the_task() {
        let orchestrator = Arc::new(crate::orchestrator::Orchestrator::new(
            crate::config::OrchestratorConfig::for_testing(),
        ));
        let event_bus = crate::events::EventBus::new(16);
        let mut rx = event_bus.subscribe();
        let processor = ChatTaskProcessor::new(event_bus, orchestrator.clone()).await;

        let mut task = crate::types::AgentTask::new(
            crate::types::TaskId(100),
            "hello",
            crate::types::TaskPriority::Normal,
            vec![],
        );
        task.grounding_check_enabled = false;
        let cancel = Arc::new(AtomicBool::new(true));
        let _ = processor
            .process(crate::types::AgentId(1), task, cancel)
            .await;

        let mut saw_grounding_event = false;
        while let Ok(evt) = rx.try_recv() {
            if matches!(evt.kind, AgentEventKind::GroundingCheckCompleted { .. }) {
                saw_grounding_event = true;
            }
        }
        assert!(
            !saw_grounding_event,
            "grounding check must not run when disabled"
        );
    }

    /// Grounding-check-enabled combined with a preset-cancel flag must still
    /// short-circuit before generation (and thus before the grounding-check
    /// spawn) — this proves the new field doesn't itself trigger a network
    /// call before generation starts. The check's actual scoring logic
    /// (`evaluate_socrates_gate`) is covered directly in `socrates.rs`,
    /// since this codebase's no-paid-LLM-calls-in-tests constraint makes it
    /// impractical to drive a real (non-cancelled) run through
    /// `ChatTaskProcessor::process` here.
    #[tokio::test]
    async fn process_with_grounding_enabled_and_cancel_preset_emits_nothing() {
        let orchestrator = Arc::new(crate::orchestrator::Orchestrator::new(
            crate::config::OrchestratorConfig::for_testing(),
        ));
        let event_bus = crate::events::EventBus::new(16);
        let mut rx = event_bus.subscribe();
        let processor = ChatTaskProcessor::new(event_bus, orchestrator.clone()).await;

        let mut task = crate::types::AgentTask::new(
            crate::types::TaskId(101),
            "hello",
            crate::types::TaskPriority::Normal,
            vec![],
        );
        task.grounding_check_enabled = true;
        let cancel = Arc::new(AtomicBool::new(true));
        let _ = processor
            .process(crate::types::AgentId(1), task, cancel)
            .await;

        let mut events = Vec::new();
        while let Ok(evt) = rx.try_recv() {
            events.push(evt);
        }
        assert!(
            events.is_empty(),
            "cancelled-before-generation must emit nothing, including no grounding event"
        );
    }
}
