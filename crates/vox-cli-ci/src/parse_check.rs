//! `vox ci json-parse-check` / `vox ci yaml-parse-check`
//!
//! Validates that every file matching the given glob is parseable JSON or YAML
//! respectively.  Replaces the `python3 -c "…"` / `python3 - <<'PY' …` blocks
//! that appeared in vox-mental-tracker.yml.

use anyhow::{Result, anyhow};
use std::path::PathBuf;

pub fn run_json(globs: &[String]) -> Result<()> {
    let paths = expand_globs(globs)?;
    if paths.is_empty() {
        println!("json-parse-check: no files matched");
        return Ok(());
    }
    let mut failed = false;
    for path in &paths {
        let contents = std::fs::read_to_string(path)
            .map_err(|e| anyhow!("json-parse-check: cannot read {}: {e}", path.display()))?;
        match serde_json::from_str::<serde_json::Value>(&contents) {
            Ok(_) => println!("OK {}", path.display()),
            Err(e) => {
                eprintln!("FAIL {}: {e}", path.display());
                failed = true;
            }
        }
    }
    if failed {
        Err(anyhow!("json-parse-check: one or more files failed"))
    } else {
        Ok(())
    }
}

pub fn run_yaml(globs: &[String]) -> Result<()> {
    let paths = expand_globs(globs)?;
    if paths.is_empty() {
        println!("yaml-parse-check: no files matched");
        return Ok(());
    }
    let mut failed = false;
    for path in &paths {
        let contents = std::fs::read_to_string(path)
            .map_err(|e| anyhow!("yaml-parse-check: cannot read {}: {e}", path.display()))?;
        match serde_yaml::from_str::<serde_yaml::Value>(&contents) {
            Ok(_) => println!("OK {}", path.display()),
            Err(e) => {
                eprintln!("FAIL {}: {e}", path.display());
                failed = true;
            }
        }
    }
    if failed {
        Err(anyhow!("yaml-parse-check: one or more files failed"))
    } else {
        Ok(())
    }
}

/// Heuristic for "this file is a script-style entry point" — mirrors
/// `vox-cli`'s `commands::check::is_script_like` so `vox ci vox-parse-check`
/// exercises the same parse path (`parse_script` vs strict `parse`) that
/// `vox check` uses on the same file.
///
/// `// vox:defactored-from vox-cli 2026-08-08` — under the ~50-line defactor
/// threshold (see `AGENTS.md` §Dependency Discipline); vox-cli-ci does not
/// take a crate edge on vox-cli for this one heuristic.
fn is_script_like(source: &str) -> bool {
    let app_markers = [
        "@page",
        "@query",
        "@mutation",
        "@server",
        "@component",
        "@table",
        "@workflow",
        "@form",
        "@push",
    ];
    let has_at_marker = app_markers.iter().any(|m| source.contains(m));
    let decl_keywords = [
        "table ",
        "query ",
        "mutation ",
        "server ",
        "component ",
        "routes ",
        "routes{",
    ];
    let has_decl_keyword = source.lines().any(|line| {
        decl_keywords
            .iter()
            .any(|k| line.trim_start().starts_with(k))
    });
    !(has_at_marker || has_decl_keyword)
}

