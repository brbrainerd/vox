//! Per-tab PTY sessions for the Vox Console. Each tab owns one PTY running the
//! user's shell; bytes stream to the UI as Tauri events, input is written back.
//! Windows spawns use ConPTY via portable-pty (no flashing console windows).

use std::collections::HashMap;
use std::io::Read;
use std::sync::Mutex;

use portable_pty::{Child, CommandBuilder, PtySize, native_pty_system};
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

/// OSC 633 shell-integration init for a given shell, written to the PTY on spawn
/// so the shell emits block-delimiting markers (A prompt-start, B prompt-end,
/// E;<command>, C pre-exec, D;<exit>). Returns `None` for shells we don't
/// integrate — those degrade to plain scrollback (unchanged behavior).
///
/// The snippets wrap the user's existing prompt rather than replacing it.
pub fn shell_integration_snippet(shell: &str) -> Option<String> {
    // Match on the shell's basename so "/usr/bin/bash" and "bash" both resolve.
    let name = shell
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(shell)
        .trim_end_matches(".exe")
        .to_ascii_lowercase();
    match name.as_str() {
        "pwsh" | "powershell" => Some(PWSH_OSC633.to_string()),
        "bash" => Some(BASH_OSC633.to_string()),
        _ => None,
    }
}

/// PowerShell OSC 633 integration. Wraps `prompt` to emit A (prompt start) and
/// B (prompt end); a PSReadLine command-validation handler emits E;<command>
/// and C just before execution; the next prompt emits D;<exit> for the previous
/// command. ESC is `$([char]27)`, BEL terminator `$([char]7)`.
const PWSH_OSC633: &str = r#"
if (-not $global:__VoxOsc633) {
  $global:__VoxOsc633 = $true
  $global:__VoxOscEsc = [char]27
  $global:__VoxOscBel = [char]7
  $global:__VoxLastExit = 0
  $global:__VoxOrigPrompt = $function:prompt
  function global:prompt {
    $code = if ($global:__VoxLastExit -ne $null) { $global:__VoxLastExit } else { 0 }
    $out = "$($global:__VoxOscEsc)]633;D;$code$($global:__VoxOscBel)"
    $out += "$($global:__VoxOscEsc)]633;A$($global:__VoxOscBel)"
    $out += (& $global:__VoxOrigPrompt)
    $out += "$($global:__VoxOscEsc)]633;B$($global:__VoxOscBel)"
    return $out
  }
  Set-PSReadLineKeyHandler -Key Enter -ScriptBlock {
    $line = $null; $cursor = $null
    [Microsoft.PowerShell.PSConsoleReadLine]::GetBufferState([ref]$line, [ref]$cursor)
    $enc = $line -replace '\\','\x5c' -replace ';','\x3b' -replace "`n",'\x0a'
    [Console]::Write("$($global:__VoxOscEsc)]633;E;$enc$($global:__VoxOscBel)")
    [Console]::Write("$($global:__VoxOscEsc)]633;C$($global:__VoxOscBel)")
    [Microsoft.PowerShell.PSConsoleReadLine]::AcceptLine()
  }
}
"#;

/// Bash OSC 633 integration. PROMPT_COMMAND emits D;<exit> then A; PS1 wraps B;
/// a DEBUG trap (guarded once per command) emits E;<command> then C.
const BASH_OSC633: &str = r#"
if [ -z "${__VOX_OSC633:-}" ]; then
  __VOX_OSC633=1
  __vox_preexec() {
    if [ -n "${__VOX_AT_PROMPT:-}" ]; then
      __VOX_AT_PROMPT=
      local cmd="${BASH_COMMAND//\\/\\x5c}"; cmd="${cmd//;/\\x3b}"; cmd="${cmd//$'\n'/\\x0a}"
      printf '\e]633;E;%s\a\e]633;C\a' "$cmd"
    fi
  }
  __vox_prompt() {
    local ec=$?
    printf '\e]633;D;%s\a\e]633;A\a' "$ec"
    __VOX_AT_PROMPT=1
  }
  trap '__vox_preexec' DEBUG
  PROMPT_COMMAND='__vox_prompt'
  PS1="${PS1}\[\e]633;B\a\]"
fi
"#;

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
    let mut writer = pair.master.take_writer().map_err(|e| e.to_string())?;

    // Inject OSC 633 shell integration so the shell emits block markers. Best
    // effort: a write failure or an unintegrated shell just means raw scrollback.
    if let Some(snippet) = shell_integration_snippet(&default_shell()) {
        use std::io::Write;
        if let Err(e) = writer.write_all(format!("{snippet}\n").as_bytes()) {
            tracing::debug!("pty: shell-integration inject failed: {e}");
        } else {
            let _ = writer.flush();
        }
    }

    // Replacing a live tab id: kill the previous child first so its shell +
    // reader thread tear down instead of leaking.
    if let Some(mut old) = manager.sessions.lock().unwrap().remove(&tab_id) {
        let _ = old.child.kill();
    }
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

    #[test]
    fn integration_snippet_supports_pwsh_and_bash() {
        assert!(shell_integration_snippet("pwsh").is_some());
        assert!(shell_integration_snippet("powershell.exe").is_some());
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
}
