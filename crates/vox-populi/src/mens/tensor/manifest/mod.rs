//! Run manifests and architecture params for checkpoints / serve validation.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use vox_tensor::data::VOCAB_SIZE;

// Legacy Burn scratch-model defaults (used only for ArchParams::default fallback).
// QLoRA on HF models does not use these — see docs/src/reference/mens-training.md.
const DEFAULT_D_MODEL: usize = 512;
const DEFAULT_N_HEADS: usize = 8;
const DEFAULT_N_LAYERS: usize = 6;

/// Bumped when new required semantics appear; readers use [`load_manifest`] (serde defaults).
pub const TRAINING_MANIFEST_SCHEMA_VERSION: u32 = 5;

/// Architecture loaded from disk or defaulted for scratch training.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchParams {
    pub vocab_size: usize,
    pub d_model: usize,
    pub n_heads: usize,
    pub n_layers: usize,
}

impl Default for ArchParams {
    fn default() -> Self {
        Self {
            vocab_size: VOCAB_SIZE,
            d_model: DEFAULT_D_MODEL,
            n_heads: DEFAULT_N_HEADS,
            n_layers: DEFAULT_N_LAYERS,
        }
    }
}

/// Checkpoint flavor for validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckpointKind {
    Lora,
    Merged,
}

/// Parameters passed to [`validate_checkpoint_manifest`].
#[derive(Debug, Clone)]
pub struct ValidateParams {
    pub kind: Option<CheckpointKind>,
    pub vocab_size: usize,
    pub d_model: usize,
    pub n_heads: usize,
    pub n_layers: usize,
}

impl ArchParams {
    pub fn from_manifest(run_dir: &Path) -> anyhow::Result<Self> {
        if let Ok(Some(m)) = load_manifest(run_dir) {
            return Ok(Self {
                vocab_size: m.vocab_size,
                d_model: m.d_model,
                n_heads: m.n_heads,
                n_layers: m.n_layers,
            });
        }
        Ok(Self::default())
    }

    pub fn to_validate_params(&self, kind: Option<CheckpointKind>) -> ValidateParams {
        ValidateParams {
            kind,
            vocab_size: self.vocab_size,
            d_model: self.d_model,
            n_heads: self.n_heads,
            n_layers: self.n_layers,
        }
    }
}

/// On-disk training manifest (written by `run_mens_training`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingManifest {
    pub vocab_size: usize,
    pub d_model: usize,
    pub n_heads: usize,
    pub n_layers: usize,
    pub base_model: Option<String>,
    pub tokenizer_path: Option<String>,
    /// Upstream lineage family label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance_base_family: Option<String>,
    /// Upstream model id used as initialization source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance_upstream_model_id: Option<String>,
    /// License class for attribution/compliance workflows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance_license_class: Option<String>,
    /// Whether downstream artifact publication requires attribution.
    #[serde(default)]
    pub provenance_attribution_required: bool,
    pub train_file: String,
    pub rank: usize,
    pub alpha: f32,
    pub seq_len: usize,
    pub epochs: usize,
    pub run_id: Option<String>,
    pub git_sha: Option<String>,
    pub device_profile: Option<String>,
    /// Optional run label; included in checkpoint filenames when set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adapter_tag: Option<String>,
    /// Training RNG seed (JSONL row order per epoch after shuffle).
    #[serde(default)]
    pub seed: u64,
    /// Optimizer gradient accumulation steps (micro-batches per step).
    #[serde(default)]
    pub grad_accum: usize,
    /// Optional `category` substring filter applied to JSONL rows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_filter: Option<String>,
    /// Hint only: compared against VRAM probe for warnings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_vram_fraction: Option<f32>,
    /// Manifest format generation (default 1 = legacy implicit).
    #[serde(default = "default_manifest_schema_v1")]
    pub manifest_schema_version: u32,
    /// Execution kernel identity at write time (`burn_lora`, `candle_qlora`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_kernel: Option<String>,
    /// Hex digest of the full `super::finetune_contract::FineTuneContract` at plan time (`finetune_contract_digest`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finetune_contract_digest: Option<String>,
    /// Candle QLoRA: optimizer steps that completed in the native training loop.
    #[serde(default)]
    pub candle_qlora_training_steps_executed: u64,
    /// Candle QLoRA: pairs skipped because last token id was out of vocab.
    #[serde(default)]
    pub candle_qlora_skips_bad_vocab: u64,
    /// Candle QLoRA: pairs skipped because last hidden could not be built from embeddings.
    #[serde(default)]
    pub candle_qlora_skips_last_hidden: u64,
    /// Candle QLoRA: pairs skipped because encoded context was shorter than 2 tokens (cannot form a next-token target).
    #[serde(default)]
    pub candle_qlora_skips_short_seq: u64,
    /// Candle QLoRA: whether all expected middle projection keys were present in base shards.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candle_qlora_proxy_stack_complete: Option<bool>,
    /// Candle QLoRA execution graph id (for objective/telemetry compatibility across trainer revisions).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candle_qlora_graph_id: Option<String>,
    /// Candle QLoRA: middle projection layers expected by layout inventory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candle_qlora_middle_layers_active: Option<usize>,
    /// Candle QLoRA: suffix CE — last **K** token positions per row (`1` = last token only).
    #[serde(default = "default_candle_qlora_ce_last_k")]
    pub candle_qlora_ce_last_k: usize,
    /// Candle QLoRA architecture label (`qwen2` / `qwen3_5`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candle_qlora_architecture: Option<String>,
    /// qwen3_5 hybrid layout: count of linear-attention layers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candle_qlora_linear_layers: Option<usize>,
    /// qwen3_5 hybrid layout: count of full-attention layers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candle_qlora_full_layers: Option<usize>,
    /// Training objective hint for operators (kernel-specific).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub training_objective_note: Option<String>,
    /// `workstation` or `mobile_edge` when set (export handoff profile).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub training_deployment_target: Option<String>,
    /// Operator note for edge conversion (see mobile-edge-ai SSOT).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub training_deployment_note: Option<String>,
    /// Reserved: baseline vs adapter delta (populate when eval harness exists).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eval_baseline_delta_note: Option<String>,
    /// Whether trajectory-aware weighting was enabled for this run.
    #[serde(default)]
    pub trajectory_weighting_enabled: bool,
    /// Multiplier for trajectory/tool-trace rows.
    #[serde(default = "default_trajectory_tool_trace_boost")]
    pub trajectory_tool_trace_boost: f32,
    /// Multiplier for failure/error trajectory rows.
    #[serde(default = "default_trajectory_failure_category_boost")]
    pub trajectory_failure_category_boost: f32,
    /// Optional rating floor to apply quality boost.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trajectory_quality_floor: Option<u8>,
    /// Multiplier for rows that meet `trajectory_quality_floor`.
    #[serde(default = "default_trajectory_quality_boost")]
    pub trajectory_quality_boost: f32,
    /// Overall dataset JSX/React contamination score (populated when vox_pure filter is active).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contamination_score: Option<f32>,
}

