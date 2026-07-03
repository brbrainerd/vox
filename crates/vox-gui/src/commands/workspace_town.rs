// crates/vox-gui/src/commands/workspace_town.rs
//! Workspace scan for the Vox Urbs town map: crates → files → line counts.
//! Read-only, cached, gitignore-aware. Feeds the treemap layout (see
//! docs/superpowers/specs/2026-07-02-vox-urbs-visualizer-rebuild-design.md).

use std::path::Path;
use std::sync::Mutex;

const MAX_FILES: usize = 20_000;
/// Rescan at most this often; the town layout is not a file watcher.
const CACHE_TTL_MS: i64 = 60_000;

#[derive(Debug, Clone, serde::Serialize)]
pub struct TownFileDto {
    /// Path relative to the workspace root, forward slashes.
    pub path: String,
    pub lines: u32,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TownCrateDto {
    pub name: String,
    /// Crate root relative to the workspace root, forward slashes.
    pub root: String,
    pub files: Vec<TownFileDto>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TownScanDto {
    pub crates: Vec<TownCrateDto>,
    /// Absolute workspace root, forward slashes — lets the UI build absolute
    /// paths for open_locator without a second command.
    pub root: String,
    pub scanned_at_ms: i64,
    pub truncated: bool,
}

/// Group scanned source files under `crates/<name>/…`; everything else under
/// a synthetic "(workspace)" crate. Pure — unit-testable without IO.
pub(crate) fn group_by_crate(files: Vec<TownFileDto>) -> Vec<TownCrateDto> {
    use std::collections::BTreeMap;
    let mut map: BTreeMap<(String, String), Vec<TownFileDto>> = BTreeMap::new();
    for f in files {
        let key = match f.path.strip_prefix("crates/") {
            Some(rest) => match rest.split('/').next() {
                Some(name) if rest.contains('/') => {
                    (name.to_string(), format!("crates/{name}"))
                }
                _ => ("(workspace)".to_string(), String::new()),
            },
            None => ("(workspace)".to_string(), String::new()),
        };
        map.entry(key).or_default().push(f);
    }
    map.into_iter()
        .map(|((name, root), files)| TownCrateDto { name, root, files })
        .collect()
}

pub(crate) fn is_source_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("rs" | "ts" | "tsx" | "vox" | "toml" | "md")
    )
}

fn scan(root: &Path) -> TownScanDto {
    let mut files = Vec::new();
    let mut truncated = false;
    let walker = ignore::WalkBuilder::new(root).hidden(true).build();
    for entry in walker.flatten() {
        if files.len() >= MAX_FILES {
            truncated = true;
            break;
        }
        let path = entry.path();
        if !path.is_file() || !is_source_file(path) {
            continue;
        }
        let Ok(rel) = path.strip_prefix(root) else { continue };
        let rel = rel.to_string_lossy().replace('\\', "/");
        // Skip build/vendor dirs the ignore crate may still surface.
        if rel.starts_with("target/") || rel.contains("node_modules/") {
            continue;
        }
        let lines = std::fs::read_to_string(path)
            .map(|c| c.lines().count() as u32)
            .unwrap_or(0);
        files.push(TownFileDto { path: rel, lines });
    }
    TownScanDto {
        crates: group_by_crate(files),
        root: root.to_string_lossy().replace('\\', "/"),
        scanned_at_ms: chrono::Utc::now().timestamp_millis(),
        truncated,
    }
}

static CACHE: Mutex<Option<TownScanDto>> = Mutex::new(None);

#[tauri::command]
pub async fn workspace_town_scan() -> Result<TownScanDto, String> {
    let now = chrono::Utc::now().timestamp_millis();
    if let Some(cached) = CACHE.lock().unwrap().clone() {
        if now - cached.scanned_at_ms < CACHE_TTL_MS {
            return Ok(cached);
        }
    }
    let root = std::env::current_dir().map_err(|e| e.to_string())?;
    let fresh = tokio::task::spawn_blocking(move || scan(&root))
        .await
        .map_err(|e| e.to_string())?;
    *CACHE.lock().unwrap() = Some(fresh.clone());
    Ok(fresh)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_crate_files_and_workspace_files() {
        let files = vec![
            TownFileDto { path: "crates/vox-db/src/lib.rs".into(), lines: 100 },
            TownFileDto { path: "crates/vox-db/src/store.rs".into(), lines: 50 },
            TownFileDto { path: "crates/vox-cli/src/main.rs".into(), lines: 10 },
            TownFileDto { path: "docs/src/intro.md".into(), lines: 5 },
        ];
        let crates = group_by_crate(files);
        let names: Vec<&str> = crates.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["(workspace)", "vox-cli", "vox-db"]);
        assert_eq!(crates[2].files.len(), 2);
        assert_eq!(crates[2].root, "crates/vox-db");
    }

    #[test]
    fn scan_walks_a_fixture_tree_and_counts_lines() {
        let dir = tempfile::tempdir().unwrap();
        let crate_src = dir.path().join("crates/mini/src");
        std::fs::create_dir_all(&crate_src).unwrap();
        std::fs::write(crate_src.join("lib.rs"), "a\nb\nc\n").unwrap();
        std::fs::write(dir.path().join("ignored.png"), b"\x89PNG").unwrap();

        let result = scan(dir.path());
        assert!(!result.truncated);
        assert!(!result.root.is_empty());
        let mini = result.crates.iter().find(|c| c.name == "mini").unwrap();
        assert_eq!(mini.files.len(), 1);
        assert_eq!(mini.files[0].lines, 3);
        assert_eq!(mini.files[0].path, "crates/mini/src/lib.rs");
    }
}
