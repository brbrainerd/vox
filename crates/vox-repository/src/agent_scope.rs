//! Agent definition paths and `scope:` parsing under a repository root.

use std::fs;
use std::path::{Path, PathBuf};

/// Directory containing agent markdown files (e.g. `my-agent.md`).
pub fn agents_dir(repo_root: &Path) -> PathBuf {
    repo_root.join(".vox").join("agents")
}

/// Repo-relative glob for agent definitions (for `scope:` documentation defaults).
pub fn agents_glob_repo_relative() -> &'static str {
    vox_config::paths::REPO_AGENTS_GLOB
}

/// Read `scope:` from `.vox/agents/{agent_name}.md` front matter (first `scope:` line wins).
///
/// Supports `scope: [a, b]` or `scope:` followed by YAML list lines `  - pat`.
pub fn load_agent_scopes(repo_root: &Path, agent_name: &str) -> Option<Vec<String>> {
    let path = agents_dir(repo_root).join(format!("{agent_name}.md"));
    let content = fs::read_to_string(&path).ok()?;
    parse_scope_from_agent_markdown(&content)
}

fn parse_scope_from_agent_markdown(content: &str) -> Option<Vec<String>> {
    let (front, _) = content
        .split_once("---\n")
        .and_then(|(_, rest)| rest.split_once("\n---"))
        .unwrap_or(("", content));

    let mut scopes: Vec<String> = Vec::new();
    let mut in_scope_list = false;

    for line in front.lines() {
        let t = line.trim();
        if t.starts_with("scope:") {
            let inside = t.trim_start_matches("scope:").trim();
            if inside.is_empty() || inside == "|" {
                in_scope_list = true;
                continue;
            }
            if inside.starts_with('[') {
                let inside = inside.trim_start_matches('[').trim_end_matches(']');
                for pat in inside.split(',') {
                    let clean = pat.trim().trim_matches('"').trim_matches('\'');
                    if !clean.is_empty() {
                        scopes.push(clean.to_string());
                    }
                }
                return Some(scopes);
            }
        } else if in_scope_list && (t.starts_with("- ") || t.starts_with('-')) {
            let pat = t.trim_start_matches('-').trim();
            let pat = pat.trim_matches('"').trim_matches('\'');
            if !pat.is_empty() {
                scopes.push(pat.to_string());
            }
        } else if in_scope_list && !t.is_empty() && !t.starts_with('#') {
            in_scope_list = false;
        }
    }

    if scopes.is_empty() {
        None
    } else {
        Some(scopes)
    }
}

/// Strip `repo_root` from `file_path`, normalize to forward slashes (for glob checks).
pub fn normalize_task_path(repo_root: &Path, file_path: &str) -> String {
    let p = PathBuf::from(file_path);
    let rel = if p.is_absolute() {
        p.strip_prefix(repo_root)
            .map(|x| x.to_path_buf())
            .unwrap_or(p)
    } else {
        p
    };
    rel.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn parse_scope_bracketed() {
        let md = "---\nname: x\nscope: [crates/**, docs/**]\n---\n";
        let s = parse_scope_from_agent_markdown(md).unwrap();
        assert_eq!(s, vec!["crates/**", "docs/**"]);
    }

    #[test]
    fn load_from_disk() {
        let dir = tempdir().unwrap();
        let agents = agents_dir(dir.path());
        fs::create_dir_all(&agents).unwrap();
        let mut f = fs::File::create(agents.join("tester.md")).unwrap();
        writeln!(f, "---").unwrap();
        writeln!(f, "name: tester").unwrap();
        writeln!(f, "scope:").unwrap();
        writeln!(f, "  - src/**").unwrap();
        writeln!(f, "---").unwrap();
        let sc = load_agent_scopes(dir.path(), "tester").unwrap();
        assert_eq!(sc, vec!["src/**"]);
    }
}
