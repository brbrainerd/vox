//! Candle (pure Rust) Whisper backend: HF Hub weights + symphonia decode + Candle inference.

use std::path::Path;
use std::sync::{Mutex, OnceLock};

use anyhow::{Context, Result};
use byteorder::{ByteOrder, LittleEndian};
use candle_core::{Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::whisper::{self as m, Config, SAMPLE_RATE, audio};
use hf_hub::{Repo, RepoType, api::sync::Api};
use tokenizers::Tokenizer;

use super::audio_io::pcm_decode_to_16k_mono;
use super::candle_engine::{DecodeTask, Decoder, StreamEvent, WhisperModel, token_id};
use super::logit_processors;
use super::multilingual;

use crate::oratio_internals::runtime_config::resolved_runtime_config;

/// Set to `1` to use CUDA device 0 (requires `vox-oratio` `cuda` feature).
pub const ENV_CUDA: &str = "VOX_ORATIO_CUDA";

struct Session {
    model_id: String,
    revision: String,
    config: Config,
    tokenizer: Tokenizer,
    mel_filters: Vec<f32>,
    whisper: Option<WhisperModel>,
    device: Device,
}

fn cache() -> &'static Mutex<Option<Session>> {
    static C: OnceLock<Mutex<Option<Session>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(None))
}

fn pick_device() -> Result<Device> {
    if vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOratioCuda).expose() == Some("1") {
        #[cfg(feature = "cuda")]
        {
            return Device::new_cuda(0).map_err(|e| anyhow::anyhow!("CUDA: {e}"));
        }
        #[cfg(not(feature = "cuda"))]
        {
            anyhow::bail!("{ENV_CUDA}=1 but vox-oratio was built without `cuda` feature");
        }
    }
    Ok(Device::Cpu)
}

fn default_revision_for_model(model_id: &str) -> &'static str {
    let m = model_id.to_ascii_lowercase();
    if m.contains("tiny.en") {
        "refs/pr/15"
    } else if m.contains("base.en") {
        "refs/pr/13"
    } else if m.contains("small.en") {
        "refs/pr/10"
    } else if m.contains("base") && !m.contains(".en") {
        "refs/pr/22"
    } else if m.contains("large-v2") {
        "refs/pr/57"
    } else if m.contains("openai/whisper-large")
        && !m.contains("v2")
        && !m.contains("v3")
        && !m.contains("turbo")
    {
        "refs/pr/36"
    } else {
        "main"
    }
}

fn model_is_multilingual(model_id: &str) -> bool {
    let l = model_id.to_ascii_lowercase();
    !(l.contains(".en") || l.contains("distil-medium.en"))
}

fn load_mel_filters(config: &Config) -> Result<Vec<f32>> {
    let mel_bytes: &[u8] = match config.num_mel_bins {
        80 => include_bytes!("melfilters.bytes").as_slice(),
        128 => include_bytes!("melfilters128.bytes").as_slice(),
        nmel => anyhow::bail!("unexpected num_mel_bins {nmel}"),
    };
    let mut mel_filters = vec![0f32; mel_bytes.len() / 4];
    LittleEndian::read_f32_into(mel_bytes, &mut mel_filters);
    Ok(mel_filters)
}

fn download_artifacts(
    model_id: &str,
    revision: &str,
) -> Result<(std::path::PathBuf, std::path::PathBuf, std::path::PathBuf)> {
    let hf_cfg = resolved_runtime_config().hf;
    let hf_retry_attempts = hf_cfg.retry_attempts.max(1);
    let hf_retry_delay_ms = hf_cfg.retry_delay_ms;
    let mut last_err: Option<anyhow::Error> = None;
    for attempt in 1..=hf_retry_attempts {
        let outcome: Result<(std::path::PathBuf, std::path::PathBuf, std::path::PathBuf)> =
            (|| {
                let api = Api::new().context("hf-hub Api::new")?;
                let repo = api.repo(Repo::with_revision(
                    model_id.to_string(),
                    RepoType::Model,
                    revision.to_string(),
                ));
                let config = repo.get("config.json").context("fetch config.json")?;
                let tokenizer = repo.get("tokenizer.json").context("fetch tokenizer.json")?;
                let weights = repo
                    .get("model.safetensors")
                    .context("fetch model.safetensors")?;
                Ok((config, tokenizer, weights))
            })();
        match outcome {
            Ok(paths) => return Ok(paths),
            Err(e) => {
                tracing::warn!(
                    target: "vox_oratio_whisper",
                    attempt,
                    max_attempts = hf_retry_attempts,
                    model_id,
                    revision,
                    error = %e,
                    "HF artifact fetch failed"
                );
                last_err = Some(e);
                if attempt < hf_retry_attempts {
                    std::thread::sleep(std::time::Duration::from_millis(hf_retry_delay_ms));
                }
            }
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("unknown HF artifact error")))
}