/// Snapshot of run settings for [`initial_training_manifest`].
///
/// With the `mens-train` feature, build via `InitialManifestRun::from_lora_config`. When
/// `LoraTrainingConfig` gains manifest-relevant fields, update that constructor and this struct together.
#[derive(Debug, Clone)]
pub struct InitialManifestRun {
    pub base_model: Option<String>,
    pub provenance_base_family: Option<String>,
    pub provenance_upstream_model_id: Option<String>,
    pub provenance_license_class: Option<String>,
    pub provenance_attribution_required: bool,
    pub rank: usize,
    pub alpha: f32,
    pub seq_len: usize,
    pub epochs: usize,
    pub run_id: Option<String>,
    pub git_sha: Option<String>,
    pub device_profile: Option<String>,
    pub adapter_tag: Option<String>,
    pub seed: u64,
    pub grad_accum: usize,
    pub context_filter: Option<String>,
    pub max_vram_fraction: Option<f32>,
    pub finetune_contract_digest: Option<String>,
    /// `Some("mobile_edge")` when training for edge export (see SSOT).
    pub training_deployment_target: Option<String>,
    /// Operator note when `training_deployment_target` is set.
    pub training_deployment_note: Option<String>,
    pub trajectory_weighting_enabled: bool,
    pub trajectory_tool_trace_boost: f32,
    pub trajectory_failure_category_boost: f32,
    pub trajectory_quality_floor: Option<u8>,
    pub trajectory_quality_boost: f32,
    pub contamination_score: Option<f32>,
}

#[cfg(feature = "mens-train")]
impl InitialManifestRun {
    #[must_use]
    pub fn from_lora_config(c: &super::training_config::LoraTrainingConfig) -> Self {
        Self {
            base_model: c.base_model.clone(),
            provenance_base_family: c.base_model_family.clone(),
            provenance_upstream_model_id: c.upstream_model_id.clone(),
            provenance_license_class: c.license_class.clone(),
            provenance_attribution_required: c.attribution_required,
            rank: c.rank,
            alpha: c.alpha,
            seq_len: c.seq_len,
            epochs: c.epochs,
            run_id: c.run_id.clone(),
            git_sha: c.git_sha.clone(),
            device_profile: c.device_profile.clone(),
            adapter_tag: c.adapter_tag.clone(),
            seed: c.seed,
            grad_accum: c.grad_accum.max(1),
            context_filter: c
                .context_filter
                .as_ref()
                .map(|cf| serde_json::to_string(cf).unwrap_or_default()),
            max_vram_fraction: c.max_vram_fraction,
            finetune_contract_digest: c.finetune_contract_digest.clone(),
            training_deployment_target: (c.deployment_target
                == super::training_config::TrainingDeploymentTarget::MobileEdge)
                .then(|| c.deployment_target.as_str().to_string()),
            training_deployment_note: (c.deployment_target
                == super::training_config::TrainingDeploymentTarget::MobileEdge)
                .then(|| super::operator_messages::MOBILE_EDGE_TRAINING_MANIFEST_NOTE.to_string()),
            trajectory_weighting_enabled: c.trajectory_weighting_enabled,
            trajectory_tool_trace_boost: c.trajectory_tool_trace_boost,
            trajectory_failure_category_boost: c.trajectory_failure_category_boost,
            trajectory_quality_floor: c.trajectory_quality_floor,
            trajectory_quality_boost: c.trajectory_quality_boost,
            contamination_score: None, // Filled during preflight/training if vox_pure used
        }
    }
}

/// Which native kernel is writing the first `training_manifest.json` (kernel-specific notes / Candle fields).
#[derive(Debug, Clone)]
pub enum InitialTrainingKernel {
    BurnLora,
    /// `proxy_stack_complete` captures preflight key coverage; `ce_last_k` is suffix CE width.
    CandleQlora {
        proxy_stack_complete: bool,
        middle_layers_active: usize,
        ce_last_k: usize,
        architecture: String,
        linear_layers: Option<usize>,
        full_layers: Option<usize>,
    },
}
include!("part_build.rs");
include!("part_persist.rs");
include!("part_io.rs");

#[cfg(test)]
mod tests;
