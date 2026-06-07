//! Microphone capture + on-device transcription for the Loquela composer.
//!
//! Design: two Tauri commands form a record-then-transcribe flow.
//! - [`start_mic_capture`] opens the default input device with `cpal`, streams
//!   f32 samples into an in-memory buffer on a background audio thread, and
//!   stores the live stream handle in Tauri managed state.
//! - [`stop_mic_capture_and_transcribe`] tears the stream down, writes the
//!   captured samples to a 16 kHz mono WAV (Whisper's expected format) with
//!   `hound`, runs [`vox_speech::transcribe_path_detailed`], and returns the
//!   refined transcript. The temp WAV is always cleaned up.
//!
//! The cpal capture itself cannot be unit-tested without an audio device, so the
//! testable surface — turning a WAV/transcript file on disk into refined text —
//! is factored into the pure [`transcribe_audio_file`] function, which the unit
//! tests exercise directly. The live cpal glue stays thin and degrades to a real
//! error (never a panic) when no input device is present.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use vox_speech::refine::CorrectionContext;

/// Whisper consumes 16 kHz mono PCM. We resample the device stream to this rate.
const TARGET_SAMPLE_RATE: u32 = 16_000;

/// Pure, testable core: refine a transcript out of an audio (or `.txt`/`.md`)
/// file on disk. This is the seam the unit tests drive — it has no dependency on
/// a live microphone, only on the file path.
///
/// Returns the refined transcript text. Errors are surfaced verbatim (e.g. a
/// missing `stt-candle` backend or an undecodable file) — never a panic.
pub fn transcribe_audio_file(path: &Path) -> Result<String, String> {
    let ctx = CorrectionContext::default();
    let detail = vox_speech::transcribe_path_detailed(path, &ctx, None)
        .map_err(|e| format!("transcription failed: {e}"))?;
    Ok(detail.refined_text)
}

/// Shared capture buffer: f32 samples plus the source device sample rate so we
/// can resample to 16 kHz on finalize.
#[derive(Default)]
struct CaptureBuffer {
    samples: Vec<f32>,
    source_sample_rate: u32,
    channels: u16,
}

/// Live recording session held in Tauri managed state between the two commands.
/// The cpal `Stream` is `!Send`, so we keep it on its owning thread and only move
/// the shared buffer + a stop signal across the command boundary.
pub struct MicSession {
    stop_tx: std::sync::mpsc::Sender<()>,
    done_rx: std::sync::mpsc::Receiver<CaptureBuffer>,
}

/// Tauri-managed slot for the (optional) active recording session.
#[derive(Default)]
pub struct MicCaptureState {
    session: Mutex<Option<MicSession>>,
}

/// Build a unique temp-WAV path under the OS temp dir for this capture.
fn temp_wav_path() -> PathBuf {
    let mut p = std::env::temp_dir();
    let id = uuid::Uuid::new_v4();
    p.push(format!("vox-mic-{id}.wav"));
    p
}

/// Write captured f32 samples to a 16 kHz mono 16-bit WAV. Downmixes to mono and
/// nearest-neighbour resamples to [`TARGET_SAMPLE_RATE`] (Whisper input format).
fn write_wav_16k_mono(buf: &CaptureBuffer, path: &Path) -> Result<(), String> {
    let channels = buf.channels.max(1) as usize;
    let src_rate = if buf.source_sample_rate == 0 {
        TARGET_SAMPLE_RATE
    } else {
        buf.source_sample_rate
    };

    // Downmix interleaved frames to mono by averaging channels.
    let mono: Vec<f32> = if channels <= 1 {
        buf.samples.clone()
    } else {
        buf.samples
            .chunks(channels)
            .map(|frame| frame.iter().copied().sum::<f32>() / channels as f32)
            .collect()
    };

    // Resample mono to the target rate (nearest-neighbour — adequate for STT).
    let resampled: Vec<f32> = if src_rate == TARGET_SAMPLE_RATE {
        mono
    } else {
        let ratio = TARGET_SAMPLE_RATE as f64 / src_rate as f64;
        let out_len = ((mono.len() as f64) * ratio).round() as usize;
        (0..out_len)
            .map(|i| {
                let src_idx = ((i as f64) / ratio).floor() as usize;
                mono.get(src_idx).copied().unwrap_or(0.0)
            })
            .collect()
    };

    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: TARGET_SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer =
        hound::WavWriter::create(path, spec).map_err(|e| format!("create WAV: {e}"))?;
    for s in resampled {
        let clamped = s.clamp(-1.0, 1.0);
        let v = (clamped * i16::MAX as f32) as i16;
        writer
            .write_sample(v)
            .map_err(|e| format!("write WAV sample: {e}"))?;
    }
    writer
        .finalize()
        .map_err(|e| format!("finalize WAV: {e}"))?;
    Ok(())
}

