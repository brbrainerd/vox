//! Whisper decoding engine derived from the Hugging Face Candle examples (Apache-2.0).
//! Source reference: `candle-examples/examples/whisper/main.rs`.

use anyhow::{Error as E, Result};
use candle_core::{D, Device, IndexOp, Tensor};
use candle_nn::ops::{log_softmax, softmax};
use rand09::SeedableRng;
use rand09::distr::Distribution;
use rand09::distr::weighted::WeightedIndex;
use tokenizers::Tokenizer;
use tracing::{debug, trace};

use super::logit_processors::{LogitProcessor, LogitStepContext, NoopLogitProcessor};
use candle_transformers::models::whisper::{self as m, Config};

/// Whisper decoding task (transcribe in-language vs translate to English).
#[derive(Clone, Copy, Debug)]
pub(crate) enum DecodeTask {
    /// Transcribe in the spoken language.
    Transcribe,
    /// Translate speech to English.
    Translate,
}

/// Loaded Whisper weights (full precision or quantized GGUF).
pub enum WhisperModel {
    Normal(m::model::Whisper),
    /// Reserved for GGUF checkpoints (`VOX_ORATIO_QUANTIZED`); not wired in the default path yet.
    #[allow(dead_code)]
    Quantized(m::quantized_model::Whisper),
}

impl WhisperModel {
    /// Whisper hyperparameters from the checkpoint.
    pub fn config(&self) -> &Config {
        match self {
            Self::Normal(m) => &m.config,
            Self::Quantized(m) => &m.config,
        }
    }

    pub fn encoder_forward(&mut self, x: &Tensor, flush: bool) -> candle_core::Result<Tensor> {
        match self {
            Self::Normal(m) => m.encoder.forward(x, flush),
            Self::Quantized(m) => m.encoder.forward(x, flush),
        }
    }

    pub fn decoder_forward(
        &mut self,
        x: &Tensor,
        xa: &Tensor,
        flush: bool,
    ) -> candle_core::Result<Tensor> {
        match self {
            Self::Normal(m) => m.decoder.forward(x, xa, flush),
            Self::Quantized(m) => m.decoder.forward(x, xa, flush),
        }
    }

