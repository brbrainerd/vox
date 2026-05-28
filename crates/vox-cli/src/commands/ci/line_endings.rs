//! Line-ending policy: LF for tracked source/docs/config; `*.ps1` exempt (CRLF allowed).
//!
//! Policy extensions are kept in sync with the repo root [`.gitattributes`](../../../../../../.gitattributes).

use anyhow::{Context, Result, anyhow};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Extensions enforced as LF-only (aligned with [`.gitattributes`](../../../../../../.gitattributes)).
const EXT_LF: &[&str] = &[
    "rs", "toml", "lock", "md", "yml", "yaml", "json", "ts", "tsx", "js", "jsx", "mjs", "mts",
    "py", "vox", "gd", "css", "html", "svg", "sh",
];

/// Run the line-ending check. Forward-only: only files changed since merge-base unless `all` is set.
pub fn run(
    repo_root: &Path,
    all: bool,
    base_override: Option<String>,
    autofix: bool,
) -> Result<()> {
    let rel_paths = if all {
        list_all_tracked_policy_paths(repo_root)?
    } else {
        list_changed_policy_paths(repo_root, base_override.as_deref())?
    };

    if rel_paths.is_empty() {
        if all {
            println!("line-endings OK (0 policy files tracked)");
        } else {
            eprintln!("line-endings: no changed files in scope; skipping");
        }
        return Ok(());
    }

    let mut checked = 0usize;
    let mut violations = Vec::new();
    for rel in &rel_paths {
        let full = repo_root.join(rel);
        if !full.is_file() {
            continue;
        }
        if is_ps1(&full) {
            continue;
        }
        if !extension_matches_policy(&full) {
            continue;
        }
        let bytes = std::fs::read(&full).with_context(|| format!("read {}", full.display()))?;
        if is_probably_binary(&bytes) {
            continue;
        }
        checked += 1;
        if violates_lf_only(&bytes) {
            if autofix {
                let lf_only: Vec<u8> = bytes.iter().copied().filter(|&b| b != b'\r').collect();
                std::fs::write(&full, lf_only)
                    .with_context(|| format!("autofix {}", full.display()))?;
            }
            violations.push(rel.display().to_string());
        }
    }

    if autofix && !violations.is_empty() {
        let mut add_args = vec!["add"];
        for v in &violations {
            add_args.push(v.as_str());
        }
        git_output(repo_root, &add_args).context("git add after autofix")?;
        println!(
            "line-endings: auto-fixed and staged {} file(s)",
            violations.len()
        );
        violations.clear();
    }

    if violations.is_empty() {
        println!("line-endings OK ({checked} file(s) checked)");
        return Ok(());
    }

    Err(anyhow!(
        "line-endings: CRLF or CR in LF-only paths (use LF; see .gitattributes / .editorconfig):\n{}",
        violations.join("\n")
    ))
}

fn extension_matches_policy(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| EXT_LF.iter().any(|&x| x.eq_ignore_ascii_case(ext)))
}

fn is_ps1(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("ps1"))
}

fn is_probably_binary(bytes: &[u8]) -> bool {
    bytes.contains(&0)
}

fn violates_lf_only(bytes: &[u8]) -> bool {
    bytes.contains(&b'\r')
}

fn git_output(repo_root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let out = // vox-arch-check: allow git-exec
        Command::new("git")
        .current_dir(repo_root)
        .args(args)
        .output()
        .context("spawn git")?;
    if !out.status.success() {
        return Err(anyhow!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(out.stdout)
}

fn list_all_tracked_policy_paths(repo_root: &Path) -> Result<Vec<PathBuf>> {
    let stdout = git_output(repo_root, &["ls-files", "-z"])?;
    let mut out = Vec::new();
    for chunk in stdout.split(|&b| b == 0) {
        if chunk.is_empty() {
            continue;
        }
        let rel = String::from_utf8_lossy(chunk);
        let p = PathBuf::from(rel.as_ref());
        if extension_matches_policy(&p) {
            out.push(p);
        }
    }
    Ok(out)
}

/// Resolve `(base, head)` for `git diff base...head` (merge-base semantics).
fn resolve_diff_refs(
    repo_root: &Path,
    base_override: Option<&str>,
) -> Result<Option<(String, String)>> {
    if let Some(b) = base_override.filter(|s| !s.is_empty()) {
        let head = std::env::var("VOX_LINE_ENDINGS_HEAD").unwrap_or_else(|_| "HEAD".to_string());
        return Ok(Some((b.to_string(), head)));
    }
    if let Ok(b) = std::env::var("VOX_LINE_ENDINGS_BASE") {
        if !b.is_empty() {
            let head =
                std::env::var("VOX_LINE_ENDINGS_HEAD").unwrap_or_else(|_| "HEAD".to_string());
            return Ok(Some((b, head)));
        }
    }
    let base_sha = std::env::var("GITHUB_BASE_SHA")
        .ok()
        .filter(|s| !s.is_empty());
    let sha = std::env::var("GITHUB_SHA").ok().filter(|s| !s.is_empty());
    if let (Some(b), Some(h)) = (base_sha, sha) {
        return Ok(Some((b, h)));
    }
    // Local / push: compare last commit to its parent
    let head = match git_output(repo_root, &["rev-parse", "--verify", "HEAD"]) {
        Ok(o) => String::from_utf8_lossy(&o).trim().to_string(),
        Err(_) => return Ok(None),
    };
    let parent = match git_output(repo_root, &["rev-parse", "--verify", "HEAD~1"]) {
        Ok(o) => String::from_utf8_lossy(&o).trim().to_string(),
        Err(_) => return Ok(None),
    };
    Ok(Some((parent, head)))
}

fn list_changed_policy_paths(
    repo_root: &Path,
    base_override: Option<&str>,
) -> Result<Vec<PathBuf>> {
    let Some((base, head)) = resolve_diff_refs(repo_root, base_override)? else {
        eprintln!(
            "line-endings: could not resolve diff base (set VOX_LINE_ENDINGS_BASE or use --base / --all)"
        );
        return Ok(Vec::new());
    };

    let range = format!("{base}...{head}");
    let stdout = match git_output(repo_root, &["diff", "--name-only", &range]) {
        Ok(s) => s,
        Err(_) => {
            // Fallback: two-commit diff when merge-base range is unavailable (e.g. shallow clone).
            git_output(repo_root, &["diff", "--name-only", &base, &head])?
        }
    };

    let mut out = Vec::new();
    for line in stdout.split(|&b| b == b'\n') {
        if line.is_empty() {
            continue;
        }
        let rel = String::from_utf8_lossy(line).to_string();
        let p = PathBuf::from(rel);
        if extension_matches_policy(&p) {
            out.push(p);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lf_only_detection() {
        assert!(!violates_lf_only(b"line\n"));
        assert!(violates_lf_only(b"line\r\n"));
        assert!(violates_lf_only(b"line\r"));
    }

    #[test]
    fn extension_policy() {
        assert!(extension_matches_policy(Path::new("foo.rs")));
        assert!(extension_matches_policy(Path::new("bar.MD")));
        assert!(!extension_matches_policy(Path::new("x.ps1")));
        assert!(!extension_matches_policy(Path::new("bin.exe")));
    }
}
