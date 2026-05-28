//! Run manifests and architecture params for checkpoints/serve validation.
//!
//! Ported from `vox-populi/src/mens/tensor/manifest/` (SP3 sub-batch C).
//! Consolidates mod.rs + part_build.rs + part_persist.rs + part_io.rs into one file.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const TRAINING_MANIFEST_SCHEMA_VERSION: u32 = 5;

// Default constants (from vox-populi's lora/part_vox.rs)
const DEFAULT_D_MODEL: usize = 512;
const DEFAULT_N_HEADS: usize = 8;
const DEFAULT_N_LAYERS: usize = 6;

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
            vocab_size: vox_tensor::data::VOCAB_SIZE,
            d_model: DEFAULT_D_MODEL,
            n_heads: DEFAULT_N_HEADS,
            n_layers: DEFAULT_N_LAYERS,
        }
    }
}

/// On-disk training manifest (written by training loop).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingManifest {
    pub vocab_size: usize,
    pub d_model: usize,
    pub n_heads: usize,
    pub n_layers: usize,
    pub base_model: Option<String>,
    pub tokenizer_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance_base_family: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance_upstream_model_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance_license_class: Option<String>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adapter_tag: Option<String>,
    #[serde(default)]
    pub seed: u64,
    #[serde(default)]
    pub grad_accum: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_filter: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_vram_fraction: Option<f32>,
    #[serde(default = "default_manifest_schema_v1")]
    pub manifest_schema_version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_kernel: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finetune_contract_digest: Option<String>,
    #[serde(default)]
    pub candle_qlora_training_steps_executed: u64,
    #[serde(default)]
    pub candle_qlora_skips_bad_vocab: u64,
    #[serde(default)]
    pub candle_qlora_skips_last_hidden: u64,
    #[serde(default)]
    pub candle_qlora_skips_short_seq: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candle_qlora_proxy_stack_complete: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candle_qlora_graph_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candle_qlora_middle_layers_active: Option<usize>,
    #[serde(default = "default_candle_qlora_ce_last_k")]
    pub candle_qlora_ce_last_k: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candle_qlora_architecture: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candle_qlora_linear_layers: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candle_qlora_full_layers: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub training_objective_note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub training_deployment_target: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub training_deployment_note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eval_baseline_delta_note: Option<String>,
    #[serde(default)]
    pub trajectory_weighting_enabled: bool,
    #[serde(default = "default_trajectory_tool_trace_boost")]
    pub trajectory_tool_trace_boost: f32,
    #[serde(default = "default_trajectory_failure_category_boost")]
    pub trajectory_failure_category_boost: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trajectory_quality_floor: Option<u8>,
    #[serde(default = "default_trajectory_quality_boost")]
    pub trajectory_quality_boost: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contamination_score: Option<f32>,
}

fn default_manifest_schema_v1() -> u32 {
    1
}
fn default_candle_qlora_ce_last_k() -> usize {
    64
}
fn default_trajectory_tool_trace_boost() -> f32 {
    1.1
}
fn default_trajectory_failure_category_boost() -> f32 {
    1.15
}
fn default_trajectory_quality_boost() -> f32 {
    1.05
}

/// Snapshot of run settings for [`initial_training_manifest`].
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
    pub training_deployment_target: Option<String>,
    pub training_deployment_note: Option<String>,
    pub trajectory_weighting_enabled: bool,
    pub trajectory_tool_trace_boost: f32,
    pub trajectory_failure_category_boost: f32,
    pub trajectory_quality_floor: Option<u8>,
    pub trajectory_quality_boost: f32,
    pub contamination_score: Option<f32>,
}

impl InitialManifestRun {
    #[must_use]
    pub fn from_lora_config(c: &crate::config::LoraTrainingConfig) -> Self {
        use crate::config::TrainingDeploymentTarget;
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
                == TrainingDeploymentTarget::MobileEdge)
                .then(|| c.deployment_target.as_str().to_string()),
            training_deployment_note: (c.deployment_target == TrainingDeploymentTarget::MobileEdge)
                .then(|| crate::operator_messages::MOBILE_EDGE_TRAINING_MANIFEST_NOTE.to_string()),
            trajectory_weighting_enabled: c.trajectory_weighting_enabled,
            trajectory_tool_trace_boost: c.trajectory_tool_trace_boost,
            trajectory_failure_category_boost: c.trajectory_failure_category_boost,
            trajectory_quality_floor: c.trajectory_quality_floor,
            trajectory_quality_boost: c.trajectory_quality_boost,
            contamination_score: None,
        }
    }
}

/// Which native kernel is writing the manifest (kernel-specific notes / Candle fields).
#[derive(Debug, Clone)]
pub enum InitialTrainingKernel {
    CandleQlora {
        proxy_stack_complete: bool,
        middle_layers_active: usize,
        ce_last_k: usize,
        architecture: String,
        linear_layers: Option<usize>,
        full_layers: Option<usize>,
    },
}

