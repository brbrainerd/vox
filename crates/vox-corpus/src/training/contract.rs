//! Workspace discovery for training SSOT paths (`training_contract.yaml`, system prompt, presets).

use std::path::{Path, PathBuf};

/// Walk up from [`std::env::current_dir`] until a `Cargo.toml` containing a `[workspace]` section is found.
///
/// Returns [`None`] if the current directory cannot be read or no workspace manifest is found.
pub fn find_workspace_root() -> Option<PathBuf> {
    vox_repository::find_cargo_workspace_root()
}

/// Resolve `path` relative to the workspace root when possible; otherwise return `path` as-is.
pub fn resolve_from_workspace(path: &Path) -> PathBuf {
    vox_repository::resolve_from_cargo_workspace(path)
}

/// Anchor a **relative** CLI path to the Cargo workspace (e.g. `--data-dir`, `--output-dir`, `--resume`).
///
/// Absolute paths are returned unchanged. When `workspace_root` is [`None`], falls back to
/// [`resolve_from_workspace`] (workspace discovery from [`std::env::current_dir`]).
pub fn normalize_workspace_relative_path(path: PathBuf, workspace_root: Option<&Path>) -> PathBuf {
    if path.is_absolute() {
        return path;
    }
    if let Some(ws) = workspace_root {
        return ws.join(&path);
    }
    resolve_from_workspace(&path)
}

/// Preferred alias for training data directories (same behavior as [`normalize_workspace_relative_path`]).
#[inline]
pub fn normalize_training_data_dir(data_dir: PathBuf, workspace_root: Option<&Path>) -> PathBuf {
    normalize_workspace_relative_path(data_dir, workspace_root)
}

/// Optional [`normalize_workspace_relative_path`] for checkpoint resume paths when relative to repo root.
#[inline]
pub fn normalize_training_resume_path(resume: PathBuf, workspace_root: Option<&Path>) -> PathBuf {
    normalize_workspace_relative_path(resume, workspace_root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_absolute_is_unchanged_with_workspace() {
        let ws = PathBuf::from("/repo");
        // vox-arch-check: allow abs-path
        let abs = PathBuf::from(if cfg!(windows) {
            r"C:\abs\train"
        } else {
            "/abs/train"
        });
        let got = normalize_workspace_relative_path(abs.clone(), Some(ws.as_path()));
        assert_eq!(got, abs);
    }

    #[test]
    fn normalize_relative_joins_workspace() {
        // vox-arch-check: allow abs-path
        let ws = PathBuf::from(if cfg!(windows) {
            r"C:\repo\vox"
        } else {
            "/repo/vox"
        });
        let got =
            normalize_workspace_relative_path(PathBuf::from("target/dogfood"), Some(ws.as_path()));
        assert_eq!(got, ws.join("target/dogfood"));
    }
}