    pub fn decoder_final_linear(&self, x: &Tensor) -> candle_core::Result<Tensor> {
        match self {
            Self::Normal(m) => m.decoder.final_linear(x),
            Self::Quantized(m) => m.decoder.final_linear(x),
        }
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct DecodingResult {
    tokens: Vec<u32>,
    text: String,
    avg_logprob: f64,
    no_speech_prob: f64,
    temperature: f64,
    compression_ratio: f64,
}

pub(crate) struct Decoder {
    model: WhisperModel,
    rng: rand09::rngs::StdRng,
    task: Option<DecodeTask>,
    timestamps: bool,
    max_initial_timestamp_index: Option<u32>,
    verbose: bool,
    tokenizer: Tokenizer,
    suppress_tokens: Tensor,
    sot_token: u32,
    transcribe_token: u32,
    translate_token: u32,
    eot_token: u32,
    no_speech_token: u32,
    no_timestamps_token: u32,
    language_token: Option<u32>,
    logit_processor: Box<dyn LogitProcessor>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) enum StreamEvent {
    SegmentStart {
        segment_index: usize,
        seek_frames: usize,
        segment_frames: usize,
    },
    SegmentText {
        segment_index: usize,
        text: String,
        avg_logprob: f64,
        no_speech_prob: f64,
        start_frame: usize,
        end_frame: usize,
    },
    TokenDecoded {
        segment_index: usize,
        token: u32,
    },
    FinalText {
        text: String,
    },
}

impl Decoder {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        model: WhisperModel,
        tokenizer: Tokenizer,
        seed: u64,
        device: &Device,
        language_token: Option<u32>,
        task: Option<DecodeTask>,
        timestamps: bool,
        max_initial_timestamp_index: Option<u32>,
        verbose: bool,
        logit_processor: Option<Box<dyn LogitProcessor>>,
    ) -> Result<Self> {
        let no_timestamps_token = token_id(&tokenizer, m::NO_TIMESTAMPS_TOKEN)
            .map_err(|e| anyhow::anyhow!("tokenizer: {e}"))?;
        let suppress_tokens: Vec<f32> = (0..model.config().vocab_size as u32)
            .map(|i| {
                if model.config().suppress_tokens.contains(&i)
                    || timestamps && i == no_timestamps_token
                {
                    f32::NEG_INFINITY
                } else {
                    0f32
                }
            })
            .collect();
        let suppress_tokens = Tensor::new(suppress_tokens.as_slice(), device)
            .map_err(|e| anyhow::anyhow!("tensor: {e}"))?;
        let sot_token = token_id(&tokenizer, m::SOT_TOKEN).map_err(|e| anyhow::anyhow!("{e}"))?;
        let transcribe_token =
            token_id(&tokenizer, m::TRANSCRIBE_TOKEN).map_err(|e| anyhow::anyhow!("{e}"))?;
        let translate_token =
            token_id(&tokenizer, m::TRANSLATE_TOKEN).map_err(|e| anyhow::anyhow!("{e}"))?;
        let eot_token = token_id(&tokenizer, m::EOT_TOKEN).map_err(|e| anyhow::anyhow!("{e}"))?;
        let no_speech_token = m::NO_SPEECH_TOKENS
            .iter()
            .find_map(|token| token_id(&tokenizer, token).ok());
        let no_speech_token = match no_speech_token {
            None => anyhow::bail!("unable to find any non-speech token"),
            Some(n) => n,
        };
        Ok(Self {
            model,
            rng: rand09::rngs::StdRng::seed_from_u64(seed),
            tokenizer,
            task,
            timestamps,
            max_initial_timestamp_index,
            verbose,
            suppress_tokens,
            sot_token,
            transcribe_token,
            translate_token,
            eot_token,
            no_speech_token,
            language_token,
            no_timestamps_token,
            logit_processor: logit_processor.unwrap_or_else(|| Box::new(NoopLogitProcessor)),
        })
    }

    fn decode(
        &mut self,
        mel: &Tensor,
        t: f64,
        mut on_token: Option<&mut dyn FnMut(u32)>,
    ) -> Result<DecodingResult> {
        self.logit_processor.reset_for_decode();
        let audio_features = self
            .model
            .encoder_forward(mel, true)
            .map_err(|e| anyhow::anyhow!("encoder: {e}"))?;
        if self.verbose {
            trace!(dims = ?audio_features.dims(), "whisper audio features");
        }
        let sample_len = self.model.config().max_target_positions / 2;
        let mut sum_logprob = 0f64;
        let mut no_speech_prob = f64::NAN;
        let mut tokens = vec![self.sot_token];
        if let Some(language_token) = self.language_token {
            tokens.push(language_token);
        }
        match self.task {
            None | Some(DecodeTask::Transcribe) => tokens.push(self.transcribe_token),
            Some(DecodeTask::Translate) => tokens.push(self.translate_token),
        }
        if !self.timestamps {
            tokens.push(self.no_timestamps_token);
        }
        for i in 0..sample_len {
            let tokens_t = Tensor::new(tokens.as_slice(), mel.device())
                .map_err(|e| anyhow::anyhow!("tensor: {e}"))?;
            let tokens_t = tokens_t.unsqueeze(0)?;
            let ys = self
                .model
                .decoder_forward(&tokens_t, &audio_features, i == 0)
                .map_err(|e| anyhow::anyhow!("decoder: {e}"))?;

            if i == 0 {
                let logits = self
                    .model
                    .decoder_final_linear(&ys.i(..1).map_err(|e| anyhow::anyhow!("{e}"))?)
                    .map_err(|e| anyhow::anyhow!("{e}"))?
                    .i(0)
                    .map_err(|e| anyhow::anyhow!("{e}"))?
                    .i(0)
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
                no_speech_prob = softmax(&logits, 0)
                    .map_err(|e| anyhow::anyhow!("{e}"))?
                    .i(self.no_speech_token as usize)
                    .map_err(|e| anyhow::anyhow!("{e}"))?
                    .to_scalar::<f32>()
                    .map_err(|e| anyhow::anyhow!("{e}"))? as f64;
            }

            let (_, seq_len, _) = ys.dims3().map_err(|e| anyhow::anyhow!("{e}"))?;
            let logits = self
                .model
                .decoder_final_linear(
                    &ys.i((..1, seq_len - 1..))
                        .map_err(|e| anyhow::anyhow!("{e}"))?,
                )
                .map_err(|e| anyhow::anyhow!("{e}"))?
                .i(0)
                .map_err(|e| anyhow::anyhow!("{e}"))?
                .i(0)
                .map_err(|e| anyhow::anyhow!("{e}"))?;

            let logits = if self.timestamps {
                self.apply_timestamp_rules(&logits, &tokens)?
            } else {
                logits
            };
            let ctx = LogitStepContext {
                step: i,
                temperature: t,
                generated_tokens: tokens.clone(),
            };
            let logits = self.logit_processor.apply(&ctx, &logits)?;

            let logits = logits
                .broadcast_add(&self.suppress_tokens)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            let next_token = if t > 0f64 {
                let prs = softmax(&(&logits / t).map_err(|e| anyhow::anyhow!("{e}"))?, 0)
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
                let logits_v: Vec<f32> = prs.to_vec1().map_err(|e| anyhow::anyhow!("{e}"))?;
                let distr = WeightedIndex::new(&logits_v).map_err(|e| anyhow::anyhow!("{e}"))?;
                distr.sample(&mut self.rng) as u32
            } else {
                let logits_v: Vec<f32> = logits.to_vec1().map_err(|e| anyhow::anyhow!("{e}"))?;
                logits_v
                    .iter()
                    .enumerate()
                    .max_by(|a, b| a.1.total_cmp(b.1))
                    .map(|(i, _)| i as u32)
                    .unwrap()
            };
            tokens.push(next_token);
            self.logit_processor.on_token_decoded(next_token);
            if let Some(ref mut emit) = on_token {
                emit(next_token);
            }
            let prob = softmax(&logits, D::Minus1)
                .map_err(|e| anyhow::anyhow!("{e}"))?
                .i(next_token as usize)
                .map_err(|e| anyhow::anyhow!("{e}"))?
                .to_scalar::<f32>()
                .map_err(|e| anyhow::anyhow!("{e}"))? as f64;
            if next_token == self.eot_token
                || tokens.len() > self.model.config().max_target_positions
            {
                break;
            }
            sum_logprob += prob.ln();
        }
        let text = self.tokenizer.decode(&tokens, true).map_err(E::msg)?;
        let avg_logprob = sum_logprob / tokens.len() as f64;

        Ok(DecodingResult {
            tokens,
            text,
            avg_logprob,
            no_speech_prob,
            temperature: t,
            compression_ratio: f64::NAN,
        })
    }

    fn decode_with_fallback(
        &mut self,
        segment: &Tensor,
        on_token: Option<&mut dyn FnMut(u32)>,
    ) -> Result<DecodingResult> {
        if let Some(mut emit) = on_token {
            let mut selected: Option<f64> = None;
            for (i, &t) in m::TEMPERATURES.iter().enumerate() {
                let dr: Result<DecodingResult> = self.decode(segment, t, None);
                if i == m::TEMPERATURES.len() - 1 {
                    if let Ok(final_dr) = dr {
                        selected = Some(final_dr.temperature);
                    } else {
                        return dr;
                    }
                    break;
                }
                match dr {
                    Ok(dr) => {
                        let needs_fallback = dr.compression_ratio > m::COMPRESSION_RATIO_THRESHOLD
                            || dr.avg_logprob < m::LOGPROB_THRESHOLD;
                        if !needs_fallback || dr.no_speech_prob > m::NO_SPEECH_THRESHOLD {
                            selected = Some(dr.temperature);
                            break;
                        }
                    }
                    Err(err) => {
                        debug!(temperature = t, error = %err, "whisper decode retry");
                    }
                }
            }
            let chosen_t = selected.unwrap_or(0.0);
            return self.decode(segment, chosen_t, Some(&mut emit));
        }

        for (i, &t) in m::TEMPERATURES.iter().enumerate() {
            let dr: Result<DecodingResult> = self.decode(segment, t, None);
            if i == m::TEMPERATURES.len() - 1 {
                return dr;
            }
            match dr {
                Ok(dr) => {
                    let needs_fallback = dr.compression_ratio > m::COMPRESSION_RATIO_THRESHOLD
                        || dr.avg_logprob < m::LOGPROB_THRESHOLD;
                    if !needs_fallback || dr.no_speech_prob > m::NO_SPEECH_THRESHOLD {
                        return Ok(dr);
                    }
                }
                Err(err) => {
                    debug!(temperature = t, error = %err, "whisper decode retry");
                }
            }
        }
        unreachable!()
    }

    fn apply_timestamp_rules(&self, input_logits: &Tensor, tokens: &[u32]) -> Result<Tensor> {
        let device = input_logits.device().clone();
        let timestamp_begin = self.no_timestamps_token + 1;
        let vocab_size = self.model.config().vocab_size as u32;

        let sample_begin = if self.language_token.is_some() { 3 } else { 2 };
        let sampled_tokens = if tokens.len() > sample_begin {
            &tokens[sample_begin..]
        } else {
            &[][..]
        };

        let mut masks = Vec::new();
        let mut mask_buffer = vec![0.0f32; vocab_size as usize];

        if !sampled_tokens.is_empty() {
            let last_was_timestamp = sampled_tokens
                .last()
                .map(|&t| t >= timestamp_begin)
                .unwrap_or(false);

            let penultimate_was_timestamp = if sampled_tokens.len() >= 2 {
                sampled_tokens[sampled_tokens.len() - 2] >= timestamp_begin
            } else {
                false
            };

            if last_was_timestamp {
                if penultimate_was_timestamp {
                    for i in 0..vocab_size {
                        mask_buffer[i as usize] = if i >= timestamp_begin {
                            f32::NEG_INFINITY
                        } else {
                            0.0
                        };
                    }
                    masks.push(
                        Tensor::new(mask_buffer.as_slice(), &device)
                            .map_err(|e| anyhow::anyhow!("{e}"))?,
                    );
                } else {
                    for i in 0..vocab_size {
                        mask_buffer[i as usize] = if i < self.eot_token {
                            f32::NEG_INFINITY
                        } else {
                            0.0
                        };
                    }
                    masks.push(
                        Tensor::new(mask_buffer.as_slice(), &device)
                            .map_err(|e| anyhow::anyhow!("{e}"))?,
                    );
                }
            }

            let timestamp_tokens: Vec<u32> = sampled_tokens
                .iter()
                .filter(|&&t| t >= timestamp_begin)
                .copied()
                .collect();

            if !timestamp_tokens.is_empty() {
                let timestamp_last = if last_was_timestamp && !penultimate_was_timestamp {
                    *timestamp_tokens.last().unwrap()
                } else {
                    timestamp_tokens.last().unwrap() + 1
                };

                for i in 0..vocab_size {
                    mask_buffer[i as usize] = if i >= timestamp_begin && i < timestamp_last {
                        f32::NEG_INFINITY
                    } else {
                        0.0
                    };
                }
                masks.push(
                    Tensor::new(mask_buffer.as_slice(), &device)
                        .map_err(|e| anyhow::anyhow!("{e}"))?,
                );
            }
        }

        if tokens.len() == sample_begin {
            for i in 0..vocab_size {
                mask_buffer[i as usize] = if i < timestamp_begin {
                    f32::NEG_INFINITY
                } else {
                    0.0
                };
            }
            masks.push(
                Tensor::new(mask_buffer.as_slice(), &device).map_err(|e| anyhow::anyhow!("{e}"))?,
            );

            if let Some(max_initial_timestamp_index) = self.max_initial_timestamp_index {
                let last_allowed = timestamp_begin + max_initial_timestamp_index;
                if last_allowed < vocab_size {
                    for i in 0..vocab_size {
                        mask_buffer[i as usize] = if i > last_allowed {
                            f32::NEG_INFINITY
                        } else {
                            0.0
                        };
                    }
                    masks.push(
                        Tensor::new(mask_buffer.as_slice(), &device)
                            .map_err(|e| anyhow::anyhow!("{e}"))?,
                    );
                }
            }
        }

        let mut logits = input_logits.clone();
        for mask in masks {
            logits = logits
                .broadcast_add(&mask)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
        }

        let log_probs = log_softmax(&logits, 0).map_err(|e| anyhow::anyhow!("{e}"))?;

        let timestamp_log_probs = log_probs
            .narrow(
                0,
                timestamp_begin as usize,
                vocab_size as usize - timestamp_begin as usize,
            )
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        let text_log_probs = log_probs
            .narrow(0, 0, timestamp_begin as usize)
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        let timestamp_logprob = {
            let max_val = timestamp_log_probs
                .max(0)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            let shifted = timestamp_log_probs
                .broadcast_sub(&max_val)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            let exp_shifted = shifted.exp().map_err(|e| anyhow::anyhow!("{e}"))?;
            let sum_exp = exp_shifted.sum(0).map_err(|e| anyhow::anyhow!("{e}"))?;
            let log_sum = sum_exp.log().map_err(|e| anyhow::anyhow!("{e}"))?;
            max_val
                .broadcast_add(&log_sum)
                .map_err(|e| anyhow::anyhow!("{e}"))?
                .to_scalar::<f32>()
                .map_err(|e| anyhow::anyhow!("{e}"))?
        };

        let max_text_token_logprob: f32 = text_log_probs
            .max(0)
            .map_err(|e| anyhow::anyhow!("{e}"))?
            .to_scalar::<f32>()
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        if timestamp_logprob > max_text_token_logprob {
            for i in 0..vocab_size {
                mask_buffer[i as usize] = if i < timestamp_begin {
                    f32::NEG_INFINITY
                } else {
                    0.0
                };
            }
            let mask_tensor =
                Tensor::new(mask_buffer.as_slice(), &device).map_err(|e| anyhow::anyhow!("{e}"))?;
            logits = logits
                .broadcast_add(&mask_tensor)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
        }

        Ok(logits)
    }

    pub(crate) fn processor_name(&self) -> &'static str {
        self.logit_processor.name()
    }

