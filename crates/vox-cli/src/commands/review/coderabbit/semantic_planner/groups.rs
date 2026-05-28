//! Ignore rules for semantic CodeRabbit planning (hard safety nets; group tables live in YAML).

/// Files per PR soft target (stays comfortably under 300-file Pro cap).
pub(crate) const DEFAULT_MAX_FILES_PER_PR: usize = 250;

/// Directories / filename suffixes that should never land in a review PR.
///
/// The `"target-"` prefix catches any `target-<name>/` variation (target-agent/, target-doc-inv2/,
/// target-ci/, etc.), mirroring the `.gitignore` `target-*/` rule.
pub(crate) static IGNORED_DIRS: &[&str] = &[
    "target/",
    "target-", // catches target-agent/, target-doc-inv2/, target-ci/, target-toestub/, etc.
    "target_", // catches target_debug/, target_release/, etc.
    ".cargo-targets/",
    "docs/book/",          // generated mdBook HTML (in .gitignore but sometimes tracked)
    ".vox-research-data/", // local SQLite cache
    ".vox/cache/",
    ".gemini/",
    ".cursor/",
    "node_modules/",
    "dist/",
    "dist1/",
    ".next/",
    "tmp_tools/", // temporary scaffolding directories
    "vendor/",
    "apps/editor/vox-vscode/out/",
    // CodeRabbit tooling — worktrees, run-state, manifests.
    // Without this, a bare `.coderabbit` entry in the manifest triggers copy_dir_recursive
    // which recurses inside worktrees and hits Windows OS error 206 (path too long).
    ".coderabbit/",
    ".coderabbit",
];

pub(crate) static IGNORED_EXTENSIONS: &[&str] = &[
    ".db", ".db-wal", ".db-shm", ".png", ".jpg", ".jpeg", ".webp", ".ico", ".dll", ".exe", ".so",
    ".dylib", ".bin", ".svg", ".woff", ".woff2",
    ".csv", // data export / error dumps (errors.csv etc.) — not source code
    ".md", // documentation — excluded from code review PRs (overridable via allow_markdown_prefixes)
    ".txt", // text — excluded from code review PRs (overridable via allow_markdown_prefixes)
    ".hbs", // handlebars templates — excluded from code review PRs
];

/// Root-level files (no `/`) that are always excluded — scratch/agent artefacts.
pub(crate) static IGNORED_ROOT_EXACT: &[&str] = &[
    "Cargo.lock",
    "package-lock.json",
    "pnpm-lock.yaml",
    "yarn.lock",
    // known agent-generated scratch scripts
    "autofix.py",
    "check_lex.py",
    "debug_offset.py",
    "debug_sync.py",
    "find_missing_cli_paths.py",
    "gen_json.py",
    "list_tools.rs",
    "parse_toestub.py",
    "patch2.py",
    "summary.py",
    "sync_registry_final.py",
    "sync_registry_v2.py",
    "sync_registry_v3.py",
    "sync_registry.py",
    "test_clone.rs",
    "test_lexer.rs",
    "test_parse.rs",
    "tmp_parse_check.rs",
    "update_tool_entries.py",
    "update_trust_policy.py",
];

/// Root-level file suffix patterns that are excluded.
pub(crate) static IGNORED_ROOT_PATTERNS: &[&str] = &[
    ".log", ".txt", // only matched for root-level files (no slash)
    ".py",  // root-level Python scripts are scratch; sub-directory ones go to their group
];
