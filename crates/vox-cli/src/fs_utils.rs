//! Shared filesystem utilities for Vox CLI commands.

use anyhow::Result;
use std::path::{Path, PathBuf};
use tracing::warn;

/// **PascalCase** `*.tsx` stem under `dir` (generated UI entry), preferring **`App`** when present, else first sorted match, else **`App`**.
pub fn find_component_name(dir: &Path) -> Result<String> {
    let mut candidates: Vec<String> = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "tsx")
            && let Some(stem) = path.file_stem()
        {
            let name = stem.to_string_lossy().to_string();
            if name.chars().next().is_some_and(|c| c.is_uppercase()) {
                candidates.push(name);
            }
        }
    }
    if candidates.iter().any(|n| n == "App") {
        return Ok("App".to_string());
    }
    candidates.sort();
    if let Some(n) = candidates.into_iter().next() {
        return Ok(n);
    }
    Ok("App".to_string())
}

/// Recursively copy a directory and all its contents.
pub fn copy_dir_recursive(from: &Path, to: &Path) -> Result<()> {
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let from_path = entry.path();
        let to_path = to.join(entry.file_name());
        if from_path.is_dir() {
            std::fs::create_dir_all(&to_path)?;
            copy_dir_recursive(&from_path, &to_path)?;
        } else {
            std::fs::copy(&from_path, &to_path)?;
        }
    }
    Ok(())
}

/// Shared Cargo `target/` dir for a generated project under the Vox workspace root.
///
/// Uses [`vox_repository::discover_repository_or_fallback`] from `start` (or the process CWD)
/// so nested `target/generated/...` builds reuse the workspace target directory.
pub fn run_target_dir_for_workspace(start: Option<&Path>) -> PathBuf {
    let start = start
        .map(Path::to_path_buf)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));
    let ctx = vox_repository::discover_repository_or_fallback(&start);
    ctx.root.join("target")
}

/// Open `url` in the system default browser (best-effort; logs on failure).
pub async fn open_browser(url: &str) {
    let url = url.trim();
    if url.is_empty() {
        return;
    }

    let url_owned = url.to_string();
    let res = tokio::task::spawn_blocking(move || open_browser_sync(&url_owned)).await;

    match res {
        Ok(Ok(())) => {}
        Ok(Err(e)) => warn!(target: "vox_cli", "open_browser: {e}"),
        Err(e) => warn!(target: "vox_cli", "open_browser join: {e}"),
    }
}

/// Best-effort home directory (`HOME`, `USERPROFILE`, `HOMEDRIVE`+`HOMEPATH`; else `.`).
pub fn user_home_dir() -> PathBuf {
    vox_config::user_home_dir()
}

/// Strip Windows `\\?\` / `\\?\UNC\` prefixes from paths (e.g. [`std::fs::canonicalize`] output).
///
/// Verbatim paths break nested Cargo `path = "..."` dependencies when forwarded as `//?/C:/...`
/// after slash normalization.
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

/// Path to the `vox-actor-runtime` crate for generated script projects (`VOX_RUNTIME_PATH` or repo layout).
pub fn resolve_vox_runtime_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("VOX_RUNTIME_PATH") {
        let pb = PathBuf::from(p);
        if pb.is_dir() {
            return Some(strip_windows_verbatim_path(pb));
        }
    }
    let start = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let ctx = vox_repository::discover_repository_or_fallback(&start);
    let candidate = ctx.root.join("crates").join("vox-actor-runtime");
    if candidate.is_dir() {
        return Some(strip_windows_verbatim_path(candidate));
    }
    None
}

fn dir_size_bytes(path: &Path) -> std::io::Result<u64> {
    let meta = path.metadata()?;
    if meta.is_file() {
        return Ok(meta.len());
    }
    if !meta.is_dir() {
        return Ok(0);
    }
    let mut total = 0u64;
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        total += dir_size_bytes(&entry.path()).unwrap_or(0);
    }
    Ok(total)
}

/// Trim `~/.vox/script-cache*` by entry count and total size (oldest first).
pub fn gc_script_cache(max_entries: usize, max_size_mb: u64) -> Result<()> {
    let max_bytes = max_size_mb.saturating_mul(1024 * 1024);
    for wasi in [false, true] {
        let root = vox_config::script_cache_dir(wasi);
        if !root.is_dir() {
            continue;
        }
        let mut entries: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();
        for e in std::fs::read_dir(&root)? {
            let e = e?;
            let p = e.path();
            if p.is_dir() {
                let mt = e
                    .metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::UNIX_EPOCH);
                entries.push((p, mt));
            }
        }
        entries.sort_by_key(|a| a.1);
        loop {
            let count = entries.len();
            let total: u64 = entries
                .iter()
                .map(|(p, _)| dir_size_bytes(p).unwrap_or(0))
                .sum();
            if count <= max_entries && total <= max_bytes {
                break;
            }
            if entries.is_empty() {
                break;
            }
            // Parallel `vox run` can create many cache dirs with identical coarse mtimes on Windows.
            // Never delete entries touched in the last few minutes — another process may still be
            // compiling into `target/` under that hash directory.
            let protect_recent = vox_config::timeouts::D_120S;
            let Some(idx) = entries.iter().position(|(_path, modified)| {
                modified
                    .elapsed()
                    .map(|e| e >= protect_recent)
                    .unwrap_or(false)
            }) else {
                break;
            };
            let (old, _) = entries.remove(idx);
            let _ = std::fs::remove_dir_all(&old);
        }
    }
    Ok(())
}

fn open_browser_sync(url: &str) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()?;
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
