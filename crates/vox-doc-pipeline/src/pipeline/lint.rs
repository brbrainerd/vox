//! Markdown lint passes for docs under `docs/src/`.
//!
// SCIENTIA — contracts/scientia/*.schema.json
// Stable serde names live in vox-research-events::schema_types; typify exhaust in schema_types::generated.
// Regenerate: cargo run -p vox-scientia-jsonschema-codegen
// Hand-maintained name map (lint reminder): discovery-signal → DiscoverySignal; finding-candidate.v1 → FindingCandidateV1;
// novelty-evidence-bundle.v1 → NoveltyEvidenceBundle; evidence-pack.v1 → EvidencePackV1; worthiness-signals.v2 → WorthinessSignalsV2.

use std::fs;
use std::path::{Path, PathBuf};

use super::anchors::{extract_marked_block, readme_anchor};
use super::types::{LintError, LintKind};

// These must match the `sections` array in contracts/documentation/docs-sidebar-section-order.v1.json.
// Display-label format (e.g. "Language Reference") is canonical; slug aliases are kept for grep-safety
// but all new files must use the display-label form.
pub(crate) const VALID_CATEGORIES: &[&str] = &[
    // ── Canonical display labels (SSOT — match sidebar JSON exactly) ──────────
    "Getting Started",
    "Tutorials",
    "How-To Guides",
    "Language Reference",
    "API Reference — Crates",
    "Examples",
    "Concepts",
    "Architecture Decisions (ADRs)",
    "Architecture SSOTs",
    "Contributors",
    "CI & Quality",
    "Operations",
    // ── archive (excluded from sidebar but still a valid category) ────────────
    "archive",
];

pub(crate) const VALID_STATUS: &[&str] = &[
    "approved",
    "current",
    "experimental",
    "legacy",
    "research",
    "roadmap",
    "deprecated",
];

pub(crate) const VALID_SCHEMA_TYPES: &[&str] =
    &["HowTo", "FAQPage", "TechArticle", "SoftwareSourceCode"];

/// Suggest the closest valid value for a rejected frontmatter field so the fix can
/// be made in one pass instead of guessing. Bias: a case-insensitive prefix match
/// (e.g. `"CI"` / `"ci"` → `"CI & Quality"`) wins over raw edit distance; otherwise
/// fall back to the minimum Levenshtein candidate within a sane distance budget.
#[must_use]
pub(crate) fn suggest<'a>(value: &str, candidates: &[&'a str]) -> Option<&'a str> {
    let needle = value.trim().to_ascii_lowercase();
    if needle.is_empty() {
        return None;
    }
    // 1. Case-insensitive prefix in either direction (handles `CI`/`ci` → `CI & Quality`).
    if let Some(hit) = candidates.iter().find(|c| {
        let lc = c.to_ascii_lowercase();
        lc.starts_with(&needle) || needle.starts_with(&lc)
    }) {
        return Some(hit);
    }
    // 2. Minimum edit distance, capped so we don't suggest something wildly different.
    let budget = needle.len().max(4) / 2 + 2;
    candidates
        .iter()
        .map(|c| (levenshtein(&needle, &c.to_ascii_lowercase()), *c))
        .filter(|(d, _)| *d <= budget)
        .min_by_key(|(d, _)| *d)
        .map(|(_, c)| c)
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0_usize; b.len() + 1];
    for (i, &ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur[j + 1] = (prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

fn repo_root_for_lint() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// Long-form plans and design drafts often use unlabeled Markdown code fences for ASCII
/// diagrams, git snippets, and mixed excerpts; requiring a language tag on every fence is noise
/// without improving publish output. Canonical tutorials and reference SSOT pages remain enforced.
#[must_use]
pub(crate) fn skip_unlabeled_code_fence_rel(rel_normalized: &str) -> bool {
    if rel_normalized.starts_with("docs/src/archive/") {
        return true;
    }
    if rel_normalized.contains("docs/superpowers/plans/") {
        return true;
    }
    if rel_normalized.contains("docs/src/architecture/planning-meta/") {
        return true;
    }
    // Entire architecture tree: diagrams, mixed excerpts, and long-form SSOT all tolerate
    // unlabeled fences; tutorials/reference/how-to remain enforced.
    rel_normalized.starts_with("docs/src/architecture/")
}

fn skip_unlabeled_code_fence(path: &Path, repo_root: &Path) -> bool {
    let normalized = path.to_string_lossy().replace('\\', "/");
    if let Ok(rel) = path.strip_prefix(repo_root) {
        let rel = rel.to_string_lossy().replace('\\', "/");
        if skip_unlabeled_code_fence_rel(&rel) {
            return true;
        }
    }
    // Walkdir / callers may use paths that don't strip cleanly against `repo_root` (drive casing,
    // extra prefix segments). Anchor on the `docs/` path segment instead.
    normalized
        .find("docs/")
        .is_some_and(|idx| skip_unlabeled_code_fence_rel(&normalized[idx..]))
}

/// Recursively walk `dir` and collect lint errors for every `.md` file.
pub(crate) fn collect_lint_errors(dir: &Path, errors: &mut Vec<LintError>) {
    let root = repo_root_for_lint();
    collect_lint_errors_target_with_root(dir, errors, &root);
}

/// Collect lint errors from either a markdown file or a directory tree.
pub(crate) fn collect_lint_errors_target(target: &Path, errors: &mut Vec<LintError>) {
    let root = repo_root_for_lint();
    collect_lint_errors_target_with_root(target, errors, &root);
}

pub(crate) fn collect_lint_errors_target_with_root(
    target: &Path,
    errors: &mut Vec<LintError>,
    repo_root: &Path,
) {
    use rayon::prelude::*;

    // Gather every lintable `.md` file first, then lint them in parallel. Each file is
    // independent (pure parse + its own `git log` subprocess), so a failure in one never
    // halts the others — every error across the whole tree is surfaced in a single run.
    let mut md_files: Vec<PathBuf> = Vec::new();
    gather_md_files(target, &mut md_files);

    let collected: Vec<LintError> = md_files
        .par_iter()
        .flat_map_iter(|path| lint_one_file(path, repo_root))
        .collect();
    errors.extend(collected);
}

/// Recursively collect lintable `.md` paths (skips `SUMMARY.md`, which is tool-generated).
fn gather_md_files(target: &Path, out: &mut Vec<PathBuf>) {
    if target.is_file() {
        if target
            .extension()
            .map(|e| e == "md" || e == "mdx")
            .unwrap_or(false)
            && !target.to_str().unwrap_or_default().contains("SUMMARY.md")
        {
            out.push(target.to_path_buf());
        }
        return;
    }
    if !target.is_dir() {
        return;
    }
    if let Ok(entries) = fs::read_dir(target) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                gather_md_files(&path, out);
            } else if path
                .extension()
                .map(|e| e == "md" || e == "mdx")
                .unwrap_or(false)
                && !path.to_str().unwrap_or_default().contains("SUMMARY.md")
            {
                out.push(path);
            }
        }
    }
}

