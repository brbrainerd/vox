//! Oratio internal types mirrored from [`vox-speech`](../../../vox-speech/) for this plugin.
//!
//! **SSOT:** [`vox-speech::runtime_config`](../../../vox-speech/src/runtime_config.rs),
//! [`vox-speech::backends::candle_whisper`](../../../vox-speech/src/backends/candle_whisper.rs).
//! The plugin stays free of a `vox-speech` crate dependency to keep the cdylib graph small;
//! when changing tunables or Whisper wiring, update both sides (or extract a tiny shared crate).

pub mod acoustic_preprocess;
pub mod contextual_bias;
pub mod domain_mode;
pub mod runtime_config;
pub mod speech_lexicon;
