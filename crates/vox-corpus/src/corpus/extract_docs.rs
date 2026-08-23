//! Markdown documentation corpus extractor for Mens training data.
//!
//! Walks `docs/src/**/*.md` and extracts fenced code blocks tagged ` ```vox `
//! as training pairs, plus section-level Q&A pairs from architecture docs.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use anyhow::Context;
use chrono::{NaiveDate, Utc};
use regex::Regex;
use serde::Deserialize;
use serde_json::json;

use vox_bounded_fs::read_utf8_path_capped;

static VOX_DOC_LINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[[^\]]+\]\(([^)]+\.vox)\)").expect("vox doc link regex"));

#[derive(Deserialize, Debug)]
#[serde(default)]
struct Frontmatter {
    training_eligible: bool,
    last_updated: Option<String>,
    title: Option<String>,
    status: Option<String>,
}

impl Default for Frontmatter {
    fn default() -> Self {
        Self {
            training_eligible: true,
            last_updated: None,
            title: None,
            status: None,
        }
    }
}

/// Configuration for documentation extraction.
#[derive(Debug, Clone)]
pub struct ExtractDocsConfig {
    /// Root directory to walk (usually `docs/src/`).
    pub root: PathBuf,
    /// Whether to extract fenced Vox code blocks.
    pub extract_code_blocks: bool,
    /// Whether to extract Q&A pairs from section headings.
    pub extract_qa_pairs: bool,
    /// Minimum section body length (chars) to extract a Q&A pair.
    pub min_section_chars: usize,
    /// Maximum number of pairs to emit (0 = unlimited).
    pub limit: usize,
}

impl Default for ExtractDocsConfig {
    fn default() -> Self {
        Self {
            root: PathBuf::from("docs/src"),
            extract_code_blocks: true,
            extract_qa_pairs: true,
            min_section_chars: 100,
            limit: 0,
        }
    }
}

/// One extracted documentation training pair.
#[derive(Debug, Clone)]
pub struct DocTrainingPair {
    /// Source file path.
    pub source_path: PathBuf,
    /// Category tag.
    pub category: String,
    /// The prompt.
    pub prompt: String,
    /// The response.
    pub response: String,
    /// Quality rating.
    pub rating: u8,
    /// Estimated difficulty (3-10).
    pub difficulty: u8,
    /// Data lane (codegen vs docs qa).
    pub lane: String,
    /// Expected response surface for this row.
    pub response_mode: String,
    /// Task family for downstream segmentation.
    pub task_family: String,
    /// Additional traceability metadata for later retrieval or review.
    pub metadata: serde_json::Value,
}

impl DocTrainingPair {
    /// Serialize to JSONL.
    #[must_use]
    pub fn to_jsonl(&self) -> String {
        let v = json!({
            "prompt": self.prompt,
            "response": self.response,
            "category": self.category,
            "rating": self.rating,
            "difficulty": self.difficulty,
            "source": self.source_path.display().to_string(),
            "format": "documentation",
            "lane": self.lane,
            "response_mode": self.response_mode,
            "task_family": self.task_family,
            "metadata": self.metadata,
        });
        v.to_string()
    }
}

/// Parse YAML frontmatter to determine eligibility and staleness penalty.
/// Returns parsed metadata and staleness penalty where penalty increases with age (0-3 scale).
fn parse_frontmatter(content: &str, path: &Path) -> (Frontmatter, bool, u8) {
    // Explicit deprecation check acts as a hard short-circuit
    if content.contains("status: deprecated")
        || content.contains("status: \"deprecated\"")
        || content.contains("status: 'deprecated'")
    {
        return (Frontmatter::default(), false, 0);
    }

    if !content.starts_with("---") {
        // Fallback for files without frontmatter
        let eligible = !(content.contains("training_eligible: false")
            || content.contains("training_eligible:false"));
        let fallback = Frontmatter {
            title: path
                .file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.replace(['-', '_'], " ")),
            ..Frontmatter::default()
        };
        return (fallback, eligible, 0);
    }

    let parts: Vec<&str> = content.splitn(3, "---").collect();
    if parts.len() < 3 {
        let eligible = !(content.contains("training_eligible: false")
            || content.contains("training_eligible:false"));
        return (Frontmatter::default(), eligible, 0);
    }

    let yaml_str = parts[1];

    // Extract using serde_yaml, fallback to true if malformed
    let fm: Frontmatter = serde_yaml::from_str(yaml_str).unwrap_or_default();

    if !fm.training_eligible {
        return (fm, false, 0);
    }

    let mut penalty = 0;
    if let Some(ref date_str) = fm.last_updated
        && let Ok(last_updated) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
    {
        let now = Utc::now().date_naive();
        let days_old = now.signed_duration_since(last_updated).num_days();

        // Penalize by 1 for every 90 days (approx 3 months), max penalty of 3
        if days_old > 0 {
            penalty = (days_old / 90).min(3) as u8;
        }
    }

    (fm, true, penalty)
}