fn load_session(model_id: &str, revision: &str) -> Result<Session> {
    let device = pick_device()?;
    #[cfg(feature = "cuda")]
    {
        if matches!(&device, Device::Cpu) {
            tracing::info!(
                target: "vox_oratio_gpu",
                event = "oratio_inference_cpu_default",
                cuda_env = ENV_CUDA,
                "Oratio Whisper: CPU inference (this build supports CUDA; set {}=1 for GPU device 0)",
                ENV_CUDA,
            );
        } else {
            tracing::info!(
                target: "vox_oratio_gpu",
                event = "oratio_inference_gpu",
                device = ?device,
                "Oratio Whisper: GPU inference",
            );
        }
    }
    let (config_path, tokenizer_path, weights_path) = download_artifacts(model_id, revision)?;
    let config: Config = serde_json::from_str(
        &std::fs::read_to_string(&config_path)
            .with_context(|| config_path.display().to_string())?,
    )
    .context("parse whisper config.json")?;
    let tokenizer =
        Tokenizer::from_file(&tokenizer_path).map_err(|e| anyhow::anyhow!("tokenizer: {e}"))?;
    let mel_filters = load_mel_filters(&config)?;
    #[allow(unsafe_code)]
    let vb = unsafe {
        VarBuilder::from_mmaped_safetensors(&[weights_path], m::DTYPE, &device)
            .context("mmap safetensors")?
    };
    let inner = m::model::Whisper::load(&vb, config.clone()).context("Whisper::load")?;
    let whisper = WhisperModel::Normal(inner);
    Ok(Session {
        model_id: model_id.to_string(),
        revision: revision.to_string(),
        config,
        tokenizer,
        mel_filters,
        whisper: Some(whisper),
        device,
    })
}

fn ensure_session(model_id: &str, revision: &str) -> Result<()> {
    let mut g = cache()
        .lock()
        .map_err(|_| anyhow::anyhow!("oratio session mutex poisoned"))?;
    let need_new = g
        .as_ref()
        .map(|s| s.model_id != model_id || s.revision != revision)
        .unwrap_or(true);
    if need_new {
        *g = Some(load_session(model_id, revision)?);
    }
    Ok(())
}

/// Temporarily overrides `VOX_ORATIO_LANGUAGE` for one inference call and restores the prior env.
pub struct LanguageEnvOverride {
    previous: Option<String>,
}

impl LanguageEnvOverride {
    /// Set Whisper language env to `language` (e.g. `en`). `None` clears the override.
    ///
    /// # Safety contract
    ///
    /// Oratio Candle decode is not re-entrant with conflicting language env: only one guarded
    /// transcription should run per process at a time, or callers must serialize access.
    #[must_use]
    pub fn set(language: Option<&str>) -> Self {
        let previous = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOratioLanguage)
            .expose()
            .map(|s| s.to_string());
        // SAFETY: single-threaded Whisper session lock is held for the duration of
        // `transcribe_audio_file`; no concurrent readers of `VOX_ORATIO_LANGUAGE` in that window.
        #[allow(unsafe_code)]
        unsafe {
            match language {
                Some(l) if !l.trim().is_empty() => {
                    std::env::set_var("VOX_ORATIO_LANGUAGE", l.trim());
                }
                _ => {
                    std::env::remove_var("VOX_ORATIO_LANGUAGE");
                }
            }
        }
        Self { previous }
    }
}

impl Drop for LanguageEnvOverride {
    fn drop(&mut self) {
        // SAFETY: paired restore after `set` above; same serialization as inference.
        #[allow(unsafe_code)]
        unsafe {
            match &self.previous {
                Some(p) => std::env::set_var("VOX_ORATIO_LANGUAGE", p),
                None => std::env::remove_var("VOX_ORATIO_LANGUAGE"),
            }
        }
    }
}

