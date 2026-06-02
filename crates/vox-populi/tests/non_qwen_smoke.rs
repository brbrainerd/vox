#![cfg(feature = "mens-train")]

use std::path::PathBuf;
use tempfile::tempdir;
use vox_populi::mens::tensor::execution_planner::ExecutionPlanner;
use vox_populi::mens::tensor::finetune_contract::{
    AdapterMethod, AdapterSpec, AdapterTargetMask, ArtifactSpec, BaseQuantMode, DataSpec, ExecSpec,
    FineTuneContract, ModelProvenanceSpec, ModelSpec, QuantSpec,
};
use vox_populi::mens::tensor::hf_load::HfArchitecture;
use vox_populi::mens::tensor::training_config::MensTokenizerMode;

fn write_config(dir: &std::path::Path, model_type: &str) -> PathBuf {
    let p = dir.join("config.json");
    let json = format!(
        r#"{{"model_type":"{}","hidden_size":32,"num_attention_heads":4,"num_hidden_layers":2,"vocab_size":100}}"#,
        model_type
    );
    std::fs::write(&p, json).expect("write config");
    p
}

#[test]
fn plan_llama_model() {
    let dir = tempdir().expect("tempdir");
    let config_path = write_config(dir.path(), "llama");

    let c = FineTuneContract {
        model: ModelSpec {
            hf_repo: None,
            weight_shards: Some(vec![dir.path().join("model.safetensors")]),
            config_json: Some(config_path),
            tokenizer_json: Some(dir.path().join("tokenizer.json")),
        },
        collateral_damage_verified: false,
        provenance: ModelProvenanceSpec::default(),
        data: DataSpec {
            train_file: None,
            tokenizer_mode: MensTokenizerMode::Hf,
            min_rating: 3,
            context_filter: None,
        },
        adapter: AdapterSpec {
            method: AdapterMethod::Qlora,
            rank: 8,
            alpha: 16.0,
            dropout: 0.0,
            targets: AdapterTargetMask::FullGraph,
        },
        quant: QuantSpec {
            base: BaseQuantMode::Nf4,
            double_quant: true,
        },
        exec: ExecSpec::default(),
        artifact: ArtifactSpec::default(),
    };

    // Smoke: planning a dense Llama-family contract must not panic/err.
    let planner = ExecutionPlanner::default();
    planner.plan(&c).expect("plan llama");

    // A dense Llama-family config is classified into the supported stacked-causal
    // family (`Qwen35`) — Vox carries no distinct `Llama` architecture; llama/mistral
    // checkpoints are trained through the same stacked-causal path.
    let layout = vox_populi::mens::tensor::hf_load::parse_transformer_layout(
        c.model.config_json.as_ref().unwrap(),
    )
    .expect("layout");
    assert_eq!(layout.architecture, HfArchitecture::Qwen35);
}

#[test]
fn plan_mistral_model() {
    let dir = tempdir().expect("tempdir");
    let config_path = write_config(dir.path(), "mistral");

    // Mistral, like Llama, maps onto the stacked-causal (`Qwen35`) family.
    let layout =
        vox_populi::mens::tensor::hf_load::parse_transformer_layout(&config_path).expect("layout");
    assert_eq!(layout.architecture, HfArchitecture::Qwen35);
}

#[test]
fn plan_phi_model() {
    let dir = tempdir().expect("tempdir");
    let config_path = write_config(dir.path(), "phi");

    // `phi` matches no Qwen/Llama/Mistral rule, so it falls through to the GPT-2
    // style classification — this pins that fallthrough so it can't regress silently.
    let layout =
        vox_populi::mens::tensor::hf_load::parse_transformer_layout(&config_path).expect("layout");
    assert_eq!(layout.architecture, HfArchitecture::Gpt2);
}