/// Extract training pairs from a single markdown file.
pub fn extract_from_md_file(
    path: &Path,
    config: &ExtractDocsConfig,
) -> anyhow::Result<Vec<DocTrainingPair>> {
    let source = read_utf8_path_capped(path)?;

    let (frontmatter, eligible, staleness_penalty) = parse_frontmatter(&source, path);
    if !eligible {
        return Ok(Vec::new());
    }

    let mut pairs = Vec::new();

    if config.extract_code_blocks {
        extract_code_blocks(&source, path, &frontmatter, staleness_penalty, &mut pairs);
    }

    if config.extract_qa_pairs {
        extract_qa_sections(
            &source,
            path,
            &frontmatter,
            staleness_penalty,
            config,
            &mut pairs,
        );
    }

    if config.limit > 0 {
        pairs.truncate(config.limit);
    }

    Ok(pairs)
}

/// Extract fenced code blocks tagged with `vox` language.
fn extract_code_blocks(
    source: &str,
    path: &Path,
    frontmatter: &Frontmatter,
    staleness_penalty: u8,
    out: &mut Vec<DocTrainingPair>,
) {
    let lines: Vec<&str> = source.lines().collect();
    let mut i = 0;
    let mut preceding_context = String::new();

    while i < lines.len() {
        let trimmed = lines[i].trim();

        // Track headings and paragraphs for context
        if trimmed.starts_with('#') {
            preceding_context = trimmed.trim_start_matches('#').trim().to_string();
        } else if !trimmed.is_empty()
            && !trimmed.starts_with("```")
            && !trimmed.starts_with("---")
            && !trimmed.starts_with('>')
        {
            // Accumulate paragraph text (last paragraph before code block)
            if preceding_context.len() < 200 {
                if !preceding_context.is_empty() {
                    preceding_context.push(' ');
                }
                preceding_context.push_str(trimmed);
            }
        }

        // Detect ```vox code blocks
        if trimmed.starts_with("```vox") || trimmed == "```vox" {
            let mut code_lines = Vec::new();
            i += 1;
            while i < lines.len() {
                if lines[i].trim() == "```" {
                    break;
                }
                code_lines.push(lines[i]);
                i += 1;
            }

            let code = code_lines.join("\n");
            if code.contains("{{#include") || code.contains("// vox:skip") {
                preceding_context.clear();
                i += 1;
                continue;
            }
            if code.len() >= 20 {
                let suffix =
                    " Use valid Vox only: annotate `fn` with `->` return types and 4-space indent.";
                let prompt = if !preceding_context.is_empty() {
                    format!(
                        "Show me Vox code for: {}{}",
                        preceding_context.chars().take(200).collect::<String>(),
                        suffix
                    )
                } else {
                    format!("Write an example Vox program.{suffix}")
                };

                out.push(DocTrainingPair {
                    source_path: path.to_path_buf(),
                    category: "documentation".to_string(),
                    prompt,
                    response: code,
                    rating: 4u8.saturating_sub(staleness_penalty).max(1),
                    difficulty: 7, // Code blocks are higher difficulty than prose
                    lane: "vox_codegen".to_string(),
                    response_mode: "code_only".to_string(),
                    task_family: "docs_code".to_string(),
                    metadata: build_metadata(
                        path,
                        frontmatter,
                        Some(&preceding_context),
                        "code_block",
                    ),
                });
            }
            preceding_context.clear();
        }

        i += 1;
    }
}

