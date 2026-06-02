//! **FineTuneContract** — semantic source of truth for a Mens fine-tune run (kernel-agnostic).
//!
//! CLI and presets map into this struct; [`super::execution_planner::ExecutionPlanner`] selects an
//! [`super::train_backend::PopuliTrainBackend`] (execution kernel) and validates capability gates.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::training_config::{MensTokenizerMode, TrainingDeploymentTarget};

/// Full contract for one training job.
#[derive(Debug, Clone)]
pub struct FineTuneContract {
    pub model: ModelSpec,
    pub provenance: ModelProvenanceSpec,
    pub data: DataSpec,
    pub adapter: AdapterSpec,
    pub quant: QuantSpec,
    pub exec: ExecSpec,
    pub artifact: ArtifactSpec,
    pub collateral_damage_verified: bool,
}

/// Base model + tokenizer resolution.
#[derive(Debug, Clone)]
pub struct ModelSpec {
    pub hf_repo: Option<String>,
    pub weight_shards: Option<Vec<PathBuf>>,
    pub config_json: Option<PathBuf>,
    pub tokenizer_json: Option<PathBuf>,
}

/// Lineage and attribution metadata for downstream artifacts.
#[derive(Debug, Clone, Default)]
pub struct ModelProvenanceSpec {
    pub base_family: Option<String>,
    pub upstream_model_id: Option<String>,
    pub license_class: Option<String>,
    pub attribution_required: bool,
}

/// Data + tokenization policy.
#[derive(Debug, Clone)]
pub struct DataSpec {
    pub train_file: Option<PathBuf>,
    pub tokenizer_mode: MensTokenizerMode,
    pub min_rating: u8,
    pub context_filter: Option<String>,
}

/// Adapter method and hyperparameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterMethod {
    Lora,
    Qlora,
}

/// Which modules receive low-rank adapters (extensible).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterTargetMask {
    #[default]
    FullGraph,
    LmHeadProxy,
}

#[derive(Debug, Clone)]
pub struct AdapterSpec {
    pub method: AdapterMethod,
    pub rank: usize,
    pub alpha: f32,
    pub dropout: f32,
    pub targets: AdapterTargetMask,
}

/// Base weight quantization (training-time).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BaseQuantMode {
    None,
    Nf4,
}

