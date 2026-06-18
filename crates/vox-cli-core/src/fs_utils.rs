//! Shared filesystem utilities for CLI tools.

use std::path::PathBuf;
use tracing::warn;

/// Open `url` in the system default browser.
pub async fn open_browser(url: &str) {
    let url = url.trim();
    if url.is_empty() {
        return;
    }

    let url_owned = url.to_string();
    let res = tokio::task::spawn_blocking(move || open_browser_sync(&url_owned)).await;

    match res {
        Ok(Ok(())) => {}
        Ok(Err(e)) => warn!("open_browser: {e}"),
        Err(e) => warn!("open_browser join: {e}"),
    }
}

fn open_browser_sync(url: &str) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        let mut cmd = std::process::Command::new("cmd");
        cmd.args(["/C", "start", "", url]);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000);
        }
        cmd.spawn()?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(url).spawn()?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open").arg(url).spawn()?;
    }
    Ok(())
}

/// Strip Windows `\\?\` / `\\?\UNC\` prefixes from paths.
#[cfg(windows)]
pub fn strip_windows_verbatim_path(path: PathBuf) -> PathBuf {
    let s = path.to_string_lossy();
    let Some(rest) = s.strip_prefix(r"\\?\") else {
        return path;
    };
    let rest = rest.replace('/', "\\");
    if let Some(unc) = rest.strip_prefix("UNC\\") {
        PathBuf::from(format!(r"\\{}", unc))
    } else {
        PathBuf::from(rest)
    }
}

#[cfg(not(windows))]
pub fn strip_windows_verbatim_path(path: PathBuf) -> PathBuf {
    path
}
