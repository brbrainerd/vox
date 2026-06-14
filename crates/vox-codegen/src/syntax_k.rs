//! Syntax-level Kolmogorov-style complexity estimators for compiler outputs.
//!
//! This module implements practical compression-based proxies over canonicalized
//! output bytes and an optional NCD delta vs a baseline payload.
//!
//! ## Telemetry hook
//! [`measure_syntax_k_event`] builds a versioned [`SyntaxKEvent`] for `research_metrics` rows (`syntax_k_event` metric type).
//! JSON schema SSOT: `contracts/eval/syntax-k-event.schema.json`. Callers (e.g. CLI `vox ci` / eval harness) gate persistence
//! on Codex attach + opt-in flags; see `docs/src/architecture/telemetry-trust-ssot.md`.

use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

use bzip2::Compression as BzCompression;
use bzip2::write::BzEncoder;
use flate2::Compression as GzCompression;
use flate2::write::GzEncoder;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha3::{Digest, Sha3_256};

use crate::web_ir::WebIrModule;

const SCHEMA_VERSION: u32 = 1;
const PROFILE_ZSTD: &str = "zstd:level=19";
const PROFILE_BZIP2: &str = "bzip2:best";
const PROFILE_GZIP: &str = "gzip:best";

/// Per-compressor observation for one output payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyntaxKCompressorResult {
    pub name: String,
    pub profile: String,
    pub compressed_bytes: usize,
    pub ratio: f64,
}

/// Per-compressor NCD against a baseline payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyntaxKNcdPerCompressor {
    pub name: String,
    pub value: f64,
}

/// NCD summary against a baseline payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyntaxKNcdSummary {
    pub per_compressor: Vec<SyntaxKNcdPerCompressor>,
    pub median: f64,
}

/// Versioned `syntax_k_event` payload shape for `research_metrics.metadata_json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyntaxKEvent {
    pub schema_version: u32,
    pub fixture_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web_ir_hash: Option<String>,
    pub target_kind: String,
    pub raw_bytes: usize,
    pub compressor_results: Vec<SyntaxKCompressorResult>,
    pub k_est_bytes: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ncd_vs_baseline: Option<SyntaxKNcdSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub support_metrics: Option<serde_json::Value>,
    pub toolchain_fingerprint: serde_json::Value,
}

/// Input for one syntax-K observation.
#[derive(Debug, Clone)]
pub struct SyntaxKInput<'a> {
    pub fixture_id: &'a str,
    pub target_kind: &'a str,
    pub bytes: &'a [u8],
    pub source_hash: Option<&'a str>,
    pub web_ir_hash: Option<&'a str>,
    pub baseline_bytes: Option<&'a [u8]>,
    pub support_metrics: Option<serde_json::Value>,
}

/// Canonical SHA3-256 lowercase hex digest of arbitrary bytes.
pub fn sha3_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha3_256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// Recursively sort JSON object keys for cross-toolchain-stable canonical bytes.
///
/// Implementation lives in [`vox_compiler::canonical_json`] so analysis-side modules
/// can use it without depending on the emit-side `syntax_k` module. Re-exported
/// here for back-compat with existing callers (tests, CLI).
pub use vox_compiler::canonical_json::sort_json_value_keys;

/// Deterministic JSON bytes for WebIR (sorted keys at every object depth).
pub fn canonical_web_ir_bytes(module: &WebIrModule) -> Result<Vec<u8>, serde_json::Error> {
    let mut v = serde_json::to_value(module)?;
    sort_json_value_keys(&mut v);
    serde_json::to_vec(&v)
}

/// Deterministic canonical bytes for emitted file outputs.
pub fn canonical_emitted_files_bytes(files: &[(String, String)]) -> Vec<u8> {
    let mut sorted: Vec<(&String, &String)> = files.iter().map(|(n, c)| (n, c)).collect();
    sorted.sort_by(|a, b| a.0.cmp(b.0));
    let mut out = Vec::<u8>::new();
    for (name, content) in sorted {
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(b"\n---\n");
        out.extend_from_slice(content.as_bytes());
        out.extend_from_slice(b"\n===\n");
    }
    out
}

fn concat_for_ncd(left: &[u8], right: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(16 + left.len() + right.len());
    out.extend_from_slice(&(left.len() as u64).to_le_bytes());
    out.extend_from_slice(left);
    out.extend_from_slice(&(right.len() as u64).to_le_bytes());
    out.extend_from_slice(right);
    out
}

fn compress_gzip(bytes: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut encoder = GzEncoder::new(Vec::new(), GzCompression::best());
    encoder.write_all(bytes)?;
    encoder.finish()
}

fn compress_bzip2(bytes: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut encoder = BzEncoder::new(Vec::new(), BzCompression::best());
    encoder.write_all(bytes)?;
    encoder.finish()
}

fn compress_zstd(bytes: &[u8]) -> std::io::Result<Vec<u8>> {
    zstd::encode_all(bytes, 19)
}