#[derive(Debug, Clone)]
pub struct QuantSpec {
    pub base: BaseQuantMode,
    pub double_quant: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ExecSpec {
    pub epochs: usize,
    pub seq_len: usize,
    pub batch_size: usize,
    pub grad_accum: usize,
    pub learning_rate: f64,
    pub warmup_steps: usize,
    pub seed: u64,
    pub resume_from: Option<PathBuf>,
    pub max_vram_fraction: Option<f32>,
    pub adapter_tag: Option<String>,
    /// Candle QLoRA: abort preflight if not every expected `o_proj` / `c_proj` key is in shards.
    pub qlora_require_full_proxy_stack: bool,
    /// Candle QLoRA: if set (0.0–1.0), abort after an epoch when `skipped_pairs / pair_attempts` exceeds this rate.
    pub qlora_max_skip_rate: Option<f32>,
    /// Candle QLoRA: skip middle `o_proj` stack; LM-head adapter only.
    pub qlora_lm_head_only: bool,
    /// Candle QLoRA: max middle projection layers in the proxy stack (`None` = no cap).
    pub qlora_proxy_max_layers: Option<usize>,
    /// Candle QLoRA: suffix LM — CE on the last **K** positions per row (1 = last token only).
    pub qlora_ce_last_k: usize,
    /// Dynamic curriculum schedule for difficult-gated training.
    pub curriculum_schedule: Option<super::training_config::CurriculumSchedule>,
}

/// Export / merge preferences.
#[derive(Debug, Clone)]
pub struct ArtifactSpec {
    /// Adapter file format generation (2 = Candle legacy meta, 3 = unified).
    pub adapter_schema_version: u32,
    /// When true, `merge-weights` / merge pipeline must not claim full attention fidelity.
    pub allow_placeholder_attention_merge: bool,
    /// Train-for-export hint (mobile edge gates + manifest).
    pub deployment_target: TrainingDeploymentTarget,
}

impl Default for ArtifactSpec {
    fn default() -> Self {
        Self {
            adapter_schema_version: 3,
            allow_placeholder_attention_merge: true,
            deployment_target: TrainingDeploymentTarget::default(),
        }
    }
}

impl FineTuneContract {
    /// Build from the flat training config + optional explicit kernel hint (CLI `--backend`).
    pub fn from_training_config(
        config: &super::training_config::LoraTrainingConfig,
        kernel_hint: super::train_backend::PopuliTrainBackend,
    ) -> Self {
        let (weight_shards, config_json) = config
            .base_model_paths
            .as_ref()
            .map(|(w, c)| (Some(w.clone()), Some(c.clone())))
            .unwrap_or((None, None));

        let method = match kernel_hint {
            super::train_backend::PopuliTrainBackend::BurnLora => AdapterMethod::Lora,
            super::train_backend::PopuliTrainBackend::CandleQlora => AdapterMethod::Qlora,
        };

        let base_quant = match kernel_hint {
            super::train_backend::PopuliTrainBackend::BurnLora => BaseQuantMode::None,
            super::train_backend::PopuliTrainBackend::CandleQlora => BaseQuantMode::Nf4,
        };

        let targets = match kernel_hint {
            super::train_backend::PopuliTrainBackend::BurnLora => AdapterTargetMask::FullGraph,
            super::train_backend::PopuliTrainBackend::CandleQlora => AdapterTargetMask::LmHeadProxy,
        };

        FineTuneContract {
            model: ModelSpec {
                hf_repo: config.base_model.clone(),
                weight_shards,
                config_json,
                tokenizer_json: config.tokenizer_path.clone(),
            },
            provenance: ModelProvenanceSpec {
                base_family: config.base_model_family.clone(),
                upstream_model_id: config.upstream_model_id.clone(),
                license_class: config.license_class.clone(),
                attribution_required: config.attribution_required,
            },
            data: DataSpec {
                train_file: config.train_file.clone(),
                tokenizer_mode: config.tokenizer_mode,
                min_rating: config.min_rating,
                context_filter: config
                    .context_filter
                    .as_ref()
                    .map(|cf| serde_json::to_string(cf).unwrap_or_default()),
            },
            adapter: AdapterSpec {
                method,
                rank: config.rank,
                alpha: config.alpha,
                dropout: 0.0,
                targets,
            },
            quant: QuantSpec {
                base: base_quant,
                double_quant: config.qlora_double_quant,
            },
            exec: ExecSpec {
                epochs: config.epochs,
                seq_len: config.seq_len,
                batch_size: config.batch_size,
                grad_accum: config.grad_accum,
                learning_rate: config.learning_rate,
                warmup_steps: config.warmup_steps,
                seed: config.seed,
                resume_from: config.resume_from.clone(),
                max_vram_fraction: config.max_vram_fraction,
                adapter_tag: config.adapter_tag.clone(),
                qlora_require_full_proxy_stack: config.qlora_require_full_proxy_stack,
                qlora_max_skip_rate: config.qlora_max_skip_rate,
                qlora_lm_head_only: config.qlora_lm_head_only,
                qlora_proxy_max_layers: config.qlora_proxy_max_layers,
                qlora_ce_last_k: config.qlora_ce_last_k,
                curriculum_schedule: config.curriculum_schedule.clone(),
            },
            artifact: ArtifactSpec {
                deployment_target: config.deployment_target,
                ..ArtifactSpec::default()
            },
            collateral_damage_verified: false,
        }
    }
}

/// Stable fingerprint of the full contract for manifests / telemetry (not a security hash).
///
/// Includes model path hints, data policy, adapter + quant + exec knobs, and artifact flags so
/// two runs with different QLoRA strictness or hyperparameters do not share the same digest.
pub fn finetune_contract_digest(c: &FineTuneContract) -> String {
    let mut h = DefaultHasher::new();

    hash_opt_str(&mut h, &c.model.hf_repo);
    hash_opt_path(&mut h, &c.model.config_json);
    hash_opt_path(&mut h, &c.model.tokenizer_json);
    hash_opt_path_vec(&mut h, &c.model.weight_shards);
    hash_opt_str(&mut h, &c.provenance.base_family);
    hash_opt_str(&mut h, &c.provenance.upstream_model_id);
    hash_opt_str(&mut h, &c.provenance.license_class);
    c.provenance.attribution_required.hash(&mut h);

    hash_opt_path(&mut h, &c.data.train_file);
    c.data.tokenizer_mode.hash(&mut h);
    c.data.min_rating.hash(&mut h);
    hash_opt_str(&mut h, &c.data.context_filter);

    c.adapter.method.hash(&mut h);
    c.adapter.rank.hash(&mut h);
    c.adapter.alpha.to_bits().hash(&mut h);
    c.adapter.dropout.to_bits().hash(&mut h);
    c.adapter.targets.hash(&mut h);

    c.quant.base.hash(&mut h);
    c.quant.double_quant.hash(&mut h);

    c.exec.epochs.hash(&mut h);
    c.exec.seq_len.hash(&mut h);
    c.exec.batch_size.hash(&mut h);
    c.exec.grad_accum.hash(&mut h);
    c.exec.learning_rate.to_bits().hash(&mut h);
    c.exec.warmup_steps.hash(&mut h);
    c.exec.seed.hash(&mut h);
    hash_opt_path(&mut h, &c.exec.resume_from);
    match c.exec.max_vram_fraction {
        None => false.hash(&mut h),
        Some(f) => {
            true.hash(&mut h);
            f.to_bits().hash(&mut h);
        }
    }
    hash_opt_str(&mut h, &c.exec.adapter_tag);
    c.exec.qlora_require_full_proxy_stack.hash(&mut h);
    match c.exec.qlora_max_skip_rate {
        None => false.hash(&mut h),
        Some(f) => {
            true.hash(&mut h);
            f.to_bits().hash(&mut h);
        }
    }
    c.exec.qlora_lm_head_only.hash(&mut h);
    match c.exec.qlora_proxy_max_layers {
        None => false.hash(&mut h),
        Some(u) => {
            true.hash(&mut h);
            u.hash(&mut h);
        }
    }
    c.exec.qlora_ce_last_k.hash(&mut h);
    if let Some(ref s) = c.exec.curriculum_schedule {
        s.epoch_1_max_difficulty.hash(&mut h);
        s.epoch_2_max_difficulty.hash(&mut h);
        s.epoch_3_max_difficulty.hash(&mut h);
        if let Some(ref ph) = s.curriculum_phases {
            ph.hash(&mut h);
        }
    }

    c.artifact.adapter_schema_version.hash(&mut h);
    c.artifact.allow_placeholder_attention_merge.hash(&mut h);
    c.artifact.deployment_target.hash(&mut h);

    format!("{:x}", h.finish())
}

fn hash_opt_path(state: &mut impl Hasher, p: &Option<PathBuf>) {
    match p {
        None => false.hash(state),
        Some(pb) => {
            true.hash(state);
            pb.hash(state);
        }
    }
}

fn hash_opt_str(state: &mut impl Hasher, p: &Option<String>) {
    match p {
        None => false.hash(state),
        Some(s) => {
            true.hash(state);
            s.hash(state);
        }
    }
}

fn hash_opt_path_vec(state: &mut impl Hasher, v: &Option<Vec<PathBuf>>) {
    match v {
        None => false.hash(state),
        Some(vec) => {
            true.hash(state);
            let mut keys: Vec<String> = vec
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect();
            keys.sort();
            keys.len().hash(state);
            for k in keys {
                k.hash(state);
            }
        }
    }
}

#[cfg(test)]
mod digest_tests {
    use super::*;
    use crate::mens::tensor::train_backend::PopuliTrainBackend;
    use crate::mens::tensor::training_config::{LoraTrainingConfig, TrainingDeploymentTarget};