/// Begin recording from the default input device into managed state.
///
/// Returns a real error (never panics) when no input device is available — e.g.
/// in headless/CI environments — so the UI can surface the actual cause.
#[tauri::command]
pub fn start_mic_capture(state: tauri::State<'_, MicCaptureState>) -> Result<(), String> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

    let mut slot = state.session.lock().map_err(|e| e.to_string())?;
    if slot.is_some() {
        return Err("microphone capture already in progress".into());
    }

    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| "no default audio input device available".to_string())?;
    let config = device
        .default_input_config()
        .map_err(|e| format!("query default input config: {e}"))?;

    let source_sample_rate = config.sample_rate().0;
    let channels = config.channels();
    let sample_format = config.sample_format();
    let stream_config: cpal::StreamConfig = config.into();

    let shared = Arc::new(Mutex::new(CaptureBuffer {
        samples: Vec::new(),
        source_sample_rate,
        channels,
    }));

    let (stop_tx, stop_rx) = std::sync::mpsc::channel::<()>();
    let (done_tx, done_rx) = std::sync::mpsc::channel::<CaptureBuffer>();

    // cpal `Stream` is `!Send`; build + own it on a dedicated thread. The thread
    // parks until a stop signal arrives, then hands the buffer back.
    let thread_shared = shared.clone();
    std::thread::spawn(move || {
        let err_fn = |err| tracing::error!("mic stream error: {err}");
        let cb_shared = thread_shared.clone();

        let build = || -> Result<cpal::Stream, String> {
            match sample_format {
                cpal::SampleFormat::F32 => device
                    .build_input_stream(
                        &stream_config,
                        move |data: &[f32], _| {
                            if let Ok(mut b) = cb_shared.lock() {
                                b.samples.extend_from_slice(data);
                            }
                        },
                        err_fn,
                        None,
                    )
                    .map_err(|e| format!("build f32 input stream: {e}")),
                cpal::SampleFormat::I16 => device
                    .build_input_stream(
                        &stream_config,
                        move |data: &[i16], _| {
                            if let Ok(mut b) = cb_shared.lock() {
                                b.samples
                                    .extend(data.iter().map(|s| *s as f32 / i16::MAX as f32));
                            }
                        },
                        err_fn,
                        None,
                    )
                    .map_err(|e| format!("build i16 input stream: {e}")),
                cpal::SampleFormat::U16 => device
                    .build_input_stream(
                        &stream_config,
                        move |data: &[u16], _| {
                            if let Ok(mut b) = cb_shared.lock() {
                                b.samples.extend(
                                    data.iter()
                                        .map(|s| (*s as f32 / u16::MAX as f32) * 2.0 - 1.0),
                                );
                            }
                        },
                        err_fn,
                        None,
                    )
                    .map_err(|e| format!("build u16 input stream: {e}")),
                other => Err(format!("unsupported sample format: {other:?}")),
            }
        };

        let stream = match build() {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("mic capture: {e}");
                // Hand back an empty buffer so stop() yields a clear error.
                let _ = done_tx.send(CaptureBuffer::default());
                return;
            }
        };
        if let Err(e) = stream.play() {
            tracing::error!("mic stream play: {e}");
            let _ = done_tx.send(CaptureBuffer::default());
            return;
        }

        // Park until stop is requested (or the sender is dropped).
        let _ = stop_rx.recv();
        drop(stream); // stop capturing

        let captured = thread_shared
            .lock()
            .map(|b| CaptureBuffer {
                samples: b.samples.clone(),
                source_sample_rate: b.source_sample_rate,
                channels: b.channels,
            })
            .unwrap_or_default();
        let _ = done_tx.send(captured);
    });

    *slot = Some(MicSession { stop_tx, done_rx });
    Ok(())
}

