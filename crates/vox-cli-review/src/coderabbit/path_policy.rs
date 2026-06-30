//! Normalized path slashes and shared exclude-prefix matching for CodeRabbit planners.
//!
//! **Semantic submit** uses [`retain_non_coderabbit_tool_paths`] when collecting changed files and
//! again before drift compare, and [`filter_paths_for_drift_compare`] so manifest / `.coderabbit/`
//! noise never false-triggers `[drift]`.

/// Normalize backslashes to forward slashes for consistent prefix checks.
#[must_use]
pub fn normalize_slashes(s: &str) -> String {
    s.replace('\\', "/")
}

/// Strip repeated `./` prefixes so `./.coderabbit/foo` and `.coderabbit/foo` match the same rules.
#[must_use]
pub fn normalize_repo_rel_path(path: &str) -> String {
    let mut n = normalize_slashes(path);
    while let Some(rest) = n.strip_prefix("./") {
        n = rest.to_string();
    }
    n.trim_start_matches('/').to_string()
}

/// True if `path` starts with any entry in `exclude_prefixes` (after slash normalization).
#[must_use]
pub fn is_excluded_by_prefixes(path: &str, exclude_prefixes: &[String]) -> bool {
    let p = normalize_repo_rel_path(path);
    exclude_prefixes
        .iter()
        .any(|ex| p.starts_with(normalize_slashes(ex).as_str()))
}

/// True for anything under **`.coderabbit/`** (worktrees, run-state, etc.).
///
/// `semantic-submit` registers git worktrees here; those paths show up as untracked and must never
/// be copied into chunk worktrees — recursive copy can nest `.coderabbit/worktrees/<chunk>/…`
/// inside itself and hit Windows path limits (e.g. OS error 206).
#[must_use]
pub fn is_coderabbit_local_tool_path(path: &str) -> bool {
    let n = normalize_repo_rel_path(path);
    n == ".coderabbit" || n.starts_with(".coderabbit/")
}

/// Removes [`is_coderabbit_local_tool_path`] entries from `paths` in place.
///
/// Returns how many paths were dropped (for logging).
pub fn retain_non_coderabbit_tool_paths(paths: &mut Vec<String>) -> usize {
    let before = paths.len();
    paths.retain(|p| !is_coderabbit_local_tool_path(p));
    before - paths.len()
}

/// Tooling-only paths omitted from drift compare: root **`.coderabbit-semantic-manifest.json`** and
/// the full **`.coderabbit/`** tree (see [`is_coderabbit_local_tool_path`]).
#[must_use]
pub fn is_semantic_submit_drift_ignored(path: &str) -> bool {
    let n = normalize_repo_rel_path(path);
    matches!(n.as_str(), ".coderabbit-semantic-manifest.json")
        || is_coderabbit_local_tool_path(path)
}

/// Paths to skip when recursively copying the working tree into a chunk worktree overlay.
///
/// Uses [`normalize_repo_rel_path`] so **`./.coderabbit/…`** matches the same rules as **`.coderabbit/…`**.
/// Also skips any path segment **`/.coderabbit/`** (defensive; tooling is normally repo-root-local).
#[must_use]
pub fn should_skip_overlay_copy_path(rel_path: &str) -> bool {
    let n = normalize_repo_rel_path(rel_path);
    is_coderabbit_local_tool_path(&n) || n.contains("/.coderabbit/")
}

/// Copy of `paths` without [`is_semantic_submit_drift_ignored`] entries, sorted for comparison.
#[must_use]
pub fn filter_paths_for_drift_compare(paths: &[String]) -> Vec<String> {
    let mut v: Vec<String> = paths
        .iter()
        .filter(|p| !is_semantic_submit_drift_ignored(p))
        .cloned()
        .collect();
    v.sort();
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exclude_prefix_normalizes_slashes() {
        assert!(is_excluded_by_prefixes(
            r"mens\data\train.jsonl",
            &["mens/data/".to_string()]
        ));
        assert!(!is_excluded_by_prefixes(
            "crates/foo.rs",
            &["mens/".to_string()]
        ));
    }

    #[test]
    fn normalize_slashes_replaces_backslash() {
        assert_eq!(normalize_slashes(r"a\b\c"), "a/b/c");
    }

    #[test]
    fn normalize_repo_rel_path_strips_dot_slash() {
        assert_eq!(
            normalize_repo_rel_path("./.coderabbit/run-state.json"),
            ".coderabbit/run-state.json"
        );
    }

    #[test]
    fn drift_filter_drops_manifest_and_run_state() {
        let paths = vec![
            "crates/a.rs".to_string(),
            ".coderabbit-semantic-manifest.json".to_string(),
            ".coderabbit/run-state.json".to_string(),
        ];
        let f = filter_paths_for_drift_compare(&paths);
        assert_eq!(f, vec!["crates/a.rs".to_string()]);
    }

    #[test]
    fn overlay_skip_normalizes_dot_slash() {
        assert!(should_skip_overlay_copy_path(
            "./.coderabbit/worktrees/cr__review-foo"
        ));
        assert!(!should_skip_overlay_copy_path("crates/vox-cli/src/lib.rs"));
        assert!(should_skip_overlay_copy_path("foo/.coderabbit/nested"));
    }

    #[test]
    fn retain_non_coderabbit_drops_tool_paths() {
        let mut v = vec![
            "a.rs".to_string(),
            ".coderabbit/x".to_string(),
            "./.coderabbit/y".to_string(),
        ];
        let n = retain_non_coderabbit_tool_paths(&mut v);
        assert_eq!(n, 2);
        assert_eq!(v, vec!["a.rs".to_string()]);
    }

    #[test]
    fn coderabbit_local_tool_paths() {
        assert!(is_coderabbit_local_tool_path(
            ".coderabbit/worktrees/cr__review-foo"
        ));
        assert!(is_coderabbit_local_tool_path(r".coderabbit\run-state.json"));
        assert!(is_coderabbit_local_tool_path(".coderabbit"));
        assert!(is_coderabbit_local_tool_path(
            "./.coderabbit/worktrees/cr__review-foo"
        ));
        assert!(!is_coderabbit_local_tool_path(
            ".coderabbit-semantic-manifest.json"
        ));
        assert!(!is_coderabbit_local_tool_path("crates/vox-cli/src/lib.rs"));
    }
}