    #[test]
    fn finetune_contract_digest_changes_with_strict_proxy_flag() {
        let cfg_off = LoraTrainingConfig {
            qlora_require_full_proxy_stack: false,
            ..Default::default()
        };
        let c_off =
            FineTuneContract::from_training_config(&cfg_off, PopuliTrainBackend::CandleQlora);
        let cfg_on = LoraTrainingConfig {
            qlora_require_full_proxy_stack: true,
            ..Default::default()
        };
        let c_on = FineTuneContract::from_training_config(&cfg_on, PopuliTrainBackend::CandleQlora);
        assert_ne!(
            finetune_contract_digest(&c_off),
            finetune_contract_digest(&c_on)
        );
    }

    #[test]
    fn finetune_contract_digest_changes_with_lm_head_only_flag() {
        let cfg_off = LoraTrainingConfig {
            qlora_lm_head_only: false,
            ..Default::default()
        };
        let c_off =
            FineTuneContract::from_training_config(&cfg_off, PopuliTrainBackend::CandleQlora);
        let cfg_on = LoraTrainingConfig {
            qlora_lm_head_only: true,
            ..Default::default()
        };
        let c_on = FineTuneContract::from_training_config(&cfg_on, PopuliTrainBackend::CandleQlora);
        assert_ne!(
            finetune_contract_digest(&c_off),
            finetune_contract_digest(&c_on)
        );
    }

