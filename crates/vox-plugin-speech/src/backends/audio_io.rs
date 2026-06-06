//! Decode audio files to mono `f32` PCM and resample to Whisper's expected 16 kHz.

use std::path::Path;

use anyhow::{Context, Result};
use candle_transformers::models::whisper::SAMPLE_RATE;
use symphonia::core::audio::{AudioBufferRef, Signal};
use symphonia::core::codecs::{CODEC_TYPE_NULL, DecoderOptions};
use symphonia::core::conv::FromSample;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Decode the first audio track to mono `f32` samples at the source sample rate.
pub fn pcm_decode(path: &Path) -> Result<(Vec<f32>, u32)> {
    let src = std::fs::File::open(path).with_context(|| path.display().to_string())?;
    let mss = MediaSourceStream::new(Box::new(src), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let meta_opts: MetadataOptions = Default::default();
    let fmt_opts: FormatOptions = Default::default();
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &fmt_opts, &meta_opts)
        .map_err(|e| anyhow::anyhow!("symphonia probe: {e}"))?;
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| anyhow::anyhow!("no decodable audio track in {}", path.display()))?;
    let dec_opts: DecoderOptions = Default::default();
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &dec_opts)
        .map_err(|_| anyhow::anyhow!("unsupported codec in {}", path.display()))?;
    let track_id = track.id;
    let sample_rate = track
        .codec_params
        .sample_rate
        .ok_or_else(|| anyhow::anyhow!("unknown sample rate"))?;
    let mut pcm_data = Vec::new();
    while let Ok(packet) = format.next_packet() {
        while !format.metadata().is_latest() {
            format.metadata().pop();
        }
        if packet.track_id() != track_id {
            continue;
        }
        match decoder
            .decode(&packet)
            .map_err(|e| anyhow::anyhow!("decode: {e}"))?
        {
            AudioBufferRef::F32(buf) => pcm_data.extend(buf.chan(0)),
            AudioBufferRef::U8(buf) => {
                pcm_data.extend(buf.chan(0).iter().map(|&s| f32::from_sample(s)));
            }
            AudioBufferRef::U16(buf) => {
                pcm_data.extend(buf.chan(0).iter().map(|&s| f32::from_sample(s)));
            }
            AudioBufferRef::U24(buf) => {
                pcm_data.extend(buf.chan(0).iter().map(|&s| f32::from_sample(s)));
            }
            AudioBufferRef::U32(buf) => {
                pcm_data.extend(buf.chan(0).iter().map(|&s| f32::from_sample(s)));
            }
            AudioBufferRef::S8(buf) => {
                pcm_data.extend(buf.chan(0).iter().map(|&s| f32::from_sample(s)));
            }
            AudioBufferRef::S16(buf) => {
                pcm_data.extend(buf.chan(0).iter().map(|&s| f32::from_sample(s)));
            }
            AudioBufferRef::S24(buf) => {
                pcm_data.extend(buf.chan(0).iter().map(|&s| f32::from_sample(s)));
            }
            AudioBufferRef::S32(buf) => {
                pcm_data.extend(buf.chan(0).iter().map(|&s| f32::from_sample(s)));
            }
            AudioBufferRef::F64(buf) => {
                pcm_data.extend(buf.chan(0).iter().map(|&s| f32::from_sample(s)));
            }
        }
    }
    Ok((pcm_data, sample_rate))
}

/// Resample mono `f32` PCM to `sr_out` Hz using Rubato (FFT resampler).
pub fn resample_pcm(pcm_in: &[f32], sr_in: u32, sr_out: u32) -> Result<Vec<f32>> {
    use rubato::{FftFixedInOut, Resampler};
    if sr_in == sr_out {
        return Ok(pcm_in.to_vec());
    }
    let chunk = 1024usize;
    let mut resampler = FftFixedInOut::<f32>::new(sr_in as usize, sr_out as usize, chunk, 1)
        .map_err(|e| anyhow::anyhow!("resampler init: {e}"))?;
    let mut pcm_out = Vec::with_capacity(
        (pcm_in.len() as f64 * f64::from(sr_out) / f64::from(sr_in)) as usize + chunk * 2,
    );
    let mut output_buffer = resampler.output_buffer_allocate(true);
    let mut pos_in = 0usize;
    while pos_in + resampler.input_frames_next() < pcm_in.len() {
        let next = resampler.input_frames_next();
        let (in_len, out_len) = resampler
            .process_into_buffer(&[&pcm_in[pos_in..pos_in + next]], &mut output_buffer, None)
            .map_err(|e| anyhow::anyhow!("resample: {e}"))?;
        pos_in += in_len;
        pcm_out.extend_from_slice(&output_buffer[0][..out_len]);
    }
    if pos_in < pcm_in.len() {
        let tail = &pcm_in[pos_in..];
        let (_in_len, out_len) = resampler
            .process_partial_into_buffer(Some(&[tail]), &mut output_buffer, None)
            .map_err(|e| anyhow::anyhow!("resample tail: {e}"))?;
        pcm_out.extend_from_slice(&output_buffer[0][..out_len]);
    }
    Ok(pcm_out)
}

/// Decode and resample to 16 kHz mono (Whisper requirement).
pub fn pcm_decode_to_16k_mono(path: &Path) -> Result<Vec<f32>> {
    let (pcm, sr) = pcm_decode(path)?;
    resample_pcm(&pcm, sr, SAMPLE_RATE as u32)
}
