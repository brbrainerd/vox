//! AudioCapture + SpeechToText implementations for the Oratio plugin.
//!
//! AudioCapture: SP7 scaffold — mic/device capture not yet implemented.
//! SpeechToText: Candle Whisper backend, extracted from vox-oratio in Unit 4.

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
                .chunks_exact(4)
                .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
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
