//! Research-document persistence. Phase 0a — fully implemented (no stubbing).
//!
//! Future phases extend this to also emit signed nanopubs (Phase 4) and
//! RO-Crate envelopes (Phase 4) alongside the Markdown doc.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// Convert a query to a filesystem-safe slug.
///
/// - Lowercase
/// - Non-alphanumerics → '-'
/// - Collapses runs of '-'
/// - Trims leading/trailing '-'
/// - At most 80 chars after dash-trim
/// - Empty → "untitled"
#[must_use]
pub fn slug_from_query(query: &str) -> String {
    let mut out = String::new();
    let mut last_dash = true; // suppress leading '-'
    for c in query.chars() {
        let lower = c.to_ascii_lowercase();
        if lower.is_ascii_alphanumeric() {
            out.push(lower);
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        return "untitled".to_string();
    }
    if out.len() > 80 {
        out.truncate(80);
        while out.ends_with('-') {
            out.pop();
        }
    }
    out
}

/// Write a research document at `<root>/docs/src/research/<slug>.md`.
pub fn write_research_doc(
    root: &Path,
    slug: &str,
    query: &str,
    answer: &str,
    model: &str,
) -> Result<PathBuf> {
    let dir = root.join("docs/src/research");
    fs::create_dir_all(&dir).with_context(|| format!("create_dir_all({:?})", dir))?;
    let path = dir.join(format!("{slug}.md"));
    let content = format!(
        "---\n\
         title: \"Research: {query_escaped}\"\n\
         description: \"Auto-generated research result.\"\n\
         category: \"research\"\n\
         status: \"draft\"\n\
         model: \"{model}\"\n\
         ---\n\n\
         # {query}\n\n\
         {answer}\n",
        query = query,
        query_escaped = query.replace('"', "\\\""),
        model = model,
        answer = answer,
    );
    fs::write(&path, content).with_context(|| format!("write({:?})", path))?;
    Ok(path)
}
