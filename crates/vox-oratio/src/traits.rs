//! Public transcript types and the default entrypoints used by CLI / MCP.

use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::refine::{CorrectionContext, CorrectionTrace};

fn try_load_lexicon_path(path: &Path) -> Option<crate::speech_lexicon::SpeechLexicon> {
    let bytes = std::fs::read(path).ok()?;
    crate::speech_lexicon::SpeechLexicon::from_json_slice(&bytes).ok()
}

/// Loads and merges speech lexicons: explicit `VOX_ORATIO_SPEECH_LEXICON_PATH`, then optional
/// `<VOX_REPOSITORY_ROOT or VOX_REPO_ROOT>/.vox/speech_lexicon.json` when those roots are set.
fn load_lexicon_from_env() -> Option<crate::speech_lexicon::SpeechLexicon> {
    let mut acc = crate::speech_lexicon::SpeechLexicon::default();
    if let Some(p) =
        vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOratioSpeechLexiconPath).expose()
    {
        let path = Path::new(p.trim());
        if let Some(lex) = try_load_lexicon_path(path) {
            acc.merge_from(lex);
        }
    }
    let repo_root_resolved = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxRepositoryRoot);
    let repo_root = repo_root_resolved.expose();
    if let Some(root) = repo_root {
        let candidate = Path::new(root.trim()).join(vox_config::paths::REPO_SPEECH_LEXICON_FILE);
        if let Some(lex) = try_load_lexicon_path(&candidate) {
            acc.merge_from(lex);
        }
    }
    if acc.is_empty() { None } else { Some(acc) }
}

fn contextual_bias_phrases_with_lex(
    lex: Option<&crate::speech_lexicon::SpeechLexicon>,
) -> Vec<String> {
    const DEFAULT_MAX: usize = 256;
    let max_phrases: usize =
        vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOratioMaxBiasPhrases)
            .expose()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_MAX);
    let contextual_on = !matches!(
        vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOratioContextualBias).expose(),
        Some(s) if s == "0" || s.eq_ignore_ascii_case("false")
    );
    if !contextual_on {
        return Vec::new();
    }
    let lex_phrases = lex
        .map(|l| l.bias_phrases_sorted(max_phrases))
        .unwrap_or_default();
    let extra: Vec<String> =
        vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOratioSessionHotwords)
            .expose()
            .map(crate::contextual_bias::parse_hotword_csv)
            .unwrap_or_default();
    crate::contextual_bias::merge_bias_phrases(lex_phrases, &extra, max_phrases)
}

fn finalize_after_refine(
    raw_text: String,
    refined: crate::refine::RefineOutput,
) -> TranscribeDetail {
    let lex = load_lexicon_from_env();
    let refined_after_lex = lex
        .as_ref()
        .map(|l| l.apply(&refined.text))
        .unwrap_or_else(|| refined.text.clone());
    let bias = contextual_bias_phrases_with_lex(lex.as_ref());
    let candidates = crate::transcript_rerank::rerank_candidates_best_first_with_context(
        crate::transcript_rerank::build_transcript_candidates(&raw_text, &refined_after_lex),
        &bias,
        Some(raw_text.as_str()),
    );
    let refined_text = candidates
        .first()
        .cloned()
        .unwrap_or_else(|| refined_after_lex.clone());
    let n_best = (candidates.len() > 1).then_some(candidates);
    TranscribeDetail {
        raw_text,
        refined_text,
        confidence: refined.confidence,
        correction_trace: refined.trace,
        n_best,
    }
}

/// File- or segment-level transcription result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transcript {
    /// Raw model or file output before refinement.
    pub raw_text: String,
    /// Optional refined text (ITN, light cleanup). `None` if refinement skipped.
    pub refined_text: Option<String>,
}

impl Transcript {
    /// Prefer refined text when present, otherwise raw.
    #[must_use]
    pub fn display_text(&self) -> &str {
        self.refined_text.as_deref().unwrap_or(&self.raw_text)
    }
}

