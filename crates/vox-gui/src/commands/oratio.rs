//! Desktop speech-to-text Tauri command — captures the default microphone and
//! transcribes it through the existing Oratio plugin (A4 / WS-4 Task 1).
//!
//! The whole capture+transcribe path is gated behind the `oratio` Cargo feature
//! (default-OFF) so the stock GUI build stays lean. When the feature is off, the
//! command is still registered but returns a clear error so the frontend can
//! surface the gap rather than crash on an unknown command.

/// Transcription result forwarded to the frontend.
/// Field names are part of a cross-agent DTO contract — do NOT rename.
#[derive(Debug, serde::Serialize)]
pub struct TranscribeResultDto {
    /// Display text (refined when available, else raw).
    pub text: String,
    /// Raw model/plugin output before refinement.
    pub raw_text: String,
    /// Refined text when refinement ran; otherwise `None`.
    pub refined_text: Option<String>,
}

#[cfg(feature = "oratio")]
mod imp {
    use super::TranscribeResultDto;
    use std::path::Path;

    /// Transcribe a fixture/audio `path` via the Oratio plugin (with `.txt`/`.md`
    /// passthrough), mapping to the frontend DTO.
    ///
    /// Pure w.r.t. capture: takes a path, so it is unit-testable with a `.txt`
    /// fixture (no mic, no model).
    pub(super) fn transcribe_path_to_dto(path: &Path) -> Result<TranscribeResultDto, String> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        let ctx = vox_speech::refine::CorrectionContext::default();

        // `.txt`/`.md` fixtures resolve deterministically without the plugin.
        let detail = if matches!(ext.as_str(), "txt" | "md") {
            vox_speech::transcribe_path_detailed(path, &ctx, None)
                .map_err(|e| format!("transcribe fixture: {e}"))?
        } else {
            let plugin = vox_plugin_host::cached_code_plugin("oratio")
                .map_err(|e| format!("oratio plugin load: {e}"))?;
            let stt = plugin
                .plugin
                .as_speech_to_text()
                .into_option()
                .ok_or_else(|| "oratio plugin missing SpeechToText accessor".to_string())?;

            let path_str = path.to_string_lossy().to_string();
            let config_json = serde_json::json!({ "language": Option::<&str>::None }).to_string();

            let transcription_json = stt
                .transcribe_path(path_str.as_str().into(), config_json.as_str().into())
                .into_result()
                .map_err(|e| format!("transcribe_path plugin: {e}"))?;

            let v: serde_json::Value = serde_json::from_str(transcription_json.as_str())
                .map_err(|e| format!("plugin returned invalid JSON: {e}"))?;
            let raw_text = v
                .get("text")
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .to_string();
            vox_speech::refine_raw_text(&raw_text, &ctx)
        };

        Ok(TranscribeResultDto {
            text: detail.text().to_string(),
            raw_text: detail.raw_text.clone(),
            refined_text: Some(detail.refined_text.clone()),
        })
    }

    /// Capture `seconds` of default-mic audio to a temp WAV, transcribe it, and
    /// clean up. Capture runs on a blocking thread.
    pub(super) async fn capture_and_transcribe(
        seconds: f32,
    ) -> Result<TranscribeResultDto, String> {
        let wav_path =
            std::env::temp_dir().join(format!("vox_gui_oratio_{}.wav", uuid::Uuid::new_v4()));

        let capture_path = wav_path.clone();
        tokio::task::spawn_blocking(move || {
            vox_ml_cli::commands::oratio_mic::record_default_input_wav(&capture_path, seconds)
        })
        .await
        .map_err(|e| format!("capture task join: {e}"))?
        .map_err(|e| format!("microphone capture: {e}"))?;

        let result = transcribe_path_to_dto(&wav_path);
        let _ = std::fs::remove_file(&wav_path);
        result
    }
}

/// Capture `seconds` of microphone audio and transcribe it via Oratio.
///
/// Built without the `oratio` feature, this returns an explanatory error so the
/// frontend toast path fires instead of an "unknown command" failure.
#[tauri::command]
pub async fn oratio_transcribe(seconds: f32) -> Result<TranscribeResultDto, String> {
    #[cfg(feature = "oratio")]
    {
        imp::capture_and_transcribe(seconds).await
    }
    #[cfg(not(feature = "oratio"))]
    {
        let _ = seconds;
        Err(
            "this build of vox-gui was compiled without the `oratio` feature; \
             rebuild with `--features oratio` to enable desktop speech-to-text"
                .to_string(),
        )
    }
}

#[cfg(all(test, feature = "oratio"))]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn txt_fixture_maps_to_dto_text() {
        // A `.txt` fixture flows through the same deterministic path as plugin
        // STT output, but needs no mic and no model.
        let mut f = tempfile::Builder::new()
            .suffix(".txt")
            .tempfile()
            .expect("temp fixture");
        write!(f, "open the orchestrator panel").expect("write fixture");
        let path = f.path();

        let dto = imp::transcribe_path_to_dto(path).expect("transcribe fixture");
        assert!(
            dto.text.to_lowercase().contains("orchestrator"),
            "expected refined text to carry the transcript, got {:?}",
            dto.text
        );
        assert!(dto.refined_text.is_some());
        assert!(!dto.raw_text.is_empty());
    }
}
