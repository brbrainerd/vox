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
    if path.starts_with(root)
        && let Ok(rel) = path.strip_prefix(root)
        && let Some(Component::Normal(first)) = rel.components().next()
    {
        let n = first.to_string_lossy();
        if n.starts_with("target-") || n.starts_with("target_") {
            return false;
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

#[cfg(test)]
mod semcov_wave3_tests {
    #![allow(unused_imports)]
    use super::*;
    use std::path::Path;

    #[test]
    fn canonical_workspace_target_appends_target() {
        let root = Path::new("/workspace/myproject");
        let result = canonical_workspace_target(root);
        assert_eq!(result, root.join("target"));
    }

    #[test]
    fn canonical_workspace_target_absolute_root() {
        let root = Path::new("/tmp/repo");
        let result = canonical_workspace_target(root);
        assert!(result.ends_with("target"));
        assert!(result.starts_with(root));
    }

    #[test]
    fn repo_path_hash_is_deterministic() {
        let root = Path::new("/some/repo/path");
        let h1 = repo_path_hash(root);
        let h2 = repo_path_hash(root);
        assert_eq!(h1, h2);
    }

    #[test]
    fn repo_path_hash_differs_for_different_paths() {
        let a = Path::new("/repo/a");
        let b = Path::new("/repo/b");
        assert_ne!(repo_path_hash(a), repo_path_hash(b));
    }

    #[test]
    fn temp_vox_slot_is_under_temp_vox_targets() {
        let root = Path::new("/repo/test");
        let slot = temp_vox_slot(root);
        let expected_prefix = std::env::temp_dir().join("vox-targets");
        assert!(
            slot.starts_with(&expected_prefix),
            "expected {:?} to start with {:?}",
            slot,
            expected_prefix
        );
    }

    #[test]
    fn temp_vox_slot_name_is_16_hex_chars() {
        let root = Path::new("/repo/test");
        let slot = temp_vox_slot(root);
        let leaf = slot.file_name().unwrap().to_string_lossy();
        assert_eq!(
            leaf.len(),
            16,
            "hex segment should be 16 chars, got {:?}",
            leaf
        );
        assert!(
            leaf.chars().all(|c| c.is_ascii_hexdigit()),
            "not all hex: {:?}",
            leaf
        );
    }

    #[test]
    fn temp_vox_slot_is_deterministic() {
        let root = Path::new("/repo/proj");
        assert_eq!(temp_vox_slot(root), temp_vox_slot(root));
    }

    #[test]
    fn transient_lane_roots_returns_two_paths() {
        let root = Path::new("/repo");
        let [nested_ci, mens_gate] = transient_lane_roots(root);
        assert!(
            nested_ci.ends_with("nested-ci"),
            "first lane should end with nested-ci, got {:?}",
            nested_ci
        );
        assert!(
            mens_gate.ends_with("mens-gate-safe"),
            "second lane should end with mens-gate-safe, got {:?}",
            mens_gate
        );
    }

    #[test]
    fn transient_lane_roots_share_parent() {
        let root = Path::new("/repo");
        let [a, b] = transient_lane_roots(root);
        assert_eq!(
            a.parent(),
            b.parent(),
            "both lanes should share the same slot parent"
        );
    }

    #[test]
    fn transient_lane_roots_differ_across_repos() {
        let root_a = Path::new("/repo/alpha");
        let root_b = Path::new("/repo/beta");
        let [a1, _] = transient_lane_roots(root_a);
        let [b1, _] = transient_lane_roots(root_b);
        assert_ne!(a1, b1, "different repos must get different lane roots");
    }

    #[test]
    fn ci_nested_target_ends_with_nested_ci() {
        let root = Path::new("/ci/workspace");
        let result = ci_nested_target(root);
        assert!(
            result.ends_with("nested-ci"),
            "ci_nested_target should end with nested-ci, got {:?}",
            result
        );
    }

    #[test]
    fn ci_nested_target_is_under_temp_vox_targets() {
        let root = Path::new("/ci/workspace");
        let result = ci_nested_target(root);
        let expected_prefix = std::env::temp_dir().join("vox-targets");
        assert!(result.starts_with(&expected_prefix));
    }

    #[test]
    fn ci_nested_target_equals_first_transient_root() {
        let root = Path::new("/ci/workspace");
        let [first, _] = transient_lane_roots(root);
        assert_eq!(ci_nested_target(root), first);
    }

    #[test]
    fn gate_isolated_target_ends_with_mens_gate_safe() {
        let root = Path::new("/workspace");
        let result = gate_isolated_target(root);
        assert!(
            result.ends_with("mens-gate-safe"),
            "gate_isolated_target should end with mens-gate-safe, got {:?}",
            result
        );
    }

    #[test]
    fn gate_isolated_target_is_under_temp_vox_targets() {
        let root = Path::new("/workspace");
        let result = gate_isolated_target(root);
        let expected_prefix = std::env::temp_dir().join("vox-targets");
        assert!(result.starts_with(&expected_prefix));
    }

    #[test]
    fn gate_isolated_target_equals_second_transient_root() {
        let root = Path::new("/workspace");
        let [_, second] = transient_lane_roots(root);
        assert_eq!(gate_isolated_target(root), second);
    }
}