/// Stop recording, finalize the WAV, transcribe it, and return refined text.
/// Always cleans up the temp WAV. Returns a real error if no capture is active
/// or transcription fails.
#[tauri::command]
pub fn stop_mic_capture_and_transcribe(
    state: tauri::State<'_, MicCaptureState>,
) -> Result<String, String> {
    let session = {
        let mut slot = state.session.lock().map_err(|e| e.to_string())?;
        slot.take()
            .ok_or_else(|| "no microphone capture in progress".to_string())?
    };

    // Signal the audio thread to stop and wait for the captured buffer.
    let _ = session.stop_tx.send(());
    let buf = session
        .done_rx
        .recv()
        .map_err(|e| format!("capture thread did not return audio: {e}"))?;

    if buf.samples.is_empty() {
        return Err("no audio captured (is a microphone connected and permitted?)".into());
    }

    let wav = temp_wav_path();
    let result = (|| {
        write_wav_16k_mono(&buf, &wav)?;
        transcribe_audio_file(&wav)
    })();

    // Always clean up the temp WAV, regardless of outcome.
    let _ = std::fs::remove_file(&wav);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// `.txt` passthrough exercises the refine glue without needing the Candle
    /// audio backend — proves `transcribe_audio_file` returns refined text.
    #[test]
    fn transcribe_audio_file_refines_text_passthrough() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("fixture.txt");
        let mut f = std::fs::File::create(&p).unwrap();
        write!(f, "hello world").unwrap();
        drop(f);

        let out = transcribe_audio_file(&p).expect("txt passthrough should refine");
        assert!(
            out.to_lowercase().contains("hello"),
            "expected refined text to contain 'hello', got {out:?}"
        );
    }

    /// A synthetic WAV is written correctly by the hound glue (valid 16 kHz mono
    /// 16-bit file) and is accepted by the transcription seam. With `stt-candle`
    /// compiled, a real Whisper backend runs; if a model isn't fetchable at test
    /// time we still must get a clear error string, never a panic.
    #[test]
    fn synthetic_wav_is_well_formed_and_routed() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("synthetic.wav");

        // 0.5s of a quiet sine at the source rate; exercises downmix+resample.
        let mut samples = Vec::new();
        let src_rate = 48_000u32;
        for i in 0..(src_rate / 2) {
            let t = i as f32 / src_rate as f32;
            // stereo interleaved
            let v = (2.0 * std::f32::consts::PI * 220.0 * t).sin() * 0.05;
            samples.push(v);
            samples.push(v);
        }
        let buf = CaptureBuffer {
            samples,
            source_sample_rate: src_rate,
            channels: 2,
        };
        write_wav_16k_mono(&buf, &p).expect("WAV write should succeed");

        // Verify hound wrote a 16 kHz mono 16-bit file.
        let reader = hound::WavReader::open(&p).expect("WAV should be readable");
        let spec = reader.spec();
        assert_eq!(spec.sample_rate, TARGET_SAMPLE_RATE);
        assert_eq!(spec.channels, 1);
        assert_eq!(spec.bits_per_sample, 16);

        // The transcription seam must not panic; it returns Ok(text) or a clear
        // Err string (e.g. model not available). Both are acceptable here.
        match transcribe_audio_file(&p) {
            Ok(_text) => {}
            Err(e) => assert!(!e.is_empty(), "error string must be non-empty"),
        }
    }

    #[test]
    fn resample_changes_length_proportionally() {
        let buf = CaptureBuffer {
            samples: vec![0.1; 48_000], // 1s mono @ 48k
            source_sample_rate: 48_000,
            channels: 1,
        };
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("r.wav");
        write_wav_16k_mono(&buf, &p).unwrap();
        let reader = hound::WavReader::open(&p).unwrap();
        // ~16k samples after 48k→16k downsample.
        let n = reader.into_samples::<i16>().count();
        assert!(
            (15_000..=17_000).contains(&n),
            "expected ~16000 samples, got {n}"
        );
    }
}
