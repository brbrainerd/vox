//! Tauri 2 plugin registration for on-device speech (`invoke` / `plugin:vox-sherpa|transcribe`).
//!
//! Enable the **`tauri-plugin`** crate feature from generated `src-tauri`.

use tauri::plugin::{Builder, TauriPlugin};
use tauri::Runtime;

use crate::{TranscribeResult, PLUGIN_ID};

/// Register the Sherpa plugin (command `transcribe` on id [`PLUGIN_ID`]).
#[must_use]
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new(PLUGIN_ID)
        .invoke_handler(tauri::generate_handler![transcribe])
        .build()
}

/// On-device transcription entry point. Mobile JNI/Swift bridges are wired separately.
#[tauri::command]
async fn transcribe() -> Result<TranscribeResult, String> {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        Err(
            "native STT bridge not yet connected from Rust to SpeechRecognizerBridge / AppleSpeechBackend"
                .to_string(),
        )
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        Err(
            "on-device transcription is only available in Android and iOS Tauri builds".to_string(),
        )
    }
}