    #[test]
    fn finetune_contract_digest_changes_with_proxy_max_layers() {
        let cfg_full = LoraTrainingConfig {
            qlora_proxy_max_layers: None,
            ..Default::default()
        };
        let c_full =
            FineTuneContract::from_training_config(&cfg_full, PopuliTrainBackend::CandleQlora);
        let cfg_cap = LoraTrainingConfig {
            qlora_proxy_max_layers: Some(4),
            ..Default::default()
        };
        let c_cap =
            FineTuneContract::from_training_config(&cfg_cap, PopuliTrainBackend::CandleQlora);
        assert_ne!(
            finetune_contract_digest(&c_full),
            finetune_contract_digest(&c_cap)
        );
    }

    #[test]
    fn finetune_contract_digest_changes_with_ce_last_k() {
        let cfg1 = LoraTrainingConfig {
            qlora_ce_last_k: 1,
            ..Default::default()
        };
        let c1 = FineTuneContract::from_training_config(&cfg1, PopuliTrainBackend::CandleQlora);
        let cfg4 = LoraTrainingConfig {
            qlora_ce_last_k: 4,
            ..Default::default()
        };
        let c4 = FineTuneContract::from_training_config(&cfg4, PopuliTrainBackend::CandleQlora);
        assert_ne!(finetune_contract_digest(&c1), finetune_contract_digest(&c4));
    }

    #[test]
    fn finetune_contract_digest_changes_with_deployment_target() {
        let cfg_w = LoraTrainingConfig {
            deployment_target: TrainingDeploymentTarget::Workstation,
            ..Default::default()
        };
        let c_w = FineTuneContract::from_training_config(&cfg_w, PopuliTrainBackend::BurnLora);
        let cfg_m = LoraTrainingConfig {
            deployment_target: TrainingDeploymentTarget::MobileEdge,
            ..Default::default()
        };
        let c_m = FineTuneContract::from_training_config(&cfg_m, PopuliTrainBackend::BurnLora);
        assert_ne!(
            finetune_contract_digest(&c_w),
            finetune_contract_digest(&c_m)
        );
    }

    #[test]
    fn finetune_contract_digest_changes_with_provenance() {
        let cfg_a = LoraTrainingConfig {
            upstream_model_id: Some("moonshotai/Kimi-K2.5".into()),
            ..Default::default()
        };
        let c_a = FineTuneContract::from_training_config(&cfg_a, PopuliTrainBackend::CandleQlora);
        let cfg_b = LoraTrainingConfig {
            upstream_model_id: Some("Qwen/Qwen3.5-4B".into()),
            ..Default::default()
        };
        let c_b = FineTuneContract::from_training_config(&cfg_b, PopuliTrainBackend::CandleQlora);
        assert_ne!(
            finetune_contract_digest(&c_a),
            finetune_contract_digest(&c_b)
        );
    }
}
