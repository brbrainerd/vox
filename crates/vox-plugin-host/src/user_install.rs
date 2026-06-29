//! User-facing skill install/remove write path. Turns a git URL or local path
//! into skill directories under a user-owned root (`.vox/skills`), removes
//! owned skill dirs (ownership-scoped), and classifies a skill dir's origin.
//!
//! Dependency-free on purpose: callers (CLI, MCP) pass in the roots they already
//! compute via `vox_config::paths`, so this crate gains no new graph edges.

use std::path::{Path, PathBuf};

use crate::skill_parser::parse_skill_md;

/// One skill installed into a user root.
#[derive(Debug, Clone)]
pub struct InstalledUserSkill {
    pub name: String,
    pub dest: PathBuf,
}

/// True when `source` looks like a git URL rather than a local directory path.
pub fn is_git_source(source: &str) -> bool {
    source.starts_with("http://")
        || source.starts_with("https://")
        || source.starts_with("git@")
        || source.ends_with(".git")
}

/// The user-owned skill root: `<ws>/.vox/skills` or, when `global`, `~/.vox/skills`.
/// Uses `dirs` (already a `vox-plugin-host` dependency, same as `vox_config::paths`).
pub fn user_skill_root(ws_root: &Path, global: bool) -> PathBuf {
    if global {
        if let Some(home) = dirs::home_dir() {
            return home.join(".vox").join("skills");
        }
    }
    ws_root.join(".vox").join("skills")
}

/// Classify a discovered skill dir by which ecosystem root it lives under.
/// ponytail: path-substring match — cheaper and just as correct as threading the
/// computed root list through every caller. Upgrade to prefix-match against the
/// real roots only if a skill dir ever legitimately contains one of these
/// substrings outside its skills root.
pub fn source_root_label(skill_dir: &Path) -> &'static str {
    let s = skill_dir.to_string_lossy().replace('\\', "/");
    if s.contains("/assets/skills/") || s.starts_with("assets/skills/") {
        "bundled"
    } else if s.contains("/.cursor/skills/") {
        "cursor"
    } else if s.contains("/.claude/skills/") {
        "claude"
    } else if s.contains("/.agents/skills/") {
        "agents"
    } else if s.contains("/.vox/skills/") {
        "vox"
    } else {
        "unknown"
    }
}

/// Only skills under a `.vox/skills` root are ours to delete.
pub fn is_removable(skill_dir: &Path) -> bool {
    source_root_label(skill_dir) == "vox"
}

/// Best-effort license signal: the name of a LICENSE file in the skill dir, else "".
pub fn license_hint(skill_dir: &Path) -> String {
    for f in ["LICENSE", "LICENSE.upstream", "LICENSE.txt", "LICENSE.md"] {
        if skill_dir.join(f).is_file() {
            return f.to_string();
        }
    }
    String::new()
}

/// Validate a skill name is a single safe path segment per the Agent Skills spec
/// (1-64 chars of `[a-z0-9-]`, no leading/trailing/double hyphen). CRITICAL: the
/// SKILL.md parser does not validate `name`, and install uses it as a path
/// component (`target_root.join(name)`), so an unchecked `name: ../../x` from an
/// untrusted source would escape the user root. Reject those here.
fn validate_skill_name(name: &str) -> Result<(), String> {
    let ok = !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !name.starts_with('-')
        && !name.ends_with('-')
        && !name.contains("--");
    if ok {
        Ok(())
    } else {
        Err(format!(
            "invalid skill name '{name}': must be 1-64 chars of [a-z0-9-] with no leading/trailing/double hyphen (Agent Skills spec)"
        ))
    }
}

