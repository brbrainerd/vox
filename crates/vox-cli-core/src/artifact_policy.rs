use std::hash::{Hash, Hasher};
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactClass {
    WorkspaceTarget,
    TransientTarget,
    MensRun,
    MensLog,
    ScriptCache,
    ScratchLog,
    StaleRename,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetLane {
    CanonicalWorkspace,
    CiNested,
    GateIsolated,
    ScriptNative,
    ScriptWasi,
}

pub fn canonical_workspace_target(root: &Path) -> PathBuf {
    let path = root.join("target");
    tracing::debug!(lane = ?TargetLane::CanonicalWorkspace, class = ?ArtifactClass::WorkspaceTarget, ?path, "Resolved canonical workspace target");
    path
}

fn repo_path_hash(root: &Path) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    root.to_string_lossy().hash(&mut hasher);
    hasher.finish()
}

fn temp_vox_slot(root: &Path) -> PathBuf {
    std::env::temp_dir()
        .join("vox-targets")
        .join(format!("{:016x}", repo_path_hash(root)))
}

/// Isolated Cargo target dirs for this repo under OS temp (`…/vox-targets/<hash>/…`).
pub fn transient_lane_roots(root: &Path) -> [PathBuf; 2] {
    let base = temp_vox_slot(root);
    [base.join("nested-ci"), base.join("mens-gate-safe")]
}

pub fn ci_nested_target(root: &Path) -> PathBuf {
    let path = temp_vox_slot(root).join("nested-ci");
    tracing::debug!(lane = ?TargetLane::CiNested, class = ?ArtifactClass::TransientTarget, ?path, "Resolved CI nested target (temp-isolation)");
    path
}

pub fn gate_isolated_target(root: &Path) -> PathBuf {
    let path = temp_vox_slot(root).join("mens-gate-safe");
    tracing::debug!(lane = ?TargetLane::GateIsolated, class = ?ArtifactClass::TransientTarget, ?path, "Resolved Gate isolated target (temp-isolation)");
    path
}

/// True when `path` may be used as `CARGO_TARGET_DIR` (or similar) for this workspace.
pub fn is_allowed_artifact_path(path: &Path, root: &Path) -> bool {
    let root_target = root.join("target");
    if path.starts_with(&root_target) {
        return true;
    }

    let temp_vox = std::env::temp_dir().join("vox-targets");
    if path.starts_with(&temp_vox) {
        return true;
    }

    let home_vox = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(|h| PathBuf::from(h).join(".vox"))
        .unwrap_or_else(|_| PathBuf::from("/nonexistent/.vox"));
    if path.starts_with(&home_vox) {
        return true;
    }

    let mens_runs = root.join("mens").join("runs");
    if path.starts_with(&mens_runs) {
        return true;
    }

    let vox_cache = root.join(".vox").join("cache");
    if path.starts_with(&vox_cache) {
        return true;
    }

    // Under workspace root: forbid repo-root `target-*` / `target_*` siblings (sprawl).
    if path.starts_with(root) {
        if let Ok(rel) = path.strip_prefix(root) {
            if let Some(Component::Normal(first)) = rel.components().next() {
                let n = first.to_string_lossy();
                if n.starts_with("target-") || n.starts_with("target_") {
                    return false;
                }
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_canonical_target_subdir() {
        let root = Path::new("/repo");
        assert!(is_allowed_artifact_path(&root.join("target"), root));
        assert!(is_allowed_artifact_path(
            &root.join("target/debug/vox"),
            root
        ));
    }

    #[test]
    fn denies_root_target_sprawl() {
        let root = Path::new("/repo");
        assert!(!is_allowed_artifact_path(&root.join("target-ci"), root));
        assert!(!is_allowed_artifact_path(&root.join("target_nested"), root));
    }

    #[test]
    fn allows_temp_vox_targets() {
        let root = Path::new("/repo");
        let p = std::env::temp_dir()
            .join("vox-targets")
            .join("abc")
            .join("nested-ci");
        assert!(is_allowed_artifact_path(&p, root));
    }
}