/// Deterministic transcription output including refinement metadata (for MCP / contracts).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscribeDetail {
    /// Raw model or file output before refinement.
    pub raw_text: String,
    /// Text after deterministic refinement.
    pub refined_text: String,
    /// Confidence estimate from the refinement stage.
    pub confidence: f32,
    /// Trace of applied refinement rules.
    pub correction_trace: Vec<CorrectionTrace>,
    /// When the STT backend exposes alternatives, list them here (best-first). Usually `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub n_best: Option<Vec<String>>,
}

impl TranscribeDetail {
    #[must_use]
    /// Display text (refined).
    pub fn text(&self) -> &str {
        &self.refined_text
    }
}

/// Human-readable description of which Oratio capabilities are active.
#[must_use]
pub fn transcript_status() -> &'static str {
    #[cfg(all(feature = "stt-sherpa", feature = "stt-candle"))]
    return "Vox Oratio: dual backends compiled — Sherpa-ONNX + Candle Whisper. \
            Active backend: VOX_ORATIO_BACKEND (auto|whisper|sherpa).";

    #[cfg(all(feature = "stt-sherpa", not(feature = "stt-candle")))]
    return "Vox Oratio: Sherpa-ONNX STT backend active. Env: VOX_ORATIO_BACKEND, \
            VOX_ORATIO_SHERPA_MODEL, VOX_ORATIO_SHERPA_MODEL_DIR.";

    #[cfg(all(feature = "stt-candle", not(feature = "stt-sherpa")))]
    return "Vox Oratio: Candle Whisper (Rust) STT enabled; symphonia decode + 16 kHz resample; \
            `.txt`/`.md` passthrough. Env: VOX_ORATIO_MODEL, VOX_ORATIO_REVISION, VOX_ORATIO_LANGUAGE, \
            VOX_ORATIO_CUDA (requires `cuda` feature); long audio: VOX_ORATIO_CHUNK_SEC, \
            VOX_ORATIO_CHUNK_OVERLAP_SEC, optional VOX_ORATIO_EMIT_PARTIAL_PATH (JSONL), \
            VOX_ORATIO_STREAM_TOKENS; constrained decode knobs: VOX_ORATIO_LOGIT_BIAS_STRENGTH, \
            VOX_ORATIO_LOGIT_BIAS_MAX_TOKENS, VOX_ORATIO_LOGIT_FORBID_TOKENS, \
            VOX_ORATIO_CONSTRAINED_TRIE, VOX_ORATIO_CONSTRAINED_PHRASES, VOX_ORATIO_TRIE_STUCK_STEPS.";

    #[cfg(all(not(feature = "stt-candle"), not(feature = "stt-sherpa")))]
    return "Vox Oratio: built without `stt-candle` or `stt-sherpa`; only `.txt`/`.md` transcript passthrough is available.";
}

/// Apply the full deterministic refinement pipeline (lexicon + rerank) to a raw transcript string.
///
/// Used by callers that obtain the raw transcription from an external source (e.g., a plugin)
/// and need the same correction pass as `transcribe_path_detailed`.
pub fn refine_raw_text(raw_text: &str, ctx: &CorrectionContext) -> TranscribeDetail {
    let refined = crate::refine::refine_transcript(raw_text, ctx);
    finalize_after_refine(raw_text.to_string(), refined)
}

