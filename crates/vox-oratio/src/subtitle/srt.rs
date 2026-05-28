#[cfg(any(feature = "stt-candle", feature = "stt-sherpa"))]
use std::fs::File;
#[cfg(any(feature = "stt-candle", feature = "stt-sherpa"))]
use std::io::Write;
#[cfg(any(feature = "stt-candle", feature = "stt-sherpa"))]
use std::path::{Path, PathBuf};

#[cfg(any(feature = "stt-candle", feature = "stt-sherpa"))]
use anyhow::Context;

use crate::backends::asr_backend::TimedSegment;

/// Formats a list of segments into an SRT subtitle string.
pub fn format_srt(segments: &[TimedSegment], max_line_width: usize, max_lines: usize) -> String {
    let mut srt_out = String::new();
    let mut block_idx = 1;

    for seg in segments {
        if seg.text.trim().is_empty() {
            continue;
        }

        let start_fmt = ms_to_srt_time(seg.start_ms);
        let end_fmt = ms_to_srt_time(seg.end_ms);

        let lines = wrap_text(&seg.text, max_line_width, max_lines);

        srt_out.push_str(&format!("{}\n", block_idx));
        srt_out.push_str(&format!("{} --> {}\n", start_fmt, end_fmt));
        for line in lines {
            srt_out.push_str(&format!("{}\n", line));
        }
        srt_out.push('\n');
        block_idx += 1;
    }

    srt_out
}

fn ms_to_srt_time(ms: u64) -> String {
    let hours = ms / 3600000;
    let mins = (ms % 3600000) / 60000;
    let secs = (ms % 60000) / 1000;
    let millis = ms % 1000;
    format!("{:02}:{:02}:{:02},{:03}", hours, mins, secs, millis)
}

fn wrap_text(text: &str, max_line_width: usize, max_lines: usize) -> Vec<String> {
    let words = text.split_whitespace();
    let mut lines = Vec::new();
    let mut current_line = String::new();

    for word in words {
        if current_line.is_empty() {
            current_line.push_str(word);
        } else if current_line.len() + 1 + word.len() <= max_line_width {
            current_line.push(' ');
            current_line.push_str(word);
        } else {
            lines.push(current_line);
            current_line = word.to_string();
        }
    }
    if !current_line.is_empty() {
        lines.push(current_line);
    }

    // Attempt to merge or truncate if exceeding max_lines.
    if lines.len() > max_lines && max_lines > 0 {
        // Just take the top ones for now. In a real system, you'd split the TimedSegment itself.
        lines.truncate(max_lines);
    }

    lines
}

/// Parse a basic SRT subtitle file format into a list of TimedSegments.
pub fn parse_srt_basic(content: &str) -> Vec<TimedSegment> {
    let mut segments = Vec::new();
    let parts = content.split("\n\n");
    for block in parts {
        let lines: Vec<&str> = block
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .collect();
        if lines.len() >= 3 {
            // roughly line 1 = index, line 2 = time, line 3+ = text
            let time_line = lines[1];
            if let Some((start_str, end_str)) = time_line.split_once(" --> ") {
                let start_ms = parse_srt_time(start_str).unwrap_or(0);
                let end_ms = parse_srt_time(end_str).unwrap_or(0);
                let text = lines[2..].join(" ");
                segments.push(TimedSegment {
                    start_ms,
                    end_ms,
                    text,
                });
            }
        }
    }
    segments
}

fn parse_srt_time(s: &str) -> Option<u64> {
    let mut parts = s.split(',');
    let hms = parts.next()?;
    let ms_str = parts.next()?;
    let mut hms_parts = hms.split(':');
    let h: u64 = hms_parts.next()?.parse().ok()?;
    let m: u64 = hms_parts.next()?.parse().ok()?;
    let s_sec: u64 = hms_parts.next()?.parse().ok()?;
    let ms: u64 = ms_str.parse().ok()?;
    Some(h * 3600000 + m * 60000 + s_sec * 1000 + ms)
}

