//! Rule 11 (P3-T7): forbid raw `Command::new("git")` outside the wrapper.
//!
//! Implementation: compile `pattern` as a regex; for every file under `file_glob`
//! that is NOT in `exempt_files`, scan line-by-line for matches. If a match is
//! preceded (within 2 lines) or followed (within 1 line) by `allow_annotation`,
//! it is suppressed. Concretely: the suppression window is [i-2, i+1] inclusive,
//! where i is the 0-based line index of the match.
//!
//! False positives we tolerate: string literals in doc comments. The annotation
//! suppression is the escape hatch. For files that legitimately contain example
//! path strings (e.g., code-audit detector minimal_repro() methods), prefer
//! adding them to `exempt_files` rather than embedding annotations inside string
//! literals — annotations inside strings suppress by coincidence of proximity,
//! not by intent, and confuse future readers.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use globset::Glob;
use regex::Regex;

/// A `[[forbidden_pattern]]` rule entry from `layers.toml`.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ForbiddenPatternRule {
    pub name: String,
    pub pattern: String,
    pub file_glob: String,
    #[serde(default)]
    pub exempt_files: Vec<String>,
    /// When `true`, matches inside test code are ignored: files under a `tests/`
    /// directory AND lines inside an inline `#[cfg(test)]` module. Use for
    /// cross-OS-portability / unsafe-code rules that target SHIPPED code —
    /// test fixtures legitimately contain literal paths, `.so` names, and
    /// `unsafe` shims. Default `false` (rule applies to test code too).
    #[serde(default)]
    pub exempt_tests: bool,
    pub allow_annotation: Option<String>,
    pub reason: String,
}

/// Per-line mask marking lines that belong to an inline `#[cfg(test)]` module
/// (the attribute line plus its brace-delimited body). Brace counting is
/// whole-line (string/comment braces are rare in test-module headers and only
/// risk *extending* the mask, which is the safe direction for an exemption).
fn cfg_test_line_mask(lines: &[&str]) -> Vec<bool> {
    let mut mask = vec![false; lines.len()];
    let mut i = 0;
    while i < lines.len() {
        if lines[i].trim_start().starts_with("#[cfg(test)]") {
            let mut depth: i32 = 0;
            let mut opened = false;
            let mut j = i;
            while j < lines.len() {
                for ch in lines[j].chars() {
                    if ch == '{' {
                        depth += 1;
                        opened = true;
                    } else if ch == '}' {
                        depth -= 1;
                    }
                }
                mask[j] = true;
                if opened && depth <= 0 {
                    break;
                }
                j += 1;
            }
            i = j + 1;
        } else {
            i += 1;
        }
    }
    mask
}

/// A single match produced by `scan`.
#[derive(Debug)]
pub struct ForbiddenPatternHit {
    pub rule: String,
    pub file: PathBuf,
    pub line: usize,
    pub matched: String,
    /// Cloned from [`ForbiddenPatternRule::reason`] so the report layer can
    /// surface the rationale alongside each hit.
    pub reason: String,
}

/// Scan every file under `repo_root` that matches `rule.file_glob` for the
/// forbidden regex pattern. Returns all hits that are not suppressed by an
/// `allow_annotation` within a [i-2, i+1] line window (2 lines before, 1 line
/// after the match at line i).
///
/// `prune_dir_names` is the merged built-in + `layers.toml` directory-name skip set
/// (see `walk_prune_dir_names` in `main.rs`).
#[cfg(test)]
pub fn scan(
    repo_root: &Path,
    rule: &ForbiddenPatternRule,
    prune_dir_names: &HashSet<String>,
) -> Result<Vec<ForbiddenPatternHit>> {
    // Test-only single-rule wrapper around the batched implementation.
    scan_all(repo_root, std::slice::from_ref(rule), prune_dir_names)
}