/// Lint a single file (frontmatter + fences + doctests) into a fresh error vector.
/// Returning an owned `Vec` keeps each unit of work independent for `rayon`.
fn lint_one_file(path: &Path, repo_root: &Path) -> Vec<LintError> {
    let mut errors = Vec::new();
    let content = vox_bounded_fs::read_utf8_path_capped(path).unwrap_or_default();
    lint_file(path, &content, repo_root, &mut errors);
    crate::pipeline::doctest::check_doctests(path, &content, &mut errors);
    errors
}

/// Run all lint checks on a single file's content.
pub(crate) fn lint_file(path: &Path, content: &str, repo_root: &Path, errors: &mut Vec<LintError>) {
    let mut fence_open = false;
    let mut fence_start_line = 0_usize;
    let mut fence_is_vox = false;
    if !content.trim_start().starts_with("---") {
        errors.push(LintError {
            file: path.to_owned(),
            line: 1,
            kind: LintKind::MissingFrontmatter,
        });
    } else {
        lint_duplicate_frontmatter(path, content, errors);
        lint_frontmatter(path, content, errors);
    }

    if content.contains("Official documentation for ")
        && content.contains("in the Vox programming language ecosystem.")
    {
        errors.push(LintError {
            file: path.to_owned(),
            line: 0,
            kind: LintKind::GenericDescription,
        });
    }

    for (idx, line) in content.lines().enumerate() {
        let line_no = idx + 1;
        let trimmed = line.trim_start();
        let backtick_count = trimmed.chars().take_while(|&c| c == '`').count();

        // A code fence marker is N backticks followed by an optional language tag (no spaces).
        // Inline code like `identifier` or `identifier` is prose text is NOT a fence.
        let after_backticks = &trimmed[backtick_count..];
        let rest_is_fence_like = after_backticks.trim().is_empty()
            || after_backticks
                .trim()
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_');

        if backtick_count >= 1 && rest_is_fence_like {
            if (1..3).contains(&backtick_count) {
                if !fence_open {
                    errors.push(LintError {
                        file: path.to_owned(),
                        line: line_no,
                        kind: LintKind::ShortCodeFence {
                            backticks: backtick_count,
                            at_line: line_no,
                        },
                    });
                }
            } else if backtick_count >= 3 {
                if fence_open {
                    fence_open = false;
                } else {
                    fence_open = true;
                    fence_start_line = line_no;
                    let lang = trimmed[backtick_count..].trim();
                    fence_is_vox = lang == "vox";
                    if lang.is_empty() && !skip_unlabeled_code_fence(path, repo_root) {
                        errors.push(LintError {
                            file: path.to_owned(),
                            line: line_no,
                            kind: LintKind::UnlabeledCodeFence { at_line: line_no },
                        });
                    }
                }
            }
        } else if fence_open && fence_is_vox {
        }

        // Also check for naked includes everywhere
        if !fence_open && trimmed.starts_with("{{#include ") {
            // Naked include check handles parsing anchors
            check_include_anchor(path, trimmed, line_no, errors);
        }
        // Fenced includes
        if fence_open && trimmed.starts_with("{{#include ") {
            check_include_anchor(path, trimmed, line_no, errors);
        }
    }

    if fence_open {
        errors.push(LintError {
            file: path.to_owned(),
            line: fence_start_line,
            kind: LintKind::UnclosedCodeFence,
        });
    }
}

fn yaml_introduces_second_frontmatter(lines: &[&str], dash_line_idx: usize) -> bool {
    let mut in_fence = false;
    // Horizontal rules also use `---`. Scan forward only outside fenced regions — otherwise
    // fields like `title:` / `description:` inside ```vox / ```rust examples trigger false positives.
    const MAX_RAW_LINES: usize = 120;
    let mut non_fence_seen = 0_usize;
    const MAX_NON_FENCE_LINES: usize = 24;

    for line in lines
        .iter()
        .copied()
        .skip(dash_line_idx.saturating_add(1))
        .take(MAX_RAW_LINES)
    {
        let trimmed_start = line.trim_start();
        if trimmed_start.starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        non_fence_seen += 1;
        if non_fence_seen > MAX_NON_FENCE_LINES {
            break;
        }
        // Real frontmatter keys sit at column 0. An indented match (e.g. a `title:`
        // object-literal key nested inside a JSX prop's backtick template literal in
        // .mdx — those aren't ``` fences, so the scan above doesn't skip them) is
        // embedded content, not a second frontmatter block.
        if line.starts_with(char::is_whitespace) {
            continue;
        }
        let t = line.trim_end();
        if t.starts_with("title:")
            || t.starts_with("category:")
            || t.starts_with("description:")
            || t.starts_with("status:")
        {
            return true;
        }
    }
    false
}

/// Detect a second YAML frontmatter block in the first ~200 lines (merge accidents).
fn lint_duplicate_frontmatter(path: &Path, content: &str, errors: &mut Vec<LintError>) {
    let lines: Vec<&str> = content.lines().take(200).collect();
    let mut dash_lines = Vec::new();
    let mut in_fence = false;
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if !in_fence && trimmed == "---" {
            dash_lines.push(i);
        }
    }
    // Normal doc: --- ... --- (open + close). Anything that looks like a *third* `---`
    // followed by YAML keys is a duplicate frontmatter block.
    if dash_lines.len() < 3 {
        return;
    }
    for &open_idx in dash_lines.iter().skip(2) {
        if yaml_introduces_second_frontmatter(&lines, open_idx) {
            errors.push(LintError {
                file: path.to_owned(),
                line: open_idx + 1,
                kind: LintKind::DuplicateFrontmatter {
                    second_block_start_line: open_idx + 1,
                },
            });
            return;
        }
    }
}

