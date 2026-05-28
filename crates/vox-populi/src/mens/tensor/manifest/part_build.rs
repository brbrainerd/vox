/// Single constructor for the initial manifest row so Burn and Candle stay in sync when [`TrainingManifest`] grows fields.
pub fn initial_training_manifest(
    arch: ArchParams,
    train_file: impl Into<String>,
    run: InitialManifestRun,
    tokenizer_path: Option<String>,
    kernel: InitialTrainingKernel,
) -> TrainingManifest {
    const BURN_OBJECTIVE: &str = "burn_lora_masked_chatml_ce";

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
        InitialTrainingKernel::BurnLora => (
            Some("burn_lora".into()),
            None,
            None,
            None,
            1usize,
            None,
            None,
            None,
            Some(BURN_OBJECTIVE.to_string()),
        ),
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
        training_deployment_target: run.training_deployment_target.clone(),
        training_deployment_note: run.training_deployment_note.clone(),
        eval_baseline_delta_note: None,
        trajectory_weighting_enabled: run.trajectory_weighting_enabled,
        trajectory_tool_trace_boost: run.trajectory_tool_trace_boost,
        trajectory_failure_category_boost: run.trajectory_failure_category_boost,
        trajectory_quality_floor: run.trajectory_quality_floor,
        trajectory_quality_boost: run.trajectory_quality_boost,
        contamination_score: run.contamination_score,
    }
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
