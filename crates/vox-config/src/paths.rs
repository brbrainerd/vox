//! Cross-platform path and directory resolution.
//!
//! Single source of truth for VOX_DATA_DIR, VOX_USER_ID, and platform data dirs.
//! Precedence: env vars > platform defaults.

use std::path::{Path, PathBuf};

/// Application directory name under the base data dir.
pub const APP_DIR_NAME: &str = "vox";
/// Default database filename.
pub const DEFAULT_DB_FILENAME: &str = "vox.db";

/// Resolve the Vox data directory. Env `VOX_DATA_DIR` overrides; else platform default.
pub fn data_dir() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("VOX_DATA_DIR")
        && !dir.is_empty()
    {
        let path = PathBuf::from(dir);
        std::fs::create_dir_all(&path).ok();
        return Some(path);
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
    if let Ok(id) = std::env::var("VOX_USER_ID")
        && !id.is_empty()
    {
        return id;
    }
    #[cfg(target_os = "windows")]
    if let Ok(user) = std::env::var("USERNAME")
        && !user.is_empty()
    {
        return user;
    }
    #[cfg(not(target_os = "windows"))]
    if let Ok(user) = std::env::var("USER")
        && !user.is_empty()
    {
        return user;
    }
    "local-user".to_string()
}

fn platform_data_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    return std::env::var("APPDATA").ok().map(PathBuf::from);

    #[cfg(target_os = "macos")]
    return Some(user_home_dir().join("Library").join("Application Support"));

    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        if let Ok(xdg) = std::env::var("XDG_DATA_HOME")
            && !xdg.is_empty()
        {
            return Some(PathBuf::from(xdg));
        }
        Some(user_home_dir().join(".local").join("share"))
    }
}

/// Best-effort user home (`HOME`, `USERPROFILE`, or `HOMEDRIVE`+`HOMEPATH`; else `.`).
pub fn user_home_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Ok(h) = std::env::var("USERPROFILE")
            && !h.is_empty()
        {
            return PathBuf::from(h);
        }
        if let (Ok(drive), Ok(path)) = (std::env::var("HOMEDRIVE"), std::env::var("HOMEPATH")) {
            let p = format!("{drive}{path}");
            if !p.is_empty() {
                return PathBuf::from(p);
            }
        }
        PathBuf::from(".")
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::env::var("HOME")
            .ok()
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
    }
}

/// `~/.vox` under [`user_home_dir`] (CLI script cache, etc.).
pub fn dot_vox_user_dir() -> PathBuf {
    user_home_dir().join(".vox")
}

/// Script compilation cache under `~/.vox/script-cache` or `~/.vox/script-cache-wasi`.
pub fn script_cache_dir(wasi_target: bool) -> PathBuf {
    let name = if wasi_target {
        "script-cache-wasi"
    } else {
        "script-cache"
    };
    dot_vox_user_dir().join(name)
}

/// `.vox/cache/repos/<repository_id>` under a repository root (MCP index, orchestrator cache).
pub fn repo_tooling_cache_dir(repo_root: &Path, repository_id: &str) -> PathBuf {
    repo_root
        .join(".vox")
        .join("cache")
        .join("repos")
        .join(repository_id)
}

/// Memory shard directory under [`repo_tooling_cache_dir`].
pub fn repo_memory_cache_dir(repo_root: &Path, repository_id: &str) -> PathBuf {
    repo_tooling_cache_dir(repo_root, repository_id).join("memory")
}

/// Portable backend artifact lane metadata under `<repo_root>/.vox/backend-artifact/`
/// (SBOM and signing attestations before OCI promotion; see portability SSOT).
#[must_use]
pub fn repo_backend_artifact_dir(repo_root: &Path) -> PathBuf {
    repo_root.join(".vox").join("backend-artifact")
}

/// SCIENTIA research mesh intake (orchestrator research broadcast → publisher scholarly pipeline).
///
/// Under `<repo_root>/.vox/scientia/research-mesh-intake/` — see `vox-publisher::research_mesh`.
#[must_use]
pub fn repo_scientia_research_mesh_intake_dir(repo_root: &Path) -> PathBuf {
    repo_root
        .join(".vox")
        .join("scientia")
        .join("research-mesh-intake")
}

/// Promoted SCIENTIA mesh ledger (JSONL) under `<repo_root>/.vox/scientia/research-mesh-promoted/`.
#[must_use]
pub fn repo_scientia_research_mesh_promoted_dir(repo_root: &Path) -> PathBuf {
    repo_root
        .join(".vox")
        .join("scientia")
        .join("research-mesh-promoted")
}

/// Basename for MCP session dirs (`.vox/sessions/<repository_id>` under repo root).
pub const MCP_SESSIONS_DIR_BASENAME: &str = ".vox/sessions";

/// MCP session persistence: `.vox/sessions/<repository_id>` (relative to repository root).
pub fn mcp_sessions_dir(repository_id: &str) -> PathBuf {
    PathBuf::from(MCP_SESSIONS_DIR_BASENAME).join(repository_id)
}

#[cfg(test)]
mod repo_path_tests {
    use super::*;

    #[test]
    fn scientia_research_mesh_intake_is_under_dot_vox() {
        let p = repo_scientia_research_mesh_intake_dir(Path::new("/workspace/repo"));
        let s = p.to_string_lossy();
        assert!(
            s.contains(".vox") && s.contains("scientia") && s.contains("research-mesh-intake"),
            "{s}"
        );
    }

    #[test]
    fn scientia_research_mesh_promoted_is_under_dot_vox() {
        let p = repo_scientia_research_mesh_promoted_dir(Path::new("/workspace/repo"));
        let s = p.to_string_lossy();
        assert!(
            s.contains(".vox") && s.contains("scientia") && s.contains("research-mesh-promoted"),
            "{s}"
        );
    }
}
