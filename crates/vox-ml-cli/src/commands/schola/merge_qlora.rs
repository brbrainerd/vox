//! `vox schola merge-qlora` — fold QLoRA adapter tensors into base f32 weights.
//!
//! Dispatches to whichever `MlBackend` plugin matches this host's capabilities
//! (`mens-candle-cuda` on an NVIDIA host, `mens-candle-metal` on Apple Silicon)
//! via `MlBackend::merge_adapter`.
//! The adapter directory must contain `adapter_manifest.json` (v3) written by training.

use std::path::PathBuf;

use anyhow::Context;
use serde::{Deserialize, Serialize};

use vox_bounded_fs::read_utf8_path_capped;
use vox_populi::mens::MERGE_QLORA_REJECTS_BURN_BIN;

// Candidate plugins for the `MlBackend` extension point, mirroring
// catalog.toml's `mens-candle-cuda`/`mens-candle-metal` entries (id +
// requires-tag). vox-plugin-host is deliberately dependency-free and cannot
// read catalog.toml itself, so this is the caller-supplied SSOT-mirror
// `resolve_extension_point` needs. Both plugins implement `merge_adapter`
// (unlike QLoRA *training*, which has no Metal backend yet — see run_train.rs).
// vox:defactored-from vox-plugin-catalog 2026-09-05
const ML_BACKEND_CANDIDATES: &[vox_plugin_host::ExtensionCandidate] = &[
    vox_plugin_host::ExtensionCandidate {
        plugin_id: "mens-candle-cuda",
        requires_tag: Some("nvidia-gpu"),
    },
    vox_plugin_host::ExtensionCandidate {
        plugin_id: "mens-candle-metal",
        requires_tag: Some("apple-silicon"),
    },
];

// ---------------------------------------------------------------------------
// Inline serde-only schema types (no candle deps).
// These match the on-disk JSON layout produced by vox-plugin-mens-candle-cuda.
// ---------------------------------------------------------------------------

/// On-disk adapter bundle descriptor v3 (current, canonical).
///
/// Used here only for structural validation before dispatching to the plugin.
/// The plugin owns the authoritative schema; this struct must accept both the
/// flat legacy layout (`base_quant` at top level) and the canonical nested layout
/// (`quant: { base_quant, double_quant }`) without failing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PopuliAdapterManifestV3 {
    pub format: String,
    pub version: u32,
    // Flattened from AdapterMethodFields in the canonical plugin schema.
    #[serde(default)]
    pub adapter_method: String,
    // Legacy flat layout — empty when the canonical nested `quant` field is used instead.
    #[serde(default)]
    pub base_quant: String,
    // Canonical nested quant layout (plugin schema).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quant: Option<serde_json::Value>,
    pub base_key_map: std::collections::HashMap<String, String>,
    pub layer_order: Vec<String>,
    pub vocab: usize,
    pub d_model: usize,
    pub rank: usize,
    pub alpha: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<serde_json::Value>,
}

