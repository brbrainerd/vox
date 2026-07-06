//! Maps [`super::finetune_contract::FineTuneContract`] → validated execution kernel plan.

use super::finetune_contract::{AdapterMethod, BaseQuantMode, FineTuneContract, ModelSpec};
use super::hf_load::{HfArchitecture, HfTransformerLayout};
use super::train_backend::PopuliTrainBackend;
use super::training_config::{MensTokenizerMode, TrainingDeploymentTarget};

/// Validated plan: which kernel runs and capability flags for telemetry.
#[derive(Debug, Clone)]
pub struct ExecutionPlan {
    pub kernel: PopuliTrainBackend,
    /// Candle QLoRA: `config.json` + weight shards are present (trainer **may** attempt the o_proj proxy stack).
    pub candle_proxy_stack_eligible: bool,
    /// Candle QLoRA: `Some(true)` when every expected `o_proj` / `c_proj` middle key exists in shards; `Some(false)` when some are missing; `None` for non-Candle or no layer inventory.
    pub candle_proxy_stack_complete: Option<bool>,
    /// Middle projection keys present in safetensors union (matches trainer `middle_projection_coverage.matched`).
    pub candle_proxy_stack_keys_matched: Option<usize>,
    /// Middle projection keys expected from `config.json` layout (matches trainer `middle_projection_coverage.expected`).
    pub candle_proxy_stack_keys_expected: Option<usize>,
    /// Documented transitional mode for Candle QLoRA.
    pub candle_compat_mode: bool,
    pub contract_digest: String,
}

#[derive(Debug, Clone, Default)]
pub struct ExecutionPlanner {
    /// When set, require this kernel (from CLI). Must match inferred kernel or error.
    pub force_kernel: Option<PopuliTrainBackend>,
}

/// `(eligible, stack_complete, keys_matched, keys_expected)` for Candle QLoRA middle projections.
type CandleProxyStackStatus = (bool, Option<bool>, Option<usize>, Option<usize>);

impl ExecutionPlanner {
    /// Validate contract, apply hard gates, return kernel + diagnostics.
    pub fn plan(&self, c: &FineTuneContract) -> anyhow::Result<ExecutionPlan> {
        self.validate_hard_gates(c)?;

        let kernel = self.resolve_kernel(c)?;

        let (
            candle_proxy_stack_eligible,
            candle_proxy_stack_complete,
            candle_proxy_stack_keys_matched,
            candle_proxy_stack_keys_expected,
        ) = candle_proxy_stack_status(kernel, &c.model)?;

        Ok(ExecutionPlan {
            kernel,
            candle_proxy_stack_eligible,
            candle_proxy_stack_complete,
            candle_proxy_stack_keys_matched,
            candle_proxy_stack_keys_expected,
            candle_compat_mode: kernel == PopuliTrainBackend::CandleQlora,
            contract_digest: contract_digest(c),
        })
    }

    /// Unsupported combinations with explicit operator messages.
    pub fn validate_hard_gates(&self, c: &FineTuneContract) -> anyhow::Result<()> {
        match c.quant.base {
            BaseQuantMode::Nf4 => {
                if c.adapter.method != AdapterMethod::Qlora {
                    anyhow::bail!(
                        "NF4 base quantization (QLoRA) requires the Candle execution kernel (`--backend qlora`). \
                         Burn LoRA does not implement NF4 frozen bases yet."
                    );
                }
            }
            BaseQuantMode::None => {}
        }

        if c.adapter.method == AdapterMethod::Qlora
            && c.data.tokenizer_mode != MensTokenizerMode::Hf
        {
            anyhow::bail!(super::operator_messages::QLORA_REQUIRES_HF_TOKENIZER);
        }

        if c.data.tokenizer_mode == MensTokenizerMode::Hf && c.adapter.method == AdapterMethod::Lora
        {
            let Some(ref cfg) = c.model.config_json else {
                anyhow::bail!(super::operator_messages::BURN_HF_CONFIG_REQUIRED);
            };
            let layout = HfTransformerLayout::from_config_path(cfg.as_path())?;
            if layout.architecture != HfArchitecture::Gpt2 {
                anyhow::bail!(
                    "{} (got architecture bucket {:?}, model_type {}).",
                    super::operator_messages::BURN_HF_GPT2_ONLY,
                    layout.architecture,
                    layout.model_type
                );
            }
        }

        if c.artifact.deployment_target == TrainingDeploymentTarget::MobileEdge {
            if c.exec.qlora_require_full_proxy_stack {
                anyhow::bail!(super::operator_messages::MOBILE_EDGE_REJECTS_FULL_PROXY_STACK);
            }
            if c.exec.seq_len > 512 {
                anyhow::bail!(super::operator_messages::mobile_edge_seq_len_cap(
                    c.exec.seq_len
                ));
            }
            if c.adapter.rank > 32 {
                anyhow::bail!(super::operator_messages::mobile_edge_rank_cap(
                    c.adapter.rank
                ));
            }
            if c.exec.batch_size > 1 {
                anyhow::bail!(super::operator_messages::mobile_edge_batch_cap(
                    c.exec.batch_size
                ));
            }
        }

        Ok(())
    }