/// Extract Q&A pairs from markdown section headings.
fn extract_qa_sections(
    source: &str,
    path: &Path,
    frontmatter: &Frontmatter,
    staleness_penalty: u8,
    config: &ExtractDocsConfig,
    out: &mut Vec<DocTrainingPair>,
) {
    let lines: Vec<&str> = source.lines().collect();
    let mut current_heading = String::new();
    let mut current_body = String::new();
    let mut heading_level = 0usize;

    // Fence-aware accumulation. A fence carrying `// vox:skip` (or an
    // unresolved `{{#include}}`) is dropped from the section body, matching
    // what `extract_code_blocks` already does for the code lane. Without this
    // the QA lane shipped skip-marked fences verbatim into `vox_docs_qa` --
    // including retired syntax the author had explicitly marked to exclude.
    let mut in_fence = false;
    let mut fence_buf: Vec<&str> = Vec::new();
    // The opening fence verbatim, so the language tag survives re-emission.
    let mut fence_open = "```";

    for line in &lines {
        let trimmed = line.trim();

        // Fence open/close, tracked before anything else so a heading-looking
        // line inside a fence cannot split the section.
        if trimmed.starts_with("```") {
            if in_fence {
                let body = fence_buf.join("\n");
                if !(body.contains("// vox:skip") || body.contains("{{#include")) {
                    current_body.push_str(fence_open);
                    current_body.push_str("\n");
                    current_body.push_str(&body);
                    current_body.push_str("\n```\n");
                }
                fence_buf.clear();
                in_fence = false;
            } else {
                in_fence = true;
                fence_open = trimmed;
            }
            continue;
        }
        if in_fence {
            fence_buf.push(line);
            continue;
        }

        // New heading
        if trimmed.starts_with('#') {
            // Flush previous section
            if !current_heading.is_empty()
                && current_body.len() >= config.min_section_chars
                && heading_level >= 2
            {
                let prompt = format!(
                    "Explain the Vox concept: {} (precise prose; any code snippets must use `->` returns.)",
                    current_heading
                );
                let mut response = current_body.trim().to_string();

                // Relational Chunking: Inject linked .vox examples directly into the training response
                {
                    let mut extra_context = String::new();
                    for cap in VOX_DOC_LINK_RE.captures_iter(&response) {
                        let target_path_str = &cap[1];
                        let abs_target =
                            path.parent().unwrap_or(Path::new("")).join(target_path_str);
                        if let Ok(can) = std::fs::canonicalize(&abs_target)
                            && let Ok(vox_code) = read_utf8_path_capped(&can)
                        {
                            extra_context.push_str("\n\n```vox\n");
                            extra_context.push_str(vox_code.trim());
                            extra_context.push_str("\n```\n");
                        }
                    }
                    response.push_str(&extra_context);
                }

                out.push(DocTrainingPair {
                    source_path: path.to_path_buf(),
                    category: "documentation".to_string(),
                    prompt,
                    response,
                    rating: 3u8.saturating_sub(staleness_penalty).max(1),
                    difficulty: 5, // Q&A prose is mid-difficulty
                    lane: "vox_docs_qa".to_string(),
                    response_mode: "prose_only".to_string(),
                    task_family: "docs_qa".to_string(),
                    metadata: build_metadata(
                        path,
                        frontmatter,
                        Some(&current_heading),
                        "qa_section",
                    ),
                });
            }

            heading_level = trimmed.chars().take_while(|&c| c == '#').count();
            current_heading = trimmed.trim_start_matches('#').trim().to_string();
            current_body.clear();
        } else if !trimmed.is_empty() {
            current_body.push_str(trimmed);
            current_body.push('\n');
        }
    }

    // Flush last section
    if !current_heading.is_empty()
        && current_body.len() >= config.min_section_chars
        && heading_level >= 2
    {
        let prompt = format!("Explain the Vox concept: {}", current_heading);
        let mut response = current_body.trim().to_string();

        // Relational Chunking: Inject linked .vox examples directly into the training response
        if let Ok(link_re) = Regex::new(r"\[[^\]]+\]\(([^)]+\.vox)\)") {
            let mut extra_context = String::new();
            for cap in link_re.captures_iter(&response.clone()) {
                let target_path_str = &cap[1];
                let abs_target = path.parent().unwrap_or(Path::new("")).join(target_path_str);
                if let Ok(can) = std::fs::canonicalize(&abs_target)
                    && let Ok(vox_code) = read_utf8_path_capped(&can)
                {
                    extra_context.push_str("\n\n```vox\n");
                    extra_context.push_str(vox_code.trim());
                    extra_context.push_str("\n```\n");
                }
            }
            response.push_str(&extra_context);
        }

        out.push(DocTrainingPair {
            source_path: path.to_path_buf(),
            category: "documentation".to_string(),
            prompt,
            response,
            rating: 3u8.saturating_sub(staleness_penalty).max(1),
            difficulty: 5,
            lane: "vox_docs_qa".to_string(),
            response_mode: "prose_only".to_string(),
            task_family: "docs_qa".to_string(),
            metadata: build_metadata(path, frontmatter, Some(&current_heading), "qa_section"),
        });
    }
}

