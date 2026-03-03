//! Cross-platform path and directory resolution.
//!
//! Single source of truth for VOX_DATA_DIR, VOX_USER_ID, and platform data dirs.
//! Precedence: env vars > platform defaults.

use std::path::PathBuf;

/// Application directory name under the base data dir.
pub const APP_DIR_NAME: &str = "vox";
/// Default database filename.
pub const DEFAULT_DB_FILENAME: &str = "vox.db";

/// Resolve the Vox data directory. Env `VOX_DATA_DIR` overrides; else platform default.
pub fn data_dir() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("VOX_DATA_DIR") {
        if !dir.is_empty() {
            let path = PathBuf::from(dir);
            std::fs::create_dir_all(&path).ok();
            return Some(path);
        }
    }
    let base = platform_data_dir()?;
    let path = base.join(APP_DIR_NAME);
    std::fs::create_dir_all(&path).ok();
    Some(path)
}

/// Default database path: `<data_dir>/vox.db`.
pub fn default_db_path() -> Option<PathBuf> {
    data_dir().map(|d| d.join(DEFAULT_DB_FILENAME))
}

/// State directory for durable objects: `<data_dir>/state/`.
pub fn state_dir() -> Option<PathBuf> {
    data_dir().map(|d| {
        let p = d.join("state");
        std::fs::create_dir_all(&p).ok();
        p
    })
}

/// Config directory: `<data_dir>/config/`.
pub fn config_dir() -> Option<PathBuf> {
    data_dir().map(|d| {
        let p = d.join("config");
        std::fs::create_dir_all(&p).ok();
        p
    })
}

/// Current user id for local usage. Env `VOX_USER_ID` or platform username or `"local-user"`.
pub fn local_user_id() -> String {
    if let Ok(id) = std::env::var("VOX_USER_ID") {
        if !id.is_empty() {
            return id;
        }
    }
    #[cfg(target_os = "windows")]
    if let Ok(user) = std::env::var("USERNAME") {
        if !user.is_empty() {
            return user;
        }
    }
    #[cfg(not(target_os = "windows"))]
    if let Ok(user) = std::env::var("USER") {
        if !user.is_empty() {
            return user;
        }
    }
    "local-user".to_string()
}

fn platform_data_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    return std::env::var("APPDATA").ok().map(PathBuf::from);

    #[cfg(target_os = "macos")]
    return home_dir().map(|h| h.join("Library").join("Application Support"));

    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
            if !xdg.is_empty() {
                return Some(PathBuf::from(xdg));
            }
        }
        home_dir().map(|h| h.join(".local").join("share"))
    }
}

#[cfg(not(target_os = "windows"))]
fn home_dir() -> Option<PathBuf> {
    std::env::var("HOME").ok().map(PathBuf::from)
}

#[cfg(target_os = "windows")]
#[allow(dead_code)]
fn home_dir() -> Option<PathBuf> {
    std::env::var("USERPROFILE")
        .ok()
        .map(PathBuf::from)
        .or_else(|| {
            let drive = std::env::var("HOMEDRIVE").ok()?;
            let path = std::env::var("HOMEPATH").ok()?;
            Some(PathBuf::from(format!("{}{}", drive, path)))
        })
}
