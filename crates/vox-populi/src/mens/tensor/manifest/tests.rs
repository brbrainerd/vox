use super::*;
use tempfile::tempdir;
use vox_tensor::data::VOCAB_SIZE;

#[test]
fn initial_training_manifest_burn_wires_kernel_and_candle_defaults() {
    let run = InitialManifestRun {
        base_model: Some("org/model".into()),
        rank: 4,
        alpha: 8.0,
        seq_len: 64,
        epochs: 2,
        run_id: Some("run-1".into()),
        git_sha: Some("deadbeef".into()),
        device_profile: Some("test-gpu".into()),
        adapter_tag: None,
        provenance_base_family: None,
        provenance_upstream_model_id: None,
        provenance_license_class: None,
        provenance_attribution_required: false,
        seed: 11,
        grad_accum: 3,
        context_filter: None,
        max_vram_fraction: None,
        finetune_contract_digest: None,
        training_deployment_target: None,
        training_deployment_note: None,
        trajectory_weighting_enabled: false,
        trajectory_tool_trace_boost: 1.1,
        trajectory_failure_category_boost: 1.15,
        trajectory_quality_floor: None,
        trajectory_quality_boost: 1.05,
        contamination_score: None,
        launch_argv: Vec::new(),
    };
    let m = initial_training_manifest(
        ArchParams {
            vocab_size: VOCAB_SIZE,
            d_model: 8,
            n_heads: 2,
            n_layers: 1,
        },
        "train.jsonl",
        run,
        None,
        InitialTrainingKernel::BurnLora,
    );
    assert_eq!(m.execution_kernel.as_deref(), Some("burn_lora"));
    assert_eq!(
        m.training_objective_note.as_deref(),
        Some("burn_lora_masked_chatml_ce")
    );
    assert_eq!(m.candle_qlora_proxy_stack_complete, None);
    assert_eq!(m.candle_qlora_graph_id.as_deref(), None);
    assert_eq!(m.candle_qlora_middle_layers_active, None);
    assert_eq!(m.candle_qlora_ce_last_k, 1);
    assert_eq!(m.candle_qlora_training_steps_executed, 0);
    assert_eq!(m.grad_accum, 3);
    assert_eq!(m.train_file, "train.jsonl");
    assert_eq!(m.base_model.as_deref(), Some("org/model"));
}

#[test]
fn initial_training_manifest_candle_sets_proxy_and_objective() {
    let run = InitialManifestRun {
        base_model: None,
        rank: 8,
        alpha: 16.0,
        seq_len: 128,
        epochs: 1,
        run_id: None,
        git_sha: None,
        device_profile: None,
        adapter_tag: None,
        provenance_base_family: None,
        provenance_upstream_model_id: None,
        provenance_license_class: None,
        provenance_attribution_required: false,
        seed: 1,
        grad_accum: 2,
        context_filter: None,
        max_vram_fraction: None,
        finetune_contract_digest: Some("digest".into()),
        training_deployment_target: None,
        training_deployment_note: None,
        trajectory_weighting_enabled: false,
        trajectory_tool_trace_boost: 1.1,
        trajectory_failure_category_boost: 1.15,
        trajectory_quality_floor: None,
        trajectory_quality_boost: 1.05,
        contamination_score: None,
        launch_argv: Vec::new(),
    };
    let tok = Some("tokenizer.json".to_string());
    let m_stack = initial_training_manifest(
        ArchParams {
            vocab_size: 1000,
            d_model: 32,
            n_heads: 4,
            n_layers: 2,
        },
        "data/train.jsonl",
        run.clone(),
        tok.clone(),
        InitialTrainingKernel::CandleQlora {
            proxy_stack_complete: true,
            middle_layers_active: 3,
            ce_last_k: 1,
            architecture: "qwen3_5".to_string(),
            linear_layers: Some(2),
            full_layers: Some(1),
        },
    );
    assert_eq!(m_stack.execution_kernel.as_deref(), Some("candle_qlora"));
    assert_eq!(
        m_stack.training_objective_note.as_deref(),
        Some("candle_qlora_full_graph_k1")
    );
    assert_eq!(
        m_stack.candle_qlora_graph_id.as_deref(),
        Some("full_graph_v1")
    );
    assert_eq!(m_stack.candle_qlora_middle_layers_active, Some(3));
    assert_eq!(m_stack.candle_qlora_ce_last_k, 1);
    assert_eq!(m_stack.candle_qlora_proxy_stack_complete, Some(true));
    assert_eq!(m_stack.tokenizer_path.as_deref(), Some("tokenizer.json"));

    let m_k8 = initial_training_manifest(
        ArchParams {
            vocab_size: 1000,
            d_model: 32,
            n_heads: 4,
            n_layers: 2,
        },
        "data/train.jsonl",
        run.clone(),
        tok.clone(),
        InitialTrainingKernel::CandleQlora {
            proxy_stack_complete: true,
            middle_layers_active: 2,
            ce_last_k: 8,
            architecture: "qwen3_5".to_string(),
            linear_layers: Some(1),
            full_layers: Some(1),
        },
    );
    assert_eq!(
        m_k8.training_objective_note.as_deref(),
        Some("candle_qlora_full_graph_k8")
    );
    assert_eq!(m_k8.candle_qlora_ce_last_k, 8);

    let m_lm = initial_training_manifest(
        ArchParams {
            vocab_size: 1000,
            d_model: 32,
            n_heads: 4,
            n_layers: 2,
        },
        "data/train.jsonl",
        run,
        tok,
        InitialTrainingKernel::CandleQlora {
            proxy_stack_complete: false,
            middle_layers_active: 0,
            ce_last_k: 1,
            architecture: "qwen2".to_string(),
            linear_layers: Some(0),
            full_layers: Some(2),
        },
    );
    assert_eq!(m_lm.candle_qlora_proxy_stack_complete, Some(false));
    assert_eq!(m_lm.candle_qlora_graph_id.as_deref(), Some("full_graph_v1"));
    assert_eq!(m_lm.candle_qlora_middle_layers_active, Some(0));
}