fn compressor_results(bytes: &[u8]) -> std::io::Result<Vec<SyntaxKCompressorResult>> {
    let raw_len = bytes.len().max(1) as f64;
    let zstd = compress_zstd(bytes)?;
    let bzip2 = compress_bzip2(bytes)?;
    let gzip = compress_gzip(bytes)?;
    Ok(vec![
        SyntaxKCompressorResult {
            name: "zstd".to_string(),
            profile: PROFILE_ZSTD.to_string(),
            compressed_bytes: zstd.len(),
            ratio: zstd.len() as f64 / raw_len,
        },
        SyntaxKCompressorResult {
            name: "bzip2".to_string(),
            profile: PROFILE_BZIP2.to_string(),
            compressed_bytes: bzip2.len(),
            ratio: bzip2.len() as f64 / raw_len,
        },
        SyntaxKCompressorResult {
            name: "gzip".to_string(),
            profile: PROFILE_GZIP.to_string(),
            compressed_bytes: gzip.len(),
            ratio: gzip.len() as f64 / raw_len,
        },
    ])
}

fn ncd_for(
    name: &str,
    cx: usize,
    cy: usize,
    left: &[u8],
    right: &[u8],
) -> std::io::Result<SyntaxKNcdPerCompressor> {
    let xy = concat_for_ncd(left, right);
    let cxy = match name {
        "zstd" => compress_zstd(&xy)?.len(),
        "bzip2" => compress_bzip2(&xy)?.len(),
        "gzip" => compress_gzip(&xy)?.len(),
        _ => 0,
    };
    let minv = cx.min(cy) as f64;
    let maxv = cx.max(cy).max(1) as f64;
    let value = (cxy as f64 - minv) / maxv;
    Ok(SyntaxKNcdPerCompressor {
        name: name.to_string(),
        value,
    })
}

fn median(vals: &mut [f64]) -> f64 {
    if vals.is_empty() {
        return 0.0;
    }
    vals.sort_by(f64::total_cmp);
    let mid = vals.len() / 2;
    if vals.len() % 2 == 1 {
        vals[mid]
    } else {
        (vals[mid - 1] + vals[mid]) / 2.0
    }
}

/// Pipeline stages for [`enrich_syntax_k_support_metrics`] (`first_failing_stage`).
#[derive(Debug, Clone, Copy)]
pub struct RepresentabilityPayload {
    pub parse_ok: bool,
    pub hir_ok: bool,
    pub web_ir_validate_ok: bool,
    /// `None` when not applicable to this Syntax-K target (e.g. raw `webir_json` bytes only).
    pub emit_preview_ok: Option<bool>,
}

/// Attach `representability`, optional `llm_surface`, and optional `runtime_projection` to Syntax-K `support_metrics`.
#[must_use]
pub fn enrich_syntax_k_support_metrics(
    base: serde_json::Value,
    rep: RepresentabilityPayload,
    llm_surface: Option<serde_json::Value>,
    runtime_projection: Option<serde_json::Value>,
) -> serde_json::Value {
    let first = if !rep.parse_ok {
        Some("parse")
    } else if !rep.hir_ok {
        Some("hir")
    } else if !rep.web_ir_validate_ok {
        Some("web_ir_validate")
    } else if rep.emit_preview_ok == Some(false) {
        Some("emit_tsx_preview")
    } else {
        None
    };
    let rep_json = json!({
        "parse_ok": rep.parse_ok,
        "hir_ok": rep.hir_ok,
        "web_ir_validate_ok": rep.web_ir_validate_ok,
        "emit_preview_ok": rep.emit_preview_ok,
        "first_failing_stage": first,
    });

    let mut obj = match base {
        serde_json::Value::Object(m) => m,
        serde_json::Value::Null => serde_json::Map::new(),
        other => {
            let mut m = serde_json::Map::new();
            m.insert("base".to_string(), other);
            m
        }
    };
    obj.insert("representability".to_string(), rep_json);
    if let Some(ls) = llm_surface {
        obj.insert("llm_surface".to_string(), ls);
    }
    if let Some(rp) = runtime_projection {
        obj.insert("runtime_projection".to_string(), rp);
    }
    serde_json::Value::Object(obj)
}

fn toolchain_fingerprint() -> serde_json::Value {
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    json!({
        "compiler": format!("vox-compiler@{}", env!("CARGO_PKG_VERSION")),
        "compressor_profiles": [PROFILE_ZSTD, PROFILE_BZIP2, PROFILE_GZIP],
        "platform": format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        "timestamp_unix_ms": now_ms,
    })
}

