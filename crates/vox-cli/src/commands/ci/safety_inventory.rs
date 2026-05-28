//! Safety / suppression-debt baseline for `vox ci safety-inventory`.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use super::test_inventory::scan_ignored_test_governance_findings_with_histogram;

/// CLI options for [`run`].
#[derive(Debug, Clone, Default)]
pub struct SafetyInventoryOpts {
    pub json_stdout: bool,
    pub output: Option<PathBuf>,
    pub check: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct SafetyInventoryV1 {
    pub schema_version: u32,
    pub rust: RustSafetyBlock,
    pub typescript: TsSafetyBlock,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct RustSafetyBlock {
    /// Occurrences of `unsafe {` under `crates/**/*.rs` (excludes `target/`).
    pub unsafe_brace_block_opens: u64,
    /// Lines containing `unsafe` and (`set_var` or `remove_var`) under `crates/**/*.rs`.
    pub lines_unsafe_with_env_mutation: u64,
    pub ignored_test_functions_total: u64,
    pub ignored_test_governance_violation_count: u64,
    /// Top 25 files by ignored test count (descending).
    pub ignored_tests_top_files: Vec<FileCountRow>,
    /// Crate-root `src/lib.rs` or `src/main.rs` files whose first 120 lines contain `#![allow`.
    pub crate_root_inner_allow_lines: u64,
    pub crate_root_inner_allow_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileCountRow {
    pub path: String,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct TsSafetyBlock {
    pub eslint_disable_directive_lines: u64,
    pub as_any_occurrences: u64,
    pub files_with_eslint_disable: Vec<String>,
}

fn repo_rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn scan_rust_unsafe_metrics(root: &Path) -> Result<(u64, u64)> {
    let crates = root.join("crates");
    let mut brace_opens = 0u64;
    let mut env_lines = 0u64;
    if !crates.is_dir() {
        return Ok((0, 0));
    }
    for ent in WalkDir::new(&crates)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !name.starts_with('.') && name != "target"
        })
        .filter_map(Result::ok)
    {
        if !ent.file_type().is_file() {
            continue;
        }
        let path = ent.path();
        if path.extension().and_then(|s| s.to_str()) != Some("rs") {
            continue;
        }
        let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        brace_opens += text.matches("unsafe {").count() as u64;
        for raw in text.lines() {
            let line = raw.trim_start();
            if !line.contains("unsafe") {
                continue;
            }
            if line.contains("set_var") || line.contains("remove_var") {
                env_lines += 1;
            }
        }
    }
    Ok((brace_opens, env_lines))
}

fn scan_crate_root_allows(root: &Path) -> Result<(u64, Vec<String>)> {
    let crates = root.join("crates");
    let mut total_lines = 0u64;
    let mut files = Vec::<String>::new();
    if !crates.is_dir() {
        return Ok((0, files));
    }
    for name in fs::read_dir(&crates).with_context(|| format!("read {}", crates.display()))? {
        let name = name?;
        if !name.file_type()?.is_dir() {
            continue;
        }
        let crate_dir = name.path();
        for stem in ["lib", "main"] {
            let p = crate_dir.join("src").join(format!("{stem}.rs"));
            if !p.is_file() {
                continue;
            }
            let text = fs::read_to_string(&p).with_context(|| format!("read {}", p.display()))?;
            let head: String = text.lines().take(120).collect::<Vec<_>>().join("\n");
            if head.contains("#![allow") {
                let rel = repo_rel(root, &p);
                let line_count = head.matches("#![allow").count() as u64;
                total_lines += line_count;
                files.push(rel);
            }
        }
    }
    files.sort();
    Ok((total_lines, files))
}

fn walk_ts_files(apps_dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    if !apps_dir.is_dir() {
        return Ok(());
    }
    for ent in WalkDir::new(apps_dir)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !name.starts_with('.') && name != "node_modules" && name != "dist"
        })
        .filter_map(Result::ok)
    {
        if !ent.file_type().is_file() {
            continue;
        }
        let p = ent.path();
        let ext = p.extension().and_then(|s| s.to_str());
        if !matches!(
            ext,
            Some("ts" | "tsx" | "mts" | "cts" | "js" | "jsx" | "mjs" | "cjs")
        ) {
            continue;
        }
        out.push(p.to_path_buf());
    }
    Ok(())
}

