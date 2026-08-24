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
    // Agent-harness state, not project source. `.claude/worktrees/` in particular
    // holds FULL checkouts of this repo (one per concurrent agent session), so
    // without this a whole-repo scan reports every finding once per worktree —
    // measured 2026-08-23: 279 of 372 `secret/env-get-shape` findings came from
    // here, swamping the 93 that are actually first-party.
    "**/.claude/**",
    // Subagent-driven-development scratch: ledgers, briefs, review packages.
    "**/.superpowers/**",
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
            // `Cargo.toml` maps to `Language::Unknown` (it has no source language),
            // but `crypto_ban` — the only rule declaring `Unknown` — needs to see
            // dependency declarations. Before this, `Scanner` dropped every Unknown
            // file here, so that detector's whole Cargo arm was dead code and a
            // first-party `aws-lc-rs = "1"` would have been caught by nothing.
            //
            // Scoped to manifests on purpose: admitting all Unknown files would feed
            // every .md/.json/.yaml/.lock in the tree through the detector loop for
            // no gain. `Cargo.lock` is excluded too — its `name = "ring"` form does
            // not match the detector's `^<crate> =` dependency-declaration regexes,
            // so it would be pure I/O. NOTE: there is deliberately no whole-graph gate —
            // a `[[bans.deny]]` for aws-lc-rs was added and reverted on 2026-08-23
            // because that crate is already in the lock and already builds. Transitive
            // crypto arrivals are reviewed in cryptography-ssot-2026.md, not gated.
            let is_manifest = path.file_name().and_then(|n| n.to_str()) == Some("Cargo.toml");
            if lang == Language::Unknown && !is_manifest {
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

    /// `Cargo.toml` has no source language, but `crypto_ban` is registered for
    /// `Language::Unknown` specifically to read dependency declarations. If the
    /// scanner drops it, that detector's Cargo arm silently never runs.
    #[test]
    fn scanner_collects_cargo_manifests_but_not_other_unknown_files() {
        let dir = std::env::temp_dir().join("toestub_manifest_scan_test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create dir");
        fs::write(dir.join("Cargo.toml"), "[dependencies]\nring = \"0.17\"\n").expect("write");
        fs::write(dir.join("Cargo.lock"), "[[package]]\nname = \"ring\"\n").expect("write");
        fs::write(dir.join("notes.md"), "hello").expect("write");

        let scanner = Scanner::new(vec![dir.clone()], &[], None);
        let names: Vec<String> = scanner
            .scan()
            .iter()
            .filter_map(|f| {
                f.path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(String::from)
            })
            .collect();

        assert_eq!(names, vec!["Cargo.toml".to_string()], "got {names:?}");

        let _ = fs::remove_dir_all(&dir);
    }

    /// `.claude/worktrees/` holds full checkouts of this repo — one per concurrent
    /// agent session — so scanning it reports every finding once per worktree.
    #[test]
    fn scanner_skips_agent_harness_state() {
        let dir = std::env::temp_dir().join("toestub_harness_exclude_test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join(".claude/worktrees/agent-x/src")).expect("mkdir");
        fs::create_dir_all(dir.join(".superpowers/sdd")).expect("mkdir");
        fs::create_dir_all(dir.join("src")).expect("mkdir");
        fs::write(
            dir.join(".claude/worktrees/agent-x/src/dup.rs"),
            "fn main() {}",
        )
        .expect("w");
        fs::write(dir.join(".superpowers/sdd/scratch.rs"), "fn main() {}").expect("w");
        fs::write(dir.join("src/real.rs"), "fn main() {}").expect("w");

        let names: Vec<String> = Scanner::new(vec![dir.clone()], &[], None)
            .scan()
            .iter()
            .filter_map(|f| {
                f.path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(String::from)
            })
            .collect();

        assert_eq!(names, vec!["real.rs".to_string()], "got {names:?}");

        let _ = fs::remove_dir_all(&dir);
    }
}