fn build_metadata(
    path: &Path,
    frontmatter: &Frontmatter,
    heading: Option<&str>,
    chunk_kind: &str,
) -> serde_json::Value {
    let heading = heading
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string);
    json!({
        "doc_path": normalize_path(path),
        "canonical_path": normalize_path(path),
        "doc_title": frontmatter.title.clone(),
        "doc_status": frontmatter.status.clone(),
        "last_updated": frontmatter.last_updated.clone(),
        "heading": heading,
        "heading_slug": heading.as_deref().map(slugify_heading),
        "chunk_kind": chunk_kind,
        "source_kind": "documentation",
    })
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn slugify_heading(s: &str) -> String {
    let mut slug = String::with_capacity(s.len());
    let mut prev_dash = false;
    for ch in s.chars().flat_map(|c| c.to_lowercase()) {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            prev_dash = false;
        } else if !prev_dash {
            slug.push('-');
            prev_dash = true;
        }
    }
    slug.trim_matches('-').to_string()
}

/// Walk a directory tree and extract pairs from all `.md` files.
///
/// Task 3.3: Automate dynamic inclusion of internal workspace crate documentation.
pub fn walk_and_extract_docs(config: &ExtractDocsConfig) -> anyhow::Result<Vec<DocTrainingPair>> {
    let mut all = Vec::new();
    walk_docs_dir(&config.root, config, &mut all)?;

    // Automated workspace discovery: find READMEs and /docs in sibling `crates/`
    if let Some(parent) = config.root.parent()
        && (parent.ends_with("docs") || parent.join("crates").is_dir())
    {
        let crates_dir = if parent.ends_with("docs") {
            parent.parent().unwrap_or(Path::new(".")).join("crates")
        } else {
            parent.join("crates")
        };

        if let Ok(entries) = std::fs::read_dir(&crates_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    // Try README.md
                    let readme = p.join("README.md");
                    if readme.is_file()
                        && let Ok(pairs) = extract_from_md_file(&readme, config)
                    {
                        all.extend(pairs);
                    }
                    // Try docs/ directory inside crate
                    let crate_docs = p.join("docs");
                    if crate_docs.is_dir() {
                        walk_docs_dir(&crate_docs, config, &mut all)?;
                    }
                }
            }
        }
    }

    Ok(all)
}

fn walk_docs_dir(
    dir: &Path,
    config: &ExtractDocsConfig,
    out: &mut Vec<DocTrainingPair>,
) -> anyhow::Result<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_docs_dir(&path, config, out)?;
        } else if path.extension().is_some_and(|e| e == "md") {
            match extract_from_md_file(&path, config) {
                Ok(mut pairs) => {
                    if config.limit > 0 {
                        let remaining = config.limit.saturating_sub(out.len());
                        pairs.truncate(remaining);
                    }
                    out.extend(pairs);
                    if config.limit > 0 && out.len() >= config.limit {
                        return Ok(());
                    }
                }
                Err(e) => {
                    eprintln!("  [doc extract] skip {}: {e}", path.display());
                }
            }
        }
    }
    Ok(())
}

