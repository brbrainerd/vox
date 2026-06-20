//! Per-tab PTY sessions for the Vox Console. Each tab owns one PTY running the
//! user's shell; bytes stream to the UI as Tauri events, input is written back.
//! Windows spawns use ConPTY via portable-pty (no flashing console windows).
//!
//! Pure logic (`default_shell`, `shell_integration_snippet`) lives in
//! `vox-terminal-core::pty`. This module owns only the Tauri command wrappers.

use std::collections::HashMap;
use std::sync::Mutex;

use tauri::Emitter;
use vox_terminal_core::pty::{spawn_pty, PtyHandle};

// Re-export pure helpers so existing call sites in this crate still compile.
pub use vox_terminal_core::pty::{default_shell, shell_integration_snippet};

/// Tauri event carrying a chunk of PTY output. Payload: { tab_id, data }.
pub const PTY_OUTPUT_EVENT: &str = "vox://pty-output";
/// Tauri event signalling a PTY exited. Payload: { tab_id }.
pub const PTY_EXIT_EVENT: &str = "vox://pty-exit";

/// Registry of live PTY sessions keyed by tab id. Managed by Tauri state.
#[derive(Default)]
pub struct PtyManager {
    sessions: Mutex<HashMap<String, PtyHandle>>,
}

impl PtyManager {
    pub fn has(&self, tab_id: &str) -> bool {
        self.sessions.lock().unwrap().contains_key(tab_id)
    }

    pub fn count(&self) -> usize {
        self.sessions.lock().unwrap().len()
    }
}

#[derive(serde::Serialize, Clone)]
struct PtyChunk {
    tab_id: String,
    data: String,
}

#[derive(serde::Serialize, Clone)]
struct PtyExit {
    tab_id: String,
}

#[tauri::command]
pub fn pty_spawn(
    app: tauri::AppHandle,
    manager: tauri::State<'_, PtyManager>,
    tab_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    let shell = default_shell();
    let (handle, mut rx) = spawn_pty(&shell, cols, rows).map_err(|e| e.to_string())?;

    // Kill any existing session for this tab before replacing.
    if let Some(mut old) = manager.sessions.lock().unwrap().remove(&tab_id) {
        old.kill();
    }
    manager.sessions.lock().unwrap().insert(tab_id.clone(), handle);

    // Forward byte stream from core's mpsc to Tauri events.
    let app_handle = app.clone();
    let id = tab_id.clone();
    tokio::spawn(async move {
        while let Some(bytes) = rx.recv().await {
            let chunk = PtyChunk {
                tab_id: id.clone(),
                data: String::from_utf8_lossy(&bytes).to_string(),
            };
            let _ = app_handle.emit(PTY_OUTPUT_EVENT, chunk);
        }
        let _ = app_handle.emit(PTY_EXIT_EVENT, PtyExit { tab_id: id });
    });
    Ok(())
}

#[tauri::command]
pub fn pty_write(
    manager: tauri::State<'_, PtyManager>,
    tab_id: String,
    data: String,
) -> Result<(), String> {
    let mut sessions = manager.sessions.lock().unwrap();
    let handle = sessions.get_mut(&tab_id).ok_or("no such pty tab")?;
    handle.write(data.as_bytes()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn pty_close(manager: tauri::State<'_, PtyManager>, tab_id: String) -> Result<(), String> {
    if let Some(mut handle) = manager.sessions.lock().unwrap().remove(&tab_id) {
        handle.kill();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_shell_is_nonempty() {
        assert!(!default_shell().is_empty());
    }

    #[test]
    fn manager_starts_empty() {
        let m = PtyManager::default();
        assert_eq!(m.count(), 0);
        assert!(!m.has("tab-1"));
    }

    #[test]
    fn integration_snippet_supports_pwsh_and_bash() {
        assert!(shell_integration_snippet("pwsh").is_some());
        assert!(shell_integration_snippet("powershell.exe").is_some());
        // vox-arch-check: allow abs-path
        assert!(shell_integration_snippet("/usr/bin/bash").is_some());
    }

    #[test]
    fn integration_snippet_skips_unknown_shells() {
        assert!(shell_integration_snippet("fish").is_none());
        assert!(shell_integration_snippet("cmd.exe").is_none());
        assert!(shell_integration_snippet("zsh").is_none());
    }

    #[test]
    fn snippets_emit_633_markers() {
        assert!(shell_integration_snippet("pwsh").unwrap().contains("]633;"));
        assert!(shell_integration_snippet("bash").unwrap().contains("]633;"));
    }

    #[test]
    fn pwsh_snippet_uses_real_last_exit_code() {
        let s = shell_integration_snippet("pwsh").unwrap();
        assert!(s.contains("$LASTEXITCODE"), "must read the real exit code");
        assert!(!s.contains("__VoxLastExit"), "stale exit var removed");
    }

    #[test]
    fn bash_snippet_preserves_user_hooks() {
        let s = shell_integration_snippet("bash").unwrap();
        assert!(
            s.contains("__VOX_PREV_PROMPT_COMMAND"),
            "chains PROMPT_COMMAND"
        );
        assert!(s.contains("trap -p DEBUG"), "preserves the DEBUG trap");
    }

    #[test]
    fn uppercase_exe_suffix_is_stripped() {
        assert!(shell_integration_snippet("PowerShell.EXE").is_some());
    }
}