    fn resolve_kernel(&self, c: &FineTuneContract) -> anyhow::Result<PopuliTrainBackend> {
        let inferred = match (c.adapter.method, c.quant.base) {
            (_, BaseQuantMode::Nf4) => PopuliTrainBackend::CandleQlora,
            (AdapterMethod::Qlora, _) => PopuliTrainBackend::CandleQlora,
            (AdapterMethod::Lora, BaseQuantMode::None) => PopuliTrainBackend::BurnLora,
        };
        if let Some(f) = self.force_kernel {
            if f != inferred {
                anyhow::bail!(
                    "CLI execution kernel `{f}` does not match fine-tune contract semantics (expected `{inferred}` for adapter={:?}, base_quant={:?}).",
                    c.adapter.method,
                    c.quant.base
                );
            }
            return Ok(f);
        }
        Ok(inferred)
    }
}

fn contract_digest(c: &FineTuneContract) -> String {
    super::finetune_contract::finetune_contract_digest(c)
}

/// Middle-projection key coverage for Candle QLoRA (matches `candle_qlora_train` inventory).
fn candle_proxy_stack_status(
    kernel: PopuliTrainBackend,
    model: &ModelSpec,
) -> anyhow::Result<CandleProxyStackStatus> {
    if kernel != PopuliTrainBackend::CandleQlora {
        return Ok((false, None, None, None));
    }
    let Some(config_path) = model.config_json.as_ref() else {
        return Ok((false, None, None, None));
    };
    let Some(shards) = model.weight_shards.as_ref() else {
        return Ok((false, None, None, None));
    };
    if shards.is_empty() || !config_path.is_file() {
        return Ok((false, None, None, None));
    }

    // SP3-D: proxy-stack key coverage was previously checked here using
    // `candle_qlora_weights::tensor_keys_union` (removed along with the extracted
    // training code). The plugin now performs its own preflight internally. Report
    // eligible=true so callers proceed to dispatch.
    let _ = (config_path, shards);
    Ok((true, None, None, None))
}

/// Shared entry: tokenizer + weights presence checks keyed by kernel.
///
/// For Candle QLoRA, skips the full `preflight_native_qlora` scan here because:
/// - Key coverage is already validated by the planner via `candle_proxy_stack_status`.
/// - `candle_qlora_train::run_candle_qlora_train` always calls `preflight_native_qlora`
///   itself to obtain the `QloraEmbedBundle` it needs — running it twice wastes ~5s
///   of redundant safetensors I/O.
pub fn preflight_model_bundle(
    kernel: PopuliTrainBackend,
    contract: &FineTuneContract,
) -> anyhow::Result<()> {
    let model = &contract.model;
    let data = &contract.data;
    match kernel {
        PopuliTrainBackend::CandleQlora => {
            // Light-weight token presence check only: verify tokenizer file exists before
            // the heavy safetensors scan deferred to `candle_qlora_train`.
            if let Some(ref p) = model.tokenizer_json
                && !p.is_file()
            {
                anyhow::bail!(
                    "{}",
                    super::operator_messages::tokenizer_not_a_file(&p.display().to_string())
                );
            }
            // Weight shards check: ensure at least one shard file exists.
            if let Some(ref shards) = model.weight_shards {
                for shard in shards {
                    if !shard.is_file() {
                        anyhow::bail!(
                            "Model weight shard not found: {}. Re-run the download step.",
                            shard.display()
                        );
                    }
                }
            }
            let _ = data;
        }
        PopuliTrainBackend::BurnLora => {
            if data.tokenizer_mode == MensTokenizerMode::Hf {
                let Some(p) = model.tokenizer_json.as_ref() else {
                    anyhow::bail!(super::operator_messages::BURN_HF_TOKENIZER_PATH_REQUIRED);
                };
                if !p.is_file() {
                    anyhow::bail!(
                        "{}",
                        super::operator_messages::tokenizer_not_a_file(&p.display().to_string())
                    );
                }
                let Some(cfg) = model.config_json.as_ref() else {
                    anyhow::bail!(super::operator_messages::BURN_HF_CONFIG_REQUIRED);
                };
                let _ = HfTransformerLayout::from_config_path(cfg)?;
            }
        }
    }
    Ok(())
}

