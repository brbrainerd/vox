//! Tauri 2 plugin registration for on-device speech (`invoke` / `plugin:vox-sherpa|transcribe`).
//!
//! Enable the **`tauri-plugin`** crate feature from generated `src-tauri`.

use tauri::Runtime;
use tauri::plugin::{Builder, TauriPlugin};

use crate::{PLUGIN_ID, TranscribeResult};

/// Register the Sherpa plugin (command `transcribe` on id [`PLUGIN_ID`]).
#[must_use]
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new(PLUGIN_ID)
        .invoke_handler(tauri::generate_handler![transcribe])
        .build()
}

/// On-device transcription entry point.
///
/// Tauri is desktop-only (ADR: scope-tauri-desktop-only); mobile apps use the
/// React Native target (`vox build --target=mobile`), where transcription goes
/// through `@vox/runtime-rn`. A desktop STT backend has not been wired yet, so
/// this command reports that honestly instead of pretending to listen.
#[tauri::command]
async fn transcribe() -> Result<TranscribeResult, String> {
    Err(
        "on-device transcription is not wired for desktop Tauri builds; mobile apps use the \
         React Native target (`vox build --target=mobile`)"
            .to_string(),
    )
}
