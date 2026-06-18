//! Static finding pages for docs SSG and integration tests.

use std::path::{Path, PathBuf};

use super::page::FindingPage;
use super::render::render_finding_page;

/// Default directory of checked-in `FindingPage` JSON fixtures (repo-relative).
pub fn default_fixtures_dir(repo_root: &Path) -> PathBuf {
    repo_root.join("contracts/scientia/fixtures/findings")
}

/// Load every `*.json` fixture under `dir`, returning `(trusty_uri, page)`.
pub fn load_finding_fixtures(dir: &Path) -> Result<Vec<(String, FindingPage)>, String> {
    if !dir.is_dir() {
        return Err(format!("fixtures dir missing: {}", dir.display()));
    }
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let raw = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let page: FindingPage = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
        out.push((page.trusty_uri.clone(), page));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

/// Render all fixtures under `dir` to a map of `trusty_uri → HTML`.
pub fn render_fixture_pages(
    dir: &Path,
) -> Result<std::collections::BTreeMap<String, String>, String> {
    let pages = load_finding_fixtures(dir)?;
    let mut out = std::collections::BTreeMap::new();
    for (uri, page) in pages {
        out.insert(uri, render_finding_page(&page));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("repo root")
    }

    #[test]
    fn sample_fixture_renders_html_with_doctype() {
        let dir = default_fixtures_dir(&repo_root());
        let rendered = render_fixture_pages(&dir).expect("fixtures render");
        let html = rendered
            .get("RA1234567890abcdef")
            .expect("sample trusty uri present");
        assert!(html.starts_with("<!doctype html>"));
        assert!(html.contains("<title>Fast Foo</title>"));
    }
}
