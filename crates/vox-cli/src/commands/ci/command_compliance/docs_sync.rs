//! Doc loading and markdown section helpers for compliance checks.

use anyhow::{Context, Result};
use std::path::Path;

use vox_bounded_fs::read_utf8_path_capped;

/// Extract the `### `vox ci …` section from the CLI reference doc (until the next `### ` heading).
pub(crate) fn ref_cli_vox_ci_section(ref_text: &str) -> Option<&str> {
    let key = "### `vox ci";
    let start = ref_text.find(key)?;
    let after = &ref_text[start + 1..];
    let rel = after.find("\n### ").unwrap_or(after.len());
    let end = start + 1 + rel;
    Some(&ref_text[start..end])
}

/// Extract the `### `vox codex` section from the CLI reference doc (until the next `### ` heading).
pub(crate) fn ref_cli_vox_codex_section(ref_text: &str) -> Option<&str> {
    let key = "### `vox codex";
    let start = ref_text.find(key)?;
    let after = &ref_text[start + 1..];
    let rel = after.find("\n### ").unwrap_or(after.len());
    let end = start + 1 + rel;
    Some(&ref_text[start..end])
}

pub(crate) fn markdown_section<'a>(doc: &'a str, heading: &str) -> Option<&'a str> {
    let start = doc.find(heading)?;
    let after = &doc[start + heading.len()..];
    let rel_end = after.find("\n## ").unwrap_or(after.len());
    Some(&doc[start..start + heading.len() + rel_end])
}

/// Body used for `check_ref_cli` needles.
///
/// When `docs/src/ref-cli.md` exists as a short redirect, its text alone does not satisfy
/// registry substring checks (e.g. `vox build`). Append the canonical reference so compliance
/// validates against the real SSOT while keeping stable URLs for legacy links.
pub(crate) fn read_cli_reference_for_compliance(repo_root: &Path) -> Result<String> {
    let legacy = repo_root.join("docs/src/ref-cli.md");
    let canonical = repo_root.join("docs/src/reference/cli.md");
    let canonical_text = read_utf8_path_capped(&canonical)
        .with_context(|| format!("read {}", canonical.display()))?;
    if legacy.is_file() {
        let legacy_text =
            read_utf8_path_capped(&legacy).with_context(|| format!("read {}", legacy.display()))?;
        return Ok(format!("{legacy_text}\n\n{canonical_text}"));
    }
    Ok(canonical_text)
}

pub(crate) fn read_env_vars_ssot_doc(repo_root: &Path) -> Result<String> {
    // Always load the prose SSOT tables — `docs/src/ref/env-vars-ssot.md` is a stub redirect only.
    let canonical = repo_root.join("docs/src/reference/env-vars.md");
    read_utf8_path_capped(&canonical).with_context(|| {
        format!(
            "read {} (canonical env-var prose SSOT)",
            canonical.display()
        )
    })
}

pub(crate) fn read_reachability_doc(repo_root: &Path) -> Result<String> {
    let p = repo_root.join("docs/src/reference/cli.md");
    read_utf8_path_capped(&p).with_context(|| {
        format!(
            "read {} (reachability matrix under 'CLI command reachability')",
            p.display()
        )
    })
}
