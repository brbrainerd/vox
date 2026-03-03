use std::path::{Path, PathBuf};

use globset::{Glob, GlobSet, GlobSetBuilder};
use walkdir::WalkDir;

use crate::rules::{Language, SourceFile};

// ---------------------------------------------------------------------------
// Default exclusions — directories we never want to scan
// ---------------------------------------------------------------------------

const DEFAULT_EXCLUDES: &[&str] = &[
    "**/target/**",
    "**/node_modules/**",
    "**/.venv/**",
    "**/.git/**",
    "**/.jj/**",
    "**/__pycache__/**",
    "**/.next/**",
    "**/.godot/**",
    "**/dist/**",
    "**/build/**",
    "**/.mypy_cache/**",
    "**/.ruff_cache/**",
    "**/.pytest_cache/**",
    "**/vendor/**",
];

/// File-system scanner that walks directories and loads source files.
pub struct Scanner {
    roots: Vec<PathBuf>,
    exclude_set: GlobSet,
    language_filter: Option<Vec<Language>>,
}

impl Scanner {
    /// Create a new scanner.
    ///
    /// * `roots` — Directories to recursively walk.
    /// * `extra_excludes` — Additional glob patterns to skip.
    /// * `language_filter` — If `Some`, only include files of these languages.
    pub fn new(
        roots: Vec<PathBuf>,
        extra_excludes: &[String],
        language_filter: Option<Vec<Language>>,
    ) -> Self {
        let mut builder = GlobSetBuilder::new();
        for pat in DEFAULT_EXCLUDES {
            if let Ok(g) = Glob::new(pat) {
                builder.add(g);
            }
        }
        for pat in extra_excludes {
            if let Ok(g) = Glob::new(pat) {
                builder.add(g);
            }
        }
        let exclude_set = builder.build().unwrap_or_else(|_| GlobSet::empty());

        Self {
            roots,
            exclude_set,
            language_filter,
        }
    }

    /// Walk all roots and return loaded source files.
    pub fn scan(&self) -> Vec<SourceFile> {
        let mut files = Vec::new();
        for root in &self.roots {
            self.walk_root(root, &mut files);
        }
        files
    }

    fn walk_root(&self, root: &Path, out: &mut Vec<SourceFile>) {
        for entry in WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();

            // Skip directories and non-files
            if !path.is_file() {
                continue;
            }

            // Check exclusions against the full path
            let path_str = path.to_string_lossy();
            // Normalise backslashes for glob matching on Windows
            let normalised = path_str.replace('\\', "/");
            if self.exclude_set.is_match(&normalised) {
                continue;
            }

            // Determine language from extension
            let lang = path
                .extension()
                .and_then(|e| e.to_str())
                .map(Language::from_extension)
                .unwrap_or(Language::Unknown);

            if lang == Language::Unknown {
                continue;
            }

            // Apply language filter
            if let Some(ref filter) = self.language_filter {
                if !filter.contains(&lang) {
                    continue;
                }
            }

            // Read file contents
            if let Ok(content) = std::fs::read_to_string(path) {
                out.push(SourceFile::new(path.to_path_buf(), content));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn scanner_finds_rust_files() {
        let dir = std::env::temp_dir().join("toestub_scanner_test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("src")).expect("create dir");
        fs::write(dir.join("src/main.rs"), "fn main() {}").expect("write file");
        fs::write(dir.join("src/readme.txt"), "hello").expect("write file");

        let scanner = Scanner::new(vec![dir.clone()], &[], None);
        let files = scanner.scan();

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].language, Language::Rust);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn scanner_respects_language_filter() {
        let dir = std::env::temp_dir().join("toestub_lang_filter_test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("src")).expect("create dir");
        fs::write(dir.join("src/main.rs"), "fn main() {}").expect("write");
        fs::write(dir.join("src/index.ts"), "export {}").expect("write");

        let scanner = Scanner::new(vec![dir.clone()], &[], Some(vec![Language::TypeScript]));
        let files = scanner.scan();

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].language, Language::TypeScript);

        let _ = fs::remove_dir_all(&dir);
    }
}
