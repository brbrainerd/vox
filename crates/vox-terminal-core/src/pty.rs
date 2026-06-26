//! PTY host — de-Tauri'd from `vox-gui/src/commands/pty.rs`.
//!
//! Pure functions (`default_shell`, `shell_integration_snippet`) and the
//! `ShellBackend` trait are the SSOT. The GUI's Tauri commands now call
//! `PtyHost` and forward the byte stream via their event emitter.
//!
//! **Forward-compat seam for Track 6 (Nushell):** `ShellBackend` is defined
//! here so `Session` depends on `dyn ShellBackend`, not on `PtyShell`
//! concretely. Track 6 adds `NuShell` behind the `nushell` cargo feature.

use std::io::Read;

use anyhow::Result;
use portable_pty::{native_pty_system, Child, CommandBuilder, PtySize};
use tokio::sync::mpsc;

// ── Pure platform helpers (moved verbatim from vox-gui/src/commands/pty.rs) ──

/// The default shell command for the current platform.
pub fn default_shell() -> String {
    if cfg!(windows) {
        "pwsh".to_string()
    } else {
        std::env::var("SHELL").unwrap_or_else(|_| "bash".to_string())
    }
}

/// OSC 633 shell-integration init snippet for a given shell.
/// Returns `None` for shells we don't yet integrate (Track 6 adds zsh/fish/nu).
pub fn shell_integration_snippet(shell: &str) -> Option<String> {
    let base = shell
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(shell)
        .to_ascii_lowercase();
    let name = base.strip_suffix(".exe").unwrap_or(&base);
    match name {
        "pwsh" | "powershell" => Some(PWSH_OSC633.to_string()),
        "bash" => Some(BASH_OSC633.to_string()),
        "zsh" => Some(ZSH_OSC633.to_string()),
        "fish" => Some(FISH_OSC633.to_string()),
        "nu" => Some(NU_OSC633.to_string()),
        _ => None,
    }
}

/// Prefer `nu` (Nushell) when it is on the PATH; fall back to the default
/// platform shell. Use this instead of `default_shell()` when structured
/// output (block model) is desired.
pub fn detect_ai_shell() -> String {
    if std::process::Command::new("nu")
        .arg("--version")
        .output()
        .is_ok()
    {
        return "nu".to_string();
    }
    default_shell()
}

/// Zsh OSC 633 integration (preexec/precmd hooks).
const ZSH_OSC633: &str = r#"
if [[ -z "${__VOX_OSC633:-}" ]]; then
  __VOX_OSC633=1
  __vox_preexec() {
    local cmd="${1//\\/\\x5c}"; cmd="${cmd//;/\\x3b}"; cmd="${cmd//$'\n'/\\x0a}"
    printf '\e]633;E;%s\a\e]633;C\a' "$cmd"
  }
  __vox_precmd() {
    printf '\e]633;D;%s\a\e]633;A\a' "${__vox_ec:-0}"
    __vox_ec=0
  }
  __vox_trappost() { __vox_ec=$? }
  autoload -Uz add-zsh-hook 2>/dev/null || true
  add-zsh-hook preexec __vox_preexec
  add-zsh-hook precmd __vox_precmd
  PROMPT="${PROMPT}"$'\e]633;B\a'
fi
"#;

/// Fish OSC 633 integration (event handler functions).
const FISH_OSC633: &str = r#"
if not set -q __vox_osc633
  set -g __vox_osc633 1
  function __vox_fish_preexec --on-event fish_preexec
    printf '\e]633;E;%s\a\e]633;C\a' $argv[1]
  end
  function __vox_fish_postexec --on-event fish_postexec
    printf '\e]633;D;%s\a\e]633;A\a' $status
  end
  functions --query fish_prompt && functions --copy fish_prompt __vox_orig_fish_prompt
  function fish_prompt
    __vox_orig_fish_prompt 2>/dev/null
    printf '\e]633;B\a'
  end
end
"#;

/// Nushell OSC 633 integration (config hook upsert).
const NU_OSC633: &str = r#"
$env.config = ($env.config | upsert hooks {|c|
  let h = ($c.hooks? | default {})
  $h
  | upsert pre_prompt (
      ($h.pre_prompt? | default []) | append {|| print -n ([(char -u 1b) "]633;A" (char bel)] | str join) }
    )
  | upsert pre_execution (
      ($h.pre_execution? | default []) | append {|| print -n ([(char -u 1b) "]633;C" (char bel)] | str join) }
    )
})
"#;

