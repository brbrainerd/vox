//! **Oratio** — Vox speech-to-text and transcript refinement.
//!
//! Default STT uses **Candle Whisper** (pure Rust + Hugging Face weights). There is no
//! whisper.cpp / clang dependency. Set `VOX_ORATIO_MODEL` / `VOX_ORATIO_REVISION` to pick
//! checkpoints. Without the `stt-candle` feature, only `.txt` / `.md` passthrough is available.

#![warn(missing_docs)]

mod language;
mod runtime_config;

pub mod acoustic_preprocess;

pub use acoustic_preprocess::{AcousticPreprocessDiagnostics, preprocess_audio_pcm_f32_reported};
pub mod ast_mapper;
pub mod contextual_bias;

pub mod backend_dispatch;
pub mod backends;

pub mod eval;
pub mod eval_srt;
pub mod failure_taxonomy;
pub mod refine;
pub mod routing;
pub mod session;
pub mod speaker_profile;
pub mod speech_intent;
pub mod speech_lexicon;
pub mod speech_normalize;
pub mod speech_policy;
/// Env-tunable stabilization thresholds for **future** streaming decoders; offline `transcribe_path` ignores this.
pub mod streaming_partial;
pub mod subtitle;
/// Policy helpers (escalation hint, cache keys) for hosts — not required for file-based STT.
pub mod tiering;
pub mod trace;
pub mod traits;
pub mod transcript_rerank;
pub mod vad;

#[cfg(feature = "serve")]
pub mod serve;

pub use backend_dispatch::create_backend;
pub use backends::asr_backend::{AsrBackend, AsrOutput};

#[cfg(feature = "stt-candle")]
pub use backends::candle_whisper::{
    ENV_CHUNK_OVERLAP_SEC, ENV_CHUNK_SEC, ENV_CUDA, ENV_EMIT_PARTIAL_PATH, ENV_MODEL, ENV_REVISION,
    ENV_STREAM_TOKENS, LanguageEnvOverride, transcribe_audio_file,
    transcribe_audio_file_with_language,
};
#[cfg(feature = "stt-candle")]
pub use backends::logit_processors::{
    ENV_CONSTRAINED_PHRASES, ENV_CONSTRAINED_TRIE, ENV_LOGIT_BIAS_MAX_TOKENS,
    ENV_LOGIT_BIAS_STRENGTH, ENV_LOGIT_FORBID_TOKENS, ENV_TRIE_STUCK_STEPS,
};

/// JSON status for the Candle backend (CPU/GPU feature flags, model env).
#[must_use]
pub fn candle_backend_status_json() -> serde_json::Value {
    #[cfg(feature = "stt-candle")]
    {
        backends::candle_whisper::candle_backend_status_json()
    }
    #[cfg(not(feature = "stt-candle"))]
    {
        serde_json::json!({ "stt_candle": false })
    }
}

pub use routing::{RouteMode, RouteResponse, route_transcript, route_transcript_with_options};
pub use runtime_config::{
    HfTunables, LlmPolicyTunables, LogitConstraintTunables, OratioRuntimeConfig, RefineTunables,
    RoutingTunables, SessionTimingDefaults, resolved_runtime_config,
    runtime_config_diagnostic_json, tool_route_min_confidence,
};
pub use session::{
    CaptureState, DeadlineDiagnostics, OratioDeadlineTaxonomy, OratioSessionConfig,
    OratioSessionResult, OratioTimings, session_config_with_runtime, transcribe_path_session,
    transcribe_path_session_with_runtime,
};
pub use speech_intent::{
    SpeechIntentAction, SpeechIntentEnvelope, build_intent_envelope,
    clarification_prompt_for_slots, missing_slot_ids,
};
pub use speech_policy::clarification_recommended;
pub use streaming_partial::{StreamingStabilizationConfig, should_commit_partial};
pub use tiering::{speech_cache_key, speech_escalation_recommended};
pub use traits::{
    TranscribeDetail, Transcript, refine_raw_text, transcribe_path, transcribe_path_detailed,
    transcript_status,
};
pub use transcript_rerank::{
    pick_best_transcript_index, pick_best_transcript_index_with_raw, rerank_candidates_best_first,
    rerank_candidates_best_first_with_context, rerank_candidates_best_first_with_raw,
};