/// Transcribe `path` with explicit refinement context and optional Whisper language override.
///
/// For `.txt` / `.md`, `language_hint` is ignored. For audio, it is forwarded to the Candle backend.
pub fn transcribe_path_detailed(
    path: &Path,
    ctx: &CorrectionContext,
    #[cfg_attr(not(feature = "stt-candle"), allow(unused_variables))] language_hint: Option<&str>,
) -> Result<TranscribeDetail> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    if matches!(ext.as_str(), "txt" | "md") {
        let raw_text = std::fs::read_to_string(path)
            .with_context(|| format!("read transcript fixture {}", path.display()))?;
        let refined = crate::refine::refine_transcript(&raw_text, ctx);
        return Ok(finalize_after_refine(raw_text, refined));
    }

    let is_audio_or_video = matches!(
        ext.as_str(),
        "wav"
            | "mp3"
            | "flac"
            | "ogg"
            | "oga"
            | "aac"
            | "m4a"
            | "opus"
            | "mp4"
            | "mkv"
            | "avi"
            | "webm"
            | "mov"
    );

    if is_audio_or_video {
        #[cfg(feature = "stt-candle")]
        {
            let (pcm, sample_rate) = match crate::backends::audio_io::pcm_decode_to_16k_mono(path) {
                Ok(res) => (res, 16_000u32),
                Err(e) => {
                    // If symphonia fails, try ffmpeg fallback for video containers
                    if matches!(ext.as_str(), "mp4" | "mkv" | "avi" | "webm" | "mov") {
                        tracing::info!("audio_io failed: {}, attempting ffmpeg fallback", e);
                        match crate::subtitle::ffmpeg_extract::extract_audio_ffmpeg(path) {
                            Ok(res) => (res, 16_000),
                            Err(e2) => {
                                anyhow::bail!(
                                    "audio extraction failed: {} (ffmpeg failed: {})",
                                    e,
                                    e2
                                )
                            }
                        }
                    } else {
                        anyhow::bail!("audio decode failed: {}", e);
                    }
                }
            };

            // Allow acoustic preprocess via AsrBackend path
            let budget_ms = vox_secrets::resolve_secret(
                vox_secrets::SecretId::VoxOratioAcousticPreprocessBudgetMs,
            )
            .expose()
            .and_then(|s| s.parse().ok())
            .unwrap_or(25u64);
            let (pcm, _diag) =
                crate::acoustic_preprocess::preprocess_audio_pcm_f32_reported(&pcm, budget_ms);

            let (_diag, whisper_lang) = crate::language::prepare_language_hint(language_hint);

            let backend = crate::backend_dispatch::create_backend()?;
            let out = backend.transcribe_pcm(&pcm, sample_rate, whisper_lang.as_deref())?;

            let refined = crate::refine::refine_transcript(&out.raw_text, ctx);
            return Ok(finalize_after_refine(out.raw_text, refined));
        }

        #[cfg(not(feature = "stt-candle"))]
        anyhow::bail!(
            "Vox Oratio: audio transcription requires stt-candle feature; \
             file: {}. Use vox-plugin-oratio for plugin-dispatched transcription.",
            path.display()
        );
    }

    anyhow::bail!(
        "Vox Oratio: unsupported extension {:?} for file {}.",
        path.extension().unwrap_or_default(),
        path.display()
    );
}

/// Transcribe `path` through the default Oratio pipeline.
///
/// # Supported inputs
///
/// - **`.txt` / `.md`**: UTF-8 content is read as the raw transcript; `refine::rules::light_trim`
///   produces [`Transcript::refined_text`].
/// - **Audio** (with `stt-candle`): common formats (e.g. wav, mp3, flac, ogg) via symphonia.
pub fn transcribe_path(path: &Path) -> Result<Transcript> {
    let d = transcribe_path_detailed(path, &CorrectionContext::default(), None)?;
    Ok(Transcript {
        raw_text: d.raw_text,
        refined_text: Some(d.refined_text),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::Builder;

    #[test]
    fn txt_fixture_roundtrip() {
        let mut f = Builder::new().suffix(".txt").tempfile().unwrap();
        writeln!(f, "  hello world  ").unwrap();
        let t = transcribe_path(f.path()).unwrap();
        assert_eq!(t.raw_text, "  hello world  \n");
        assert_eq!(t.refined_text.as_deref(), Some("hello world"));
        assert_eq!(t.display_text(), "hello world");
    }
}