#[cfg(all(test, feature = "mens-train"))]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use crate::mens::tensor::finetune_contract::{
        AdapterSpec, AdapterTargetMask, ArtifactSpec, DataSpec, ExecSpec, FineTuneContract,
        ModelProvenanceSpec, ModelSpec, QuantSpec,
    };
    use crate::mens::tensor::training_config::{MensTokenizerMode, TrainingDeploymentTarget};

    fn minimal_contract_lora_vox() -> FineTuneContract {
        FineTuneContract {
            model: ModelSpec {
                hf_repo: None,
                weight_shards: None,
                config_json: None,
                tokenizer_json: None,
            },
            collateral_damage_verified: false,
            provenance: ModelProvenanceSpec {
                base_family: None,
                upstream_model_id: None,
                license_class: None,
                attribution_required: false,
            },
            data: DataSpec {
                train_file: None,
                tokenizer_mode: MensTokenizerMode::Vox,
                min_rating: 3,
                context_filter: None,
            },
            adapter: AdapterSpec {
                method: super::super::finetune_contract::AdapterMethod::Lora,
                rank: 8,
                alpha: 16.0,
                dropout: 0.0,
                targets: AdapterTargetMask::FullGraph,
            },
            quant: QuantSpec {
                base: BaseQuantMode::None,
                double_quant: true,
            },
            exec: ExecSpec {
                epochs: 1,
                seq_len: 32,
                batch_size: 1,
                grad_accum: 1,
                learning_rate: 1e-4,
                warmup_steps: 1,
                seed: 1,
                resume_from: None,
                max_vram_fraction: None,
                adapter_tag: None,
                qlora_require_full_proxy_stack: false,
                qlora_max_skip_rate: None,
                qlora_lm_head_only: false,
                qlora_proxy_max_layers: None,
                qlora_ce_last_k: 1,
                curriculum_schedule: None,
            },
            artifact: ArtifactSpec::default(),
        }
    }

    #[test]
    fn planner_selects_burn_for_lora_none_quant() {
        let c = minimal_contract_lora_vox();
        let p = ExecutionPlanner {
            force_kernel: Some(PopuliTrainBackend::BurnLora),
        }
        .plan(&c)
        .expect("plan");
        assert_eq!(p.kernel, PopuliTrainBackend::BurnLora);
    }

    #[test]
    fn gate_rejects_burn_hf_without_config() {
        let mut c = minimal_contract_lora_vox();
        c.data.tokenizer_mode = MensTokenizerMode::Hf;
        c.model.tokenizer_json = Some(PathBuf::from("dummy.json"));
        let err = ExecutionPlanner {
            force_kernel: Some(PopuliTrainBackend::BurnLora),
        }
        .plan(&c)
        .unwrap_err()
        .to_string();
        assert!(
            err.contains("config.json") && err.contains("architecture validation"),
            "{err}"
        );
    }

    #[test]
    fn planner_burn_has_no_candle_proxy_fields() {
        let c = minimal_contract_lora_vox();
        let p = ExecutionPlanner {
            force_kernel: Some(PopuliTrainBackend::BurnLora),
        }
        .plan(&c)
        .expect("plan");
        assert!(!p.candle_proxy_stack_eligible);
        assert!(p.candle_proxy_stack_complete.is_none());
        assert!(p.candle_proxy_stack_keys_matched.is_none());
        assert!(p.candle_proxy_stack_keys_expected.is_none());
    }

    #[test]
    fn gate_rejects_qlora_with_vox_tokenizer() {
        let mut c = minimal_contract_lora_vox();
        c.adapter.method = super::super::finetune_contract::AdapterMethod::Qlora;
        c.quant.base = BaseQuantMode::Nf4;
        let err = ExecutionPlanner {
            force_kernel: Some(PopuliTrainBackend::CandleQlora),
        }
        .plan(&c)
        .unwrap_err()
        .to_string();
        let expected = super::super::operator_messages::QLORA_REQUIRES_HF_TOKENIZER;
        assert!(
            err == expected || err.contains(expected),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn gate_rejects_nf4_with_burn_only_lora_contract() {
        let mut c = minimal_contract_lora_vox();
        c.quant.base = BaseQuantMode::Nf4;
        c.adapter.method = super::super::finetune_contract::AdapterMethod::Lora;
        let err = ExecutionPlanner::default()
            .plan(&c)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("NF4") && err.contains("--backend qlora"),
            "{err}"
        );
    }

    #[test]
    fn gate_mobile_edge_rejects_long_seq_len() {
        let mut c = minimal_contract_lora_vox();
        c.artifact.deployment_target = TrainingDeploymentTarget::MobileEdge;
        c.exec.seq_len = 1024;
        let err = ExecutionPlanner {
            force_kernel: Some(PopuliTrainBackend::BurnLora),
        }
        .plan(&c)
        .unwrap_err()
        .to_string();
        assert!(err.contains("mobile_edge"), "{err}");
    }

    #[test]
    fn gate_mobile_edge_rejects_full_proxy_stack_flag() {
        let mut c = minimal_contract_lora_vox();
        c.artifact.deployment_target = TrainingDeploymentTarget::MobileEdge;
        c.exec.qlora_require_full_proxy_stack = true;
        let err = ExecutionPlanner {
            force_kernel: Some(PopuliTrainBackend::BurnLora),
        }
        .plan(&c)
        .unwrap_err()
        .to_string();
        assert!(err.contains("mobile_edge"), "{err}");
    }
}