/// Verify every `.vox` file matched by the given glob(s) lexes and parses
/// cleanly (no type-check — this is a syntax-only regression guard). Exits
/// non-zero if any file produces an Error-severity parse diagnostic.
///
/// This exists because corpus-wide lexer/parser changes (e.g. the
/// `Token::Unknown` catch-all landed in commit `c3446892847e`) had no test
/// walking `scripts/**/*.vox` / `apps/**/*.vox` — four files broke silently
/// until a manual audit found them. Wire this into pre-push / CI as
/// `vox ci vox-parse-check "scripts/**/*.vox" "apps/**/*.vox"`.
///
/// Running that sweep against the full corpus on 2026-08-08 surfaced 8
/// further pre-existing failures, all fixed the same day:
/// `scripts/fix-doc-categories.vox`, `scripts/profile-crate-count.vox`,
/// `scripts/start-marquee.vox`, `scripts/sync-cursor-skills.vox`,
/// `scripts/sync_golden_vox.vox`, `scripts/test_for.vox` -- a real gap in
/// the tolerant-`;` Return-statement coverage, plus two mechanical
/// migrations off retired `!`/decorator syntax. The remaining two,
/// `apps/marquee/chat/src/main.vox` and `apps/marquee/todo-auth/src/main.vox`,
/// needed a parser feature, not a corpus fix: both compose `@auth(...)` with
/// a bare `query`/`mutation`/`server` declaration, which AGENTS.md's Grammar
/// Unification section documents as supported (`@auth(scheme: bearer)
/// table Task { … }`) but the parser didn't actually implement for
/// `query`/`mutation`/`server` specifically -- see
/// `crates/vox-compiler/src/parser/descent/decl/head_fn.rs`'s
/// `parse_fn_decl_detect_kind` / the kind-keyword arm inside
/// `parse_fn_decl_inner`'s decorator-collection loop for the fix.
pub fn run_vox(globs: &[String]) -> Result<()> {
    let paths = expand_globs(globs)?;
    if paths.is_empty() {
        println!("vox-parse-check: no files matched");
        return Ok(());
    }
    let mut failed = false;
    for path in &paths {
        let source = std::fs::read_to_string(path)
            .map_err(|e| anyhow!("vox-parse-check: cannot read {}: {e}", path.display()))?;
        let tokens = vox_compiler::lexer::lex(&source);
        let result = if is_script_like(&source) {
            vox_compiler::parser::parse_script(tokens)
        } else {
            vox_compiler::parser::parse(tokens)
        };
        match result {
            Ok(_) => println!("OK {}", path.display()),
            Err(errors) => {
                eprintln!("FAIL {}:", path.display());
                for e in &errors {
                    eprintln!("  {:?} {}", e.severity, e.message);
                }
                failed = true;
            }
        }
    }
    if failed {
        Err(anyhow!("vox-parse-check: one or more files failed to parse"))
    } else {
        Ok(())
    }
}

