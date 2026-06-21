//! Resolve a spoke capability tag -> concrete fine-tunable base that fits VRAM.
//! Overlay source: `train_bases:` in mens/config/gpu-specs.yaml. Pure core +
//! a thin disk loader; reuses vram_autodetect for the live VRAM number.
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct TrainBase {
    pub hf_id: String,
    pub floor_mb: u32,
    #[serde(default)]
    pub methods: Vec<String>,
}

/// Largest candidate for `tag` whose `floor_mb <= vram_mb`. Errors if the tag is
/// unknown or nothing fits (fail-closed — never silently pick a too-big base).
pub fn pick_base<'a>(
    overlay: &'a HashMap<String, Vec<TrainBase>>,
    tag: &str,
    vram_mb: u32,
) -> anyhow::Result<&'a TrainBase> {
    let candidates = overlay.get(tag).ok_or_else(|| {
        anyhow::anyhow!("unknown base tag '{tag}' (not in gpu-specs train_bases)")
    })?;
    candidates
        .iter()
        .filter(|b| b.floor_mb <= vram_mb)
        .max_by_key(|b| b.floor_mb)
        .ok_or_else(|| anyhow::anyhow!("no '{tag}' base fits {vram_mb}MB VRAM"))
}

#[derive(Debug, Deserialize)]
struct GpuSpecsTrainBases {
    #[serde(default)]
    train_bases: HashMap<String, Vec<TrainBase>>,
}

pub fn load_overlay(root: &std::path::Path) -> anyhow::Result<HashMap<String, Vec<TrainBase>>> {
    let p = root.join("mens/config/gpu-specs.yaml");
    let s =
        std::fs::read_to_string(&p).map_err(|e| anyhow::anyhow!("read {}: {e}", p.display()))?;
    let parsed: GpuSpecsTrainBases = serde_yaml::from_str(&s)
        .map_err(|e| anyhow::anyhow!("parse train_bases in gpu-specs.yaml: {e}"))?;
    Ok(parsed.train_bases)
}

/// Resolve `base.model` to a concrete HF id.
/// - concrete id (contains '/') -> pass-through (no VRAM needed).
/// - capability tag -> overlay + VRAM fit.
///
/// `vram_mb_override`: Some(v) for tests / known hosts; None -> vram_autodetect.
/// On None VRAM with a tag, returns Err (fail-closed) — callers that must NOT
/// require a GPU (e.g. --skip-train dry-runs) should treat Err as "defer to the
/// existing default-model path" rather than aborting (see Phase 2 / §E).
pub fn resolve_base_model(
    root: &std::path::Path,
    base_model: &str,
    vram_mb_override: Option<u32>,
) -> anyhow::Result<String> {
    if base_model.contains('/') {
        return Ok(base_model.to_string());
    }
    let overlay = load_overlay(root)?;
    let vram_mb = match vram_mb_override {
        Some(v) => v,
        None => {
            let gb =
                crate::mens::tensor::vram_autodetect::get_system_vram_gb().ok_or_else(|| {
                    anyhow::anyhow!("no GPU VRAM detected; cannot size base tag '{base_model}'")
                })?;
            (gb * 1024.0) as u32
        }
    };
    Ok(pick_base(&overlay, base_model, vram_mb)?.hf_id.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn overlay() -> HashMap<String, Vec<TrainBase>> {
        let mut m = HashMap::new();
        m.insert(
            "strong_code_default".into(),
            vec![
                TrainBase {
                    hf_id: "small".into(),
                    floor_mb: 6000,
                    methods: vec!["qlora".into()],
                },
                TrainBase {
                    hf_id: "big".into(),
                    floor_mb: 11000,
                    methods: vec!["qlora".into()],
                },
            ],
        );
        m
    }

    #[test]
    fn picks_largest_that_fits() {
        assert_eq!(
            pick_base(&overlay(), "strong_code_default", 16384)
                .unwrap()
                .hf_id,
            "big"
        );
        assert_eq!(
            pick_base(&overlay(), "strong_code_default", 8000)
                .unwrap()
                .hf_id,
            "small"
        );
    }

    #[test]
    fn errors_when_none_fit() {
        assert!(pick_base(&overlay(), "strong_code_default", 4000).is_err());
    }

    #[test]
    fn errors_unknown_tag() {
        assert!(pick_base(&overlay(), "nope", 16384).is_err());
    }

    #[test]
    fn resolves_repo_tag_with_injected_vram() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap();
        let id = resolve_base_model(root, "strong_code_default", Some(16384)).unwrap();
        assert!(id.contains("Qwen"), "got {id}");
    }

    #[test]
    fn concrete_id_passthrough_needs_no_vram() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap();
        assert_eq!(
            resolve_base_model(root, "org/My-Model", None).unwrap(),
            "org/My-Model"
        );
    }
}
