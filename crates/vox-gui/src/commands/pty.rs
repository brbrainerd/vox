//! Per-tab PTY sessions for the Vox Console. Each tab owns one PTY running the
//! user's shell; bytes stream to the UI as Tauri events, input is written back.
//! Windows spawns use ConPTY via portable-pty (no flashing console windows).

use std::collections::HashMap;
use std::io::Read;
use std::sync::Mutex;

use portable_pty::{native_pty_system, Child, CommandBuilder, PtySize};
use tauri::Emitter;

/// Tauri event carrying a chunk of PTY output. Payload: { tab_id, data }.
pub const PTY_OUTPUT_EVENT: &str = "vox://pty-output";
/// Tauri event signalling a PTY exited. Payload: { tab_id }.
pub const PTY_EXIT_EVENT: &str = "vox://pty-exit";

/// The default shell command per platform. Configurable later via settings.
pub fn default_shell() -> String {
    if cfg!(windows) {
        "pwsh".to_string()
    } else {
        std::env::var("SHELL").unwrap_or_else(|_| "bash".to_string())
    }
}

/// Registry of live PTY sessions keyed by tab id. Managed by Tauri state.
#[derive(Default)]
pub struct PtyManager {
    sessions: Mutex<HashMap<String, PtySession>>,
}

struct PtySession {
    writer: Box<dyn std::io::Write + Send>,
    /// The shell child. Kept so `pty_close` can terminate it — dropping the
    /// writer alone leaves the process (and its reader thread) running.
    child: Box<dyn Child + Send + Sync>,
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
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| e.to_string())?;

    let cmd = CommandBuilder::new(default_shell());
    let child = pair.slave.spawn_command(cmd).map_err(|e| e.to_string())?;
    drop(pair.slave);

    let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
    let writer = pair.master.take_writer().map_err(|e| e.to_string())?;

    manager
        .sessions
        .lock()
        .unwrap()
        .insert(tab_id.clone(), PtySession { writer, child });

    // Stream output on a blocking thread (portable-pty reader is sync). Hold the
    // master alive in the thread so the PTY stays open for its lifetime.
    let master = pair.master;
    let app_handle = app.clone();
    let id = tab_id.clone();
    std::thread::spawn(move || {
        let _keep_master = master;
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let chunk = PtyChunk {
                        tab_id: id.clone(),
                        data: String::from_utf8_lossy(&buf[..n]).to_string(),
                    };
                    let _ = app_handle.emit(PTY_OUTPUT_EVENT, chunk);
                }
            }
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
    use std::io::Write;
    let mut sessions = manager.sessions.lock().unwrap();
    let session = sessions.get_mut(&tab_id).ok_or("no such pty tab")?;
    session
        .writer
        .write_all(data.as_bytes())
        .map_err(|e| e.to_string())?;
    session.writer.flush().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn pty_close(manager: tauri::State<'_, PtyManager>, tab_id: String) -> Result<(), String> {
    if let Some(mut session) = manager.sessions.lock().unwrap().remove(&tab_id) {
        // Kill the shell so the reader thread unblocks (EOF) and the master FD is
        // dropped; otherwise closing a tab would leak the process + thread.
        let _ = session.child.kill();
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
}