/// Find skill dirs (those containing `SKILL.md`) under `base`, supporting the two
/// common repo layouts: the repo root *is* the skill, or `*/` and `skills/*/`.
fn find_skill_dirs(base: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if base.join("SKILL.md").is_file() {
        out.push(base.to_path_buf());
    }
    for sub in ["", "skills"] {
        let scan = if sub.is_empty() {
            base.to_path_buf()
        } else {
            base.join(sub)
        };
        if let Ok(rd) = std::fs::read_dir(&scan) {
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() && p.join("SKILL.md").is_file() {
                    out.push(p);
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Recursively copy `src` into `dst`, skipping any `.git` directory.
fn copy_tree(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        // Do not follow symlinks — a source tree could link outside itself
        // (escape) or to itself (loop). Skip them entirely.
        if entry.file_type()?.is_symlink() {
            continue;
        }
        if from.file_name().is_some_and(|n| n == ".git") {
            continue;
        }
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_tree(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

/// Shallow-clone a git URL into a temp dir. Suppresses a console window on Windows.
fn clone_repo(url: &str) -> Result<tempfile::TempDir, String> {
    let tmp = tempfile::tempdir().map_err(|e| e.to_string())?;
    let mut cmd = std::process::Command::new("git");
    cmd.args(["clone", "--depth", "1", url]).arg(tmp.path());
    // ponytail: git-native stall guard, not a wall-clock cap. Aborts if a
    // transfer drops below 1 KB/s for 30s (hostile/dead remote); a fast huge
    // repo still completes — the add is user-initiated and HITL-gated.
    cmd.env("GIT_HTTP_LOW_SPEED_LIMIT", "1000")
        .env("GIT_HTTP_LOW_SPEED_TIME", "30");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let out = cmd
        .output()
        .map_err(|e| format!("could not run git: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git clone failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(tmp)
}

/// Install skill(s) from `source` (git URL or local path) into the user root.
/// Validates each SKILL.md's frontmatter via the standard parser before copying;
/// never executes `scripts/`. Returns the skills actually installed.
pub fn install_to_user_root(
    source: &str,
    ws_root: &Path,
    global: bool,
    skill_filter: Option<&str>,
) -> Result<Vec<InstalledUserSkill>, String> {
    // Keep any cloned temp dir alive for the duration of the copy.
    let _clone_guard;
    let base: PathBuf = if is_git_source(source) {
        let tmp = clone_repo(source)?;
        let path = tmp.path().to_path_buf();
        _clone_guard = Some(tmp);
        path
    } else {
        _clone_guard = None;
        let p = PathBuf::from(source);
        if !p.is_dir() {
            return Err(format!("source path not found: {source}"));
        }
        p
    };

    let dirs = find_skill_dirs(&base);
    if dirs.is_empty() {
        return Err("no SKILL.md found in source".to_string());
    }

    let target_root = user_skill_root(ws_root, global);
    std::fs::create_dir_all(&target_root).map_err(|e| e.to_string())?;

    let mut installed = Vec::new();
    for dir in dirs {
        let body = std::fs::read_to_string(dir.join("SKILL.md")).map_err(|e| e.to_string())?;
        let bundle = parse_skill_md(&body).map_err(|e| format!("{}: {e}", dir.display()))?;
        let name = bundle.manifest.name.clone();
        if let Some(filter) = skill_filter {
            if filter != name {
                continue;
            }
        }
        // Reject path-escaping names BEFORE using `name` as a path component.
        validate_skill_name(&name)?;
        let dest = target_root.join(&name);
        // Clean re-install: drop any prior copy so stale files don't linger.
        // `dest` is `<.vox/skills>/<validated-name>`, so this never escapes.
        if dest.exists() {
            std::fs::remove_dir_all(&dest).map_err(|e| e.to_string())?;
        }
        copy_tree(&dir, &dest).map_err(|e| e.to_string())?;
        installed.push(InstalledUserSkill { name, dest });
    }

    if installed.is_empty() {
        return Err(match skill_filter {
            Some(f) => format!("no skill named '{f}' found in source"),
            None => "no installable skill found in source".to_string(),
        });
    }
    Ok(installed)
}

/// Remove a user-installed skill by id or name. `roots` are the discovery roots
/// (from `vox_config::paths::skill_search_roots`). Refuses anything not under a
/// `.vox/skills` root (bundled / other-tool dirs are read-only). Returns the
/// deleted directory.
pub fn remove_user_skill(id_or_name: &str, roots: &[PathBuf]) -> Result<PathBuf, String> {
    let found = crate::external_skills::discover_external_skills(roots);
    let ext = found
        .iter()
        .find(|e| e.bundle.manifest.id == id_or_name || e.bundle.manifest.name == id_or_name)
        .ok_or_else(|| format!("no discovered skill '{id_or_name}'"))?;
    if !is_removable(&ext.path) {
        return Err(format!(
            "'{id_or_name}' is read-only (not under .vox/skills); cannot remove"
        ));
    }
    std::fs::remove_dir_all(&ext.path).map_err(|e| e.to_string())?;
    Ok(ext.path.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_skill(dir: &Path, name: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(
            dir.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: Body for {name}\n---\n\n# {name}\n"),
        )
        .unwrap();
    }

    #[test]
    fn is_git_source_detects_urls_not_paths() {
        assert!(is_git_source("https://github.com/foo/bar"));
        assert!(is_git_source("git@github.com:foo/bar.git"));
        assert!(is_git_source("./local/repo.git"));
        assert!(!is_git_source("./local/skills"));
        assert!(!is_git_source("C:/Users/me/skills"));
    }

    #[test]
    fn source_root_label_classifies_by_path() {
        assert_eq!(source_root_label(Path::new("/x/assets/skills/tdd")), "bundled");
        assert_eq!(source_root_label(Path::new("/x/.cursor/skills/tdd")), "cursor");
        assert_eq!(source_root_label(Path::new("/x/.claude/skills/tdd")), "claude");
        assert_eq!(source_root_label(Path::new("/x/.agents/skills/tdd")), "agents");
        assert_eq!(source_root_label(Path::new("/x/.vox/skills/tdd")), "vox");
        assert_eq!(source_root_label(Path::new("/x/somewhere/tdd")), "unknown");
    }

    #[test]
    fn only_vox_skills_is_removable() {
        assert!(is_removable(Path::new("/x/.vox/skills/tdd")));
        assert!(!is_removable(Path::new("/x/assets/skills/tdd")));
        assert!(!is_removable(Path::new("/x/.cursor/skills/tdd")));
    }

    #[test]
    fn license_hint_reports_present_license_file() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("s");
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(license_hint(&dir), "");
        std::fs::write(dir.join("LICENSE.upstream"), "MIT").unwrap();
        assert_eq!(license_hint(&dir), "LICENSE.upstream");
    }

    #[test]
    fn find_skill_dirs_handles_root_and_subdir_layouts() {
        let tmp = tempfile::tempdir().unwrap();
        // single skill at repo root
        write_skill(&tmp.path().join("single"), "single");
        // skills/<name> layout
        write_skill(&tmp.path().join("multi/skills/alpha"), "alpha");
        write_skill(&tmp.path().join("multi/skills/beta"), "beta");

        let single = find_skill_dirs(&tmp.path().join("single"));
        assert_eq!(single.len(), 1);

        let multi = find_skill_dirs(&tmp.path().join("multi"));
        assert_eq!(multi.len(), 2);
    }

    #[test]
    fn install_local_path_copies_tree_and_validates() {
        let src = tempfile::tempdir().unwrap();
        write_skill(&src.path().join("skills/alpha"), "alpha");
        std::fs::write(src.path().join("skills/alpha/LICENSE"), "MIT").unwrap();

        let ws = tempfile::tempdir().unwrap();
        let installed =
            install_to_user_root(&src.path().to_string_lossy(), ws.path(), false, None).unwrap();

        assert_eq!(installed.len(), 1);
        assert_eq!(installed[0].name, "alpha");
        let dest = ws.path().join(".vox/skills/alpha");
        assert!(dest.join("SKILL.md").is_file());
        assert!(dest.join("LICENSE").is_file());
    }

    #[test]
    fn install_filters_by_skill_name() {
        let src = tempfile::tempdir().unwrap();
        write_skill(&src.path().join("skills/alpha"), "alpha");
        write_skill(&src.path().join("skills/beta"), "beta");

        let ws = tempfile::tempdir().unwrap();
        let installed =
            install_to_user_root(&src.path().to_string_lossy(), ws.path(), false, Some("beta"))
                .unwrap();
        assert_eq!(installed.len(), 1);
        assert_eq!(installed[0].name, "beta");
        assert!(!ws.path().join(".vox/skills/alpha").exists());
    }

    #[test]
    fn install_rejects_malformed_frontmatter() {
        let src = tempfile::tempdir().unwrap();
        let dir = src.path().join("skills/bad");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("SKILL.md"), "no frontmatter here").unwrap();

        let ws = tempfile::tempdir().unwrap();
        let err = install_to_user_root(&src.path().to_string_lossy(), ws.path(), false, None)
            .unwrap_err();
        assert!(
            err.contains("bad")
                || err.to_lowercase().contains("frontmatter")
                || err.to_lowercase().contains("parse")
        );
    }

    #[test]
    fn install_rejects_path_traversal_name() {
        // A malicious SKILL.md whose `name` escapes the user root must be refused
        // before any file is written outside `.vox/skills`.
        let src = tempfile::tempdir().unwrap();
        let dir = src.path().join("evil");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("SKILL.md"),
            "---\nname: ../../escape\ndescription: malicious skill that tries to escape\n---\n\n# x\n",
        )
        .unwrap();

        let ws = tempfile::tempdir().unwrap();
        let err = install_to_user_root(&src.path().to_string_lossy(), ws.path(), false, None)
            .unwrap_err();
        assert!(err.contains("invalid skill name"), "got: {err}");
        // Nothing was written outside the skills root.
        assert!(!ws.path().parent().unwrap().join("escape").exists());
    }

    #[test]
    fn validate_skill_name_accepts_spec_names_rejects_unsafe() {
        assert!(validate_skill_name("test-driven-development").is_ok());
        assert!(validate_skill_name("pdf").is_ok());
        assert!(validate_skill_name("../../escape").is_err());
        assert!(validate_skill_name("Foo").is_err()); // uppercase
        assert!(validate_skill_name("-lead").is_err());
        assert!(validate_skill_name("trail-").is_err());
        assert!(validate_skill_name("double--hyphen").is_err());
        assert!(validate_skill_name("has/slash").is_err());
        assert!(validate_skill_name("").is_err());
    }

    #[test]
    fn remove_deletes_owned_dir_and_refuses_others() {
        let ws = tempfile::tempdir().unwrap();
        let owned = ws.path().join(".vox/skills/mine");
        write_skill(&owned, "mine");
        let foreign = ws.path().join(".claude/skills/theirs");
        write_skill(&foreign, "theirs");

        let roots = vec![ws.path().join(".vox/skills"), ws.path().join(".claude/skills")];

        // foreign root -> refused, dir still present
        let err = remove_user_skill("theirs", &roots).unwrap_err();
        assert!(err.contains("read-only") || err.contains("cannot remove"));
        assert!(foreign.join("SKILL.md").is_file());

        // owned root -> deleted
        let removed = remove_user_skill("mine", &roots).unwrap();
        assert_eq!(removed, owned);
        assert!(!owned.exists());
    }
}
