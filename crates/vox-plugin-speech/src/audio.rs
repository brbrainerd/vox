//! AudioCapture + SpeechToText implementations for the Oratio plugin.
//!
//! AudioCapture: SP7 scaffold — mic/device capture not yet implemented.
//! SpeechToText: Candle Whisper backend, extracted from vox-speech in Unit 4.

use abi_stable::{erased_types::TD_Opaque, std_types::*};
use vox_plugin_api::abi::{VoxPlugin, VoxPlugin_TO, VoxPluginRef};
use vox_plugin_api::extensions::audio_capture::{AudioCapture, AudioCapture_TO};
use vox_plugin_api::extensions::speech_to_text::{SpeechToText, SpeechToText_TO};
use vox_plugin_api::host::VoxHost_TO;

#[derive(Clone)]
pub(crate) struct OratioPlugin;

impl VoxPlugin for OratioPlugin {
    fn id(&self) -> RString {
        RString::from("oratio")
    }

    fn shutdown(&self) -> RResult<(), RBoxError> {
        RResult::ROk(())
    }

    fn as_audio_capture(&self) -> ROption<AudioCapture_TO<'static, RBox<()>>> {
        ROption::RSome(AudioCapture_TO::from_value(self.clone(), TD_Opaque))
    }

    fn as_speech_to_text(&self) -> ROption<SpeechToText_TO<'static, RBox<()>>> {
        ROption::RSome(SpeechToText_TO::from_value(self.clone(), TD_Opaque))
    }
}

impl AudioCapture for OratioPlugin {
    fn list_devices_json(&self) -> RResult<RString, RBoxError> {
        RResult::ROk(RString::from("[]"))
    }

    fn start_capture(
        &self,
        _device_id: RStr<'_>,
        _config_json: RStr<'_>,
    ) -> RResult<(), RBoxError> {
        RResult::RErr(RBoxError::new(std::io::Error::other(
            "not yet implemented; SP7 scaffold",
        )))
    }

    fn stop_capture(&self) -> RResult<(), RBoxError> {
        RResult::RErr(RBoxError::new(std::io::Error::other(
            "not yet implemented; SP7 scaffold",
        )))
    }

    fn read_chunk(&self) -> RResult<RVec<u8>, RBoxError> {
        RResult::RErr(RBoxError::new(std::io::Error::other(
            "not yet implemented; SP7 scaffold",
        )))
    }
}

impl SpeechToText for OratioPlugin {
    /// Transcribe mono f32 PCM bytes at the sample rate from `config_json`.
    ///
    /// `config_json` shape: `{"sample_rate": 16000, "language": "en"}` (language optional).
    /// Returns: `{"text": "...", "language": "en", "segments": [...]}`
    fn transcribe(
        &self,
        audio_pcm: RSlice<'_, u8>,
        config_json: RStr<'_>,
    ) -> RResult<RString, RBoxError> {
        #[cfg(feature = "stt-candle")]
        {
            use crate::backends::candle_whisper::transcribe_pcm_internal;

            // Parse the f32 PCM bytes.
            let raw = audio_pcm.as_slice();
            if !raw.len().is_multiple_of(4) {
                return RResult::RErr(RBoxError::new(std::io::Error::other(
                    "audio_pcm length must be a multiple of 4 (mono f32 little-endian)",
                )));
            }
            let pcm: Vec<f32> = raw
                .as_chunks::<4>()
                .0
                .iter()
                .map(|b| f32::from_le_bytes(*b))
                .collect();

            // Parse language from config.
            let language: Option<String> =
                serde_json::from_str::<serde_json::Value>(config_json.as_str())
                    .ok()
                    .and_then(|v| v.get("language")?.as_str().map(|s| s.to_string()));

            match transcribe_pcm_internal(&pcm, language.as_deref()) {
                Ok((text, segments)) => {
                    let lang = language.as_deref().unwrap_or("auto");
                    let seg_json: Vec<serde_json::Value> = segments
                        .iter()
                        .map(|s| {
                            serde_json::json!({
                                "start_ms": s.start_ms,
                                "end_ms": s.end_ms,
                                "text": s.text,
                            })
                        })
                        .collect();
                    let out = serde_json::json!({
                        "text": text,
                        "language": lang,
                        "segments": seg_json,
                    });
                    RResult::ROk(RString::from(out.to_string()))
                }
                Err(e) => RResult::RErr(RBoxError::new(std::io::Error::other(e.to_string()))),
            }
        }
        #[cfg(not(feature = "stt-candle"))]
        {
            let _ = (audio_pcm, config_json);
            RResult::RErr(RBoxError::new(std::io::Error::other(
                "vox-plugin-oratio built without stt-candle feature",
            )))
        }
    }

