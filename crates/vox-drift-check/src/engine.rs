use crate::cache::FeatureCache;
use crate::extractor::LanguageExtractor;
#[cfg(feature = "drift-typescript")]
use crate::extractors::typescript::TypeScriptExtractor;
use crate::extractors::{rust::RustExtractor, vox::VoxExtractor};
use crate::features::ExtractedFeatures;
use anyhow::Result;
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use vox_code_audit::rules::Language;
use walkdir::WalkDir;

pub struct WorkspaceFeatures {
    pub files: Vec<ExtractedFeatures>,
    pub workspace_version: String,
}

pub struct DriftEngine {
    root: PathBuf,
}

impl DriftEngine {
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
        }
    }

    pub fn extract_workspace(&self) -> Result<WorkspaceFeatures> {
        let paths = self.collect_source_files();
        let cache = FeatureCache::from_workspace(&self.root);
        let files: Vec<ExtractedFeatures> = paths
            .par_iter()
            .filter_map(|p| {
                let content = std::fs::read_to_string(p).ok()?;
                let lang = detect_language(p);
                let extractor: &dyn LanguageExtractor = match lang {
                    Language::Rust => &RustExtractor,
                    #[cfg(feature = "drift-typescript")]
                    Language::TypeScript => &TypeScriptExtractor,
                    Language::Vox => &VoxExtractor,
                    _ => return None,
                };
                let hash = FeatureCache::hash_file(&content);
                if let Some(mut cached) = cache.load(&hash) {
                    // Always use the current path — the cached entry may have been
                    // recorded from a different location (e.g. an agent worktree) with
                    // identical content.
                    cached.file = p.clone();
                    return Some(cached);
                }
                let result = extractor.extract(p, &content).ok()?;
                cache.store(&hash, &result).ok();
                Some(result)
            })
            .collect();

        let workspace_version = read_workspace_version(&self.root);
        Ok(WorkspaceFeatures {
            files,
            workspace_version,
        })
    }

    pub fn run_all(&self) -> Result<Vec<vox_code_audit::rules::Finding>> {
        use crate::rules::{WorkspaceContext, all_drift_rules};
        use crate::sweep::all_sweep_rules;

        let ws = self.extract_workspace()?;
        let ctx = WorkspaceContext {
            workspace_version: ws.workspace_version.clone(),
            workspace_root: self.root.clone(),
        };

        let mut findings = Vec::new();

        // Sweep rules (cross-file)
        for rule in all_sweep_rules() {
            findings.extend(rule.sweep(&ws.files));
        }

        // Targeted drift rules (per-file with workspace ctx)
        let drift_rules = all_drift_rules();
        for file_features in &ws.files {
            for rule in &drift_rules {
                if rule.languages().contains(&file_features.language) {
                    findings.extend(rule.check(file_features, &ctx));
                }
            }
        }

        Ok(findings)
    }

    fn collect_source_files(&self) -> Vec<PathBuf> {
        WalkDir::new(&self.root)
            .into_iter()
            .filter_entry(|e| {
                let name = e.file_name().to_string_lossy();
                !matches!(
                    name.as_ref(),
                    "target" | "node_modules" | ".git" | "archive" | ".claude"
                ) && name != ".vox-cache"
            })
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .map(|e| e.into_path())
            .filter(|p| !path_contains_claude(p))
            .filter(|p| {
                matches!(
                    detect_language(p),
                    Language::Rust | Language::TypeScript | Language::Vox
                )
            })
            .collect()
    }
}

/// Returns true if any path component equals `.claude` (agent worktree root).
/// Belt-and-suspenders guard alongside the `filter_entry` name check above.
fn path_contains_claude(path: &Path) -> bool {
    path.components().any(|c| c.as_os_str() == ".claude")
}

pub fn detect_language(path: &Path) -> Language {
    match path.extension().and_then(|e| e.to_str()) {
        Some("rs") => Language::Rust,
        Some("ts") | Some("tsx") | Some("js") | Some("jsx") => Language::TypeScript,
        Some("vox") => Language::Vox,
        _ => Language::Unknown,
    }
}

fn read_workspace_version(root: &Path) -> String {
    let cargo = root.join("Cargo.toml");
    std::fs::read_to_string(cargo)
        .ok()
        .and_then(|s| {
            let t: toml::Value = toml::from_str(&s).ok()?;
            t.get("workspace")?
                .get("package")?
                .get("version")?
                .as_str()
                .map(String::from)
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn engine_finds_rust_files_and_extracts() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("foo.rs"),
            r#"fn foo() { let x = "hello"; }"#,
        )
        .unwrap();
        fs::write(dir.path().join("bar.ts"), r#"const x = "world";"#).unwrap();

        let eng = DriftEngine::new(dir.path());
        let ws = eng.extract_workspace().unwrap();
        let rust_files: Vec<_> = ws
            .files
            .iter()
            .filter(|f| f.file.extension().is_some_and(|e| e == "rs"))
            .collect();
        assert!(!rust_files.is_empty());
        assert!(
            rust_files[0]
                .string_literals
                .iter()
                .any(|l| l.value == "hello")
        );
    }
}
