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

/// Fail-closed placeholder guard for the real train / dispatch path.
///
/// All Qwen3 ladder rungs ship with `@PLACEHOLDER-*` revisions in
/// `gpu-specs.yaml` until a real HF commit SHA is pinned. Spending money to
/// train against an unpinned base is unsafe (non-reproducible, may resolve to a
/// moving `main`), so this guard rejects any resolved id/revision whose text
/// contains "PLACEHOLDER" (case-insensitive).
///
/// Call this at the base-resolution boundary on the **actual** train/dispatch
/// path (after resolving the concrete `hf_id`), NOT on a `--dry-run`/plan path —
/// planning may still print a plan containing a placeholder id and exit 0.
pub fn ensure_not_placeholder(resolved_hf_id: &str) -> anyhow::Result<()> {
    if resolved_hf_id.to_ascii_lowercase().contains("placeholder") {
        anyhow::bail!(
            "base revision is a placeholder ('{resolved_hf_id}') — pin a real HF commit SHA \
             in gpu-specs.yaml (hf_id@<sha>) before training"
        );
    }
    Ok(())
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

    #[test]
    fn qwen3_code_24g_returns_14b_qlora() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap();
        let overlay = load_overlay(root).expect("load overlay");
        let base = pick_base(&overlay, "qwen3_code", 24_000).expect("14B fits at 24GB");
        assert!(
            base.hf_id.contains("Qwen3-14B"),
            "expected Qwen3-14B at 24GB, got: {}",
            base.hf_id
        );
        assert!(
            base.hf_id.contains('@'),
            "hf_id must be revision-pinned (contains @): {}",
            base.hf_id
        );
    }

    #[test]
    fn qwen3_code_fail_closed_below_floor() {
        // Lowest qwen3_code rung has floor_mb=2000 (CPU/dev tier, ~0.6B QLoRA).
        // Anything below 2000 MB must fail-closed — no base should be returned.
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap();
        let overlay = load_overlay(root).expect("load overlay");
        let result = pick_base(&overlay, "qwen3_code", 1_000);
        assert!(
            result.is_err(),
            "below 2GB floor must fail-closed, got {:?}",
            result.ok().map(|b| &b.hf_id)
        );
    }

    #[test]
    fn placeholder_id_rejected_on_real_path() {
        // BLOCKER 3: a resolved id whose revision is a placeholder must fail-closed
        // before any download / dispatch on the real train path.
        let resolved = "Qwen/Qwen3-14B@PLACEHOLDER-c4e8f122";
        let err = ensure_not_placeholder(resolved).unwrap_err();
        assert!(
            err.to_string().to_lowercase().contains("placeholder"),
            "error must mention placeholder, got: {err}"
        );
        // Case-insensitive match.
        assert!(ensure_not_placeholder("org/Model@placeholder-abc").is_err());
    }

    #[test]
    fn pinned_id_passes_placeholder_guard() {
        // A real pinned revision (no PLACEHOLDER text) must pass.
        assert!(ensure_not_placeholder("Qwen/Qwen3-14B@a1b2c3d4e5f6").is_ok());
        assert!(ensure_not_placeholder("Qwen/Qwen2.5-Coder-7B-Instruct").is_ok());
    }

    #[test]
    fn qwen3_code_48g_prefers_unquantized_14b() {
        // At 48GB: 14B-LoRA (floor ~44GB) should beat 14B-QLoRA (floor ~20GB)
        // because max_by_key(floor_mb) picks the highest floor that fits.
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap();
        let overlay = load_overlay(root).expect("load overlay");
        let base = pick_base(&overlay, "qwen3_code", 49_152).expect("14B-LoRA fits at 48GB"); // 48 GB GPU expressed in MB; the LoRA un-quantized rung has floor_mb=44_000
        assert!(
            base.methods.iter().any(|m| m == "lora" || m == "full_lora"),
            "at 48GB should prefer LoRA (un-quantized) over QLoRA, but got methods: {:?}",
            base.methods
        );
        assert!(
            base.hf_id.contains("Qwen3-14B"),
            "should be 14B, got: {}",
            base.hf_id
        );
    }
}
