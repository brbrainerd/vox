//! Tauri command bridge: wires `vox-terminal-core::Session` into the GUI.
//!
//! Each tab gets its own `Session` (the OSC-633 block-model state machine).
//! The GUI feeds raw PTY bytes via `term_pty_bytes`, then reads the parsed
//! block list via `term_get_blocks`. `term_submit` classifies the raw input
//! line so the UI can route it (shell vs. vox-native vs. agent vs. slash-cmd).

use std::collections::HashMap;
use std::sync::Mutex;

use vox_terminal_core::block::{BlockStatus, OutputChunk};
use vox_terminal_core::input::classify;
use vox_terminal_core::session::Session;

/// JSON-serializable projection of a `Block`.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct BlockJson {
    pub id: u64,
    pub status: String,
    pub input: String,
    pub output_text: String,
    pub exit_code: Option<i32>,
}

fn block_to_json(b: &vox_terminal_core::block::Block) -> BlockJson {
    let status = match b.status {
        BlockStatus::Running => "running",
        BlockStatus::Ok => "ok",
        BlockStatus::Failed => "failed",
        BlockStatus::Cancelled => "cancelled",
    };
    let output_text: String = b
        .output
        .iter()
        .map(|c: &OutputChunk| c.text.as_str())
        .collect();
    BlockJson {
        id: b.id.0,
        status: status.to_string(),
        input: b.input.clone(),
        output_text,
        exit_code: b.exit_code,
    }
}

/// Registry of `Session` instances keyed by tab id. Managed by Tauri state.
#[derive(Default)]
pub struct TerminalSessionManager {
    sessions: Mutex<HashMap<String, Session>>,
}

/// Feed raw PTY bytes into the session's OSC-633 parser.
/// Creates a new session for the tab if one does not yet exist.
#[tauri::command]
pub fn term_pty_bytes(
    manager: tauri::State<'_, TerminalSessionManager>,
    tab_id: String,
    data: String,
) -> Result<(), String> {
    let mut sessions = manager.sessions.lock().unwrap();
    let session = sessions
        .entry(tab_id.clone())
        .or_insert_with(|| Session::new(tab_id.as_str()));
    session.on_pty_bytes(data.as_bytes());
    Ok(())
}

/// Return the current block list for a tab as JSON-serializable structs.
/// Creates a new (empty) session if the tab has not been seen before.
#[tauri::command]
pub fn term_get_blocks(
    manager: tauri::State<'_, TerminalSessionManager>,
    tab_id: String,
) -> Result<Vec<BlockJson>, String> {
    let mut sessions = manager.sessions.lock().unwrap();
    let session = sessions
        .entry(tab_id.clone())
        .or_insert_with(|| Session::new(tab_id.as_str()));
    Ok(session.blocks().iter().map(block_to_json).collect())
}

/// Classify a raw input line without writing it anywhere.
///
/// Returns a string tag so the UI can decide how to route the command:
/// `"Shell"`, `"VoxNative"`, `"Agent"`, or `"Command:<name>"`.
#[tauri::command]
pub fn term_submit(
    _manager: tauri::State<'_, TerminalSessionManager>,
    _tab_id: String,
    input: String,
) -> Result<String, String> {
    use vox_terminal_core::input::InputIntent;
    let tag = match classify(&input) {
        InputIntent::Shell(_) => "Shell".to_string(),
        InputIntent::VoxNative(_) => "VoxNative".to_string(),
        InputIntent::Agent(_) => "Agent".to_string(),
        InputIntent::Command { name, .. } => format!("Command:{name}"),
    };
    Ok(tag)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_tab_has_no_blocks() {
        let mgr = TerminalSessionManager::default();
        let mut sessions = mgr.sessions.lock().unwrap();
        let s = sessions
            .entry("t1".to_string())
            .or_insert_with(|| Session::new("t1"));
        assert!(s.blocks().is_empty());
    }

    #[test]
    fn classify_shell_prefix() {
        use vox_terminal_core::input::InputIntent;
        assert!(matches!(classify("!ls"), InputIntent::Shell(_)));
    }

    #[test]
    fn classify_slash_command() {
        use vox_terminal_core::input::InputIntent;
        assert!(matches!(
            classify("/help"),
            InputIntent::Command { name, .. } if name == "help"
        ));
    }

    #[test]
    fn classify_agent_prefix() {
        use vox_terminal_core::input::InputIntent;
        assert!(matches!(classify("/ai hello"), InputIntent::Agent(_)));
    }
}