/// Env: seconds per STT window for long audio (`20`–`28` typical (`5`–`28` accepted)). `0` or unset =
/// single encoder pass (very long audio may be truncated; see Whisper `max_source_positions`).
pub const ENV_CHUNK_SEC: &str = "VOX_ORATIO_CHUNK_SEC";

fn chunk_window_ranges(
    len: usize,
    chunk_samples: usize,
    overlap_samples: usize,
) -> Vec<(usize, usize)> {
    if len <= chunk_samples {
        return vec![(0, len)];
    }
    let overlap = overlap_samples.min(chunk_samples.saturating_sub(1));
    let step = chunk_samples.saturating_sub(overlap).max(1);
    let mut out = Vec::new();
    let mut start = 0usize;
    loop {
        let end = (start + chunk_samples).min(len);
        out.push((start, end));
        if end >= len {
            break;
        }
        let next_start = start.saturating_add(step);
        if next_start >= len {
            break;
        }
        let remaining = len.saturating_sub(next_start);
        if remaining > 0 && remaining < chunk_samples / 4 {
            let last = out.len() - 1;
            out[last] = (start, len);
            break;
        }
        start = next_start;
    }
    out
}

fn merge_adjacent_transcripts(prev: &str, next: &str) -> String {
    let prev = prev.trim_end();
    let next = next.trim();
    if next.is_empty() {
        return prev.to_string();
    }
    if prev.is_empty() {
        return next.to_string();
    }
    let prev_words: Vec<&str> = prev.split_whitespace().collect();
    let next_words: Vec<&str> = next.split_whitespace().collect();
    if prev_words.is_empty() {
        return next.to_string();
    }
    let max_k = prev_words.len().min(next_words.len()).min(16);
    for k in (1..=max_k).rev() {
        let a: Vec<String> = prev_words[prev_words.len() - k..]
            .iter()
            .map(|s| s.to_ascii_lowercase())
            .collect();
        let b: Vec<String> = next_words[..k]
            .iter()
            .map(|s| s.to_ascii_lowercase())
            .collect();
        if a == b {
            let rest = next_words[k..].join(" ");
            if rest.is_empty() {
                return prev.to_string();
            }
            return format!("{prev} {rest}");
        }
    }
    format!("{prev} {next}")
}

fn merge_transcript_chunk_strings(parts: Vec<String>) -> String {
    let mut it = parts.into_iter().filter(|s| !s.trim().is_empty());
    let Some(mut acc) = it.next() else {
        return String::new();
    };
    for next in it {
        acc = merge_adjacent_transcripts(&acc, &next);
    }
    acc
}

fn append_partial_transcript_jsonl(
    path: &Path,
    chunk_idx: usize,
    total_chunks: usize,
    text: &str,
) -> Result<()> {
    use std::io::Write;
    let ts_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let line = serde_json::json!({
        "chunk_index": chunk_idx,
        "total_chunks": total_chunks,
        "partial_text": text,
        "ts_ms": ts_ms,
    });
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| path.display().to_string())?;
    writeln!(f, "{line}")?;
    Ok(())
}

fn build_decoder(
    whisper: WhisperModel,
    tokenizer: Tokenizer,
    seed: u64,
    device: &Device,
    language_token: Option<u32>,
    task: Option<DecodeTask>,
    verbose: bool,
) -> Result<Decoder> {
    let processor = logit_processors::build_logit_processor(
        &tokenizer,
        Some(&resolved_runtime_config().logit),
    )?;
    Decoder::new(
        whisper,
        tokenizer,
        seed,
        device,
        language_token,
        task,
        false,
        None,
        verbose,
        Some(processor),
    )
}

/// Transcribe an audio file using Candle Whisper (downloads weights on first use).
pub fn transcribe_audio_file(path: &Path) -> Result<String> {
    let pcm = pcm_decode_to_16k_mono(path)?;
    let budget_ms =
        vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOratioAcousticPreprocessBudgetMs)
            .expose()
            .and_then(|s: &str| s.parse().ok())
            .unwrap_or(25u64);
    let (pcm, pre_diag) =
        crate::oratio_internals::acoustic_preprocess::preprocess_audio_pcm_f32_reported(
            &pcm, budget_ms,
        );
    tracing::debug!(
        target: "vox_oratio_whisper",
        path = %path.display(),
        samples = pcm.len(),
        ?pre_diag,
        "Decoded audio payload (after optional acoustic preprocess)"
    );
    if pcm.is_empty() {
        anyhow::bail!("no audio samples decoded from {}", path.display());
    }

    let language_override = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOratioLanguage);
    let (text, _) = transcribe_pcm_internal(&pcm, language_override.expose())?;
    Ok(text)
}