fn lint_frontmatter(path: &Path, content: &str, errors: &mut Vec<LintError>) {
    let Some(after_dash) = content.strip_prefix("---") else {
        return;
    };
    let Some(end) = after_dash.find("---") else {
        return;
    };
    let yaml = &after_dash[..end];
    let mut saw_category = false;
    let mut status: Option<String> = None;
    let mut training_eligible = false;
    let mut saw_training_rationale = false;

    for (idx, raw_line) in yaml.lines().enumerate() {
        let line_no = idx + 2;
        let line = raw_line.trim();
        if let Some(value) = line.strip_prefix("category:") {
            saw_category = true;
            let value = value.trim().trim_matches(|c| c == '"' || c == '\'');
            if !VALID_CATEGORIES.contains(&value) {
                errors.push(LintError {
                    file: path.to_owned(),
                    line: line_no,
                    kind: LintKind::UnknownCategory {
                        value: value.to_string(),
                    },
                });
            }
        } else if let Some(value) = line.strip_prefix("status:") {
            let value = value.trim().trim_matches(|c| c == '"' || c == '\'');
            status = Some(value.to_string());
            if !VALID_STATUS.contains(&value) {
                errors.push(LintError {
                    file: path.to_owned(),
                    line: line_no,
                    kind: LintKind::UnknownStatus {
                        value: value.to_string(),
                    },
                });
            }
        } else if let Some(value) = line.strip_prefix("schema_type:") {
            let val = value.trim().trim_matches(|c| c == '"' || c == '\'');
            if !VALID_SCHEMA_TYPES.contains(&val) {
                errors.push(LintError {
                    file: path.to_owned(),
                    line: line_no,
                    kind: LintKind::UnknownSchemaType {
                        value: val.to_string(),
                    },
                });
            }
        } else if let Some(value) = line.strip_prefix("training_eligible:") {
            let value = value.trim().trim_matches(|c| c == '"' || c == '\'');
            if value == "true" {
                training_eligible = true;
            }
        } else if line.starts_with("training_rationale:") {
            saw_training_rationale = true;
        } else if line.starts_with("last_updated:") {
            let is_archive = path
                .to_string_lossy()
                .replace('\\', "/")
                .contains("/archive/");
            if !is_archive {
                errors.push(LintError {
                    file: path.to_owned(),
                    line: line_no,
                    kind: LintKind::HandAuthoredLastUpdated,
                });
            }
        }
    }

    if !saw_category {
        errors.push(LintError {
            file: path.to_owned(),
            line: 1,
            kind: LintKind::MissingCategory,
        });
    }

    if training_eligible
        && !saw_training_rationale
        && let Some(st) = status
        && (st == "research" || st == "roadmap")
    {
        errors.push(LintError {
            file: path.to_owned(),
            line: 1,
            kind: LintKind::MissingTrainingRationale,
        });
    }
}

fn check_include_anchor(path: &Path, line: &str, line_no: usize, errors: &mut Vec<LintError>) {
    let Some(start) = line.find("{{#include ") else {
        return;
    };
    let Some(end) = line[start..].find("}}") else {
        return;
    };
    let include_body = &line[start + 11..start + end].trim();

    let parts: Vec<&str> = include_body.split(':').collect();
    let target_file = parts[0];
    let anchor = if parts.len() > 1 {
        Some(parts[1])
    } else {
        None
    };

    // Resolve target path relative to current file's dir
    let mut target_path = path.parent().unwrap_or(Path::new("")).to_path_buf();
    target_path.push(target_file);

    // Normalize path to some degree for reading, assuming docs/src as root of md files
    // But since target_file is usually `../../../examples/...` we just read it relative to cwd
    let content_res = vox_bounded_fs::read_utf8_path_capped(&target_path);
    if content_res.is_err() {
        errors.push(LintError {
            file: path.to_owned(),
            line: line_no,
            kind: LintKind::BrokenIncludeFile {
                file: target_file.to_string(),
            },
        });
        return;
    }
    if let Ok(content) = content_res {
        if let Some(anchor_name) = anchor {
            // Looking for `// ANCHOR: anchor_name`
            let needle = format!("ANCHOR: {}", anchor_name);
            if !content.contains(&needle) {
                errors.push(LintError {
                    file: path.to_owned(),
                    line: line_no,
                    kind: LintKind::BrokenIncludeAnchor {
                        file: target_file.to_string(),
                        anchor: anchor_name.to_string(),
                    },
                });
            }
        } else {
            // Whole file include. Warn if it has `// ---` at the top
            if content.starts_with("// ---") {
                errors.push(LintError {
                    file: path.to_owned(),
                    line: line_no,
                    kind: LintKind::WholeFileIncludeHasTrainingHeader {
                        file: target_file.to_string(),
                    },
                });
            }
        }
    }
}

/// README.md sections kept in sync with docs/src/index.mdx via matching
/// `<!-- ANCHOR: name --> ... <!-- ANCHOR_END: name -->` (README) and
/// `{/* SYNC-FROM-README: name */} ... {/* SYNC-END: name */}` (index.mdx) markers.
///
/// Only `why_vox` is still a true 1:1 sync target. `how_vox` and `tier_table`
/// were condensed on the homepage (2026-07-23 redesign, see
/// docs/superpowers/specs/2026-07-23-docs-homepage-maintainability-design.md)
/// — their full-detail canonical homes are docs/src/explanation/expl-architecture.md
/// and docs/src/reference/stability.md respectively, linked from the homepage,
/// not duplicated on it.
const SYNCED_BLOCKS: &[&str] = &["why_vox"];

fn mdx_sync_block(mdx: &str, name: &str) -> Option<String> {
    let start = format!("{{/* SYNC-FROM-README: {name} */}}");
    let end = format!("{{/* SYNC-END: {name} */}}");
    extract_marked_block(mdx, &start, &end)
}