/// Generates an SRT file by processing audio from an input media path.
/// Handles audio extraction, preprocessing, and speech-to-text inference.
#[cfg(any(feature = "stt-candle", feature = "stt-sherpa"))]
pub fn generate_srt_file(
    input_path: String,
    explicit_output: Option<String>,
    language: Option<String>,
    line_width: usize,
    max_lines: usize,
    ground_truth_srt: Option<String>,
    _persist: bool,
) -> anyhow::Result<Option<(f64, f64, f32)>> {
    let path = Path::new(&input_path);
    if !path.exists() {
        anyhow::bail!("Input file does not exist: {}", path.display());
    }

    let output_path = if let Some(p) = explicit_output {
        PathBuf::from(p)
    } else {
        path.with_extension("srt")
    };

    println!("Extracting audio and transcribing (this may take a while)...");

    let ext = path.extension().unwrap_or_default().to_string_lossy();
    let is_audio_or_video = matches!(
        ext.as_ref(),
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

    if !is_audio_or_video {
        anyhow::bail!("Unsupported file extension: {}", ext);
    }

    let (mut pcm, sample_rate) = match crate::backends::audio_io::pcm_decode_to_16k_mono(path) {
        Ok(res) => (res, 16_000),
        Err(e) => {
            if matches!(ext.as_ref(), "mp4" | "mkv" | "avi" | "webm" | "mov") {
                println!("Symphonia failed: {}. Trying ffmpeg fallback...", e);
                match super::ffmpeg_extract::extract_audio_ffmpeg(path) {
                    Ok(res) => (res, 16_000),
                    Err(e2) => {
                        anyhow::bail!("audio extraction failed: {} (ffmpeg failed: {})", e, e2)
                    }
                }
            } else {
                anyhow::bail!("audio decode failed: {}", e);
            }
        }
    };

    let budget_ms =
        vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOratioAcousticPreprocessBudgetMs)
            .expose()
            .and_then(|s: &str| s.parse().ok())
            .unwrap_or(25u64);
    pcm = crate::acoustic_preprocess::preprocess_audio_pcm_f32_reported(&pcm, budget_ms).0;

    let (_diag, whisper_lang) = crate::language::prepare_language_hint(language.as_deref());
    let backend = crate::backend_dispatch::create_backend()?;

    let out = backend
        .transcribe_pcm(&pcm, sample_rate, whisper_lang.as_deref())
        .context("transcribe_pcm failed")?;

    if out.segments.is_empty() {
        println!("Warning: Backbone returned no segments. Fallback block generation used.");
        // Try to fake a single segment if there is raw text.
        if !out.raw_text.trim().is_empty() {
            let duration_ms = (pcm.len() as f64 / 16.0) as u64; // roughly 16 samples per ms
            let srt = format_srt(
                &[TimedSegment {
                    start_ms: 0,
                    end_ms: duration_ms,
                    text: out.raw_text,
                }],
                line_width,
                max_lines,
            );

            let mut file = File::create(&output_path).context("Failed to create SRT file")?;
            file.write_all(srt.as_bytes())
                .context("Failed to write SRT file")?;
            println!("Subtitle generated: {}", output_path.display());
            return Ok(None);
        }
        anyhow::bail!("No transcription segments returned and text was empty.");
    }

    let srt_content = format_srt(&out.segments, line_width, max_lines);

    let mut file = File::create(&output_path).context("Failed to create SRT file")?;
    file.write_all(srt_content.as_bytes())
        .context("Failed to write SRT file")?;

    println!("Subtitle generated: {}", output_path.display());

    let mut metrics = None;

    if let Some(gt_path) = ground_truth_srt
        && let Ok(gt_content) = std::fs::read_to_string(&gt_path)
    {
        let gt_segs = parse_srt_basic(&gt_content);
        let offset = crate::eval_srt::mean_timing_offset_ms(&gt_segs, &out.segments);
        println!("Ground Truth Evaluation against: {}", gt_path);
        println!("Mean Timing Offset (TER proxy): {:.1} ms", offset);

        // Collect single raw text line for basic WER comparison
        let gt_text: Vec<_> = gt_segs.iter().map(|s| s.text.as_str()).collect();
        let actual_text: Vec<_> = out.segments.iter().map(|s| s.text.as_str()).collect();
        let wer = crate::eval::word_error_rate(&gt_text.join(" "), &actual_text.join(" "));
        let cer = crate::eval::char_error_rate(&gt_text.join(" "), &actual_text.join(" "));
        println!("Overall WER: {:.2}%", wer * 100.0);
        println!("Overall CER: {:.2}%", cer * 100.0);

        metrics = Some((wer, cer, offset));
    }

    Ok(metrics)
}

/// Stub used when no STT backend feature is enabled — returns an error indicating
/// `stt-candle` (or another STT feature) must be enabled to generate SRT files.
#[cfg(not(any(feature = "stt-candle", feature = "stt-sherpa")))]
pub fn generate_srt_file(
    input_path: String,
    _explicit_output: Option<String>,
    _language: Option<String>,
    _line_width: usize,
    _max_lines: usize,
    _ground_truth_srt: Option<String>,
    _persist: bool,
) -> anyhow::Result<Option<(f64, f64, f32)>> {
    anyhow::bail!(
        "generate_srt_file requires stt-candle or stt-sherpa feature; file: {}",
        input_path
    )
}