/// Batched scan: walk `repo_root` ONCE, read each candidate file ONCE, and
/// match every rule's regex against the loaded text. With N rules and F files
/// matching at least one rule's glob, this is O(walk + F·N) instead of the
/// O(N·(walk + F)) the per-rule `scan()` does. On the live workspace
/// (~3K .rs files, 9 patterns) the saving is on the order of minutes.
pub fn scan_all(
    repo_root: &Path,
    rules: &[ForbiddenPatternRule],
    prune_dir_names: &HashSet<String>,
) -> Result<Vec<ForbiddenPatternHit>> {
    if rules.is_empty() {
        return Ok(Vec::new());
    }
    // Pre-compile regex + glob for each rule once.
    struct Compiled<'a> {
        rule: &'a ForbiddenPatternRule,
        regex: Regex,
        glob: globset::GlobMatcher,
    }
    let mut compiled: Vec<Compiled> = Vec::with_capacity(rules.len());
    for rule in rules {
        let regex = Regex::new(&rule.pattern)
            .with_context(|| format!("compile forbidden_pattern regex for '{}'", rule.name))?;
        let glob = Glob::new(&rule.file_glob)?.compile_matcher();
        compiled.push(Compiled { rule, regex, glob });
    }

    let mut hits = Vec::new();
    for path in super::walk_repo_files(repo_root, prune_dir_names) {
        // walk_repo_files already filters out directories, so this is the file path.
        let rel = path.strip_prefix(repo_root).unwrap_or(&path);
        let rel_unix = rel.to_string_lossy().replace('\\', "/");

        // First, determine which rules apply to this file (by glob and exempt set).
        // If none match, we skip the read entirely.
        let mut applicable: Vec<&Compiled> = Vec::new();
        for c in &compiled {
            if !c.glob.is_match(rel) {
                continue;
            }
            if c.rule.exempt_files.iter().any(|e| e == &rel_unix) {
                continue;
            }
            applicable.push(c);
        }
        if applicable.is_empty() {
            continue;
        }

        let body = match std::fs::read_to_string(&path) {
            Ok(b) => b,
            Err(_) => continue, // skip binary / unreadable files
        };
        let lines: Vec<&str> = body.lines().collect();

        // Test-context exemption inputs, computed once per file (only the rules
        // that opt in via `exempt_tests` consult these).
        let in_tests_dir = rel.components().any(|comp| comp.as_os_str() == "tests");
        let test_mask = if applicable.iter().any(|c| c.rule.exempt_tests) {
            cfg_test_line_mask(&lines)
        } else {
            Vec::new()
        };

        for c in applicable {
            for (i, line) in lines.iter().enumerate() {
                if !c.regex.is_match(line) {
                    continue;
                }
                if c.rule.exempt_tests
                    && (in_tests_dir || test_mask.get(i).copied().unwrap_or(false))
                {
                    continue;
                }
                if let Some(ann) = c.rule.allow_annotation.as_deref() {
                    let lo = i.saturating_sub(2);
                    let hi = (i + 1).min(lines.len().saturating_sub(1));
                    if (lo..=hi).any(|j| lines[j].contains(ann)) {
                        continue;
                    }
                }
                hits.push(ForbiddenPatternHit {
                    rule: c.rule.name.clone(),
                    file: rel.to_path_buf(),
                    line: i + 1,
                    matched: c
                        .regex
                        .find(line)
                        .map(|m| m.as_str().to_string())
                        .unwrap_or_default(),
                    reason: c.rule.reason.clone(),
                });
            }
        }
    }
    Ok(hits)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    fn make_rule() -> ForbiddenPatternRule {
        ForbiddenPatternRule {
            name: "raw-git-exec".into(),
            exempt_tests: false,
            pattern: r#"Command::new\("git"\)"#.into(),
            file_glob: "crates/**/*.rs".into(),
            exempt_files: vec!["crates/vox-vcs-git/src/git_exec.rs".into()],
            allow_annotation: Some("// vox-arch-check: allow git-exec".into()),
            reason: "All git invocations must go through GitExec.".into(),
        }
    }

    fn write_fixture(dir: &tempfile::TempDir, rel_path: &str, content: &str) {
        let path = dir.path().join(rel_path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
    }

    #[test]
    fn raw_git_outside_git_exec_is_flagged() {
        let dir = tempfile::tempdir().unwrap();
        write_fixture(
            &dir,
            "crates/my-crate/src/main.rs",
            r#"fn bad() { let _ = Command::new("git"); }"#,
        );
        let rule = make_rule();
        let hits = scan(dir.path(), &rule, &crate::built_in_walk_prune_names()).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].rule, "raw-git-exec");
        assert!(hits[0].matched.contains("Command::new(\"git\")"));
    }

    #[test]
    fn exempt_file_is_not_flagged() {
        let dir = tempfile::tempdir().unwrap();
        write_fixture(
            &dir,
            "crates/vox-vcs-git/src/git_exec.rs",
            r#"fn run() { let _ = Command::new("git"); }"#,
        );
        let rule = make_rule();
        let hits = scan(dir.path(), &rule, &crate::built_in_walk_prune_names()).unwrap();
        assert_eq!(hits.len(), 0, "exempt file must not produce hits");
    }

    #[test]
    fn allow_annotation_on_preceding_line_suppresses_hit() {
        let dir = tempfile::tempdir().unwrap();
        write_fixture(
            &dir,
            "crates/my-crate/src/lib.rs",
            "// vox-arch-check: allow git-exec\nlet _ = Command::new(\"git\");\n",
        );
        let rule = make_rule();
        let hits = scan(dir.path(), &rule, &crate::built_in_walk_prune_names()).unwrap();
        assert_eq!(hits.len(), 0, "annotated call must be suppressed");
    }

    #[test]
    fn allow_annotation_on_same_line_suppresses_hit() {
        let dir = tempfile::tempdir().unwrap();
        write_fixture(
            &dir,
            "crates/my-crate/src/lib.rs",
            "let _ = Command::new(\"git\"); // vox-arch-check: allow git-exec\n",
        );
        let rule = make_rule();
        let hits = scan(dir.path(), &rule, &crate::built_in_walk_prune_names()).unwrap();
        assert_eq!(hits.len(), 0, "inline annotation must be suppressed");
    }

    #[test]
    fn non_rs_file_under_crates_is_not_scanned() {
        let dir = tempfile::tempdir().unwrap();
        // .toml file should not match the `crates/**/*.rs` glob.
        write_fixture(&dir, "crates/my-crate/Cargo.toml", r#"[package]"#);
        let rule = make_rule();
        // No .rs files → no hits.
        let hits = scan(dir.path(), &rule, &crate::built_in_walk_prune_names()).unwrap();
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn nested_target_rs_not_scanned_even_if_matches_glob() {
        let dir = tempfile::tempdir().unwrap();
        write_fixture(
            &dir,
            "crates/my-crate/target/out/generated.rs",
            r#"fn x() { let _ = Command::new("git"); }"#,
        );
        let rule = make_rule();
        let hits = scan(dir.path(), &rule, &crate::built_in_walk_prune_names()).unwrap();
        assert_eq!(hits.len(), 0, "must not recurse into target/: {hits:?}");
    }

    #[test]
    fn multiple_violations_in_same_file_all_reported() {
        let dir = tempfile::tempdir().unwrap();
        write_fixture(
            &dir,
            "crates/my-crate/src/util.rs",
            "let a = Command::new(\"git\");\nlet b = Command::new(\"git\");\n",
        );
        let rule = make_rule();
        let hits = scan(dir.path(), &rule, &crate::built_in_walk_prune_names()).unwrap();
        assert_eq!(hits.len(), 2);
    }

    fn shell_spawn_rule() -> ForbiddenPatternRule {
        ForbiddenPatternRule {
            name: "no-hardcoded-shell-spawn".into(),
            exempt_tests: false,
            pattern: r#"Command::new\(\s*"(cmd|cmd\.exe|powershell|pwsh|sh|bash)""#.into(),
            file_glob: "crates/**/*.rs".into(),
            exempt_files: vec![
                "crates/vox-cli-core/src/fs_utils.rs".into(),
                "crates/vox-cli/src/fs_utils.rs".into(),
                "crates/vox-scientia/src/replay/sandbox.rs".into(),
                "crates/vox-ml-cli/src/commands/mens/plugin_heal.rs".into(),
            ],
            allow_annotation: Some("// vox-arch-check: allow shell-spawn".into()),
            reason: "Shell/PowerShell spawns must be cfg(windows)-gated or resolved via which::which(pwsh|powershell).".into(),
        }
    }

    #[test]
    fn hardcoded_pwsh_spawn_is_flagged() {
        let dir = tempfile::tempdir().unwrap();
        // No suppression annotation in fixture — expect 1 hit.
        write_fixture(
            &dir,
            "crates/x/src/a.rs",
            "fn f() { let _ = Command::new(\"pwsh\"); }",
        );
        let hits = scan(
            dir.path(),
            &shell_spawn_rule(),
            &crate::built_in_walk_prune_names(),
        )
        .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].rule, "no-hardcoded-shell-spawn");
    }

    #[test]
    fn annotated_shell_spawn_is_suppressed() {
        let dir = tempfile::tempdir().unwrap();
        write_fixture(
            &dir,
            "crates/x/src/b.rs",
            "// vox-arch-check: allow shell-spawn\nlet _ = Command::new(\"cmd\");\n",
        );
        let hits = scan(
            dir.path(),
            &shell_spawn_rule(),
            &crate::built_in_walk_prune_names(),
        )
        .unwrap();
        assert_eq!(hits.len(), 0);
    }

    fn abs_path_rule() -> ForbiddenPatternRule {
        ForbiddenPatternRule {
            name: "no-hardcoded-abs-path".into(),
            exempt_tests: false,
            // Absolute Unix roots OR a Windows drive letter, inside a string literal.
            pattern: r#""(/(tmp|usr|etc|var|home|opt|bin|root)\b|[A-Za-z]:\\)"#.into(),
            file_glob: "crates/**/*.rs".into(),
            exempt_files: vec![],
            allow_annotation: Some("// vox-arch-check: allow abs-path".into()),
            reason: "Hardcoded absolute paths break across OSes; use std::env::temp_dir()/dirs/Path::join.".into(),
        }
    }

    #[test]
    fn hardcoded_tmp_path_is_flagged() {
        let dir = tempfile::tempdir().unwrap();
        // No suppression annotation in fixture — expect 1 hit.
        write_fixture(&dir, "crates/x/src/c.rs", "let p = \"/tmp/contracts\";");
        let hits = scan(
            dir.path(),
            &abs_path_rule(),
            &crate::built_in_walk_prune_names(),
        )
        .unwrap();
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn hardcoded_drive_path_is_flagged() {
        let dir = tempfile::tempdir().unwrap();
        // No suppression annotation in fixture — expect 1 hit.
        write_fixture(
            &dir,
            "crates/x/src/d.rs",
            "let p = \"C:\\\\Users\\\\Default\";",
        );
        let hits = scan(
            dir.path(),
            &abs_path_rule(),
            &crate::built_in_walk_prune_names(),
        )
        .unwrap();
        assert_eq!(hits.len(), 1);
    }

    fn dynlib_ext_rule() -> ForbiddenPatternRule {
        ForbiddenPatternRule {
            name: "no-hardcoded-dynlib-ext".into(),
            exempt_tests: false,
            // A quoted filename ending in a platform-specific shared-lib suffix.
            pattern: r#""[^"]*\.(so|dll|dylib)""#.into(),
            file_glob: "crates/**/*.rs".into(),
            exempt_files: vec![],
            allow_annotation: Some("// vox-arch-check: allow dynlib-ext".into()),
            reason: "Shared-lib suffix differs per OS (.so/.dll/.dylib); derive it from target, do not hardcode.".into(),
        }
    }

    #[test]
    fn exempt_tests_skips_tests_dir_and_cfg_test_blocks() {
        let dir = tempfile::tempdir().unwrap();
        // (1) integration test under a `tests/` dir — skipped when exempt_tests.
        write_fixture(&dir, "crates/x/tests/it.rs", "let p = \"/tmp/fixture\";");
        // (2) inline #[cfg(test)] block in a src file — the literal inside is
        //     skipped, but a literal in shipped code (top) is still flagged.
        write_fixture(
            &dir,
            "crates/x/src/lib.rs",
            "fn ship() { let _ = \"/etc/real\"; }\n#[cfg(test)]\nmod tests {\n    fn t() { let _ = \"/tmp/fixture\"; }\n}\n",
        );
        let mut rule = abs_path_rule();
        rule.exempt_tests = true;
        let hits = scan(dir.path(), &rule, &crate::built_in_walk_prune_names()).unwrap();
        assert_eq!(hits.len(), 1, "only shipped /etc/real survives; got {hits:?}");
        assert!(
            hits[0]
                .file
                .to_string_lossy()
                .replace('\\', "/")
                .ends_with("src/lib.rs")
        );
        assert_eq!(hits[0].line, 1);
    }

    #[test]
    fn hardcoded_so_suffix_is_flagged() {
        let dir = tempfile::tempdir().unwrap();
        // No suppression annotation in fixture — expect 1 hit.
        write_fixture(&dir, "crates/x/src/e.rs", "let lib = \"libfoo.so\";");
        let hits = scan(
            dir.path(),
            &dynlib_ext_rule(),
            &crate::built_in_walk_prune_names(),
        )
        .unwrap();
        assert_eq!(hits.len(), 1);
    }
}