/// Rewrite every occurrence of `needle` (a markdown-link opener like `"](crates/"`
/// or an HTML-attribute opener like `"=\"crates/"`, always ending in the bare repo-
/// relative prefix e.g. `crates/`) into its absolute GitHub URL form, choosing
/// `tree/main/` for a directory target (path ends in `/`) or `blob/main/` for a
/// file target (anything else) — README links to both crate directories and
/// individual files (e.g. a single `.rs` or `.yaml`) under the same `crates/`
/// prefix, and only the trailing slash distinguishes which GitHub URL shape is
/// correct. `open_len` is the length of the needle's non-path prefix (`"]("`. or
/// `"=\""` are both 2 bytes) and `terminator` is the character that closes the
/// link/attribute (`)` or `"`).
fn rewrite_repo_relative_links(s: &str, needle: &str, open_len: usize, terminator: char) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(idx) = rest.find(needle) {
        out.push_str(&rest[..idx + open_len]);
        let after = &rest[idx + open_len..];
        let Some(close_idx) = after.find(terminator) else {
            out.push_str(after);
            rest = "";
            break;
        };
        let path = &after[..close_idx];
        let github_prefix = if path.ends_with('/') {
            "https://github.com/vox-foundation/vox/tree/main/"
        } else {
            "https://github.com/vox-foundation/vox/blob/main/"
        };
        out.push_str(github_prefix);
        out.push_str(path);
        rest = &after[close_idx..];
    }
    out.push_str(rest);
    out
}

