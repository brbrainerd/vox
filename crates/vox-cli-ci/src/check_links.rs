use anyhow::{Context, Result};
use chrono::NaiveDate;
use owo_colors::OwoColorize;
use regex::Regex;
use serde::Deserialize;
use std::path::Path;
use walkdir::WalkDir;

use vox_bounded_fs::read_utf8_path_capped;

const LINK_ALLOWLIST_REL: &str = "contracts/documentation/link-allowlist.v1.yaml";

#[derive(Debug, Deserialize)]
struct LinkAllowlistFile {
    #[allow(dead_code)]
    schema_version: u32,
    #[serde(default)]
    allowlist: Vec<LinkAllowEntry>,
}

#[derive(Debug, Deserialize)]
struct LinkAllowEntry {
    source: String,
    target: String,
    #[allow(dead_code)]
    reason: String,
    expires: String,
}

fn load_allowlist(repo_root: &Path) -> Result<Vec<LinkAllowEntry>> {
    let path = repo_root.join(LINK_ALLOWLIST_REL);
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let raw = read_utf8_path_capped(&path).with_context(|| format!("read {}", path.display()))?;
    let parsed: LinkAllowlistFile =
        serde_yaml::from_str(&raw).with_context(|| format!("parse {}", LINK_ALLOWLIST_REL))?;
    Ok(parsed.allowlist)
}

