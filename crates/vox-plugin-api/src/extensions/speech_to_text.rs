//! SpeechToText extension point — Whisper / similar STT engines.

use abi_stable::{sabi_trait, std_types::*};

pub const SPEECH_TO_TEXT_REVISION: u32 = 2;

#[sabi_trait]
pub trait SpeechToText: Send + Sync {
    fn revision(&self) -> u32 {
        SPEECH_TO_TEXT_REVISION
    }

    /// Transcribe a single audio buffer. `audio_pcm` is mono f32 PCM (little-endian) at
    /// the sample rate declared in `config_json`. Returns transcription as
    /// JSON: `{"text": "...", "language": "en", "segments": [...]}`
    fn transcribe(
        &self,
        audio_pcm: RSlice<'_, u8>,
        config_json: RStr<'_>,
    ) -> RResult<RString, RBoxError>;

    /// Begin a streaming transcription session. Returns an opaque `session_id`.
    fn begin_stream(&self, config_json: RStr<'_>) -> RResult<RString, RBoxError>;

    /// Push an audio chunk into an active streaming session.
    /// Returns a partial transcription JSON update (may be empty `{}`).
    fn push_audio(
        &self,
        session_id: RStr<'_>,
        audio_pcm: RSlice<'_, u8>,
    ) -> RResult<RString, RBoxError>;

    /// End a streaming session and return the final transcription JSON.
    fn end_stream(&self, session_id: RStr<'_>) -> RResult<RString, RBoxError>;

    /// Transcribe an audio file at `path`. Plugin handles decoding internally.
    /// Equivalent to vox-speech's `transcribe_path_detailed`. Returns transcription JSON
    /// matching `transcribe()`'s shape.
    fn transcribe_path(
        &self,
        path: RStr<'_>,
        config_json: RStr<'_>,
    ) -> RResult<RString, RBoxError> {
        let _ = (path, config_json);
        RResult::RErr(RBoxError::new(std::io::Error::other(
            "transcribe_path not implemented by this SpeechToText backend",
        )))
    }
}
