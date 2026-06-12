//! Docs index for omni-search: walks `docs/src/**/*.md` frontmatter.
//!
//! Authored docs are required to carry `title`/`description` frontmatter (see
//! documentation-governance.md), so a frontmatter walk is a complete index.

use std::path::PathBuf;
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
}

pub(crate) fn parse_frontmatter(content: &str) -> Option<Frontmatter> {
    let rest = content.strip_prefix("---")?;
    let end = rest.find("\n---")?;
    let block = &rest[..end];
    let mut title = None;
    let mut description = None;
    for line in block.lines() {
        if let Some(v) = line.strip_prefix("title:") {
            title = Some(v.trim().trim_matches('"').trim_matches('\'').to_string());
        } else if let Some(v) = line.strip_prefix("description:") {
            description = Some(v.trim().trim_matches('"').trim_matches('\'').to_string());
        }
    }
    Some(Frontmatter {
        title: title?,
        description: description.unwrap_or_default(),
    })
}

fn walk_docs(root: &std::path::Path, out: &mut Vec<DocEntry>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_docs(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Some(fm) = parse_frontmatter(&content) {
                    out.push(DocEntry {
                        title: fm.title,
                        description: fm.description,
                        path: path.to_string_lossy().to_string(),
                    });
                }
            }
        }
    }
}

static DOCS_CACHE: OnceLock<Vec<DocEntry>> = OnceLock::new();

#[tauri::command]
pub async fn vox_docs_index() -> Result<Vec<DocEntry>, String> {
    Ok(DOCS_CACHE
        .get_or_init(|| {
            let hint = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            let repo = vox_repository::discover_repository_or_fallback(&hint);
            let mut out = Vec::new();
            walk_docs(&repo.root.join("docs").join("src"), &mut out);
            out.sort_by(|a, b| a.title.cmp(&b.title));
            out
        })
        .clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_frontmatter_title_and_description() {
        let md = "---\ntitle: Mesh SSOT\ndescription: Seven phases of mesh work\ncategory: architecture\n---\n\n# Body\n";
        let fm = parse_frontmatter(md).expect("frontmatter");
        assert_eq!(fm.title, "Mesh SSOT");
        assert_eq!(fm.description, "Seven phases of mesh work");
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
}
