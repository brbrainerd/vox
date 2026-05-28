//! `vox ci retirement-audit` — enforce removal of `vox-deprecated-since` markers whose `retire-by` version has been reached.

use anyhow::{Context, Result, anyhow};
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

/// Semver subset: `major.minor.patch` (pre-release suffix after `+`/`-` ignored for comparison core).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct SemverTriple {
    major: u64,
    minor: u64,
    patch: u64,
}

impl FromStr for SemverTriple {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let s = s.trim();
        let core = s.split(['-', '+']).next().unwrap_or(s);
        let mut parts = core.split('.');
        let major: u64 = parts
            .next()
            .context("semver major")?
            .parse()
            .context("semver major parse")?;
        let minor: u64 = parts.next().unwrap_or("0").parse().unwrap_or(0);
        let patch: u64 = parts.next().unwrap_or("0").parse().unwrap_or(0);
        Ok(SemverTriple {
            major,
            minor,
            patch,
        })
    }
}

impl std::fmt::Display for SemverTriple {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

fn workspace_version(repo_root: &Path) -> Result<SemverTriple> {
    let cargo_toml =
        fs::read_to_string(repo_root.join("Cargo.toml")).context("read root Cargo.toml")?;
    let mut in_workspace_package = false;
    let mut ver_line: Option<&str> = None;
    for line in cargo_toml.lines() {
        let t = line.trim();
        if t.starts_with('[') && t.ends_with(']') {
            in_workspace_package = t == "[workspace.package]";
            continue;
        }
        if in_workspace_package && t.starts_with("version = \"") {
            ver_line = Some(t);
            break;
        }
    }
    let ver_line = ver_line.context("find [workspace.package] version = in Cargo.toml")?;
    let q1 = ver_line.find('"').context("version quote")?;
    let q2 = ver_line[q1 + 1..]
        .find('"')
        .map(|i| q1 + 1 + i)
        .context("version end quote")?;
    let v = &ver_line[q1 + 1..q2];
    SemverTriple::from_str(v).with_context(|| format!("parse workspace version `{v}`"))
}

fn should_skip_dir(name: &str) -> bool {
    matches!(
        name,
        ".git" | "target" | "node_modules" | "dist" | "web-dist" | ".venv"
    )
}

fn collect_text_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("read_dir {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            if should_skip_dir(&name) {
                continue;
            }
            if path.ends_with("docs/src/archive") {
                continue;
            }
            collect_text_files(&path, out)?;
            continue;
        }
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        if matches!(
            ext,
            "rs" | "md" | "vox" | "ts" | "tsx" | "yaml" | "yml" | "toml" | "json"
        ) {
            out.push(path);
        }
    }
    Ok(())
}

/// Run retirement audit from repository root.
///
/// Two checks:
///
/// 1. **Overdue deprecation markers** — scans all source files for
///    `vox-deprecated-since = "X" retire-by = "Y"` annotations and fails if
///    the `retire-by` version is ≤ the workspace version.
///
/// 2. **CR-L6 contract presence** — verifies that
///    `contracts/retirement/retired-surfaces.v1.yaml` exists and is a
///    non-empty YAML document.  The deep parity check (cross-referencing rule
///    IDs against registered detectors) runs via
///    `cargo test -p vox-code-audit -- retirement_parity` and is covered in CI
///    through the workspace test suite.  This surface-level check catches the
///    case where the contract file is accidentally deleted.
pub fn run(repo_root: &Path) -> Result<()> {
    let workspace_ver = workspace_version(repo_root)?;
    let mut files = Vec::new();
    collect_text_files(repo_root, &mut files)?;

    let re = regex::Regex::new(
        r#"vox-deprecated-since\s*=\s*"([^"]+)"\s+retire-by\s*=\s*"([^"]+)"#,
    )
    .expect("retirement marker regex");

    let mut count = 0usize;
    let mut overdue = Vec::new();
    for path in files {
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        for caps in re.captures_iter(&text) {
            count += 1;
            let retire_by = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            let Ok(rb) = SemverTriple::from_str(retire_by) else {
                eprintln!(
                    "retirement-audit: skip non-semver retire-by `{retire_by}` in {}",
                    path.display()
                );
                continue;
            };
            if workspace_ver >= rb {
                overdue.push(format!(
                    "{} — retire-by {retire_by} (workspace {workspace_ver})",
                    path.display()
                ));
            }
        }
    }

    println!("retirement-audit: found {count} `vox-deprecated-since` marker(s); workspace version = {workspace_ver}");
    if !overdue.is_empty() {
        for line in &overdue {
            eprintln!("OVERDUE: {line}");
        }
        return Err(anyhow!(
            "retirement-audit: {} overdue marker(s) — remove or bump retire-by",
            overdue.len()
        ));
    }

    // CR-L6 contract presence check (P1.5).
    let contract_path =
        repo_root.join("contracts/retirement/retired-surfaces.v1.yaml");
    if !contract_path.exists() {
        return Err(anyhow!(
            "retirement-audit: CR-L6 contract missing at contracts/retirement/retired-surfaces.v1.yaml — \
             this file is required for retirement-guard parity (CR-L6). \
             Create it or restore from git history."
        ));
    }
    let contract_yaml = fs::read_to_string(&contract_path)
        .context("read contracts/retirement/retired-surfaces.v1.yaml")?;
    // Structural check: the file must contain at least one `- id:` entry.
    let surface_count = contract_yaml.lines().filter(|l| l.trim_start().starts_with("- id:")).count();
    if surface_count == 0 {
        return Err(anyhow!(
            "retirement-audit: contracts/retirement/retired-surfaces.v1.yaml contains \
             no surface entries — the file may be empty or malformed."
        ));
    }
    println!(
        "retirement-audit: CR-L6 contract OK ({surface_count} surface(s) declared; \
         deep parity verified by `cargo test -p vox-code-audit -- retirement_parity`)"
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn workspace_version_reads_root_manifest() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("repo root")
            .to_path_buf();
        let v = workspace_version(&root).expect("version");
        assert!(v.major > 0 || v.minor > 0 || v.patch > 0);
    }

    #[test]
    fn semver_ordering() {
        let a = SemverTriple::from_str("0.5.0").unwrap();
        let b = SemverTriple::from_str("0.7.0").unwrap();
        assert!(a < b);
    }
}