fn expand_globs(patterns: &[String]) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for pattern in patterns {
        let matches: Vec<_> = glob::glob(pattern)
            .map_err(|e| anyhow!("invalid glob pattern {pattern:?}: {e}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| anyhow!("glob error for {pattern:?}: {e}"))?;
        paths.extend(matches);
    }
    paths.sort();
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use tempfile::TempDir;

    fn write_fixture(dir: &Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, body).expect("write fixture");
        path
    }

    #[test]
    fn json_parse_check_accepts_valid_json() {
        let tmp = TempDir::new().expect("tmpdir");
        write_fixture(tmp.path(), "ok.json", r#"{"a": 1}"#);
        let glob = format!("{}/ok.json", tmp.path().display());
        run_json(&[glob]).expect("valid json");
    }

    #[test]
    fn json_parse_check_rejects_invalid_json() {
        let tmp = TempDir::new().expect("tmpdir");
        write_fixture(tmp.path(), "bad.json", "{not json");
        let glob = format!("{}/bad.json", tmp.path().display());
        assert!(run_json(&[glob]).is_err());
    }

    #[test]
    fn yaml_parse_check_accepts_valid_yaml() {
        let tmp = TempDir::new().expect("tmpdir");
        write_fixture(tmp.path(), "ok.yaml", "key: value\n");
        let glob = format!("{}/ok.yaml", tmp.path().display());
        run_yaml(&[glob]).expect("valid yaml");
    }

    #[test]
    fn vox_parse_check_accepts_valid_script() {
        let tmp = TempDir::new().expect("tmpdir");
        write_fixture(tmp.path(), "ok.vox", "print(\"hi\")\n");
        let glob = format!("{}/ok.vox", tmp.path().display());
        run_vox(&[glob]).expect("valid vox script");
    }

    #[test]
    fn vox_parse_check_tolerates_bare_return_semicolon() {
        // Regression fixture for the exact class of bug this gate was built
        // to catch: `Token::Unknown` (commit c3446892847e) made a bare
        // `return;` a hard parse error (`parse_stmt`'s Return arm only
        // treated Newline/RBrace/Eof as "no value", not the tolerated `;`),
        // and no test walked the `.vox` script corpus to catch it. That gap
        // in the Return arm is now closed (it also matches
        // `Token::Unknown(';')`, letting `skip_tolerated_semicolon` warn on
        // and consume the leftover `;` the same way any other statement
        // boundary does) — this now must parse, with a warning, not fail.
        let tmp = TempDir::new().expect("tmpdir");
        write_fixture(tmp.path(), "ok_return.vox", "fn main() {\n    return;\n}\n");
        let glob = format!("{}/ok_return.vox", tmp.path().display());
        run_vox(&[glob]).expect("tolerated bare 'return;' must not fail the gate");
    }

    #[test]
    fn vox_parse_check_tolerates_statement_boundary_semicolons() {
        // Trailing `;` at a real statement boundary is a Warning, not an
        // Error (parser/descent/mod.rs `skip_tolerated_semicolon`) — must
        // not fail the gate.
        let tmp = TempDir::new().expect("tmpdir");
        write_fixture(
            tmp.path(),
            "warn.vox",
            "fn main() {\n    let x = 1;\n    print(x);\n}\n",
        );
        let glob = format!("{}/warn.vox", tmp.path().display());
        run_vox(&[glob]).expect("tolerated trailing semicolons must not fail the gate");
    }

    #[test]
    fn vox_parse_check_no_match_is_ok() {
        let tmp = TempDir::new().expect("tmpdir");
        let glob = format!("{}/nonexistent_*.vox", tmp.path().display());
        run_vox(&[glob]).expect("no matches is not a failure");
    }

    #[test]
    fn yaml_parse_check_rejects_invalid_yaml() {
        let tmp = TempDir::new().expect("tmpdir");
        write_fixture(tmp.path(), "bad.yaml", "key: [unclosed\n");
        let glob = format!("{}/bad.yaml", tmp.path().display());
        assert!(run_yaml(&[glob]).is_err());
    }

    #[test]
    fn expand_globs_invalid_pattern_errors() {
        let err = expand_globs(&["***invalid***".to_string()]);
        assert!(err.is_err());
    }
}

#[cfg(test)]
mod semcov_wave5_tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn expand_globs_empty_patterns_returns_empty_vec() {
        let result = expand_globs(&[]).expect("no patterns is OK");
        assert!(result.is_empty());
    }

    #[test]
    fn expand_globs_no_match_returns_empty_vec() {
        let tmp = TempDir::new().expect("tmpdir");
        let pattern = format!("{}/nonexistent_*.xyz", tmp.path().display());
        let result = expand_globs(&[pattern]).expect("valid pattern with no match");
        assert!(result.is_empty());
    }

    #[test]
    fn expand_globs_returns_sorted_paths() {
        let tmp = TempDir::new().expect("tmpdir");
        fs::write(tmp.path().join("b.json"), "{}").expect("write b");
        fs::write(tmp.path().join("a.json"), "{}").expect("write a");
        let pattern = format!("{}/?.json", tmp.path().display());
        let paths = expand_globs(&[pattern]).expect("valid glob");
        assert_eq!(paths.len(), 2);
        // expand_globs sorts before returning
        assert!(paths[0] < paths[1], "paths must be sorted");
        assert!(paths[0].ends_with("a.json"));
        assert!(paths[1].ends_with("b.json"));
    }

    #[test]
    fn expand_globs_invalid_pattern_returns_error() {
        let err = expand_globs(&["[invalid".to_string()]);
        assert!(err.is_err(), "invalid glob must be an error");
    }
}