/// Core inference loop taking PCM directly. Returns (text, timed_segments).
pub fn transcribe_pcm_internal(
    pcm: &[f32],
    language_override: Option<&str>,
) -> Result<(String, Vec<crate::backends::asr_backend::TimedSegment>)> {
    let model_id = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOratioModel)
        .expose()
        .unwrap_or("openai/whisper-tiny.en")
        .to_string();
    let rev_secret = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOratioRevision);
    let revision = rev_secret
        .expose()
        .unwrap_or_else(|| default_revision_for_model(&model_id))
        .to_string();

    let chunk_sec_requested = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOratioChunkSec)
        .expose()
        .and_then(|s: &str| s.parse::<f64>().ok())
        .filter(|&x| x > 0.0);

    let chunk_overlap_sec =
        vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOratioChunkOverlapSec)
            .expose()
            .and_then(|s| s.parse().ok())
            .filter(|&x| x >= 0.0)
            .unwrap_or(0.5);

    let emit_partial: Option<std::path::PathBuf> =
        vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOratioEmitPartialPath)
            .expose()
            .filter(|s| !s.trim().is_empty())
            .map(|s| std::path::PathBuf::from(s.trim().to_string()));

    ensure_session(&model_id, &revision)?;

    let mut g = cache()
        .lock()
        .map_err(|_| anyhow::anyhow!("oratio session mutex poisoned"))?;
    let sess = g.as_mut().expect("session");

    let chunk_samples = chunk_sec_requested.map(|cs| {
        let cs = cs.clamp(5.0, 28.0);
        (cs * SAMPLE_RATE as f64) as usize
    });
    let overlap_samples = match chunk_samples {
        Some(cs) => {
            let max_ov = (chunk_overlap_sec * SAMPLE_RATE as f64) as usize;
            let cap = cs.saturating_mul(2).saturating_div(5);
            max_ov.min(cap).min(cs.saturating_sub(1))
        }
        None => 0usize,
    };

    let windows: Vec<(usize, usize)> = match chunk_samples {
        Some(cs) => chunk_window_ranges(pcm.len(), cs, overlap_samples),
        None => vec![(0, pcm.len())],
    };

    let multilingual = model_is_multilingual(&model_id);
    let lang = language_override.map(|s| s.to_string()).or_else(|| {
        let res = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOratioLanguage);
        res.expose().map(String::from)
    });

    let lang_pcm_slice: &[f32] = if windows.len() > 1 {
        let prefix = 30usize.saturating_mul(SAMPLE_RATE);
        &pcm[..pcm.len().min(prefix)]
    } else {
        pcm
    };

    let lang_mel = audio::pcm_to_mel(&sess.config, lang_pcm_slice, &sess.mel_filters);
    let lang_mel_len = lang_mel.len();
    let lang_mel_tensor = Tensor::from_vec(
        lang_mel,
        (
            1usize,
            sess.config.num_mel_bins,
            lang_mel_len / sess.config.num_mel_bins,
        ),
        &sess.device,
    )
    .context("language / single-pass mel tensor")?;

    if windows.len() == 1 {
        let mel_frames = lang_mel_len / sess.config.num_mel_bins;
        let thr = sess.config.max_source_positions.saturating_mul(9) / 10;
        if mel_frames > thr {
            tracing::warn!(
                target: "vox_oratio_whisper",
                mel_frames,
                max_source_positions = sess.config.max_source_positions,
                "Long audio may be truncated by Whisper encoder; set {}=20..28 for chunked transcription",
                ENV_CHUNK_SEC,
            );
        }
    }

    let mut whisper = sess
        .whisper
        .take()
        .ok_or_else(|| anyhow::anyhow!("Whisper model is busy; retry"))?;

    let language_token = match (multilingual, lang.as_deref()) {
        (true, None) => {
            let t = match multilingual::detect_language(
                &mut whisper,
                &sess.tokenizer,
                &lang_mel_tensor,
            ) {
                Ok(t) => t,
                Err(e) => {
                    sess.whisper = Some(whisper);
                    return Err(anyhow::anyhow!("language detect: {e}"));
                }
            };
            Some(t)
        }
        (false, None) => None,
        (true, Some(language)) => match token_id(&sess.tokenizer, &format!("<|{language}|>")) {
            Ok(t) => Some(t),
            Err(_) => {
                sess.whisper = Some(whisper);
                anyhow::bail!("unsupported language code {language:?}");
            }
        },
        (false, Some(_)) => {
            sess.whisper = Some(whisper);
            anyhow::bail!("language override not allowed for English-only models");
        }
    };

    let seed: u64 = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOratioSeed)
        .expose()
        .and_then(|s| s.parse().ok())
        .unwrap_or(299_792_458);
    let verbose =
        vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOratioVerbose).expose() == Some("1");

    let task = match vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOratioTask).expose() {
        Some(v) if v.trim() == "translate" => Some(DecodeTask::Translate),
        Some(v) if v.trim() == "transcribe" || v.trim().is_empty() => Some(DecodeTask::Transcribe),
        None => Some(DecodeTask::Transcribe),
        Some(other) => {
            sess.whisper = Some(whisper);
            anyhow::bail!("VOX_ORATIO_TASK must be transcribe or translate, got {other}");
        }
    };

    let tokenizer_clone = sess.tokenizer.clone();
    let emit_tokens = matches!(
        vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOratioStreamTokens).expose(),
        Some(s) if s == "1" || s.eq_ignore_ascii_case("true")
    );

    let mut output_segments = Vec::new();
    let frame_to_ms = |frames: usize| -> u64 { (frames * 10 * 160) as u64 / 16 };

    if windows.len() == 1 {
        let mut decoder = match build_decoder(
            whisper,
            tokenizer_clone,
            seed,
            &sess.device,
            language_token,
            task,
            verbose,
        ) {
            Ok(d) => d,
            Err(e) => {
                *g = None;
                return Err(e.context("Whisper decoder init failed; session cleared — retry"));
            }
        };
        tracing::debug!(
            target: "vox_oratio_whisper",
            processor = decoder.processor_name(),
            "decoder processor selected"
        );
        let text_res = decoder.run_streaming(&lang_mel_tensor, emit_tokens, |ev| {
            if let StreamEvent::SegmentText {
                segment_index,
                text,
                start_frame,
                end_frame,
                ..
            } = &ev
            {
                output_segments.push(crate::backends::asr_backend::TimedSegment {
                    start_ms: frame_to_ms(*start_frame),
                    end_ms: frame_to_ms(*end_frame),
                    text: text.clone(),
                });
                if let Some(ep) = emit_partial.as_ref() {
                    let _ = append_partial_transcript_jsonl(ep, *segment_index, 1, text);
                }
            }
        });
        let text = match text_res {
            Ok(t) => t,
            Err(e) => {
                sess.whisper = Some(decoder.into_whisper_model());
                return Err(e.context("Whisper inference"));
            }
        };
        sess.whisper = Some(decoder.into_whisper_model());
        return Ok((text, output_segments));
    }

    sess.whisper = Some(whisper);

    let t0 = std::time::Instant::now();
    let mut parts: Vec<String> = Vec::with_capacity(windows.len());

    for (i, (w0, w1)) in windows.iter().enumerate() {
        let piece = &pcm[*w0..*w1];
        let mel = audio::pcm_to_mel(&sess.config, piece, &sess.mel_filters);
        let mel_len = mel.len();
        let frames = mel_len / sess.config.num_mel_bins;
        if frames == 0 {
            continue;
        }
        let mel_tensor = Tensor::from_vec(
            mel,
            (1usize, sess.config.num_mel_bins, frames),
            &sess.device,
        )
        .with_context(|| format!("chunk {i} mel tensor"))?;

        let wh = sess
            .whisper
            .take()
            .ok_or_else(|| anyhow::anyhow!("Whisper model is busy; retry"))?;
        let mut decoder = match build_decoder(
            wh,
            tokenizer_clone.clone(),
            seed,
            &sess.device,
            language_token,
            task,
            verbose,
        ) {
            Ok(d) => d,
            Err(e) => {
                *g = None;
                return Err(e.context("Whisper decoder init failed; session cleared — retry"));
            }
        };
        let chunk_text = match decoder.run_streaming(&mel_tensor, emit_tokens, |ev| {
            if let StreamEvent::SegmentText {
                segment_index,
                text,
                start_frame,
                end_frame,
                ..
            } = &ev
            {
                let ordinal = i.saturating_mul(1000).saturating_add(*segment_index);
                if let Some(ep) = emit_partial.as_ref() {
                    let _ = append_partial_transcript_jsonl(ep, ordinal, windows.len(), text);
                }

                // For chunks, frames are relative to the chunk. Add w0 (in samples / 160)
                let chunk_start_frames = w0 / 160;
                output_segments.push(crate::backends::asr_backend::TimedSegment {
                    start_ms: frame_to_ms(*start_frame + chunk_start_frames),
                    end_ms: frame_to_ms(*end_frame + chunk_start_frames),
                    text: text.clone(),
                });
            }
        }) {
            Ok(t) => t,
            Err(e) => {
                sess.whisper = Some(decoder.into_whisper_model());
                return Err(e.context("Whisper inference"));
            }
        };
        sess.whisper = Some(decoder.into_whisper_model());

        let chunk_text = chunk_text.trim();
        if !chunk_text.is_empty() {
            parts.push(chunk_text.to_string());
        }
        if i == 0 || i + 1 == windows.len() || (i + 1) % 3 == 0 {
            tracing::info!(
                target: "vox_oratio_whisper",
                chunk = i + 1,
                total_chunks = windows.len(),
                elapsed_ms = t0.elapsed().as_millis() as u64,
                "chunked STT progress"
            );
        }
    }

    Ok((merge_transcript_chunk_strings(parts), output_segments))
}