fn repo_rel_normalized(repo_root: &Path, path: &Path) -> String {
    path.strip_prefix(repo_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Strip trailing `:LINE` / `:LINE-LINE` suffixes from Markdown links (common for Rust line refs).
fn strip_source_line_suffix(target_path: &str) -> &str {
    let re = Regex::new(r"(?s)^(.+?):\d+(?:-\d+)?$").expect("regex");
    let Some(caps) = re.captures(target_path) else {
        return target_path;
    };
    let base = caps.get(1).map(|m| m.as_str()).unwrap_or(target_path);
    if looks_like_file_path_with_extension(base) {
        base
    } else {
        target_path
    }
}

fn looks_like_file_path_with_extension(path: &str) -> bool {
    let base = path.rsplit(['/', '\\']).next().unwrap_or(path);
    base.contains('.') && !base.ends_with('.') && Path::new(base).extension().is_some()
}

fn allowlist_skips(
    allowlist: &[LinkAllowEntry],
    source_rel: &str,
    target_full: &str,
    today: NaiveDate,
) -> bool {
    for e in allowlist {
        let src = e.source.replace('\\', "/");
        let tgt = e.target.replace('\\', "/");
        if src != source_rel || tgt != target_full {
            continue;
        }
        if let Ok(exp) = NaiveDate::parse_from_str(&e.expires, "%Y-%m-%d") {
            if exp < today {
                println!(
                    "{} allowlist entry expired (still skipping): {} -> {} (expired {})",
                    "WARN".yellow(),
                    source_rel.cyan(),
                    target_full.yellow(),
                    e.expires
                );
            }
        }
        return true;
    }
    false
}

pub fn run(repo_root: &Path, target: Option<&Path>) -> Result<()> {
    let docs_src = repo_root.join("docs").join("src");
    let docs_dir = repo_root.join("docs");

    println!(
        "{} Checking internal links in docs and root guides...",
        "INIT".bright_blue(),
    );

    let allowlist = load_allowlist(repo_root)?;
    let today = chrono::Utc::now().date_naive();
    let mut total_links = 0;
    let mut broken_links = Vec::new();
    let mut nesting_errors = Vec::new();

    // Match [text](target) where target is not external or anchor-only
    let link_re = Regex::new(r"\[[^\]]+\]\(([^)]+)\)").unwrap();

    let mut markdown_files = Vec::new();

    if let Some(t) = target {
        let abs_t = if t.is_absolute() {
            t.to_path_buf()
        } else {
            repo_root.join(t)
        };
        if abs_t.is_file() && abs_t.extension().is_some_and(|ext| ext == "md") {
            markdown_files.push(abs_t);
        } else if abs_t.is_dir() {
            collect_markdown_files(&abs_t, &mut markdown_files);
        } else {
            println!(
                "{} Target not found or invalid: {:?}",
                "SKIP".yellow(),
                abs_t
            );
            return Ok(());
        }
    } else {
        if !docs_src.exists() {
            println!(
                "{} Docs source not found at {:?}",
                "SKIP".yellow(),
                docs_src
            );
            return Ok(());
        }
        collect_markdown_files(&docs_src, &mut markdown_files);
        collect_root_guides(repo_root, &mut markdown_files);
        collect_docs_root_guides(&docs_dir, &mut markdown_files);
    }

    for path in markdown_files {
        // Enforce max depth for files inside docs/src
        if path.starts_with(&docs_src) {
            let rel = path.strip_prefix(&docs_src).unwrap();
            let depth = rel.components().count();
            if depth > 3 {
                nesting_errors.push(path.to_path_buf());
            }
        }

        let content =
            read_utf8_path_capped(&path).with_context(|| format!("Failed to read {:?}", path))?;

        let parent_dir = path.parent().unwrap_or(repo_root);

        // Ignore Markdown links inside fenced code blocks — they are usually not links.
        let mut in_fence = false;
        for line in content.lines() {
            let trimmed_start = line.trim_start();
            if trimmed_start.starts_with("```") {
                in_fence = !in_fence;
                continue;
            }
            if in_fence {
                continue;
            }

            for cap in link_re.captures_iter(line) {
                let target_full = &cap[1];

                // Skip external links and local anchors
                if target_full.starts_with("http")
                    || target_full.starts_with("#")
                    || target_full.starts_with("mailto:")
                {
                    continue;
                }

                total_links += 1;

                // Split into path and optional anchor
                let mut parts = target_full.splitn(2, '#');
                let target_path_str = parts.next().unwrap_or(target_full);
                let anchor = parts.next();

                if target_path_str.is_empty() {
                    continue;
                }

                let normalized_path = strip_source_line_suffix(target_path_str.trim());
                let source_rel = repo_rel_normalized(repo_root, &path);
                if allowlist_skips(&allowlist, &source_rel, target_full, today) {
                    continue;
                }

                // Resolve path (strip trailing `/` so directory targets resolve on all platforms)
                let target_path = if normalized_path.starts_with("/") {
                    repo_root.join(
                        normalized_path
                            .trim_start_matches('/')
                            .trim_end_matches('/'),
                    )
                } else {
                    parent_dir.join(normalized_path.trim_end_matches('/'))
                };

                // Normalize and check existence
                if !target_path.exists() {
                    broken_links.push((
                        path.to_path_buf(),
                        target_full.to_string(),
                        target_path,
                        "missing_file",
                    ));
                } else if let Some(anchor_text) = anchor {
                    if !check_anchor(&target_path, anchor_text) {
                        broken_links.push((
                            path.to_path_buf(),
                            target_full.to_string(),
                            target_path,
                            "missing_anchor",
                        ));
                    }
                }
            }
        }
    }

    println!("{} Checked {} internal links.", "DONE".green(), total_links);

    let mut failed = false;

    if !nesting_errors.is_empty() {
        println!(
            "{} Found {} inappropriately nested files (> 3 levels deep in docs/src):",
            "FAIL".red().bold(),
            nesting_errors.len()
        );
        for f in &nesting_errors {
            println!(
                "  {}",
                f.strip_prefix(repo_root).unwrap_or(f).display().yellow()
            );
        }
        failed = true;
    }

    if !broken_links.is_empty() {
        println!(
            "{} Found {} broken links:",
            "FAIL".red().bold(),
            broken_links.len()
        );
        for (source, target_str, resolved, reason) in &broken_links {
            let rel_source = source.strip_prefix(repo_root).unwrap_or(source);
            println!(
                "  {} -> {} (resolved: {:?}) [{}]",
                rel_source.display().cyan(),
                target_str.yellow(),
                resolved.display().dimmed(),
                reason.red()
            );
        }
        failed = true;
    }

    if failed {
        Err(anyhow::anyhow!("Documentation integrity check failed"))
    } else {
        println!(
            "{} All internal links and structures are valid!",
            "PASS".green().bold()
        );
        Ok(())
    }
}

fn check_anchor(path: &Path, anchor: &str) -> bool {
    let content = match read_utf8_path_capped(path) {
        Ok(c) => c,
        Err(_) => return false,
    };

    // Convert anchor to a header string. Example: "phase-1:-harden-the-core-ci-link-checker"
    // This is a simplistic check, matching `# some text` ignoring case and punctuation where needed.
    // For robust matching, we look for an explicit inline `<a id="...">` or `<a name="...">` OR a header line.

    let a_id_pattern = format!("id=\"{anchor}\"");
    let a_name_pattern = format!("name=\"{anchor}\"");
    if content.contains(&a_id_pattern) || content.contains(&a_name_pattern) {
        return true;
    }

    // Try finding headers
    let lines = content.lines();
    for line in lines {
        if line.starts_with('#') {
            let text = line.trim_start_matches('#').trim();
            // Basic markdown anchor generation: lowercase, replace spaces with hyphen, remove punctuation
            let generated = text
                .to_lowercase()
                .chars()
                .filter(|c| c.is_alphanumeric() || c.is_whitespace() || *c == '-')
                .collect::<String>()
                .replace(" ", "-");
            if generated == anchor {
                return true;
            }
        }
    }

    false
}

fn collect_markdown_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        // Skip tombstoned archive trees per AGENTS.md §Archival Protocol —
        // archived docs are kept for human reference and may legitimately link
        // to since-moved code or pre-archive doc paths.
        if path.components().any(|c| c.as_os_str() == "archive") {
            continue;
        }
        if entry.file_type().is_file() && path.extension().is_some_and(|ext| ext == "md") {
            out.push(path.to_path_buf());
        }
    }
}

fn collect_root_guides(repo_root: &Path, out: &mut Vec<std::path::PathBuf>) {
    for rel in ["README.md", "AGENTS.md", "CONTRIBUTING.md"] {
        let path = repo_root.join(rel);
        if path.is_file() {
            out.push(path);
        }
    }
}

fn collect_docs_root_guides(docs_dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    if !docs_dir.is_dir() {
        return;
    }
    for entry in WalkDir::new(docs_dir)
        .max_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() && entry.path().extension().is_some_and(|ext| ext == "md") {
            out.push(entry.path().to_path_buf());
        }
    }
}
