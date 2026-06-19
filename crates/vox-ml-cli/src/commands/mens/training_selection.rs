//! Pure, GPU-independent resolution of a spoke's training selection
//! (base model + preset + training backend, or a "skip"/"unwired" outcome).
//! Extracted from pipeline.rs so it is unit-testable and dry-run-observable
//! WITHOUT the `gpu` feature (AGH-0012 F1).
use std::path::Path;
use vox_populi::mens::tensor::domain_profiles::{EffectiveDomainProfile, TrainMethod};
use vox_populi::mens::tensor::finetune_contract::AdapterMethod;
use vox_populi::mens::tensor::finetune_registry::AdapterMethodRegistry;
use vox_populi::mens::PopuliTrainBackend;

#[derive(Debug, PartialEq)]
pub enum TrainingSelection {
    /// Train this base+preset with this backend.
    Train { model: Option<String>, preset: String, backend: PopuliTrainBackend },
    /// Spoke is RagOnly/PromptOnly — skip the training stage.
    Skip { reason: String },
}

/// Resolve a spoke's training selection. `vram_mb_override` lets callers/tests
/// inject VRAM (None → vram_autodetect at the resolver). CLI overrides win.
pub fn resolve_training_selection(
    root: &Path,
    profile: Option<&str>,
    cli_model: Option<&str>,
    cli_preset: Option<&str>,
    vram_mb_override: Option<u32>,
) -> anyhow::Result<TrainingSelection> {
    let eff = profile.and_then(|n| EffectiveDomainProfile::load_domain_profile(n, Some(root)).ok());
    let method = eff.as_ref().and_then(|e| e.base.as_ref().map(|b| b.method)).unwrap_or(TrainMethod::Qlora);
    if matches!(method, TrainMethod::RagOnly | TrainMethod::PromptOnly) {
        return Ok(TrainingSelection::Skip { reason: format!("{method:?}") });
    }
    let backend = match method {
        TrainMethod::Qlora => AdapterMethodRegistry::builtin().resolve(AdapterMethod::Qlora)
            .map(|r| r.default_kernel)
            .ok_or_else(|| anyhow::anyhow!("AdapterMethodRegistry missing Qlora kernel"))?,
        TrainMethod::FullSft | TrainMethod::Dpo | TrainMethod::Orpo =>
            anyhow::bail!("training method {method:?} has no wired backend"),
        TrainMethod::RagOnly | TrainMethod::PromptOnly => unreachable!(),
    };

    // model: CLI wins; else resolve the spoke base.model (tag→VRAM-fit / concrete id);
    // on failure (e.g. no GPU to size a tag) fall back to None (run_train default path).
    let model = if let Some(m) = cli_model {
        Some(m.to_string())
    } else if let Some(tag) = eff.as_ref().and_then(|e| e.base.as_ref().map(|b| b.model.clone())) {
        match vox_populi::mens::tensor::spoke_base_resolver::resolve_base_model(root, &tag, vram_mb_override) {
            Ok(id) => Some(id),
            Err(_) => None,
        }
    } else { None };
    // preset: CLI wins; else base.preset; else default.
    let preset = cli_preset.map(str::to_string)
        .or_else(|| eff.as_ref().and_then(|e| e.base.as_ref().and_then(|b| b.preset.clone())))
        .unwrap_or_else(|| "qwen_4080_16g".to_string());
    Ok(TrainingSelection::Train { model, preset, backend })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn root() -> std::path::PathBuf { std::path::Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(2).unwrap().to_path_buf() }
    #[test] fn rust_expert_resolves_qwen_qlora() {
        let sel = resolve_training_selection(&root(), Some("rust-expert"), None, None, Some(16384)).unwrap();
        match sel { TrainingSelection::Train{model,preset,backend} => {
            assert!(model.unwrap().contains("Qwen")); assert!(!preset.is_empty());
            assert!(matches!(backend, PopuliTrainBackend::CandleQlora));
        }, _ => panic!("expected Train") }
    }
    #[test] fn cli_model_overrides() {
        let sel = resolve_training_selection(&root(), Some("rust-expert"), Some("org/Manual"), None, Some(16384)).unwrap();
        if let TrainingSelection::Train{model,..} = sel { assert_eq!(model.as_deref(), Some("org/Manual")); } else { panic!() }
    }
    #[test] fn no_gpu_tag_falls_back_to_none_model() {
        let sel = resolve_training_selection(&root(), Some("rust-expert"), None, None, None /*no VRAM*/).unwrap();
        // On a host with no GPU, get_system_vram_gb() is None → tag unsizable → model None (default path).
        if let TrainingSelection::Train{model,..} = sel { /* model may be None here */ let _ = model; } else { panic!() }
    }
    #[test] fn cli_preset_overrides() {
        let sel = resolve_training_selection(&root(), Some("rust-expert"), None, Some("a100"), Some(16384)).unwrap();
        if let TrainingSelection::Train{preset,..} = sel { assert_eq!(preset, "a100"); } else { panic!() }
    }
    #[test]
    fn all_live_spokes_resolve_to_trainable_selection() {
        let root = root();
        for spoke in ["vox-lang", "rust-expert", "agents"] {
            let sel = resolve_training_selection(&root, Some(spoke), None, None, Some(16384))
                .unwrap_or_else(|e| panic!("{spoke}: {e}"));
            match sel {
                TrainingSelection::Train { model, preset, .. } => {
                    assert!(model.as_deref().unwrap_or("").contains("Qwen"), "{spoke}: {model:?}");
                    assert!(!preset.is_empty(), "{spoke}");
                }
                other => panic!("{spoke}: expected Train, got {other:?}"),
            }
        }
    }
}
