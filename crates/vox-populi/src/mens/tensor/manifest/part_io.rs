pub fn load_manifest(run_dir: &Path) -> anyhow::Result<Option<TrainingManifest>> {
    let p = run_dir.join("training_manifest.json");
    if !p.is_file() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&p)?;
    let m: TrainingManifest = serde_json::from_str(&raw)?;
    if m.manifest_schema_version < TRAINING_MANIFEST_SCHEMA_VERSION {
        tracing::debug!(
            path = %p.display(),
            file_schema = m.manifest_schema_version,
            current = TRAINING_MANIFEST_SCHEMA_VERSION,
            "older training manifest (reader tolerant; fields default-filled)"
        );
    }
    Ok(Some(m))
}

/// Ensure checkpoint file exists and is non-trivial; cross-check manifest when present.
pub fn validate_checkpoint_manifest(
    checkpoint: &Path,
    run_dir: &Path,
    params: ValidateParams,
) -> anyhow::Result<()> {
    let meta = std::fs::metadata(checkpoint)?;
    if meta.len() < 256 {
        anyhow::bail!(
            "checkpoint {} too small ({} bytes)",
            checkpoint.display(),
            meta.len()
        );
    }
    if let Ok(Some(m)) = load_manifest(run_dir) {
        if m.vocab_size != params.vocab_size {
            anyhow::bail!(
                "manifest vocab_size {} != expected {}",
                m.vocab_size,
                params.vocab_size
            );
        }
        if m.d_model != params.d_model
            || m.n_heads != params.n_heads
            || m.n_layers != params.n_layers
        {
            anyhow::bail!(
                "manifest arch mismatch vs checkpoint validation params (d_model/n_heads/n_layers)"
            );
        }
    }
    let _ = params.kind;
    Ok(())
}