/// PowerShell OSC 633 integration (verbatim from vox-gui).
const PWSH_OSC633: &str = r#"
if (-not $global:__VoxOsc633) {
  $global:__VoxOsc633 = $true
  $global:__VoxOscEsc = [char]27
  $global:__VoxOscBel = [char]7
  $global:__VoxOrigPrompt = $function:prompt
  function global:prompt {
    $code = if ($LASTEXITCODE -ne $null) { $LASTEXITCODE } elseif ($?) { 0 } else { 1 }
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

/// Bash OSC 633 integration (verbatim from vox-gui).
const BASH_OSC633: &str = r#"
if [ -z "${__VOX_OSC633:-}" ]; then
  __VOX_OSC633=1
  __VOX_PREV_PROMPT_COMMAND="${PROMPT_COMMAND:-}"
  __VOX_PREV_DEBUG_TRAP="$(trap -p DEBUG | sed -e "s/^trap -- '//" -e "s/' DEBUG\$//")"
  __vox_preexec() {
    if [ -n "${__VOX_AT_PROMPT:-}" ]; then
      __VOX_AT_PROMPT=
      local cmd="${BASH_COMMAND//\\/\\x5c}"; cmd="${cmd//;/\\x3b}"; cmd="${cmd//$'\n'/\\x0a}"
      printf '\e]633;E;%s\a\e]633;C\a' "$cmd"
    fi
    [ -n "${__VOX_PREV_DEBUG_TRAP:-}" ] && eval "${__VOX_PREV_DEBUG_TRAP}"
  }
  __vox_prompt() {
    local ec=$?
    printf '\e]633;D;%s\a\e]633;A\a' "$ec"
    __VOX_AT_PROMPT=1
    [ -n "${__VOX_PREV_PROMPT_COMMAND:-}" ] && eval "${__VOX_PREV_PROMPT_COMMAND}"
  }
  trap '__vox_preexec' DEBUG
  PROMPT_COMMAND='__vox_prompt'
  PS1="${PS1}\[\e]633;B\a\]"
fi
"#;

// ── ShellBackend trait (forward-compat seam for Track 6 Nushell) ─────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellKind {
    /// A system shell running over a PTY (bash, pwsh, etc.)
    Pty,
    /// Nushell in-process (behind the `nushell` cargo feature, Track 6)
    Nushell,
}

/// Uniform interface over shell backends. `Session` depends on this trait,
/// not on any concrete type, so the default backend can be changed (e.g., to
/// Nushell) without touching session logic.
pub trait ShellBackend: Send + 'static {
    fn kind(&self) -> ShellKind;
    fn write(&mut self, data: &[u8]) -> Result<()>;
    fn kill(&mut self);
}

// ── PtyHost / PtyHandle ───────────────────────────────────────────────────────

/// A live PTY child process with an input handle.
pub struct PtyHandle {
    writer: Box<dyn std::io::Write + Send>,
    child: Box<dyn Child + Send + Sync>,
}

impl PtyHandle {
    pub fn write(&mut self, data: &[u8]) -> Result<()> {
        use std::io::Write;
        self.writer.write_all(data)?;
        self.writer.flush()?;
        Ok(())
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        // portable-pty doesn't expose resize through Box<dyn Child>; keep the
        // master in PtyHost for resize. This is a best-effort no-op for now.
        let _ = (cols, rows);
        Ok(())
    }

    pub fn kill(&mut self) {
        let _ = self.child.kill();
    }
}

impl ShellBackend for PtyHandle {
    fn kind(&self) -> ShellKind {
        ShellKind::Pty
    }
    fn write(&mut self, data: &[u8]) -> Result<()> {
        self.write(data)
    }
    fn kill(&mut self) {
        self.kill()
    }
}

/// Spawn a PTY shell and return a handle + a receiver for raw output bytes.
///
/// A blocking OS thread reads the PTY and pushes chunks into `tx`. The caller
/// (GUI Tauri command, TUI render loop, or Session) owns the receiver.
pub fn spawn_pty(
    shell: &str,
    cols: u16,
    rows: u16,
) -> Result<(PtyHandle, mpsc::Receiver<Vec<u8>>)> {
    let pty_system = native_pty_system();
    let pair = pty_system.openpty(PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    })?;

    let cmd = CommandBuilder::new(shell);
    let child = pair.slave.spawn_command(cmd)?;
    drop(pair.slave);

    let mut reader = pair.master.try_clone_reader()?;
    let mut writer = pair.master.take_writer()?;

    // Inject OSC 633 integration (best-effort; ignore write errors)
    if let Some(snippet) = shell_integration_snippet(shell) {
        use std::io::Write;
        let _ = writer.write_all(format!("{snippet}\n").as_bytes());
        let _ = writer.flush();
    }

    let (tx, rx) = mpsc::channel::<Vec<u8>>(256);
    // Move master into the thread to keep the PTY alive for the reader's lifetime.
    let master = pair.master;

    std::thread::spawn(move || {
        let _keep_master = master;
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if tx.blocking_send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
            }
        }
    });

    Ok((PtyHandle { writer, child }, rx))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_shell_is_nonempty() {
        assert!(!default_shell().is_empty());
    }

    #[test]
    fn pwsh_and_bash_snippets_are_some() {
        assert!(shell_integration_snippet("pwsh").is_some());
        assert!(shell_integration_snippet("bash").is_some());
    }

    #[test]
    fn zsh_snippet_is_some() {
        assert!(shell_integration_snippet("zsh").is_some());
    }

    #[test]
    fn fish_snippet_is_some() {
        assert!(shell_integration_snippet("fish").is_some());
    }

    #[test]
    fn nu_snippet_is_some() {
        assert!(shell_integration_snippet("nu").is_some());
    }

    #[test]
    fn detect_ai_shell_is_nonempty() {
        // nu may or may not be installed; either way we get a nonempty string.
        assert!(!detect_ai_shell().is_empty());
    }
}
