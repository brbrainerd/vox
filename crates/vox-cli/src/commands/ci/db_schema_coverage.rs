//! `vox ci db-schema-coverage` — verifies every CREATE TABLE statement in
//! the workspace lives in a crate listed under `tiers.a_relational.{owners,
//! temporary_exceptions}` of `contracts/db/data-storage-policy.v1.yaml`.
//!
//! This is the mechanical version of "no parallel persistence layers": when
//! a crate adds a new table, its crate name must appear in `owners` (or
//! `temporary_exceptions`), forcing the policy update to land in the same PR.

use anyhow::{Context, Result, anyhow};
use regex::Regex;
use serde::Deserialize;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
struct Policy {
    tiers: Tiers,
}

#[derive(Debug, Deserialize)]
struct Tiers {
    a_relational: TierA,
}

#[derive(Debug, Deserialize)]
struct TierA {
    #[serde(default)]
    owners: Vec<String>,
    #[serde(default)]
    temporary_exceptions: Vec<String>,
}

#[derive(Debug)]
struct Hit {
    crate_name: String,
    file: PathBuf,
    line: usize,
    table: String,
}

/// SQL keywords that the regex may capture spuriously (e.g. when the table
/// name is a `{placeholder}` in a Rust format string).
const SQL_KEYWORD_FALSE_POSITIVES: &[&str] = &["IF", "NOT", "EXISTS", "TABLE"];

pub fn run(root: &Path) -> Result<()> {
    let policy_path = root.join("contracts/db/data-storage-policy.v1.yaml");
    let yaml = fs::read_to_string(&policy_path)
        .with_context(|| format!("read {}", policy_path.display()))?;
    let policy: Policy = serde_yaml::from_str(&yaml).with_context(|| {
        format!(
            "parse data-storage policy at {} (expected `tiers.a_relational.{{owners, temporary_exceptions}}`)",
            policy_path.display()
        )
    })?;

    let mut allowed: BTreeSet<String> = policy.tiers.a_relational.owners.iter().cloned().collect();
    allowed.extend(
        policy
            .tiers
            .a_relational
            .temporary_exceptions
            .iter()
            .cloned(),
    );

    // Match `CREATE TABLE [IF NOT EXISTS] <name>` where <name> is a real
    // identifier (not a `{...}` placeholder).
    let create_re =
        Regex::new(r"(?i)CREATE\s+TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?([a-zA-Z_][a-zA-Z0-9_]*)")
            .expect("create_table regex");

    let crates_dir = root.join("crates");
    let mut hits: Vec<Hit> = Vec::new();
    walk(&crates_dir, &mut |path| {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !(name.ends_with(".rs") || name.ends_with(".sql")) {
            return Ok(());
        }
        let path_s = path.to_string_lossy();
        // Skip generated and target paths.
        if path_s.contains("/target/") || path_s.contains("\\target\\") {
            return Ok(());
        }
        let body = match fs::read_to_string(path) {
            Ok(b) => b,
            Err(_) => return Ok(()),
        };
        if !body.contains("CREATE") {
            return Ok(());
        }
        let crate_name = crate_of(path, &crates_dir).unwrap_or_default();
        // For `.rs` files: lines after `#[cfg(test)]` are test code, not
        // production schema. Find the first occurrence and skip everything
        // beyond it (cheap heuristic — production CREATE TABLE strings live
        // in non-test modules at the top of the file or in dedicated schema
        // files).
        let test_cutoff: Option<usize> = if name.ends_with(".rs") {
            body.lines()
                .position(|l| l.trim_start().starts_with("#[cfg(test)]"))
        } else {
            None
        };
        for (line_idx, line) in body.lines().enumerate() {
            if let Some(cut) = test_cutoff {
                if line_idx >= cut {
                    break;
                }
            }
            // Skip Rust comment lines (`//` or `///`). These often mention
            // CREATE TABLE in prose without being real DDL.
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") {
                continue;
            }
            // Pre-filter: skip lines where the table name is a `{...}` placeholder.
            // Such lines look like `CREATE TABLE {tbl}` and trigger the keyword false positive.
            if line.contains("CREATE") && line.contains('{') {
                // Heuristic: only skip when `{` appears after `TABLE` keyword.
                if let Some(table_pos) = line.find("TABLE") {
                    if line[table_pos..].contains('{') {
                        // brace appears after TABLE → likely a placeholder.
                        continue;
                    }
                }
            }
            if let Some(c) = create_re.captures(line) {
                let captured = c.get(1).unwrap().as_str();
                // Drop SQL keyword false positives (defense in depth).
                if SQL_KEYWORD_FALSE_POSITIVES
                    .iter()
                    .any(|kw| kw.eq_ignore_ascii_case(captured))
                {
                    continue;
                }
                hits.push(Hit {
                    crate_name: crate_name.clone(),
                    file: path.to_path_buf(),
                    line: line_idx + 1,
                    table: captured.to_string(),
                });
            }
        }
        Ok(())
    })?;

    let mut violations: Vec<String> = Vec::new();
    for h in &hits {
        if allowed.contains(&h.crate_name) {
            continue;
        }
        violations.push(format!(
            "  {}:{}  table `{}` in crate `{}` (not in tiers.a_relational.{{owners, temporary_exceptions}})",
            h.file.strip_prefix(root).unwrap_or(&h.file).display(),
            h.line,
            h.table,
            h.crate_name,
        ));
    }

    if !violations.is_empty() {
        return Err(anyhow!(
            "db-schema-coverage: {} CREATE TABLE statement(s) in non-owner crates:\n{}\n\nFix: add the crate to `tiers.a_relational.{{owners, temporary_exceptions}}` in contracts/db/data-storage-policy.v1.yaml (and to docs/agents/turso-import-allowlist.txt), or move the schema into vox-db's SCHEMA_FRAGMENTS.",
            violations.len(),
            violations.join("\n"),
        ));
    }

    println!(
        "db-schema-coverage OK ({} CREATE TABLE statements, all in owner crates)",
        hits.len()
    );
    Ok(())
}