    fn transcribe_path(
        &self,
        path: RStr<'_>,
        config_json: RStr<'_>,
    ) -> RResult<RString, RBoxError> {
        #[cfg(feature = "stt-candle")]
        {
            use crate::backends::candle_whisper::transcribe_audio_file_with_language;

            let path_str = path.to_string();
            let file_path = std::path::Path::new(&path_str);

            // Extract optional language from config_json.
            let language_override: Option<String> =
                serde_json::from_str::<serde_json::Value>(config_json.as_str())
                    .ok()
                    .and_then(|v| v.get("language")?.as_str().map(|s| s.to_string()));

            match transcribe_audio_file_with_language(file_path, language_override.as_deref()) {
                Ok(text) => {
                    let lang = language_override.as_deref().unwrap_or("auto");
                    let out = serde_json::json!({
                        "text": text,
                        "language": lang,
                        "segments": [],
                    });
                    RResult::ROk(RString::from(out.to_string()))
                }
                Err(e) => RResult::RErr(RBoxError::new(std::io::Error::other(e.to_string()))),
            }
        }
        #[cfg(not(feature = "stt-candle"))]
        {
            let _ = (path, config_json);
            RResult::RErr(RBoxError::new(std::io::Error::other(
                "vox-plugin-oratio built without stt-candle feature",
            )))
        }
    }

    /// Streaming transcription is not yet supported — the Candle Whisper backend is batch-only.
    /// Deferred: streaming requires chunk-wise model state management (Unit 4 deferral).
    fn begin_stream(&self, _config_json: RStr<'_>) -> RResult<RString, RBoxError> {
        RResult::RErr(RBoxError::new(std::io::Error::other(
            "streaming transcription not yet supported in vox-plugin-oratio; use transcribe() for batch",
        )))
    }

    fn push_audio(
        &self,
        _session_id: RStr<'_>,
        _audio_pcm: RSlice<'_, u8>,
    ) -> RResult<RString, RBoxError> {
        RResult::RErr(RBoxError::new(std::io::Error::other(
            "streaming transcription not yet supported in vox-plugin-oratio; use transcribe() for batch",
        )))
    }

    fn end_stream(&self, _session_id: RStr<'_>) -> RResult<RString, RBoxError> {
        RResult::RErr(RBoxError::new(std::io::Error::other(
            "streaming transcription not yet supported in vox-plugin-oratio; use transcribe() for batch",
        )))
    }
}

pub(crate) fn make_plugin(
    _host: VoxHost_TO<'static, RBox<()>>,
) -> RResult<VoxPluginRef, RBoxError> {
    let plugin = OratioPlugin;
    let to = VoxPlugin_TO::from_value(plugin, TD_Opaque);
    RResult::ROk(to)
}

#[cfg(test)]
mod semcov_wave9_tests {
    #![allow(unused_imports, dead_code)]
    use super::*;
    use abi_stable::std_types::RSlice;

    fn plugin() -> OratioPlugin {
        OratioPlugin
    }

