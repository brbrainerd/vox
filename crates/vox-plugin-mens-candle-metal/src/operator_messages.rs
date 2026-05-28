//! Operator-facing error strings shared by the preflight and training modules.
//!
//! Copied verbatim from `vox-populi/src/mens/tensor/operator_messages.rs` (SP3 sub-batch B).

/// Planner + native QLoRA preflight: tokenizer mode gate.
pub const QLORA_REQUIRES_HF_TOKENIZER: &str = "QLoRA (`--backend qlora`) requires `--tokenizer hf` and a Hugging Face `tokenizer.json` on disk.";

/// QLoRA preflight: missing tokenizer path on config.
pub const QLORA_NEEDS_TOKENIZER_PATH: &str = "QLoRA needs a tokenizer path: pass `--model <hf_repo>` so `tokenizer.json` is downloaded, or set the tokenizer path explicitly.";

/// QLoRA preflight: missing base weights on config.
pub const QLORA_NEEDS_HF_WEIGHTS: &str = "QLoRA needs HF base weights: pass `--model <hf_repo>` to download `config.json` and `*.safetensors`, or set model paths explicitly.";

/// Burn + HF tokenizer: missing path (planner / preflight bundle).
pub const BURN_HF_TOKENIZER_PATH_REQUIRED: &str = "Burn LoRA with `--tokenizer hf` requires a tokenizer path (`--model <hf_repo>` or an explicit tokenizer file).";

/// Burn + HF tokenizer: missing config.
pub const BURN_HF_CONFIG_REQUIRED: &str = "Burn LoRA with `--tokenizer hf` requires `config.json` (`--model <hf_repo>` or cached weights) for architecture validation.";

/// Burn + HF tokenizer: unsupported architecture (non–GPT-2-shaped layout).
pub const BURN_HF_GPT2_ONLY: &str = "Burn LoRA with `--tokenizer hf` supports **GPT-2-shaped** HF `config.json` only. For Llama/Mistral/Qwen-style layouts use `--backend qlora` until Burn HF graph parity lands.";

/// SSOT path referenced in strict QLoRA / training docs (display only).
pub const POPULI_TRAINING_SSOT_RELPATH: &str = "docs/src/architecture/mens-training-ssot.md";

/// SSOT for mobile / edge training handoff (display only).
pub const MOBILE_EDGE_AI_SSOT_RELPATH: &str = "docs/src/architecture/mobile-edge-ai-ssot.md";

/// `training_manifest.json` note when `training_deployment_target` is `mobile_edge`.
pub const MOBILE_EDGE_TRAINING_MANIFEST_NOTE: &str = "mobile_edge: train off-device for LiteRT-LM / Core ML / vendor edge runtimes; conversion from Mens artifacts is operator-owned. See docs/src/architecture/mobile-edge-ai-ssot.md.";

/// CLI: mobile edge profile requires CPU device.
pub const MOBILE_EDGE_REQUIRES_CPU_DEVICE: &str = "`--deployment-target mobile_edge` (or `--preset mobile_edge`) requires `--device cpu` so adapters are not tied to desktop GPU stacks. See docs/src/architecture/mobile-edge-ai-ssot.md.";

/// Planner: `--qlora-require-full-proxy-stack` conflicts with mobile edge export gates.
pub const MOBILE_EDGE_REJECTS_FULL_PROXY_STACK: &str = "mobile_edge deployment target rejects `--qlora-require-full-proxy-stack` in the current full-graph trainer path. Use workstation-targeted training and convert artifacts for edge deployment separately. See docs/src/architecture/mobile-edge-ai-ssot.md.";

/// Planner: sequence length cap for mobile edge.
pub fn mobile_edge_seq_len_cap(got: usize) -> String {
    format!(
        "mobile_edge deployment target requires `--seq-len` <= 512 (got {got}). See docs/src/architecture/mobile-edge-ai-ssot.md."
    )
}

/// Planner: LoRA rank cap for mobile edge.
pub fn mobile_edge_rank_cap(got: usize) -> String {
    format!(
        "mobile_edge deployment target requires `--rank` <= 32 (got {got}). See docs/src/architecture/mobile-edge-ai-ssot.md."
    )
}

/// Planner: batch size for mobile edge.
pub fn mobile_edge_batch_cap(got: usize) -> String {
    format!(
        "mobile_edge deployment target requires `--batch-size` == 1 (got {got}). See docs/src/architecture/mobile-edge-ai-ssot.md."
    )
}

#[must_use]
pub fn tokenizer_not_a_file(path_display: &str) -> String {
    format!(
        "Tokenizer path is not an existing file: {path_display}. Next: fix the path or pass `--model <hf_repo>` so the tokenizer is populated."
    )
}

#[must_use]
pub fn hf_config_missing(path_display: &str) -> String {
    format!(
        "HF config.json is missing or not a file: {path_display}. Next: point `--model` at the same revision as your safetensors."
    )
}

#[must_use]
pub fn no_safetensors_shards() -> String {
    "No safetensors shards are listed for the base model. Next: pass `--model <hf_repo>` or list weight shards explicitly.".to_string()
}