fn walk(dir: &Path, f: &mut dyn FnMut(&Path) -> Result<()>) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            // Skip irrelevant subdirs. Crucially, skip `tests/` and `fixtures/`
            // — test fixtures contain CREATE TABLE statements but are not
            // production schema.
            if matches!(
                name,
                "target" | ".git" | "node_modules" | "dist" | "tests" | "fixtures"
            ) {
                continue;
            }
            walk(&path, f)?;
        } else {
            f(&path)?;
        }
    }
    Ok(())
}

fn crate_of(file: &Path, crates_dir: &Path) -> Option<String> {
    let rel = file.strip_prefix(crates_dir).ok()?;
    rel.components()
        .next()
        .and_then(|c| c.as_os_str().to_str())
        .map(String::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn write(p: &Path, content: &str) {
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(p, content).unwrap();
    }

    #[test]
    fn flags_table_in_unowned_crate() {
        let td = tempdir().unwrap();
        let root = td.path();
        write(
            &root.join("contracts/db/data-storage-policy.v1.yaml"),
            "tiers:\n  a_relational:\n    owners: [vox-db]\n",
        );
        write(
            &root.join("crates/vox-rogue/src/lib.rs"),
            "const S: &str = \"CREATE TABLE rogue_table (id INT)\";",
        );
        let err = run(root).unwrap_err().to_string();
        assert!(
            err.contains("rogue_table"),
            "error must name the rogue table; got: {err}"
        );
        assert!(
            err.contains("vox-rogue"),
            "error must name the rogue crate; got: {err}"
        );
    }

    #[test]
    fn passes_when_table_in_owner_crate() {
        let td = tempdir().unwrap();
        let root = td.path();
        write(
            &root.join("contracts/db/data-storage-policy.v1.yaml"),
            "tiers:\n  a_relational:\n    owners: [vox-db]\n",
        );
        write(
            &root.join("crates/vox-db/src/x.rs"),
            "const S: &str = \"CREATE TABLE memories (id INT)\";",
        );
        run(root).expect("table in owner crate should pass");
    }

    #[test]
    fn passes_when_table_in_temporary_exception() {
        let td = tempdir().unwrap();
        let root = td.path();
        write(
            &root.join("contracts/db/data-storage-policy.v1.yaml"),
            "tiers:\n  a_relational:\n    owners: [vox-db]\n    temporary_exceptions: [vox-package]\n",
        );
        write(
            &root.join("crates/vox-package/src/x.rs"),
            "const S: &str = \"CREATE TABLE local_store (id INT)\";",
        );
        run(root).expect("table in temporary-exception crate should pass");
    }

    #[test]
    fn skips_format_placeholder_lines() {
        // Lines like `CREATE TABLE {tbl}` are Rust format strings, not real DDL.
        let td = tempdir().unwrap();
        let root = td.path();
        write(
            &root.join("contracts/db/data-storage-policy.v1.yaml"),
            "tiers:\n  a_relational:\n    owners: [vox-db]\n",
        );
        write(
            &root.join("crates/vox-codegen/src/x.rs"),
            "let s = format!(\"CREATE TABLE IF NOT EXISTS {tbl} (id INT)\");",
        );
        // Should NOT flag vox-codegen because the placeholder line is filtered.
        run(root).expect("placeholder lines must be skipped");
    }

    #[test]
    fn skips_tests_directory() {
        let td = tempdir().unwrap();
        let root = td.path();
        write(
            &root.join("contracts/db/data-storage-policy.v1.yaml"),
            "tiers:\n  a_relational:\n    owners: [vox-db]\n",
        );
        write(
            &root.join("crates/vox-anything/tests/fixture.sql"),
            "CREATE TABLE test_only (id INT);",
        );
        // Should pass because `tests/` is excluded from the walk.
        run(root).expect("tests/ directory must be skipped");
    }
}