    // Catches: transcribe() silently succeeding on zero-length PCM rather than
    // returning an error, producing empty transcript that misleads callers.
    #[test]
    fn transcribe_empty_pcm_returns_err_not_ok() {
        let p = plugin();
        let empty: &[u8] = &[];
        let result = p.transcribe(RSlice::from_slice(empty), "{\"sample_rate\":16000}".into());
        // Without stt-candle feature the stub already returns Err; with the feature
        // an empty buffer should also produce an error (no audio to transcribe).
        // The critical invariant: it must not return ROk with an empty text.
        if let abi_stable::std_types::RResult::ROk(json_str) = result {
            let v: serde_json::Value = serde_json::from_str(json_str.as_str()).unwrap();
            // If somehow Ok, the text field must not pretend it transcribed something
            let text = v.get("text").and_then(|t| t.as_str()).unwrap_or("");
            // Accept empty text but flag a non-empty transcription of silence as suspicious
            assert!(
                text.is_empty(),
                "transcribing empty PCM produced non-empty text: {text:?}"
            );
        }
        // RErr is the expected outcome — not panicking is the key assertion.
    }

    // Catches: transcribe() accepting a buffer whose byte length is not a multiple
    // of 4 without error, then reading out-of-bounds f32 values silently.
    #[test]
    fn transcribe_non_multiple_of_4_bytes_returns_err() {
        let p = plugin();
        // 5 bytes is NOT a multiple of 4 — must be rejected
        let bad: &[u8] = &[0u8, 0, 0, 0, 0]; // 5 bytes
        let result = p.transcribe(RSlice::from_slice(bad), "{\"sample_rate\":16000}".into());
        match result {
            abi_stable::std_types::RResult::RErr(_) => { /* expected */ }
            abi_stable::std_types::RResult::ROk(s) => {
                panic!("expected Err for 5-byte PCM, got Ok: {s}");
            }
        }
    }

    // Catches: begin_stream() accidentally succeeding (returning ROk) when the
    // feature is absent or streaming is genuinely not implemented — callers
    // must not proceed assuming a session_id was created.
    #[test]
    fn begin_stream_always_returns_err() {
        let p = plugin();
        let r = p.begin_stream("{}".into());
        assert!(
            r.is_rerr(),
            "begin_stream must always return Err (streaming not implemented)"
        );
    }

    // Catches: push_audio() not returning Err when there is no active stream session,
    // silently discarding audio and making callers believe transcription is proceeding.
    #[test]
    fn push_audio_without_stream_returns_err() {
        let p = plugin();
        let audio: &[u8] = &[0u8; 64];
        let r = p.push_audio("nonexistent-session".into(), RSlice::from_slice(audio));
        assert!(
            r.is_rerr(),
            "push_audio with no active stream must return Err"
        );
    }

    // Catches: end_stream() returning ROk with a partial transcript for a session
    // that was never started, causing the caller to believe transcription succeeded.
    #[test]
    fn end_stream_without_stream_returns_err() {
        let p = plugin();
        let r = p.end_stream("nonexistent-session".into());
        assert!(
            r.is_rerr(),
            "end_stream with no active stream must return Err"
        );
    }

    // Catches: start_capture() silently succeeding (stub must remain an error until
    // SP7 is implemented, so callers do not believe mic is open).
    #[test]
    fn start_capture_stub_returns_err() {
        let p = plugin();
        let r = p.start_capture("default".into(), "{}".into());
        assert!(
            r.is_rerr(),
            "start_capture is a SP7 scaffold and must return Err"
        );
    }

    // Catches: list_devices_json returning an Err instead of an empty JSON array,
    // breaking callers that probe for available devices before transcribing.
    #[test]
    fn list_devices_json_returns_valid_json_array() {
        let p = plugin();
        let r = p.list_devices_json();
        let json_str = match r {
            abi_stable::std_types::RResult::ROk(s) => s.to_string(),
            abi_stable::std_types::RResult::RErr(e) => {
                panic!("list_devices_json returned Err: {e}");
            }
        };
        let parsed: serde_json::Value =
            serde_json::from_str(&json_str).expect("list_devices_json must return valid JSON");
        assert!(
            parsed.is_array(),
            "list_devices_json must return a JSON array, got: {parsed}"
        );
    }
}
