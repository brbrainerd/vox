//! Machine-readable serving handoff.
//!
//! Ported verbatim from `vox-populi/src/mens/tensor/external_serving_handoff.rs` (SP3 sub-batch C).

use std::path::Path;

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ExternalServingHandoffV1 {
    pub schema: &'static str,
    pub schema_version: i32,
    pub backend_family: &'static str,
    pub base_model: String,
    pub tokenizer_source: String,
    pub artifact_dir: String,
    pub prompt_format: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adapter_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub openai_base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

impl ExternalServingHandoffV1 {
    const SCHEMA: &'static str = "vox_external_serving_handoff_v1";

    #[must_use]
    pub fn schola_training_run(run_dir: &Path, base_model: &str, adapter_filename: &str) -> Self {
        let tok = run_dir.join("tokenizer.json");
        Self {
            schema: Self::SCHEMA,
            schema_version: 1,
            backend_family: "openai-compatible",
            base_model: base_model.to_string(),
            tokenizer_source: tok.display().to_string(),
            artifact_dir: run_dir.display().to_string(),
            prompt_format: "qwen_chatml_im_start".to_string(),
            adapter_path: Some(adapter_filename.to_string()),
            openai_base_url: None,
            notes: Some(
                "Local: vox-schola serve (OpenAI /v1/chat/completions + Ollama-shaped /api/generate, /api/chat). Set POPULI_URL to http://HOST:PORT and POPULI_MODEL to match --model-name or run directory name."
                    .to_string(),
            ),
        }
    }
}

pub fn write_handoff(out: &Path, handoff: &ExternalServingHandoffV1) -> anyhow::Result<()> {
    let path = out.join("external_serving_handoff_v1.json");
    std::fs::write(path, serde_json::to_string_pretty(handoff)?)?;
    Ok(())
}