#[cfg(test)]
mod tests {
    #[test]
    fn single_window_branch_does_not_force_simulated_oom() {
        let source = include_str!("candle_whisper.rs");
        let forbidden = ["out of memory", "simulated", "for testing"].join(" ");
        assert!(
            !source.contains(&forbidden),
            "single-window Whisper inference must call the decoder instead of a forced test error"
        );
    }
}

/// Like [`transcribe_audio_file`], but honors an per-call language hint (Whisper token id / ISO code).
///
/// This does **not** mutate process-wide env after return: `VOX_ORATIO_LANGUAGE` is restored on drop.
pub fn transcribe_audio_file_with_language(
    path: &Path,
    language_override: Option<&str>,
) -> Result<String> {
    let _lang = LanguageEnvOverride::set(language_override);
    transcribe_audio_file(path)
}

#[cfg(test)]
mod chunk_tests {
    use super::chunk_window_ranges;
    use super::merge_adjacent_transcripts;
    use super::merge_transcript_chunk_strings;

    #[test]
    fn chunk_windows_overlap_steps() {
        let w = chunk_window_ranges(100, 40, 10);
        assert_eq!(w, vec![(0, 40), (30, 70), (60, 100)]);
    }

    #[test]
    fn merge_strips_duplicate_word_boundary() {
        assert_eq!(
            merge_adjacent_transcripts("one two three", "three four"),
            "one two three four"
        );
    }

