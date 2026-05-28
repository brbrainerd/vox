//! Oratio internal types mirrored from [`vox-oratio`](../../../vox-oratio/) for this plugin.
//!
//! **SSOT:** [`vox-oratio::runtime_config`](../../../vox-oratio/src/runtime_config.rs),
//! [`vox-oratio::backends::candle_whisper`](../../../vox-oratio/src/backends/candle_whisper.rs).
//! The plugin stays free of a `vox-oratio` crate dependency to keep the cdylib graph small;
//! when changing tunables or Whisper wiring, update both sides (or extract a tiny shared crate).

pub mod acoustic_preprocess;
pub mod contextual_bias;
pub mod domain_mode;
pub mod runtime_config;
pub mod speech_lexicon;
