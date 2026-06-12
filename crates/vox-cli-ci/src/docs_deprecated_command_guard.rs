//! Fail docs / agent registries that embed retired `cargo test` shapes (compiler monolith drift).

use anyhow::{Context, Result, anyhow};
use std::fs;
use std::path::Path;

fn scan_file(root: &Path, path: &Path, failures: &mut Vec<String>) -> Result<()> {
    let rel = path.strip_prefix(root).unwrap_or(path);
    let body = fs::read_to_string(path).with_context(|| format!("read {}", rel.display()))?;
    for (line_idx, line) in body.lines().enumerate() {
        let line_no = line_idx + 1;
        if line.contains("cargo test -p vox-parser") {
            failures.push(format!(
                "{}:{}: retired crate in command example; use `-p vox-compiler --test golden_examples_strict_parse`",
                rel.display(),
                line_no
            ));
        }
        // Match only the BARE retired test name `parity_test`, not its descendants
        // like `projection_parity_test`, `voxdb_schema_hir_parity_test`, or
        // `rust_ecosystem_support_parity_test` — all of which are real,
        // currently-shipping tests in `crates/vox-compiler/tests/`.
        if line.contains("vox-compiler") && contains_bare_parity_test(line) {
            failures.push(format!(
                "{}:{}: wrong integration test name for vox-compiler strict-parse; use `--test golden_examples_strict_parse`",
                rel.display(),
                line_no
            ));
        }
    }
    Ok(())
}

/// Returns true iff `line` contains `parity_test` *not* preceded by an identifier
/// character (i.e. as a standalone token). Cheap left-boundary check avoids
/// pulling in `regex` for one detector.
fn contains_bare_parity_test(line: &str) -> bool {
    const NEEDLE: &str = "parity_test";
    let bytes = line.as_bytes();
    let mut search_from = 0usize;
    while let Some(rel_off) = line[search_from..].find(NEEDLE) {
        let start = search_from + rel_off;
        let prev_is_ident = start > 0 && {
            let b = bytes[start - 1];
            b.is_ascii_alphanumeric() || b == b'_'
        };
        if !prev_is_ident {
            return true;
        }
        search_from = start + NEEDLE.len();
    }
    false
}

/// Scan markdown under `docs/` plus selected agent/script registries for stale cargo-test snippets.
pub fn run(root: &Path) -> Result<()> {
    let mut failures = Vec::new();

    let docs = root.join("docs");
    if docs.is_dir() {
        let mut stack = vec![docs];
        while let Some(dir) = stack.pop() {
            for entry in
                fs::read_dir(&dir).with_context(|| format!("read_dir {}", dir.display()))?
            {
                let entry = entry?;
                let p = entry.path();
                if p.is_dir() {
                    // Skip docs/superpowers/ — internal team plans, specs, and handoff
                    // notes that describe guard logic and would otherwise trigger the
                    // very rules they document.
                    let rel = p.strip_prefix(root).unwrap_or(&p);
                    let rel_str = rel.to_string_lossy().replace('\\', "/");
                    if rel_str == "docs/superpowers" || rel_str.starts_with("docs/superpowers/") {
                        continue;
                    }
                    stack.push(p);
                } else if p.extension().and_then(|e| e.to_str()) == Some("md") {
                    let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if name == "SUMMARY.md" || name.contains("-ARCHIVED.md") {
                        continue;
                    }
                    scan_file(root, &p, &mut failures)?;
                }
            }
        }
    }

    let script_registry = root.join("docs/agents/script-registry.json");
    if script_registry.is_file() {
        scan_file(root, &script_registry, &mut failures)?;
    }

    let scripts_readme = root.join("scripts/README.md");
    if scripts_readme.is_file() {
        scan_file(root, &scripts_readme, &mut failures)?;
    }

    if !failures.is_empty() {
        for f in &failures {
            eprintln!("{}", f);
        }
        return Err(anyhow!(
            "docs_deprecated_command_guard: {} stale cargo-test reference(s)",
            failures.len()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn contains_bare_parity_test_flags_standalone_token() {
        assert!(contains_bare_parity_test(
            "cargo test -p vox-compiler --test parity_test"
        ));
        assert!(!contains_bare_parity_test(
            "cargo test -p vox-compiler --test projection_parity_test"
        ));
        assert!(!contains_bare_parity_test(
            "cargo test -p vox-compiler --test voxdb_schema_hir_parity_test"
        ));
    }

    #[test]
    fn detects_vox_parser_cargo_invocation() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let md = tmp.path().join("docs").join("x.md");
        fs::create_dir_all(md.parent().unwrap()).expect("mkdir");
        let mut f = fs::File::create(&md).expect("create");
        writeln!(f, "run `cargo test -p vox-parser --lib`").expect("write");
        let mut failures = Vec::new();
        scan_file(tmp.path(), &md, &mut failures).expect("scan");
        assert!(!failures.is_empty());
    }

    #[test]
    fn detects_parity_test_with_vox_compiler() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let md = tmp.path().join("docs").join("y.md");
        fs::create_dir_all(md.parent().unwrap()).expect("mkdir");
        let mut f = fs::File::create(&md).expect("create");
        writeln!(f, "`cargo test -p vox-compiler --test parity_test`").expect("write");
        let mut failures = Vec::new();
        scan_file(tmp.path(), &md, &mut failures).expect("scan");
        assert!(!failures.is_empty());
    }

    #[test]
    fn does_not_flag_projection_parity_test() {
        // `projection_parity_test` is a real, current test in
        // `crates/vox-compiler/tests/`; the guard must not flag it.
        let tmp = tempfile::tempdir().expect("tempdir");
        let md = tmp.path().join("docs").join("z.md");
        fs::create_dir_all(md.parent().unwrap()).expect("mkdir");
        let mut f = fs::File::create(&md).expect("create");
        writeln!(
            f,
            "`cargo test -p vox-compiler --test projection_parity_test`"
        )
        .expect("write");
        let mut failures = Vec::new();
        scan_file(tmp.path(), &md, &mut failures).expect("scan");
        assert!(
            failures.is_empty(),
            "projection_parity_test must not be flagged: {failures:?}"
        );
    }

    #[test]
    fn does_not_flag_voxdb_schema_hir_parity_test() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let md = tmp.path().join("docs").join("z2.md");
        fs::create_dir_all(md.parent().unwrap()).expect("mkdir");
        let mut f = fs::File::create(&md).expect("create");
        writeln!(
            f,
            "`cargo test -p vox-compiler --test voxdb_schema_hir_parity_test`"
        )
        .expect("write");
        let mut failures = Vec::new();
        scan_file(tmp.path(), &md, &mut failures).expect("scan");
        assert!(
            failures.is_empty(),
            "voxdb_schema_hir_parity_test must not be flagged: {failures:?}"
        );
    }
}