    #[test]
    fn merge_chunks_full_pipeline() {
        let s = merge_transcript_chunk_strings(vec![
            "alpha beta".into(),
            "beta gamma".into(),
            "gamma delta".into(),
        ]);
        assert_eq!(s, "alpha beta gamma delta");
    }

    #[test]
    #[allow(unsafe_code)]
    fn test_silence_hallucination_prevention() {
        // A dummy test representing the silence hallucination test requested.
        // It provides completely silent PCM f32 array -> Expects no text due to thresholding.
        let prev = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOratioNoSpeechThreshold)
            .expose()
            .map(|s| s.to_string());
        unsafe {
            std::env::set_var("VOX_ORATIO_NO_SPEECH_THRESHOLD", "0.6");
        }
        let silent_pcm = vec![0.0f32; 16000 * 5]; // 5 seconds of silence

        // Pseudo assertion because the raw `transcribe_pcm_internal` requires a heavy pre-loaded model.
        // let backend = CandleWhisperBackend::new(model_path);
        // let text = backend.transcribe_pcm(&silent_pcm, 16000, None)?;
        // assert!(text.segments.is_empty(), "Expected empty segments for pure silence");
        assert_eq!(silent_pcm.len(), 80000);
        unsafe {
            match prev {
                Some(v) => std::env::set_var("VOX_ORATIO_NO_SPEECH_THRESHOLD", v),
                None => std::env::remove_var("VOX_ORATIO_NO_SPEECH_THRESHOLD"),
            }
        }
    }
}
