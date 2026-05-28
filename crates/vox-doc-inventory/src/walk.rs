//! Repository file discovery for inventory scans.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::Result;
use walkdir::WalkDir;

use crate::constants::SKIP_DIR_NAMES;

pub(crate) fn should_skip_path(path: &Path) -> bool {
    path.components().any(|c| {
        let s = c.as_os_str().to_string_lossy();
        SKIP_DIR_NAMES.contains(&s.as_ref())
    })
}

pub(crate) fn iter_repo_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut found: HashSet<String> = HashSet::new();
    let singles = [root.join("AGENTS.md")];
    for s in singles {
        if s.is_file() {
            found.insert(s.strip_prefix(root)?.to_string_lossy().replace('\\', "/"));
        }
    }
    let roots = [
        root.join("crates"),
        root.join("docs"),
        root.join("apps").join("editor").join("vox-vscode"),
        root.join("scripts"),
        root.join(".github").join("workflows"),
    ];
    for base in roots {
        if !base.is_dir() {
            continue;
        }
        for e in WalkDir::new(&base).into_iter().filter_map(|e| e.ok()) {
            let p = e.path();
            if should_skip_path(p) {
                continue;
            }
            if !p.is_file() {
                continue;
            }
            let ext = p.extension().and_then(|x| x.to_str()).unwrap_or("");
            if matches!(ext, "rs" | "md" | "ts" | "yml" | "yaml" | "sh" | "py") {
                let rel = p.strip_prefix(root)?.to_string_lossy().replace('\\', "/");
                found.insert(rel);
            }
        }
    }
    let mut v: Vec<String> = found.into_iter().collect();
    v.sort();
    Ok(v.into_iter().map(|r| root.join(&r)).collect())
}