/// Apply the known, intentional README->index.mdx link/markup transforms so the
/// two blocks compare equal when they're genuinely in sync. Whitespace is also
/// collapsed, since line-wrap differences between the two files carry no meaning.
fn normalize_for_compare(s: &str) -> String {
    let transformed = s
        // Used by why_vox (its two figure images).
        .replace("docs/src/assets/", "./assets/")
        // The following `docs/src/X/` -> `./X/` swaps, the CHANGELOG.md case, and the
        // `examples/`/`crates/` rewrites below existed only for former how_vox/tier_table
        // content (decorator-reference links, ADR links, crate/example links, the
        // changelog link in tier_table's footer, ...) — why_vox never used any of them.
        // Now that SYNCED_BLOCKS is just `["why_vox"]` (how_vox/tier_table were condensed
        // off the homepage in the 2026-07-23 redesign — their full detail now lives at
        // docs/src/explanation/expl-architecture.md and docs/src/reference/stability.md,
        // linked rather than duplicated), these rules are dead in production: no content
        // any real lint_readme_sync() call sees exercises them. They're kept (rather than
        // deleted) because the `readme_sync_*` unit tests below still exercise them
        // directly via arbitrary test block names, to guard the general-purpose
        // normalization logic in case SYNCED_BLOCKS ever grows again.
        .replace("docs/src/reference/", "./reference/")
        .replace("docs/src/how-to/", "./how-to/")
        .replace("docs/src/architecture/", "./architecture/")
        .replace("docs/src/adr/", "./adr/")
        .replace("docs/src/explanation/", "./explanation/")
        // index.mdx is valid JSX, so self-closing void elements there (<img />,
        // <br />, ...) render the same content as README's plain-HTML, non-self-
        // closed forms (<img>, <br>). This is a blanket `" />"` -> `">"` fold —
        // it doesn't inspect which tag it's touching — so it relies on neither
        // file's synced blocks ever using a *meaningfully* self-closing tag
        // (e.g. a real empty custom element) whose self-closing-ness carries
        // content, not just markup-dialect noise. (Still used by why_vox today;
        // was also used by how_vox/tier_table before they were condensed.)
        .replace(" />", ">")
        // A bare top-level file link (e.g. `CHANGELOG.md`, relative to the repo
        // root since README lives there) always becomes a `blob/main/` GitHub URL.
        // Dead in production now (was tier_table-only) — see the dead-code note above.
        .replace(
            "](CHANGELOG.md)",
            "](https://github.com/vox-foundation/vox/blob/main/CHANGELOG.md)",
        );
    // Dead in production now (was how_vox/tier_table-only, crate and example links) —
    // see the dead-code note above.
    let transformed = rewrite_repo_relative_links(&transformed, "](crates/", 2, ')');
    let transformed = rewrite_repo_relative_links(&transformed, "=\"crates/", 2, '"');
    let transformed = rewrite_repo_relative_links(&transformed, "](examples/", 2, ')');
    transformed.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Core comparison logic, separated from file I/O so it's directly unit-testable.
fn lint_readme_sync_content(
    readme: &str,
    mdx: &str,
    mdx_path: &Path,
    blocks: &[&str],
    errors: &mut Vec<LintError>,
) {
    for &name in blocks {
        let Some(readme_block) = readme_anchor(readme, name) else {
            errors.push(LintError {
                file: mdx_path.to_owned(),
                line: 1,
                kind: LintKind::ReadmeSyncMissingAnchor {
                    block: name.to_string(),
                },
            });
            continue;
        };
        let Some(mdx_block) = mdx_sync_block(mdx, name) else {
            errors.push(LintError {
                file: mdx_path.to_owned(),
                line: 1,
                kind: LintKind::ReadmeSyncMissingBlock {
                    block: name.to_string(),
                },
            });
            continue;
        };
        if normalize_for_compare(&readme_block) != normalize_for_compare(&mdx_block) {
            errors.push(LintError {
                file: mdx_path.to_owned(),
                line: 1,
                kind: LintKind::ReadmeSyncDrift {
                    block: name.to_string(),
                },
            });
        }
    }
}

/// Whole-repo check: compares README.md against docs/src/index.mdx. Called once
/// per lint run (not per-file) from `mod.rs`. Reads plain repo-root-relative
/// paths directly (matching this tool's own convention of assuming the process
/// cwd is the repo root — see mod.rs's own `Path::new("docs/src")` with no
/// root-joining) rather than taking a caller-supplied repo_root, since the
/// natural helper for that (`repo_root_for_lint()`) is private to this module
/// and mod.rs can't call it across module boundaries.
pub(crate) fn lint_readme_sync(errors: &mut Vec<LintError>) {
    lint_readme_sync_paths(
        Path::new("README.md"),
        Path::new("docs/src/index.mdx"),
        errors,
    );
}

/// `lint_readme_sync`'s logic, with the two source paths as parameters so it's
/// directly unit-testable (e.g. pointing `readme_path` at a nonexistent file to
/// exercise the missing-source-file error without touching the real repo files).
fn lint_readme_sync_paths(readme_path: &Path, mdx_path: &Path, errors: &mut Vec<LintError>) {
    // A missing source file must be a loud lint error, not a quiet early return — the
    // whole point of this check is to never let drift go unnoticed, and a silent no-op
    // here (e.g. after one of the two files gets moved or renamed) would defeat that.
    let Ok(readme) = vox_bounded_fs::read_utf8_path_capped(readme_path) else {
        errors.push(LintError {
            file: readme_path.to_owned(),
            line: 1,
            kind: LintKind::ReadmeSyncSourceMissing {
                path: readme_path.display().to_string(),
            },
        });
        return;
    };
    let Ok(mdx) = vox_bounded_fs::read_utf8_path_capped(mdx_path) else {
        errors.push(LintError {
            file: mdx_path.to_owned(),
            line: 1,
            kind: LintKind::ReadmeSyncSourceMissing {
                path: mdx_path.display().to_string(),
            },
        });
        return;
    };
    lint_readme_sync_content(&readme, &mdx, mdx_path, SYNCED_BLOCKS, errors);
}

/// README.md's `tier_table` ANCHOR block still carries its own full copy of the
/// stability matrix; docs/src/reference/stability.md is the canonical, full-detail
/// home for the same content (linked from the condensed homepage summary — see
/// `SYNCED_BLOCKS`'s doc comment). Nothing keeps these two in sync automatically,
/// so this is a second, independent drift check with its own file pair, its own
/// extraction (stability.md has no ANCHOR-style markers, so it's found by its
/// fixed intro sentence instead), and its own link-scheme normalization (stability.md
/// lives one directory deeper than index.mdx, so the same `docs/src/X/` prefixes
/// resolve to different relative paths).
const TIER_TABLE_BLOCK: &str = "tier_table";

/// docs/src/reference/stability.md has no ANCHOR/SYNC markers of its own (it's a
/// plain reference page, not built to be excerpted) — its table content is instead
/// found by this fixed sentence, which opens both it and README's `tier_table`
/// ANCHOR block. Content from here to end-of-file is what's comparable to README's
/// ANCHOR block; everything before it (frontmatter, the `# Stability Matrix` H1,
/// the "See also" cross-link line) has no README counterpart.
const STABILITY_TABLE_MARKER: &str = "Vox is marching toward a production-hardened v1.0 release.";

fn stability_doc_table(content: &str) -> Option<String> {
    let idx = content.find(STABILITY_TABLE_MARKER)?;
    Some(content[idx..].trim().to_string())
}

/// Apply the known, intentional README->stability.md link/markup transforms so the
/// two blocks compare equal when they're genuinely in sync. Distinct from
/// `normalize_for_compare` because stability.md sits one directory deeper
/// (docs/src/reference/) than index.mdx (docs/src/) does, so the same
/// `docs/src/X/`-prefixed README links resolve to different relative paths from
/// each target file, plus stability.md wraps its table in an `## Stability Tiers`
/// heading where README uses a bold `**Stability Tiers:**` line.
fn normalize_for_stability_compare(s: &str) -> String {
    let transformed = s
        // Links to a sibling page in the same directory as stability.md itself
        // (docs/src/reference/) collapse the whole prefix, not just swap it.
        .replace("docs/src/reference/", "./")
        // docs/src/adr/ and docs/src/architecture/ are siblings of docs/src/reference/,
        // one level up from stability.md.
        .replace("docs/src/adr/", "../adr/")
        .replace("docs/src/architecture/", "../architecture/")
        // stability.md is a standalone page with a real `##` heading; README's ANCHOR
        // block uses a bold line instead since it's embedded in a larger page.
        .replace("## Stability Tiers", "**Stability Tiers:**")
        .replace(
            "](CHANGELOG.md)",
            "](https://github.com/vox-foundation/vox/blob/main/CHANGELOG.md)",
        );
    let transformed = rewrite_repo_relative_links(&transformed, "](crates/", 2, ')');
    transformed.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Core comparison logic, separated from file I/O so it's directly unit-testable.
fn lint_readme_stability_sync_content(
    readme: &str,
    stability: &str,
    stability_path: &Path,
    errors: &mut Vec<LintError>,
) {
    let Some(readme_block) = readme_anchor(readme, TIER_TABLE_BLOCK) else {
        errors.push(LintError {
            file: stability_path.to_owned(),
            line: 1,
            kind: LintKind::ReadmeStabilitySyncMissingAnchor {
                block: TIER_TABLE_BLOCK.to_string(),
            },
        });
        return;
    };
    let Some(stability_block) = stability_doc_table(stability) else {
        errors.push(LintError {
            file: stability_path.to_owned(),
            line: 1,
            kind: LintKind::ReadmeStabilitySyncMissingBlock {
                block: TIER_TABLE_BLOCK.to_string(),
            },
        });
        return;
    };
    if normalize_for_stability_compare(&readme_block)
        != normalize_for_stability_compare(&stability_block)
    {
        errors.push(LintError {
            file: stability_path.to_owned(),
            line: 1,
            kind: LintKind::ReadmeStabilitySyncDrift {
                block: TIER_TABLE_BLOCK.to_string(),
            },
        });
    }
}

/// Whole-repo check: compares README.md's `tier_table` ANCHOR block against
/// docs/src/reference/stability.md. Called once per lint run (not per-file) from
/// `mod.rs`, unconditionally (like `lint_readme_sync`) since it's two cheap file
/// reads, not a directory walk.
pub(crate) fn lint_readme_stability_sync(errors: &mut Vec<LintError>) {
    lint_readme_stability_sync_paths(
        Path::new("README.md"),
        Path::new("docs/src/reference/stability.md"),
        errors,
    );
}

/// `lint_readme_stability_sync`'s logic, with the two source paths as parameters so
/// it's directly unit-testable.
fn lint_readme_stability_sync_paths(
    readme_path: &Path,
    stability_path: &Path,
    errors: &mut Vec<LintError>,
) {
    let Ok(readme) = vox_bounded_fs::read_utf8_path_capped(readme_path) else {
        errors.push(LintError {
            file: readme_path.to_owned(),
            line: 1,
            kind: LintKind::ReadmeStabilitySyncSourceMissing {
                path: readme_path.display().to_string(),
            },
        });
        return;
    };
    let Ok(stability) = vox_bounded_fs::read_utf8_path_capped(stability_path) else {
        errors.push(LintError {
            file: stability_path.to_owned(),
            line: 1,
            kind: LintKind::ReadmeStabilitySyncSourceMissing {
                path: stability_path.display().to_string(),
            },
        });
        return;
    };
    lint_readme_stability_sync_content(&readme, &stability, stability_path, errors);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn duplicate_frontmatter_detects_second_yaml_block() {
        let mut errs = Vec::new();
        let md_path = Path::new("fixture.md");
        let repo = Path::new(".");
        let content = "---\ntitle: First\ncategory: architecture\n---\n---\ntitle: Second\ncategory: architecture\n---\n# Body\n";
        lint_file(md_path, content, repo, &mut errs);
        assert!(
            errs.iter()
                .any(|e| matches!(e.kind, LintKind::DuplicateFrontmatter { .. })),
            "expected duplicate frontmatter lint, got {errs:?}"
        );
    }

    #[test]
    fn hand_authored_last_updated_is_a_hard_error() {
        let mut errs = Vec::new();
        let md_path = Path::new("fixture.md");
        let content = "---\ntitle: \"Fixture\"\ndescription: \"A fixture page for testing.\"\ncategory: \"Concepts\"\nlast_updated: \"2026-05-05\"\n---\n\nBody.\n";
        lint_frontmatter(md_path, content, &mut errs);
        assert!(
            errs.iter()
                .any(|e| matches!(e.kind, LintKind::HandAuthoredLastUpdated)),
            "expected a HandAuthoredLastUpdated error, got: {errs:?}"
        );
    }

    #[test]
    fn frontmatter_without_last_updated_is_clean() {
        let mut errs = Vec::new();
        let md_path = Path::new("fixture.md");
        let content = "---\ntitle: \"Fixture\"\ndescription: \"A fixture page for testing.\"\ncategory: \"Concepts\"\n---\n\nBody.\n";
        lint_frontmatter(md_path, content, &mut errs);
        assert!(
            !errs
                .iter()
                .any(|e| matches!(e.kind, LintKind::HandAuthoredLastUpdated)),
            "did not expect a HandAuthoredLastUpdated error, got: {errs:?}"
        );
    }

    #[test]
    fn hand_authored_last_updated_is_exempt_under_archive() {
        let mut errs = Vec::new();
        let md_path = Path::new("docs/src/archive/old-doc.md");
        let content = "---\ntitle: \"Fixture\"\ndescription: \"A fixture page for testing.\"\ncategory: \"Concepts\"\nlast_updated: \"2026-05-05\"\n---\n\nBody.\n";
        lint_frontmatter(md_path, content, &mut errs);
        assert!(
            !errs
                .iter()
                .any(|e| matches!(e.kind, LintKind::HandAuthoredLastUpdated)),
            "did not expect a HandAuthoredLastUpdated error under archive/, got: {errs:?}"
        );
    }

    #[test]
    fn single_frontmatter_has_no_duplicate_diagnostic() {
        let mut errs = Vec::new();
        let md_path = Path::new("fixture.md");
        let repo = Path::new(".");
        let content = "---\ntitle: Only\ncategory: architecture\n---\n# Body\n";
        lint_file(md_path, content, repo, &mut errs);
        assert!(
            !errs
                .iter()
                .any(|e| matches!(e.kind, LintKind::DuplicateFrontmatter { .. }))
        );
    }

    #[test]
    fn duplicate_frontmatter_ignores_triple_dash_inside_code_fence() {
        let mut errs = Vec::new();
        let md_path = Path::new("fixture.md");
        let repo = Path::new(".");
        let content = "---\ntitle: Only\ncategory: architecture\n---\n\n```md\n---\ntitle: Template example\n---\n```\n# Body\n";
        lint_file(md_path, content, repo, &mut errs);
        assert!(
            !errs
                .iter()
                .any(|e| matches!(e.kind, LintKind::DuplicateFrontmatter { .. }))
        );
    }

    #[test]
    fn duplicate_frontmatter_ignores_yaml_like_lines_inside_vox_fence_after_horizontal_rule() {
        let mut errs = Vec::new();
        let md_path = Path::new("fixture.md");
        let repo = Path::new(".");
        let content = "---\ntitle: Doc\ncategory: reference\n---\n\n## Section\n\n---\n\n```vox\ntable Task {\n    title: str\n}\n```\n";
        lint_file(md_path, content, repo, &mut errs);
        assert!(
            !errs
                .iter()
                .any(|e| matches!(e.kind, LintKind::DuplicateFrontmatter { .. }))
        );
    }

    #[test]
    fn duplicate_frontmatter_ignores_indented_key_in_mdx_template_literal() {
        let mut errs = Vec::new();
        let mdx_path = Path::new("fixture.mdx");
        let repo = Path::new(".");
        // Regression: a JSX/MDX prop holding a backtick template literal isn't a ``` fence,
        // so an indented object-literal key like `    title: str` inside it must not be
        // mistaken for a second frontmatter block's `title:` key.
        let content = "---\ntitle: Doc\ncategory: reference\n---\n\n## Section\n\n---\n\n<Playground code={`table Task {\n    title: str\n}`} />\n";
        lint_file(mdx_path, content, repo, &mut errs);
        assert!(
            !errs
                .iter()
                .any(|e| matches!(e.kind, LintKind::DuplicateFrontmatter { .. })),
            "expected no duplicate-frontmatter false positive from indented title: inside JSX template literal, got: {errs:?}"
        );
    }

    #[test]
    fn suggest_maps_case_and_prefix_variants_to_canonical_category() {
        // The exact two wrong values that cost two push round-trips before this hint existed.
        assert_eq!(suggest("CI", VALID_CATEGORIES), Some("CI & Quality"));
        assert_eq!(suggest("ci", VALID_CATEGORIES), Some("CI & Quality"));
        assert_eq!(
            suggest("getting started", VALID_CATEGORIES),
            Some("Getting Started")
        );
    }

    #[test]
    fn suggest_maps_typo_to_nearest_status_by_edit_distance() {
        assert_eq!(suggest("experimentl", VALID_STATUS), Some("experimental"));
        assert_eq!(suggest("rodmap", VALID_STATUS), Some("roadmap"));
    }

    #[test]
    fn suggest_returns_none_for_unrelated_garbage() {
        assert_eq!(suggest("zzzzzzzzzzzz", VALID_STATUS), None);
        assert_eq!(suggest("", VALID_CATEGORIES), None);
    }

    #[test]
    fn parallel_collect_surfaces_every_file_error_in_one_pass() {
        use std::io::Write;
        let dir = std::env::temp_dir().join(format!("voxdoclint-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        for (name, body) in [
            ("a.md", "---\ntitle: A\ncategory: nonsense\n---\n# A\n"),
            (
                "b.md",
                "---\ntitle: B\nstatus: bogus\ncategory: Concepts\n---\n# B\n",
            ),
        ] {
            let mut f = fs::File::create(dir.join(name)).unwrap();
            f.write_all(body.as_bytes()).unwrap();
        }
        let mut errors = Vec::new();
        collect_lint_errors_target_with_root(&dir, &mut errors, Path::new("."));
        assert!(
            errors
                .iter()
                .any(|e| matches!(e.kind, LintKind::UnknownCategory { .. })),
            "expected category error from a.md, got {errors:?}"
        );
        assert!(
            errors
                .iter()
                .any(|e| matches!(e.kind, LintKind::UnknownStatus { .. })),
            "expected status error from b.md, got {errors:?}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn skip_unlabeled_fence_suppression_matches_plan_and_design_paths() {
        assert!(skip_unlabeled_code_fence_rel(
            "docs/src/architecture/2026-05-08-crate-org-followup-plan.md"
        ));
        assert!(skip_unlabeled_code_fence_rel(
            "docs/src/architecture/2026-05-08-crate-org-followup-design.md"
        ));
        assert!(skip_unlabeled_code_fence_rel(
            "docs/src/architecture/mesh-phase3-vcs-gossip-plan-2026.md"
        ));
        assert!(skip_unlabeled_code_fence_rel(
            "docs/superpowers/plans/ci/2026-05-03-local-ci-pre-push-and-job-split.md"
        ));
        assert!(skip_unlabeled_code_fence_rel(
            "docs/src/architecture/planning-meta/02-fast-llm-instruction-plan.md"
        ));
        assert!(skip_unlabeled_code_fence_rel(
            "docs/src/architecture/data-storage-ssot-2026.md"
        ));
        assert!(!skip_unlabeled_code_fence_rel("docs/src/reference/cli.md"));
        assert!(skip_unlabeled_code_fence_rel(
            "docs/src/archive/research-2026-q1/example.md"
        ));
    }

    #[test]
    fn gather_md_files_includes_mdx() {
        let tmp = std::env::temp_dir().join("vox_doc_pipeline_mdx_test");
        let _ = std::fs::create_dir_all(&tmp);
        let mdx_path = tmp.join("index.mdx");
        std::fs::write(&mdx_path, "---\ntitle: \"x\"\n---\nbody").unwrap();
        let mut out = Vec::new();
        gather_md_files(&tmp, &mut out);
        assert!(
            out.iter().any(|p| p == &mdx_path),
            "expected gather_md_files to include index.mdx, got: {out:?}"
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn readme_sync_detects_matching_block_after_known_transforms() {
        let readme = "<!-- ANCHOR: demo -->\nSee [the crate](crates/vox-db/) and ![x](docs/src/assets/pic.png).\n<!-- ANCHOR_END: demo -->\n";
        let mdx = "{/* SYNC-FROM-README: demo */}\nSee [the crate](https://github.com/vox-foundation/vox/tree/main/crates/vox-db/) and ![x](./assets/pic.png).\n{/* SYNC-END: demo */}\n";
        let mut errors = Vec::new();
        lint_readme_sync_content(
            readme,
            mdx,
            Path::new("docs/src/index.mdx"),
            &["demo"],
            &mut errors,
        );
        assert!(
            errors.is_empty(),
            "expected no drift after known transforms, got: {errors:?}"
        );
    }

    #[test]
    fn readme_sync_flags_real_drift() {
        let readme =
            "<!-- ANCHOR: demo -->\nSee [the crate](crates/vox-db/).\n<!-- ANCHOR_END: demo -->\n";
        let mdx = "{/* SYNC-FROM-README: demo */}\nSee [a totally different crate](https://github.com/vox-foundation/vox/tree/main/crates/vox-other/).\n{/* SYNC-END: demo */}\n";
        let mut errors = Vec::new();
        lint_readme_sync_content(
            readme,
            mdx,
            Path::new("docs/src/index.mdx"),
            &["demo"],
            &mut errors,
        );
        assert!(
            errors
                .iter()
                .any(|e| matches!(&e.kind, LintKind::ReadmeSyncDrift { block } if block == "demo")),
            "expected a ReadmeSyncDrift for 'demo', got: {errors:?}"
        );
    }

    #[test]
    fn readme_sync_flags_missing_mdx_block() {
        let readme = "<!-- ANCHOR: demo -->\nSee the crate.\n<!-- ANCHOR_END: demo -->\n";
        let mdx = "no sync block here at all\n";
        let mut errors = Vec::new();
        lint_readme_sync_content(
            readme,
            mdx,
            Path::new("docs/src/index.mdx"),
            &["demo"],
            &mut errors,
        );
        assert!(
            errors.iter().any(
                |e| matches!(&e.kind, LintKind::ReadmeSyncMissingBlock { block } if block == "demo")
            ),
            "expected a ReadmeSyncMissingBlock for 'demo', got: {errors:?}"
        );
    }

    #[test]
    fn readme_sync_flags_missing_source_file() {
        let mut errors = Vec::new();
        lint_readme_sync_paths(
            Path::new("this/path/does/not/exist/README.md"),
            Path::new("docs/src/index.mdx"),
            &mut errors,
        );
        assert!(
            errors.iter().any(|e| matches!(
                &e.kind,
                LintKind::ReadmeSyncSourceMissing { path } if path.contains("README.md")
            )),
            "expected a ReadmeSyncSourceMissing error for the unreadable README, got: {errors:?}"
        );
        assert!(
            errors
                .iter()
                .any(|e| e.file.to_string_lossy().contains("README.md")),
            "error's `file` field should point at the actually-missing README, not index.mdx, got: {errors:?}"
        );
    }

    #[test]
    fn readme_sync_detects_matching_block_via_html_attribute_crate_link() {
        // Exercises the `="crates/` rewrite branch specifically (as opposed to the
        // markdown `](crates/` form covered by the other readme_sync_ tests) — this is
        // the shape README's how_vox content actually uses for inline `<a href="...">`
        // links, e.g. `<a href="crates/vox-workflow-runtime/">`.
        let readme = "<!-- ANCHOR: demo -->\nSee <a href=\"crates/vox-db/\">vox-db</a>.\n<!-- ANCHOR_END: demo -->\n";
        let mdx = "{/* SYNC-FROM-README: demo */}\nSee <a href=\"https://github.com/vox-foundation/vox/tree/main/crates/vox-db/\">vox-db</a>.\n{/* SYNC-END: demo */}\n";
        let mut errors = Vec::new();
        lint_readme_sync_content(
            readme,
            mdx,
            Path::new("docs/src/index.mdx"),
            &["demo"],
            &mut errors,
        );
        assert!(
            errors.is_empty(),
            "expected no drift after the html-attribute crate-link transform, got: {errors:?}"
        );
    }

    // --- README <-> docs/src/reference/stability.md drift check ---
    //
    // Separate from the readme_sync_* tests above: this compares README's `tier_table`
    // ANCHOR block against docs/src/reference/stability.md's table content (a different
    // file, with its own frontmatter/H1/"See also" preamble and its own link-scheme, since
    // it lives one directory deeper than index.mdx).

    #[test]
    fn readme_stability_sync_detects_matching_block_after_known_transforms() {
        let readme = "<!-- ANCHOR: tier_table -->\nVox is marching toward a production-hardened v1.0 release.\n\n| Feature Area | Status | Context & Maturity |\n|:---|:---|:---|\n| Database Engine | 🔵 Stable | [vox-db](crates/vox-db/) with Turso integration. |\n| Socrates Research | 🟡 Preview | [Socrates protocol](docs/src/reference/socrates-protocol.md) for fact-checking. |\n| Orchestrator Core | 🔵 Stable | See [Superpowers](docs/src/architecture/superpowers-ssot.md). |\n\n**Stability Tiers:**\n- 🔵 **Stable**: API locked.\n\nHistory: [`CHANGELOG.md`](CHANGELOG.md).\n<!-- ANCHOR_END: tier_table -->\n";
        let stability = "---\ntitle: \"Stability Matrix\"\n---\n\n# Stability Matrix\n\nSee also: [v1.0 release criteria](../architecture/v1-release-criteria.md).\n\nVox is marching toward a production-hardened v1.0 release.\n\n| Feature Area | Status | Context & Maturity |\n|:---|:---|:---|\n| Database Engine | 🔵 Stable | [vox-db](https://github.com/vox-foundation/vox/tree/main/crates/vox-db/) with Turso integration. |\n| Socrates Research | 🟡 Preview | [Socrates protocol](./socrates-protocol.md) for fact-checking. |\n| Orchestrator Core | 🔵 Stable | See [Superpowers](../architecture/superpowers-ssot.md). |\n\n## Stability Tiers\n\n- 🔵 **Stable**: API locked.\n\nHistory: [`CHANGELOG.md`](https://github.com/vox-foundation/vox/blob/main/CHANGELOG.md).\n";
        let mut errors = Vec::new();
        lint_readme_stability_sync_content(
            readme,
            stability,
            Path::new("docs/src/reference/stability.md"),
            &mut errors,
        );
        assert!(
            errors.is_empty(),
            "expected no drift after known transforms, got: {errors:?}"
        );
    }

    #[test]
    fn readme_stability_sync_flags_real_drift() {
        let readme = "<!-- ANCHOR: tier_table -->\nVox is marching toward a production-hardened v1.0 release.\n\n| Database Engine | 🔵 Stable |\n<!-- ANCHOR_END: tier_table -->\n";
        let stability = "# Stability Matrix\n\nVox is marching toward a production-hardened v1.0 release.\n\n| Database Engine | 🟠 Emergent |\n";
        let mut errors = Vec::new();
        lint_readme_stability_sync_content(
            readme,
            stability,
            Path::new("docs/src/reference/stability.md"),
            &mut errors,
        );
        assert!(
            errors.iter().any(|e| matches!(
                &e.kind,
                LintKind::ReadmeStabilitySyncDrift { block } if block == "tier_table"
            )),
            "expected a ReadmeStabilitySyncDrift, got: {errors:?}"
        );
    }

    #[test]
    fn readme_stability_sync_flags_missing_stability_block() {
        let readme = "<!-- ANCHOR: tier_table -->\nVox is marching toward a production-hardened v1.0 release.\n<!-- ANCHOR_END: tier_table -->\n";
        let stability = "# Stability Matrix\n\nno recognizable table marker here at all\n";
        let mut errors = Vec::new();
        lint_readme_stability_sync_content(
            readme,
            stability,
            Path::new("docs/src/reference/stability.md"),
            &mut errors,
        );
        assert!(
            errors.iter().any(|e| matches!(
                &e.kind,
                LintKind::ReadmeStabilitySyncMissingBlock { block } if block == "tier_table"
            )),
            "expected a ReadmeStabilitySyncMissingBlock, got: {errors:?}"
        );
    }

    #[test]
    fn readme_stability_sync_flags_missing_readme_anchor() {
        let readme = "no ANCHOR block here at all\n";
        let stability =
            "# Stability Matrix\n\nVox is marching toward a production-hardened v1.0 release.\n";
        let mut errors = Vec::new();
        lint_readme_stability_sync_content(
            readme,
            stability,
            Path::new("docs/src/reference/stability.md"),
            &mut errors,
        );
        assert!(
            errors.iter().any(|e| matches!(
                &e.kind,
                LintKind::ReadmeStabilitySyncMissingAnchor { block } if block == "tier_table"
            )),
            "expected a ReadmeStabilitySyncMissingAnchor, got: {errors:?}"
        );
    }

    #[test]
    fn readme_stability_sync_flags_missing_source_file() {
        let mut errors = Vec::new();
        lint_readme_stability_sync_paths(
            Path::new("this/path/does/not/exist/README.md"),
            Path::new("docs/src/reference/stability.md"),
            &mut errors,
        );
        assert!(
            errors.iter().any(|e| matches!(
                &e.kind,
                LintKind::ReadmeStabilitySyncSourceMissing { path } if path.contains("README.md")
            )),
            "expected a ReadmeStabilitySyncSourceMissing error for the unreadable README, got: {errors:?}"
        );
        assert!(
            errors
                .iter()
                .any(|e| e.file.to_string_lossy().contains("README.md")),
            "error's `file` field should point at the actually-missing README, not stability.md, got: {errors:?}"
        );
    }
}