pub fn initial_training_manifest(
    arch: ArchParams,
    train_file: impl Into<String>,
    run: InitialManifestRun,
    tokenizer_path: Option<String>,
    kernel: InitialTrainingKernel,
) -> TrainingManifest {
    let (
        execution_kernel,
        candle_proxy,
        candle_graph_id,
        candle_middle_active,
        candle_ce_k,
        candle_arch,
        candle_linear_layers,
        candle_full_layers,
        objective,
    ) = match kernel {
        InitialTrainingKernel::CandleQlora {
            proxy_stack_complete,
            middle_layers_active,
            ce_last_k,
            architecture,
            linear_layers,
            full_layers,
        } => {
            let k = ce_last_k;
            let graph_id = "full_graph_v1";
            let obj = if k == 0 {
                "candle_qlora_full_graph_full_assistant_ce".to_string()
            } else {
                format!("candle_qlora_full_graph_k{k}")
            };
            (
                Some("candle_qlora".into()),
                Some(proxy_stack_complete),
                Some(graph_id.to_string()),
                Some(middle_layers_active),
                k,
                Some(architecture),
                linear_layers,
                full_layers,
                Some(obj),
            )
        }
    };

    TrainingManifest {
        vocab_size: arch.vocab_size,
        d_model: arch.d_model,
        n_heads: arch.n_heads,
        n_layers: arch.n_layers,
        base_model: run.base_model,
        tokenizer_path,
        provenance_base_family: run.provenance_base_family,
        provenance_upstream_model_id: run.provenance_upstream_model_id,
        provenance_license_class: run.provenance_license_class,
        provenance_attribution_required: run.provenance_attribution_required,
        train_file: train_file.into(),
        rank: run.rank,
        alpha: run.alpha,
        seq_len: run.seq_len,
        epochs: run.epochs,
        run_id: run.run_id,
        git_sha: run.git_sha,
        device_profile: run.device_profile,
        adapter_tag: run.adapter_tag,
        seed: run.seed,
        grad_accum: run.grad_accum,
        context_filter: run.context_filter,
        max_vram_fraction: run.max_vram_fraction,
        manifest_schema_version: TRAINING_MANIFEST_SCHEMA_VERSION,
        execution_kernel,
        finetune_contract_digest: run.finetune_contract_digest,
        candle_qlora_training_steps_executed: 0,
        candle_qlora_skips_bad_vocab: 0,
        candle_qlora_skips_last_hidden: 0,
        candle_qlora_skips_short_seq: 0,
        candle_qlora_proxy_stack_complete: candle_proxy,
        candle_qlora_graph_id: candle_graph_id,
        candle_qlora_middle_layers_active: candle_middle_active,
        candle_qlora_ce_last_k: candle_ce_k,
        candle_qlora_architecture: candle_arch,
        candle_qlora_linear_layers: candle_linear_layers,
        candle_qlora_full_layers: candle_full_layers,
        training_objective_note: objective,
        training_deployment_target: run.training_deployment_target,
        training_deployment_note: run.training_deployment_note,
        eval_baseline_delta_note: None,
        trajectory_weighting_enabled: run.trajectory_weighting_enabled,
        trajectory_tool_trace_boost: run.trajectory_tool_trace_boost,
        trajectory_failure_category_boost: run.trajectory_failure_category_boost,
        trajectory_quality_floor: run.trajectory_quality_floor,
        trajectory_quality_boost: run.trajectory_quality_boost,
        contamination_score: run.contamination_score,
    }
}

pub struct ManifestWriteResult {
    pub manifest_path: PathBuf,
}

pub fn write_training_manifest(
    out: &Path,
    m: TrainingManifest,
) -> anyhow::Result<ManifestWriteResult> {
    let p = out.join("training_manifest.json");
    std::fs::write(&p, serde_json::to_string_pretty(&m)?)?;
    Ok(ManifestWriteResult { manifest_path: p })
}

/// Merge Candle QLoRA run statistics into `training_manifest.json` after training.
pub fn finalize_candle_qlora_training_manifest(
    out: &Path,
    steps_executed: u64,
    skips_bad_vocab: u64,
    skips_last_hidden: u64,
    skips_short_seq: u64,
    proxy_stack_complete: bool,
) -> anyhow::Result<()> {
    let p = out.join("training_manifest.json");
    if !p.is_file() {
        anyhow::bail!("missing training manifest at {}", p.display());
    }
    let raw = std::fs::read_to_string(&p)?;
    let mut m: TrainingManifest = serde_json::from_str(&raw)?;
    m.manifest_schema_version = TRAINING_MANIFEST_SCHEMA_VERSION;
    m.candle_qlora_training_steps_executed = steps_executed;
    m.candle_qlora_skips_bad_vocab = skips_bad_vocab;
    m.candle_qlora_skips_last_hidden = skips_last_hidden;
    m.candle_qlora_skips_short_seq = skips_short_seq;
    m.candle_qlora_proxy_stack_complete = Some(proxy_stack_complete);
    m.eval_baseline_delta_note = Some("not_computed_in_tree_run_separate_eval_jsonl".to_string());
    std::fs::write(&p, serde_json::to_string_pretty(&m)?)?;
    Ok(())
}