    fn decode_result_text(&self, dr: &DecodingResult) -> Result<String> {
        if self.timestamps {
            let mut segment_parts = Vec::<String>::new();
            let mut tokens_to_decode = vec![];
            for &token in &dr.tokens {
                if token == self.sot_token || token == self.eot_token {
                    continue;
                }
                if token > self.no_timestamps_token {
                    if !tokens_to_decode.is_empty() {
                        let text = self
                            .tokenizer
                            .decode(&tokens_to_decode, true)
                            .map_err(E::msg)?;
                        if !text.is_empty() {
                            segment_parts.push(text);
                        }
                        tokens_to_decode.clear();
                    }
                } else {
                    tokens_to_decode.push(token);
                }
            }
            if !tokens_to_decode.is_empty() {
                let text = self
                    .tokenizer
                    .decode(&tokens_to_decode, true)
                    .map_err(E::msg)?;
                if !text.is_empty() {
                    segment_parts.push(text);
                }
            }
            return Ok(segment_parts
                .join(" ")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" "));
        }
        Ok(dr.text.clone())
    }

    pub(crate) fn run_streaming(
        &mut self,
        mel: &Tensor,
        emit_tokens: bool,
        mut on_event: impl FnMut(StreamEvent),
    ) -> Result<String> {
        let (_, _, content_frames) = mel.dims3().map_err(|e| anyhow::anyhow!("{e}"))?;
        let mut seek = 0;
        let mut parts = Vec::new();
        let mut segment_index = 0usize;
        while seek < content_frames {
            let start = std::time::Instant::now();
            let segment_size = usize::min(content_frames - seek, m::N_FRAMES);
            on_event(StreamEvent::SegmentStart {
                segment_index,
                seek_frames: seek,
                segment_frames: segment_size,
            });
            let mel_segment = mel
                .narrow(2, seek, segment_size)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            let mut token_cb = |token: u32| {
                on_event(StreamEvent::TokenDecoded {
                    segment_index,
                    token,
                });
            };
            let dr = if emit_tokens {
                self.decode_with_fallback(&mel_segment, Some(&mut token_cb))?
            } else {
                self.decode_with_fallback(&mel_segment, None)?
            };
            seek += segment_size;
            let no_speech_threshold =
                vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOratioNoSpeechThreshold)
                    .expose()
                    .and_then(|s: &str| s.parse::<f64>().ok())
                    .unwrap_or(0.6);
            if dr.no_speech_prob > no_speech_threshold {
                debug!(seek, ?dr, "whisper: no speech segment skipped");
                segment_index = segment_index.saturating_add(1);
                continue;
            }
            let segment_text = self.decode_result_text(&dr)?.trim().to_string();
            if !segment_text.is_empty() {
                parts.push(segment_text.clone());
                on_event(StreamEvent::SegmentText {
                    segment_index,
                    text: segment_text,
                    avg_logprob: dr.avg_logprob,
                    no_speech_prob: dr.no_speech_prob,
                    start_frame: seek,
                    end_frame: seek + segment_size,
                });
            }
            if self.verbose {
                trace!(seek, elapsed = ?start.elapsed(), "whisper segment");
            }
            segment_index = segment_index.saturating_add(1);
        }
        let final_text = parts
            .join(" ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        on_event(StreamEvent::FinalText {
            text: final_text.clone(),
        });
        Ok(final_text)
    }

    #[allow(dead_code)]
    pub(crate) fn run_collected(&mut self, mel: &Tensor) -> Result<String> {
        self.run_streaming(mel, false, |_| {})
    }

    pub(crate) fn into_whisper_model(self) -> WhisperModel {
        self.model
    }
}

pub(crate) fn token_id(tokenizer: &Tokenizer, token: &str) -> candle_core::Result<u32> {
    match tokenizer.token_to_id(token) {
        None => candle_core::bail!("no token-id for {token}"),
        Some(id) => Ok(id),
    }
}
