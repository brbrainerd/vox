//! Candle Whisper STT backend for vox-plugin-oratio.
//! All modules behind `#[cfg(feature = "stt-candle")]` matching the vox-speech convention.

pub mod asr_backend;

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
