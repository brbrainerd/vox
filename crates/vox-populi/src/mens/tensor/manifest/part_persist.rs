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
