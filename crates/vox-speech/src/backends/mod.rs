//! STT backends for Oratio. Default: Candle Whisper. Optional: Sherpa-ONNX.

pub mod asr_backend;
#[cfg(feature = "stt-sherpa")]
pub mod sherpa_model_config;
#[cfg(feature = "stt-sherpa")]
pub mod sherpa_onnx;

#[cfg(feature = "stt-candle")]
pub mod audio_io;
#[cfg(feature = "stt-candle")]
mod candle_engine;
#[cfg(feature = "stt-candle")]
pub mod candle_whisper;
#[cfg(feature = "stt-candle")]
pub mod logit_processors;
#[cfg(feature = "stt-candle")]
mod multilingual;

#[cfg(feature = "cloud")]
pub mod cloud_offload;