/// Write extracted doc pairs to a JSONL file.
pub fn write_docs_to_jsonl(pairs: &[DocTrainingPair], output: &Path) -> anyhow::Result<usize> {
    use std::io::Write;
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(output)
        .with_context(|| format!("open output {}", output.display()))?;
    for pair in pairs {
        writeln!(f, "{}", pair.to_jsonl())?;
    }
    Ok(pairs.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fence the author marked `// vox:skip` must not reach the QA lane.
    ///
    /// The code lane already filters these (see `extract_code_blocks`), but the
    /// QA lane accumulated every non-empty line including fenced blocks. The
    /// result: `docs/src/architecture/wire-format-v1-ssot.md` is
    /// `training_eligible: true` and carries six `@endpoint(kind: ...)`
    /// occurrences -- syntax that became a hard parse error on 2026-06-30 --
    /// inside `vox:skip`-marked fences, and shipped all of them verbatim into
    /// `vox_docs_qa`. The marker the author wrote to keep it out of training
    /// data did nothing in this lane.
    const SKIP_FENCE_MD: &str = r#"# Wire Format

## Endpoint Shape

The v1 wire format describes each route in terms of its declaration. This
paragraph is long enough to clear the minimum section length so the section
is actually emitted as a training pair by the extractor under test.

```vox
// vox:skip — illustrative endpoint definition
@endpoint(kind: query) fn get_user(id: int) -> User {
    return db.User.get(id)
}
```

More prose after the fence, also part of the section body.
"#;

    #[test]
    fn qa_lane_excludes_skip_marked_fences() {
        let mut out = Vec::new();
        let config = ExtractDocsConfig {
            min_section_chars: 40,
            ..ExtractDocsConfig::default()
        };
        extract_qa_sections(
            SKIP_FENCE_MD,
            Path::new("wire-format.md"),
            &Frontmatter::default(),
            0,
            &config,
            &mut out,
        );
        assert!(!out.is_empty(), "the section should still be extracted");
        let body = &out[0].response;
        assert!(
            !body.contains("@endpoint"),
            "a vox:skip fence must not reach the QA lane; got: {body}"
        );
        assert!(
            body.contains("wire format describes"),
            "surrounding prose must survive; got: {body}"
        );
        assert!(
            body.contains("More prose after the fence"),
            "prose after the fence must survive; got: {body}"
        );
    }

    #[test]
    fn qa_lane_keeps_unmarked_fences() {
        // Only skip-marked fences are dropped. An ordinary example is still
        // legitimate Q&A context and must survive.
        const OK_MD: &str = r#"# Actors

## Counter

This section explains the counter actor and is long enough to be emitted as
a training pair by the extractor under test without being filtered out.

```vox
actor Counter {
    state count: int = 0
}
```
"#;
        let mut out = Vec::new();
        let config = ExtractDocsConfig {
            min_section_chars: 40,
            ..ExtractDocsConfig::default()
        };
        extract_qa_sections(
            OK_MD,
            Path::new("actors.md"),
            &Frontmatter::default(),
            0,
            &config,
            &mut out,
        );
        assert!(!out.is_empty());
        assert!(
            out[0].response.contains("actor Counter"),
            "unmarked fences must survive: {}",
            out[0].response
        );
    }

    const SAMPLE_MD: &str = r#"# Vox Actors

## Actor Model

Vox actors are isolated entities with mailbox-based message passing.
Each actor has its own state and handles messages sequentially.
This prevents data races without explicit locks.

```vox
actor Counter {
    state count: int = 0

    on Increment() to int {
        count = count + 1
        return count
    }
}
```

## Workflows

Durable execution is a first-class feature.
"#;

    #[test]
    fn extracts_vox_code_block() {
        let config = ExtractDocsConfig::default();
        let _pairs = extract_from_md_file(Path::new("test.md"), &config);
        // Can't test with real file, test the extraction logic directly
        let mut out = Vec::new();
        extract_code_blocks(
            SAMPLE_MD,
            Path::new("test.md"),
            &Frontmatter::default(),
            0,
            &mut out,
        );
        assert!(!out.is_empty(), "should extract vox code block");
        assert!(out[0].response.contains("actor Counter"));
        assert_eq!(out[0].metadata["chunk_kind"], "code_block");
    }

    #[test]
    fn extracts_qa_sections() {
        let config = ExtractDocsConfig {
            min_section_chars: 50,
            ..Default::default()
        };
        let mut out = Vec::new();
        extract_qa_sections(
            SAMPLE_MD,
            Path::new("test.md"),
            &Frontmatter::default(),
            0,
            &config,
            &mut out,
        );
        assert!(!out.is_empty(), "should extract at least one Q&A pair");
        assert!(out[0].prompt.contains("Actor Model"));
        assert_eq!(out[0].metadata["chunk_kind"], "qa_section");
    }

    #[test]
    fn ignores_mdbook_include_directives() {
        const MD_WITH_INCLUDE: &str = r#"# Tutorial
Check out this code:

```vox
{{#include ../../../examples/golden/getting_started.vox:logic}}
```
"#;
        let mut out = Vec::new();
        extract_code_blocks(
            MD_WITH_INCLUDE,
            Path::new("test.md"),
            &Frontmatter::default(),
            0,
            &mut out,
        );
        assert!(
            out.is_empty(),
            "should ignore code block containing mdbook include"
        );
    }
}