pub fn run_merge_qlora(
    base_shards: Vec<PathBuf>,
    adapter: PathBuf,
    meta: PathBuf,
    output: PathBuf,
    quantize: Option<String>,
) -> anyhow::Result<()> {
    if base_shards.is_empty() {
        anyhow::bail!("pass at least one `--base-shard` safetensors path");
    }
    for p in &base_shards {
        if !p.is_file() {
            anyhow::bail!("base shard not found: {}", p.display());
        }
    }
    if !adapter.is_file() {
        anyhow::bail!("adapter not found: {}", adapter.display());
    }
    if adapter
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("bin"))
    {
        anyhow::bail!("{MERGE_QLORA_REJECTS_BURN_BIN}");
    }
    if !meta.is_file() {
        anyhow::bail!("meta JSON not found: {}", meta.display());
    }

    // Parse v3 manifest (validate it's readable before dispatching to plugin).
    let raw = read_utf8_path_capped(&meta).with_context(|| format!("read {}", meta.display()))?;
    let manifest: PopuliAdapterManifestV3 = serde_json::from_str(&raw)
        .with_context(|| format!("parse adapter manifest v3 from {}", meta.display()))?;

    // Ensure adapter_manifest.json exists next to the adapter .safetensors so
    // the plugin can find it. If the user pointed --meta at a file in the adapter
    // dir with a different name, copy it as adapter_manifest.json.
    let adapter_dir = adapter
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let canonical_manifest = adapter_dir.join("adapter_manifest.json");
    if meta.canonicalize().ok() != canonical_manifest.canonicalize().ok() {
        std::fs::write(&canonical_manifest, &raw)
            .with_context(|| format!("write {}", canonical_manifest.display()))?;
    }

    // Use the parent directory of the first shard as the base model directory.
    let base_dir = base_shards[0]
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| std::path::PathBuf::from("."));

    // Dispatch to whichever MlBackend plugin matches this host's capabilities
    // (CUDA on an NVIDIA host, Metal on Apple Silicon), not a hardcoded id —
    // see vox_plugin_host::resolve_extension_point.
    let result = (|| -> anyhow::Result<()> {
        let plugin_id = vox_plugin_host::resolve_extension_point(
            "MlBackend",
            ML_BACKEND_CANDIDATES,
            &vox_plugin_host::probe(),
        )
        .context("no ML backend plugin matches this host's capabilities")?;
        let plugin = vox_plugin_host::cached_code_plugin(plugin_id).with_context(|| {
            format!("{plugin_id} plugin not found — install vox-plugin-{plugin_id}")
        })?;
        let backend = plugin
            .plugin
            .as_ml_backend()
            .into_option()
            .ok_or_else(|| anyhow::anyhow!("{plugin_id} plugin does not provide MlBackend"))?;
        backend
            .merge_adapter(
                base_dir.to_string_lossy().as_ref().into(),
                adapter.to_string_lossy().as_ref().into(),
                output.to_string_lossy().as_ref().into(),
            )
            .into_result()
            .map_err(|e| anyhow::anyhow!("merge_adapter: {e}"))
    })();

    result?;

    eprintln!("Wrote merged tensors (subset) to {}", output.display());
    let base = manifest
        .base_model
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("unknown");
    let handoff = vox_populi::mens::tensor::external_serving_handoff::ExternalServingHandoffV1::merged_qlora_subset(
        &output,
        base,
        None,
    );
    let handoff_dir = output
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    if let Err(e) =
        vox_populi::mens::tensor::external_serving_handoff::write_handoff(&handoff_dir, &handoff)
    {
        tracing::warn!("external_serving_handoff_v1.json not written: {e}");
    } else {
        eprintln!(
            "Wrote {}",
            handoff_dir
                .join("external_serving_handoff_v1.json")
                .display()
        );
    }

    // Optional: recombine the merged subset over the full base weights and
    // quantize the result. `output` is the merged-subset FILE; `base_dir` is the
    // directory holding the base model (config.json + shards), derived above as
    // the parent of the first `--base-shard`.
    if let Some(mixture_str) = quantize.as_deref() {
        let mixture = crate::commands::quantize::parse_mixture(mixture_str)?;
        let out_parent = output
            .parent()
            .map(std::path::Path::to_path_buf)
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        // recombine writes <recombined>/model.safetensors + copies base config.json
        let recombined = out_parent.join("recombined_full");
        // Clear any stale recombined dir so a prior sharded run's
        // model.safetensors.index.json can't mislead the reader.
        let _ = std::fs::remove_dir_all(&recombined);
        vox_quantize::recombine::recombine(&base_dir, &output, &recombined)
            .with_context(|| format!("recombine over base {}", base_dir.display()))?;
        let q_out = out_parent.join("quantized");
        // Clear any stale quantized dir from a prior run before reuse.
        let _ = std::fs::remove_dir_all(&q_out);
        let report = vox_quantize::quantize(&vox_quantize::QuantizeRequest {
            input_dir: recombined.clone(),
            output_dir: q_out.clone(),
            mixture,
            verify: true,
            device: vox_quantize::DevicePref::Auto, // GPU when available
        })
        .with_context(|| "quantize recombined model")?;
        println!(
            "Quantized merged model -> {} ({:.2}x)",
            q_out.display(),
            report.compression_ratio
        );
        let _ = std::fs::remove_dir_all(&recombined);
    }

    Ok(())
}
