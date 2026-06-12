//! Facade-backed [`Embedder`] used by the CLI for real sub-clustering.
//!
//! All embedding I/O goes through `vox_actor_runtime::llm::llm_embed`; no
//! provider hostnames or SDKs leak in here. See AGENTS.md
//! §Model-Agnostic LLM Boundary. The deterministic test path uses the mock
//! embedders in `cluster.rs`; this one is only constructed by the CLI.

use crate::cluster::Embedder;
use async_trait::async_trait;
use std::time::Duration;

/// Embeds finding rationales through the model-agnostic facade.
///
/// `model` is resolved upstream by the caller (the orchestrator model registry
/// for the CLI). `provider: "auto"` defers vendor selection to the facade.
pub struct LlmEmbedder {
    /// Resolved embedding model id (no vendor hostname).
    pub model: String,
    /// Per-call timeout.
    pub timeout: Duration,
}

impl LlmEmbedder {
    fn config(&self) -> vox_actor_runtime::llm::LlmConfig {
        vox_actor_runtime::llm::LlmConfig {
            provider: "auto".into(),
            model: self.model.clone(),
            cost_per_1k: None,
            base_url: None,
            api_key: None,
            temperature: None,
            top_p: None,
            max_tokens: None,
            response_format: None,
            tools: None,
            tool_choice: None,
            timeout_ms: Some(self.timeout.as_millis() as u64),
            telemetry_session_id: None,
            telemetry_user_id: None,
            telemetry_task_category: Some("CodeEffortJudge".into()),
            telemetry_strength_tag: None,
            telemetry_trace_id: None,
            telemetry_attempt_number: Some(1),
            telemetry_skip_interaction: false,
        }
    }
}

#[async_trait]
impl Embedder for LlmEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, String> {
        let options = vox_actor_runtime::ActivityOptions::default().with_timeout(self.timeout);
        match vox_actor_runtime::llm::llm_embed(&options, text, self.config()).await {
            vox_actor_runtime::ActivityResult::Ok(Ok(v)) => Ok(v),
            vox_actor_runtime::ActivityResult::Ok(Err(e)) => Err(format!("embed error: {e}")),
            vox_actor_runtime::ActivityResult::Failed(e) => Err(format!("activity error: {e:?}")),
            vox_actor_runtime::ActivityResult::Cancelled => Err("activity cancelled".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_uses_auto_provider_and_resolved_model() {
        // No vendor hostname is baked in; the model id is whatever the caller
        // resolved, and provider defers to the facade.
        let e = LlmEmbedder {
            model: "mens/embed-1".into(),
            timeout: Duration::from_secs(30),
        };
        let cfg = e.config();
        assert_eq!(cfg.provider, "auto");
        assert_eq!(cfg.model, "mens/embed-1");
        assert_eq!(cfg.timeout_ms, Some(30_000));
        // Embedding calls never request a chat response format.
        assert!(cfg.response_format.is_none());
    }
}