#[cfg(feature = "mens-train")]
#[test]
fn initial_manifest_run_from_lora_config_grad_accum_clamped() {
    use super::super::training_config::LoraTrainingConfig;

    let c = LoraTrainingConfig {
        base_model: Some("hf/model".into()),
        rank: 11,
        grad_accum: 0,
        ..Default::default()
    };
    let snap = InitialManifestRun::from_lora_config(&c);
    assert_eq!(snap.grad_accum, 1);
    assert_eq!(snap.rank, 11);
    let m = initial_training_manifest(
        ArchParams::default(),
        "train.jsonl",
        snap,
        None,
        InitialTrainingKernel::BurnLora,
    );
    assert_eq!(m.grad_accum, 1);
    assert_eq!(m.base_model.as_deref(), Some("hf/model"));
}

#[cfg(feature = "mens-train")]
#[test]
fn initial_manifest_run_mobile_edge_sets_deployment_fields() {
    use super::super::training_config::{LoraTrainingConfig, TrainingDeploymentTarget};

    let c = LoraTrainingConfig {
        deployment_target: TrainingDeploymentTarget::MobileEdge,
        ..Default::default()
    };
    let snap = InitialManifestRun::from_lora_config(&c);
    assert_eq!(
        snap.training_deployment_target.as_deref(),
        Some("mobile_edge")
    );
    assert!(snap.training_deployment_note.is_some());
}

#[test]
fn training_manifest_roundtrip_grad_accum() {
    let dir = tempdir().expect("tempdir");
    let m = TrainingManifest {
        vocab_size: VOCAB_SIZE,
        d_model: 8,
        n_heads: 2,
        n_layers: 1,
        base_model: None,
        tokenizer_path: None,
        provenance_base_family: None,
        provenance_upstream_model_id: None,
        provenance_license_class: None,
        provenance_attribution_required: false,
        train_file: "train.jsonl".into(),
        rank: 4,
        alpha: 8.0,
        seq_len: 64,
        epochs: 1,
        run_id: None,
        git_sha: None,
        device_profile: None,
        adapter_tag: None,
        seed: 0,
        grad_accum: 7,
        context_filter: None,
        max_vram_fraction: None,
        manifest_schema_version: TRAINING_MANIFEST_SCHEMA_VERSION,
        execution_kernel: None,
        finetune_contract_digest: None,
        candle_qlora_training_steps_executed: 0,
        candle_qlora_skips_bad_vocab: 0,
        candle_qlora_skips_last_hidden: 0,
        candle_qlora_skips_short_seq: 0,
        candle_qlora_proxy_stack_complete: None,
        candle_qlora_graph_id: None,
        candle_qlora_middle_layers_active: None,
        candle_qlora_ce_last_k: 64,
        candle_qlora_architecture: None,
        candle_qlora_linear_layers: None,
        candle_qlora_full_layers: None,
        training_objective_note: None,
        training_deployment_target: None,
        training_deployment_note: None,
        eval_baseline_delta_note: None,
        trajectory_weighting_enabled: false,
        trajectory_tool_trace_boost: 1.1,
        trajectory_failure_category_boost: 1.15,
        trajectory_quality_floor: None,
        trajectory_quality_boost: 1.05,
        contamination_score: None,
        launch_argv: Vec::new(),
    };
    write_training_manifest(dir.path(), m).expect("write");
    let loaded = load_manifest(dir.path()).expect("load").expect("some");
    assert_eq!(loaded.grad_accum, 7);
}