/// Measure practical syntax-K event payload for one canonical output object.
pub fn measure_syntax_k_event(input: SyntaxKInput<'_>) -> std::io::Result<SyntaxKEvent> {
    let results = compressor_results(input.bytes)?;
    let k_est = results
        .iter()
        .map(|r| r.compressed_bytes)
        .min()
        .unwrap_or_default();

    let ncd_vs_baseline = if let Some(base) = input.baseline_bytes {
        let base_results = compressor_results(base)?;
        let mut per = Vec::new();
        for r in &results {
            if let Some(base_r) = base_results.iter().find(|x| x.name == r.name) {
                per.push(ncd_for(
                    &r.name,
                    r.compressed_bytes,
                    base_r.compressed_bytes,
                    input.bytes,
                    base,
                )?);
            }
        }
        let mut mvals: Vec<f64> = per.iter().map(|x| x.value).collect();
        Some(SyntaxKNcdSummary {
            per_compressor: per,
            median: median(&mut mvals),
        })
    } else {
        None
    };

    Ok(SyntaxKEvent {
        schema_version: SCHEMA_VERSION,
        fixture_id: input.fixture_id.to_string(),
        source_hash: input.source_hash.map(ToOwned::to_owned),
        web_ir_hash: input.web_ir_hash.map(ToOwned::to_owned),
        target_kind: input.target_kind.to_string(),
        raw_bytes: input.bytes.len(),
        compressor_results: results,
        k_est_bytes: k_est,
        ncd_vs_baseline,
        support_metrics: input.support_metrics,
        toolchain_fingerprint: toolchain_fingerprint(),
    })
}

#[cfg(test)]
mod semcov_wave14_tests {
    use super::*;

    #[test]
    fn sha3_hex_empty_input_is_not_empty_string() {
        // Catches: hasher returning the empty string for an empty payload instead of
        // the SHA3-256 digest of the empty sequence (64 hex chars).
        let h = sha3_hex(b"");
        assert_eq!(h.len(), 64, "SHA3-256 of empty bytes must be 64 hex chars, got {:?}", h);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()), "digest must be hex, got {:?}", h);
    }

    #[test]
    fn sha3_hex_deterministic_same_input() {
        // Invariant: sha3_hex is a pure function — same bytes must always yield
        // the same digest. Catches non-determinism from timestamp/salt mixing.
        let a = sha3_hex(b"hello world");
        let b = sha3_hex(b"hello world");
        assert_eq!(a, b, "sha3_hex must be deterministic");
    }

    #[test]
    fn sha3_hex_different_inputs_differ() {
        // Boundary: single-bit difference must produce different digests.
        // Catches accidental truncation that zeroes the digest.
        let a = sha3_hex(b"a");
        let b = sha3_hex(b"b");
        assert_ne!(a, b, "distinct inputs must yield distinct digests");
    }

    #[test]
    fn canonical_emitted_files_bytes_order_independent() {
        // Invariant: file order must not affect the canonical byte sequence.
        // Catches: sort missing, so insertion order bleeds into the hash.
        let files_ab = vec![
            ("alpha.rs".to_string(), "content A".to_string()),
            ("beta.rs".to_string(), "content B".to_string()),
        ];
        let files_ba = vec![
            ("beta.rs".to_string(), "content B".to_string()),
            ("alpha.rs".to_string(), "content A".to_string()),
        ];
        assert_eq!(
            canonical_emitted_files_bytes(&files_ab),
            canonical_emitted_files_bytes(&files_ba),
            "file order must not affect canonical bytes"
        );
    }

    #[test]
    fn canonical_emitted_files_bytes_empty_is_empty() {
        // Boundary: zero files must produce empty bytes — catches a sentinel header
        // being emitted even when the file list is empty.
        assert!(
            canonical_emitted_files_bytes(&[]).is_empty(),
            "empty file list must produce zero bytes"
        );
    }

    #[test]
    fn canonical_emitted_files_bytes_different_names_differ() {
        // Catches: name not included in the canonical bytes so two different files
        // with the same content collapse to identical outputs.
        let a = canonical_emitted_files_bytes(&[("x.rs".to_string(), "same".to_string())]);
        let b = canonical_emitted_files_bytes(&[("y.rs".to_string(), "same".to_string())]);
        assert_ne!(a, b, "different file names must produce different canonical bytes");
    }

    #[test]
    fn enrich_support_metrics_all_ok_has_no_first_failing_stage() {
        // Invariant: when every stage passes, first_failing_stage must be null.
        // Catches: logic short-circuiting on parse_ok=true but then defaulting
        // first_failing_stage to a non-null value.
        let rep = RepresentabilityPayload {
            parse_ok: true,
            hir_ok: true,
            web_ir_validate_ok: true,
            emit_preview_ok: Some(true),
        };
        let v = enrich_syntax_k_support_metrics(serde_json::Value::Null, rep, None, None);
        let first = &v["representability"]["first_failing_stage"];
        assert!(first.is_null(), "all-pass payload must have null first_failing_stage, got {first}");
    }

    #[test]
    fn enrich_support_metrics_parse_fail_wins_over_hir_fail() {
        // State-ordering: parse failure must be reported even when hir_ok is also
        // false. Catches: OR-chain evaluated in wrong order, reporting "hir" instead.
        let rep = RepresentabilityPayload {
            parse_ok: false,
            hir_ok: false,
            web_ir_validate_ok: false,
            emit_preview_ok: Some(false),
        };
        let v = enrich_syntax_k_support_metrics(serde_json::Value::Null, rep, None, None);
        assert_eq!(
            v["representability"]["first_failing_stage"].as_str(),
            Some("parse"),
            "parse must win as first_failing_stage, got {v}"
        );
    }
}
