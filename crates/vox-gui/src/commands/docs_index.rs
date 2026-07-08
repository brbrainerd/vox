//! Docs index for omni-search: walks `docs/src/**/*.md` frontmatter.
//!
//! Only **help** categories (how-to, tutorial, reference, contributor) are indexed
//! for the Omnibar — architecture/SSOT docs are excluded unless opened by path.

use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct DocEntry {
    pub title: String,
    pub description: String,
    pub path: String, // absolute path for open_locator
}

pub(crate) struct Frontmatter {
    pub title: String,
    pub description: String,
    pub category: Option<String>,
}

const HELP_CATEGORIES: &[&str] = &["how-to", "tutorial", "reference", "contributor"];

pub(crate) fn parse_frontmatter(content: &str) -> Option<Frontmatter> {
    let rest = content.strip_prefix("---")?;
    let end = rest.find("\n---")?;
    let block = &rest[..end];
    let mut title = None;
    let mut description = None;
    let mut category = None;
    for line in block.lines() {
        if let Some(v) = line.strip_prefix("title:") {
            title = Some(v.trim().trim_matches('"').trim_matches('\'').to_string());
        } else if let Some(v) = line.strip_prefix("description:") {
            description = Some(v.trim().trim_matches('"').trim_matches('\'').to_string());
        } else if let Some(v) = line.strip_prefix("category:") {
            category = Some(v.trim().trim_matches('"').trim_matches('\'').to_string());
        }
    }
    Some(Frontmatter {
        title: title?,
        description: description.unwrap_or_default(),
        category,
    })
}

pub(crate) fn is_help_doc(fm: &Frontmatter, path: &Path) -> bool {
    let path_str = path.to_string_lossy().replace('\\', "/");
    if path_str.contains("docs/src/archive") {
        return false;
    }
    fm.category
        .as_deref()
        .is_some_and(|c| HELP_CATEGORIES.contains(&c))
}

fn walk_docs(root: &Path, out: &mut Vec<DocEntry>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_docs(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("md")
            && let Ok(content) = std::fs::read_to_string(&path)
            && let Some(fm) = parse_frontmatter(&content)
        {
            if !is_help_doc(&fm, &path) {
                continue;
            }
            out.push(DocEntry {
                title: fm.title,
                description: fm.description,
                path: path.to_string_lossy().to_string(),
            });
        }
    }
}

fn repo_docs_root() -> PathBuf {
    let hint = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let repo = vox_repository::discover_repository_or_fallback(&hint);
    repo.root.join("docs").join("src")
}

fn normalize_path_components(path: &Path) -> Result<PathBuf, String> {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(p) => out.push(p.as_os_str()),
            Component::RootDir => out.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    return Err("doc path must be under docs/src".into());
                }
            }
            Component::Normal(c) => out.push(c),
        }
    }
    Ok(out)
}

/// Resolve `path` under `docs_root`, rejecting `..` escapes (testable without repo I/O).
pub(crate) fn resolve_path_under_root(docs_root: &Path, path: &str) -> Result<PathBuf, String> {
    let docs_root = normalize_path_components(docs_root)?;
    let candidate = PathBuf::from(path);
    let joined = if candidate.is_absolute() {
        candidate
    } else {
        docs_root.join(candidate)
    };
    let resolved = normalize_path_components(&joined)?;
    if !resolved.starts_with(&docs_root) {
        return Err("doc path must be under docs/src".into());
    }
    if resolved.extension().and_then(|e| e.to_str()) != Some("md") {
        return Err("doc path must be a markdown file".into());
    }
    Ok(resolved)
}

fn resolve_doc_path(path: &str) -> Result<PathBuf, String> {
    resolve_path_under_root(&repo_docs_root(), path)
}

static DOCS_CACHE: OnceLock<Vec<DocEntry>> = OnceLock::new();

#[tauri::command]
pub async fn vox_docs_index() -> Result<Vec<DocEntry>, String> {
    Ok(DOCS_CACHE
        .get_or_init(|| {
            let mut out = Vec::new();
            walk_docs(&repo_docs_root(), &mut out);
            out.sort_by(|a, b| a.title.cmp(&b.title));
            out
        })
        .clone())
}

#[tauri::command]
pub async fn read_doc_markdown(path: String) -> Result<String, String> {
    let resolved = resolve_doc_path(&path)?;
    std::fs::read_to_string(&resolved).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn parses_frontmatter_title_and_description() {
        let md = "---\ntitle: Mesh SSOT\ndescription: Seven phases of mesh work\ncategory: architecture\n---\n\n# Body\n";
        let fm = parse_frontmatter(md).expect("frontmatter");
        assert_eq!(fm.title, "Mesh SSOT");
        assert_eq!(fm.description, "Seven phases of mesh work");
        assert_eq!(fm.category.as_deref(), Some("architecture"));
    }

    #[test]
    fn missing_frontmatter_returns_none() {
        assert!(parse_frontmatter("# Just a heading\n").is_none());
    }

    #[test]
    fn strips_surrounding_quotes() {
        let md = "---\ntitle: \"Quoted Title\"\ndescription: 'single'\n---\nbody";
        let fm = parse_frontmatter(md).expect("frontmatter");
        assert_eq!(fm.title, "Quoted Title");
        assert_eq!(fm.description, "single");
    }

    #[test]
    fn excludes_architecture_category() {
        let fm = Frontmatter {
            title: "Arch".into(),
            description: String::new(),
            category: Some("architecture".into()),
        };
        assert!(!is_help_doc(&fm, Path::new("docs/src/architecture/foo.md"),));
    }

    #[test]
    fn includes_how_to() {
        let fm = Frontmatter {
            title: "How".into(),
            description: String::new(),
            category: Some("how-to".into()),
        };
        assert!(is_help_doc(&fm, Path::new("docs/src/how-to/foo.md"),));
    }

    #[test]
    fn rejects_path_traversal_outside_docs_root() {
        let root = Path::new("/repo/docs/src");
        assert!(resolve_path_under_root(root, "../../../etc/passwd").is_err());
        assert!(resolve_path_under_root(root, "reference/../../outside.md").is_err());
    }

    #[test]
    fn accepts_relative_help_doc_under_root() {
        let root = Path::new("/repo/docs/src");
        let resolved = resolve_path_under_root(root, "reference/cli.md").expect("resolve");
        assert!(resolved.ends_with("reference/cli.md"));
    }
}
