//! Project-scoped `VOX.md` memory — workspace DNA, always injected at session start.
//!
//! Mirrors Claude Code's `CLAUDE.md`: a checked-in file of project rules, build
//! commands, architecture, and conventions that the agent always sees. Distinct from
//! the account-scoped long-term `MEMORY.md` ([`super::LongTermMemory`]): `VOX.md` lives
//! at the workspace root and travels with the repository.
//!
//! Supports `@path` imports (one per line): a trimmed line of the form `@relative/file.md`,
//! `@/abs/file.md`, or `@~/file.md` inlines that file's content, recursively, to a max
//! depth of [`MAX_IMPORT_DEPTH`]. Cycles are broken via a canonical-path visited set;
//! missing imports and depth-cap hits leave an HTML comment marker rather than failing.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use vox_config::paths::REPO_VOX_MD_FILE;

/// Maximum `@import` recursion depth (mirrors Claude Code's depth-5 cap).
pub const MAX_IMPORT_DEPTH: usize = 5;

/// Candidate project-file locations, in precedence order.
const PROJECT_FILE_NAMES: &[&str] = &["VOX.md", REPO_VOX_MD_FILE];

/// Discover and load the workspace `VOX.md`, resolving `@path` imports into one
/// ready-to-inject block. Returns `None` when no project file exists or it is empty.
pub fn load_project_context(workspace_root: &Path) -> Option<String> {
    let root_file = PROJECT_FILE_NAMES
        .iter()
        .map(|n| workspace_root.join(n))
        .find(|p| p.is_file())?;

    let mut visited = HashSet::new();
    let body = expand_file(&root_file, 0, &mut visited);
    if body.trim().is_empty() {
        return None;
    }
    Some(format!("## Project Memory (VOX.md)\n\n{body}"))
}

/// Read `file`, inlining `@path` import lines recursively. Pure aside from filesystem
/// reads; never panics — unreadable/missing/looping imports degrade to inline markers.
fn expand_file(file: &Path, depth: usize, visited: &mut HashSet<PathBuf>) -> String {
    // Cycle guard: canonicalize so symlinks / `./` variants map to one key.
    let key = file.canonicalize().unwrap_or_else(|_| file.to_path_buf());
    if !visited.insert(key) {
        return format!("<!-- VOX.md import cycle skipped: {} -->\n", file.display());
    }

    let content = match std::fs::read_to_string(file) {
        Ok(c) => c,
        Err(e) => {
            return format!(
                "<!-- VOX.md import unreadable ({e}): {} -->\n",
                file.display()
            );
        }
    };
    let base_dir = file.parent().map(Path::to_path_buf).unwrap_or_default();

    let mut out = String::new();
    for line in content.lines() {
        match parse_import_line(line) {
            Some(import) => {
                if depth + 1 > MAX_IMPORT_DEPTH {
                    out.push_str(&format!(
                        "<!-- VOX.md import depth limit ({MAX_IMPORT_DEPTH}) reached; skipped {import} -->\n"
                    ));
                    continue;
                }
                match resolve_import_path(&import, &base_dir) {
                    Some(path) if path.is_file() => {
                        let sub = expand_file(&path, depth + 1, visited);
                        out.push_str(&sub);
                        if !sub.ends_with('\n') {
                            out.push('\n');
                        }
                    }
                    _ => out.push_str(&format!("<!-- VOX.md import not found: {import} -->\n")),
                }
            }
            None => {
                out.push_str(line);
                out.push('\n');
            }
        }
    }
    out
}

/// If `line` is an import directive (trimmed line of `@<path>` with no internal
/// whitespace), return the path token. Otherwise `None`. Prose containing `@` mid-line
/// or `@ foo` (with a space) is left untouched.
fn parse_import_line(line: &str) -> Option<String> {
    let t = line.trim();
    let rest = t.strip_prefix('@')?;
    if rest.is_empty() || rest.contains(char::is_whitespace) {
        return None;
    }
    Some(rest.to_string())
}

