use std::path::{Path, PathBuf};

use globset::{Glob, GlobSet, GlobSetBuilder};
use walkdir::WalkDir;

use crate::rules::{Language, SourceFile};

// ---------------------------------------------------------------------------
// Default exclusions — directories we never want to scan
// ---------------------------------------------------------------------------

const DEFAULT_EXCLUDES: &[&str] = &[
    // Vendored fork sources / upstream patches — not Vox-owned; god-object and scaling noise only.
    "**/patches/**",
    // mdBook and other generated web assets under docs.
    "**/docs/book/**",
    "**/apps/editor/vox-vscode/out/**",
    "**/tools/dashboard/**",
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
    // `include!` fragments for split integration tests — not standalone crates; avoids duplicate noise.
    "**/tests/pipeline/includes/**",
];

/// Number of leading lines scanned for a `@generated` marker. The convention
/// places it in the file header banner; bounding the window keeps a passing
/// mention deep in a generator's own source from excluding that generator.
const GENERATED_HEADER_LINES: usize = 5;

/// True when `content` carries a `@generated` header marker.
///
/// Regenerating the file is the only way to change it, so a finding against one
/// is unactionable noise: the fix belongs in the generator, and the detector
/// output points at the wrong place. This matters most for the whole-`crates/`
/// TOESTUB sweep, where a single 9,999-line generated file
/// (`vox-research-events/src/schema_types.generated.rs`) trips the god-object
/// Error threshold with no fix a contributor can apply.
///
/// Keyed on the header rather than a `*.generated.rs` glob because the
/// convention is not uniformly reflected in filenames —
/// `vox-gui/src/config/generated_fields.rs` is generated but does not match
/// that glob, while `vox-arch-check/src/main.rs` mentions the marker only
/// because it *emits* it.
fn is_generated(content: &str) -> bool {
    content
        .lines()
        .take(GENERATED_HEADER_LINES)
        .any(|l| l.contains("@generated"))
}

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

            // `Language::Unknown` is normally skipped, but a few files are scanned
            // by NAME rather than by extension. Cargo manifests are the case that
            // matters: `from_extension("toml")` is `Unknown`, so dropping every
            // Unknown here silently made `CryptoBanDetector`'s Cargo branch
            // unreachable — its `supported_langs` lists `Unknown` precisely so
            // manifests would reach it, and its tests passed only because they
            // construct `SourceFile` directly and bypass this loop.
            let scanned_by_name = matches!(
                path.file_name().and_then(|n| n.to_str()),
                Some("Cargo.toml" | "Cargo.lock")
            );

            if lang == Language::Unknown && !scanned_by_name {
                continue;
            }

            // Apply language filter
            if let Some(ref filter) = self.language_filter
                && !filter.contains(&lang)
            {
                continue;
            }

            // Read file contents
            if let Ok(content) = vox_bounded_fs::read_utf8_path_capped(path) {
                if is_generated(&content) {
                    continue;
                }
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

#[cfg(test)]
mod generated_marker_tests {
    use super::{GENERATED_HEADER_LINES, is_generated};

    #[test]
    fn header_marked_file_is_generated() {
        // Shape of vox-research-events/src/schema_types.generated.rs.
        assert!(is_generated("// @generated-hash a1b2c3\npub struct X;\n"));
        // The marker may sit a few lines into the banner.
        assert!(is_generated(
            "//! Title\n//!\n// @generated by tool\npub struct X;\n"
        ));
    }

    #[test]
    fn generator_that_merely_emits_the_marker_is_not_excluded() {
        // vox-arch-check/src/main.rs writes the header into ITS OUTPUT; the
        // mention is far below the banner, so the generator stays in scope.
        let mut src = String::from("//! A generator.\n");
        for _ in 0..GENERATED_HEADER_LINES {
            src.push_str("use std::fs;\n");
        }
        src.push_str("const H: &str = \"// @generated-hash\";\n");
        assert!(!is_generated(&src));
    }

    #[test]
    fn ordinary_source_is_not_generated() {
        assert!(!is_generated("//! Hand-written.\npub fn f() {}\n"));
        assert!(!is_generated(""));
    }
}

#[cfg(test)]
mod manifest_scanning_tests {
    use super::Scanner;

    /// Regression: `CryptoBanDetector` carries a Cargo-manifest branch and lists
    /// `Language::Unknown` in `supported_langs` so manifests reach it — but the
    /// scanner dropped every `Unknown` file, so that branch never executed in
    /// production. Its own unit tests passed because they build `SourceFile`
    /// directly and never go through `scan()`.
    ///
    /// This test goes through `scan()` on purpose. It is the check that was
    /// missing, and it fails against the pre-fix scanner.
    #[test]
    fn scan_delivers_cargo_manifests_to_detectors() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"x\"\n")
            .expect("write manifest");
        std::fs::write(dir.path().join("lib.rs"), "pub fn f() {}\n").expect("write rs");

        let files = Scanner::new(vec![dir.path().to_path_buf()], &[], None).scan();

        assert!(
            files.iter().any(|f| f.path.ends_with("Cargo.toml")),
            "Cargo.toml must reach detectors; a crypto gate that cannot see \
             manifests is a gate that has never run"
        );
    }

    /// The fix is by filename, not by "stop skipping Unknown" — a blanket change
    /// would hand every detector every `.png`, `.lock`, and `.bin` in the tree.
    #[test]
    fn scan_still_skips_unrecognised_extensions() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("notes.md"), "# hi\n").expect("write md");
        std::fs::write(dir.path().join("blob.bin"), "\x00\x01").expect("write bin");

        let files = Scanner::new(vec![dir.path().to_path_buf()], &[], None).scan();

        assert!(
            !files.iter().any(|f| f.path.ends_with("blob.bin")),
            "unrecognised extensions must still be skipped"
        );
    }
}