fn scan_typescript_suppressions(root: &Path) -> Result<TsSafetyBlock> {
    let mut paths = Vec::<PathBuf>::new();
    walk_ts_files(&root.join("apps"), &mut paths)?;

    let mut eslint_lines = 0u64;
    let mut as_any = 0u64;
    let mut eslint_files = Vec::<String>::new();

    for p in &paths {
        let text = fs::read_to_string(p).with_context(|| format!("read {}", p.display()))?;
        let mut f_es = false;
        for raw in text.lines() {
            let t = raw.trim_start();
            if t.contains("eslint-disable") {
                eslint_lines += 1;
                f_es = true;
            }
            let c1 = t.matches(" as any").count() as u64;
            let c2 = t.matches("as any)").count() as u64;
            if c1 + c2 > 0 {
                as_any += c1 + c2;
            }
        }
        if f_es {
            eslint_files.push(repo_rel(root, p));
        }
    }
    eslint_files.sort();
    eslint_files.dedup();

    Ok(TsSafetyBlock {
        eslint_disable_directive_lines: eslint_lines,
        as_any_occurrences: as_any,
        files_with_eslint_disable: eslint_files,
    })
}

/// Build the current safety inventory snapshot (no I/O except reading sources).
pub fn build_safety_inventory(root: &Path) -> Result<SafetyInventoryV1> {
    let (unsafe_brace_block_opens, lines_unsafe_with_env_mutation) =
        scan_rust_unsafe_metrics(root)?;
    let (ignored_total, findings, per_file) =
        scan_ignored_test_governance_findings_with_histogram(root)?;
    let gov_v = findings.len() as u64;

    let mut rows: Vec<FileCountRow> = per_file
        .into_iter()
        .map(|(path, count)| FileCountRow { path, count })
        .collect();
    rows.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.path.cmp(&b.path)));
    rows.truncate(25);

    let (crate_root_inner_allow_lines, crate_root_inner_allow_files) =
        scan_crate_root_allows(root)?;
    let typescript = scan_typescript_suppressions(root)?;

    Ok(SafetyInventoryV1 {
        schema_version: 1,
        rust: RustSafetyBlock {
            unsafe_brace_block_opens,
            lines_unsafe_with_env_mutation,
            ignored_test_functions_total: ignored_total,
            ignored_test_governance_violation_count: gov_v,
            ignored_tests_top_files: rows,
            crate_root_inner_allow_lines,
            crate_root_inner_allow_files,
        },
        typescript,
    })
}

pub fn run(root: &Path, opts: SafetyInventoryOpts) -> Result<()> {
    let inv = build_safety_inventory(root)?;
    let json = serde_json::to_string_pretty(&inv)?;

    if let Some(check_path) = &opts.check {
        let raw = fs::read_to_string(check_path)
            .with_context(|| format!("read check baseline {}", check_path.display()))?;
        let expected: SafetyInventoryV1 = serde_json::from_str(&raw)
            .with_context(|| format!("parse baseline {}", check_path.display()))?;
        if expected != inv {
            anyhow::bail!(
                "`vox ci safety-inventory` output differs from {} — run with `--output` to refresh the committed baseline.",
                check_path.display()
            );
        }
        return Ok(());
    }

    if let Some(out) = &opts.output {
        if let Some(parent) = out.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).with_context(|| format!("mkdir {}", parent.display()))?;
        }
        let mut f = fs::File::create(out).with_context(|| format!("write {}", out.display()))?;
        f.write_all(json.as_bytes())?;
        f.flush()?;
    }

    if opts.json_stdout || (opts.output.is_none() && opts.check.is_none()) {
        println!("{json}");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safety_inventory_builds_from_workspace() {
        let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let root = loop {
            if dir.join("crates").is_dir() && dir.join("AGENTS.md").is_file() {
                break dir;
            }
            assert!(
                dir.pop(),
                "repo root not found from {}",
                env!("CARGO_MANIFEST_DIR")
            );
        };
        let inv = build_safety_inventory(&root).unwrap();
        assert_eq!(inv.schema_version, 1);
        assert!(inv.rust.ignored_test_functions_total > 10);
    }
}