/// Resolve an import token to an absolute path. `~/x` → user home; absolute paths as-is;
/// otherwise relative to the importing file's directory.
fn resolve_import_path(import: &str, base_dir: &Path) -> Option<PathBuf> {
    if let Some(rest) = import.strip_prefix("~/") {
        return Some(vox_config::paths::user_home_dir().join(rest));
    }
    let p = Path::new(import);
    if p.is_absolute() {
        Some(p.to_path_buf())
    } else {
        Some(base_dir.join(p))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn none_when_no_vox_md() {
        let dir = tempdir().unwrap();
        assert_eq!(load_project_context(dir.path()), None);
    }

    #[test]
    fn loads_root_vox_md_with_header() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("VOX.md"), "Build: cargo test\n").unwrap();
        let out = load_project_context(dir.path()).expect("some");
        assert!(out.starts_with("## Project Memory (VOX.md)"));
        assert!(out.contains("Build: cargo test"));
    }

    #[test]
    fn discovers_dot_vox_fallback() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".vox")).unwrap();
        fs::write(dir.path().join(REPO_VOX_MD_FILE), "fallback rules\n").unwrap();
        let out = load_project_context(dir.path()).expect("some");
        assert!(out.contains("fallback rules"));
    }

    #[test]
    fn root_vox_md_wins_over_dot_vox() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".vox")).unwrap();
        fs::write(dir.path().join("VOX.md"), "ROOT\n").unwrap();
        fs::write(dir.path().join(REPO_VOX_MD_FILE), "DOTVOX\n").unwrap();
        let out = load_project_context(dir.path()).expect("some");
        assert!(out.contains("ROOT"));
        assert!(!out.contains("DOTVOX"));
    }

    #[test]
    fn resolves_relative_import() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("VOX.md"),
            "Top line\n@conventions.md\nTail\n",
        )
        .unwrap();
        fs::write(dir.path().join("conventions.md"), "imported body\n").unwrap();
        let out = load_project_context(dir.path()).expect("some");
        assert!(out.contains("Top line"));
        assert!(out.contains("imported body"));
        assert!(out.contains("Tail"));
        assert!(
            !out.contains("@conventions.md"),
            "import line should be replaced"
        );
    }

    #[test]
    fn missing_import_emits_marker_not_panic() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("VOX.md"), "@nope.md\n").unwrap();
        let out = load_project_context(dir.path()).expect("some");
        assert!(out.contains("import not found"));
        assert!(out.contains("nope.md"));
    }

    #[test]
    fn depth_cap_truncates_deep_chains() {
        let dir = tempdir().unwrap();
        // a -> b -> c -> d -> e -> f -> g (7 levels); cap is 5.
        fs::write(dir.path().join("VOX.md"), "@a.md\n").unwrap();
        for (cur, next) in [
            ("a.md", "b.md"),
            ("b.md", "c.md"),
            ("c.md", "d.md"),
            ("d.md", "e.md"),
            ("e.md", "f.md"),
            ("f.md", "g.md"),
        ] {
            fs::write(dir.path().join(cur), format!("body-{cur}\n@{next}\n")).unwrap();
        }
        fs::write(dir.path().join("g.md"), "body-g.md\n").unwrap();
        let out = load_project_context(dir.path()).expect("some");
        assert!(
            out.contains("depth limit"),
            "expected a depth-limit marker: {out}"
        );
        // Shallow levels are present; the deepest is not inlined.
        assert!(out.contains("body-a.md"));
        assert!(!out.contains("body-g.md"));
    }

    #[test]
    fn cycle_guard_terminates() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("VOX.md"), "@a.md\n").unwrap();
        fs::write(dir.path().join("a.md"), "body-a\n@b.md\n").unwrap();
        fs::write(dir.path().join("b.md"), "body-b\n@a.md\n").unwrap(); // cycle back to a
        let out = load_project_context(dir.path()).expect("some");
        assert!(out.contains("body-a"));
        assert!(out.contains("body-b"));
        // Terminates (no hang) and does not infinitely duplicate.
        assert_eq!(out.matches("body-b").count(), 1);
    }
}
