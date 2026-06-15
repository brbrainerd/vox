//! Process-spawn helpers that prevent blank console-window flashes on Windows.
//!
//! The orchestrator daemon is spawned by the GUI (a windowed-subsystem binary on
//! Windows).  Any child processes it spawns with plain `Command::new()` pop a
//! blank console window on the desktop.  The helpers here set `CREATE_NO_WINDOW`
//! automatically; they are no-ops on other platforms.

/// Returns a [`std::process::Command`] with `CREATE_NO_WINDOW` set on Windows.
#[allow(unused_mut)]
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
