//! Discovery of bare SKILL.md skill directories (agentskills.io layout) —
//! `<root>/<skill-dir>/SKILL.md`, no `Plugin.toml`. This is the universal
//! ecosystem layout (Claude Code, Cursor, Codex, Copilot, …) that every other
//! harness reads from `.claude/skills`/`.agents/skills`.
//!
//! Complements [`crate::discover`], which owns `Plugin.toml`-based plugin
//! skills. Both feed the same [`crate::SkillRegistry`] (SSOT).

use crate::skill_bundle::VoxSkillBundle;
use crate::skill_parser::parse_skill_md;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// A skill found in an external (non-plugin) skill root.
pub struct ExternalSkill {
    /// Directory containing the `SKILL.md`.
    pub path: PathBuf,
    /// Parsed bundle (manifest + raw body).
    pub bundle: VoxSkillBundle,
}

/// Walk each root's immediate subdirectories for `SKILL.md`, highest-precedence
/// root first; the first skill seen for a given manifest id wins (mirrors the
/// install-first-wins ordering callers rely on for shadowing). Missing roots
/// and unparseable `SKILL.md` files are skipped with a warning.
pub fn discover_external_skills(roots: &[PathBuf]) -> Vec<ExternalSkill> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut out = Vec::new();
    for root in roots {
        if !root.is_dir() {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(root) else {
            continue;
        };
        let mut dirs: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        dirs.sort(); // deterministic order within a root
        for dir in dirs {
            let md = dir.join("SKILL.md");
            if !md.is_file() {
                continue;
            }
            match std::fs::read_to_string(&md)
                .map_err(|e| e.to_string())
                .and_then(|s| parse_skill_md(&s).map_err(|e| e.to_string()))
            {
                Ok(bundle) => {
                    if dir_name_mismatch(&dir, &bundle.manifest.name) {
                        tracing::warn!(
                            path = %md.display(), name = %bundle.manifest.name,
                            "skill name does not match directory name (agentskills.io spec violation); loading anyway"
                        );
                    }
                    if seen.insert(bundle.manifest.id.clone()) {
                        out.push(ExternalSkill { path: dir, bundle });
                    }
                }
                Err(e) => {
                    tracing::warn!(path = %md.display(), error = %e, "skipping unparseable SKILL.md");
                }
            }
        }
    }
    out
}

fn dir_name_mismatch(dir: &Path, name: &str) -> bool {
    dir.file_name()
        .map(|d| d.to_string_lossy() != name)
        .unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_skill(root: &Path, dir: &str, frontmatter_name: &str) {
        let d = root.join(dir);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(
            d.join("SKILL.md"),
            format!(
                "---\nname: {frontmatter_name}\ndescription: Test skill body for {frontmatter_name}\n---\n\n# Body\n"
            ),
        )
        .unwrap();
    }

    #[test]
    fn discovers_yaml_skill_dirs_under_root() {
        let tmp = tempfile::tempdir().unwrap();
        write_skill(
            tmp.path(),
            "test-driven-development",
            "test-driven-development",
        );
        write_skill(tmp.path(), "brainstorming", "brainstorming");
        let found = discover_external_skills(&[tmp.path().to_path_buf()]);
        let mut ids: Vec<&str> = found
            .iter()
            .map(|s| s.bundle.manifest.id.as_str())
            .collect();
        ids.sort();
        assert_eq!(ids, vec!["brainstorming", "test-driven-development"]);
        assert!(found.iter().all(|s| s.bundle.skill_md.contains("# Body")));
    }

    #[test]
    fn first_root_wins_on_id_collision() {
        let hi = tempfile::tempdir().unwrap();
        let lo = tempfile::tempdir().unwrap();
        write_skill(hi.path(), "tdd", "tdd");
        write_skill(lo.path(), "tdd", "tdd");
        let found = discover_external_skills(&[hi.path().to_path_buf(), lo.path().to_path_buf()]);
        assert_eq!(found.len(), 1);
        assert!(found[0].path.starts_with(hi.path()));
    }

    #[test]
    fn skips_unparseable_and_missing_roots() {
        let tmp = tempfile::tempdir().unwrap();
        let bad = tmp.path().join("broken");
        std::fs::create_dir_all(&bad).unwrap();
        std::fs::write(bad.join("SKILL.md"), "no frontmatter at all").unwrap();
        let missing = tmp.path().join("does-not-exist");
        let found = discover_external_skills(&[tmp.path().to_path_buf(), missing]);
        assert!(found.is_empty());
    }

    #[test]
    fn ignores_dirs_without_skill_md() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("not-a-skill")).unwrap();
        write_skill(tmp.path(), "real", "real");
        let found = discover_external_skills(&[tmp.path().to_path_buf()]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].bundle.manifest.id, "real");
    }
}
