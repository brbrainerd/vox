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

        // Step 6: success.
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
}
