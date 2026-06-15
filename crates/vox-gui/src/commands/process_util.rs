//! Process-spawn helpers that prevent blank console-window flashes on Windows.
//!
//! On Windows every `Command::new()` without `CREATE_NO_WINDOW` causes a
//! momentary blank console to appear when spawning child processes from a GUI
//! (windowed subsystem) binary.  The helpers here set the flag automatically
//! and are no-ops on other platforms.

/// Returns a [`std::process::Command`] with `CREATE_NO_WINDOW` set on Windows,
/// preventing blank console window flashes when spawning child processes from
/// the GUI.
#[allow(unused_mut)] // Windows-only mutation via creation_flags
pub fn quiet_command(program: impl AsRef<std::ffi::OsStr>) -> std::process::Command {
    let mut cmd = std::process::Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Returns a [`tokio::process::Command`] with `CREATE_NO_WINDOW` set on Windows.
///
/// Use this instead of [`tokio::process::Command::new`] for async spawns from
/// the GUI backend.
#[allow(unused_mut)]
pub fn quiet_tokio_command(program: impl AsRef<std::ffi::OsStr>) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(program);
    #[cfg(windows)]
    {
        #[allow(unused_imports)]
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}
