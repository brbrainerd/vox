//! Markdown lint passes for docs under `docs/src/`.
//!
// SCIENTIA — contracts/scientia/*.schema.json
// Stable serde names live in vox-research-events::schema_types; typify exhaust in schema_types::generated.
// Regenerate: cargo run -p vox-scientia-jsonschema-codegen
// Hand-maintained name map (lint reminder): discovery-signal → DiscoverySignal; finding-candidate.v1 → FindingCandidateV1;
// novelty-evidence-bundle.v1 → NoveltyEvidenceBundle; evidence-pack.v1 → EvidencePackV1; worthiness-signals.v2 → WorthinessSignalsV2.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::NaiveDate;

use super::types::{LintError, LintKind};

// These must match the `sections` array in contracts/documentation/docs-sidebar-section-order.v1.json.
// Display-label format (e.g. "Language Reference") is canonical; slug aliases are kept for grep-safety
// but all new files must use the display-label form.
const VALID_CATEGORIES: &[&str] = &[
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

const VALID_STATUS: &[&str] = &[
    "approved",
    "current",
    "experimental",
    "legacy",
    "research",
    "roadmap",
    "deprecated",
];

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
    if target.is_file() {
        if target.extension().map(|e| e == "md").unwrap_or(false) {
            let rel = target.to_str().unwrap_or_default();
            if rel.contains("SUMMARY.md") {
                return;
            }
            let content =
                vox_bounded_fs::read_utf8_path_capped(target).unwrap_or_else(|_| String::new());
            lint_file(target, &content, repo_root, errors);
            crate::pipeline::doctest::check_doctests(target, &content, errors);
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
                collect_lint_errors_target_with_root(&path, errors, repo_root);
            } else if path.extension().map(|e| e == "md").unwrap_or(false) {
                let rel = path.to_str().unwrap_or_default();
                if rel.contains("SUMMARY.md") {
                    continue;
                }
                let content =
                    vox_bounded_fs::read_utf8_path_capped(&path).unwrap_or_else(|_| String::new());
                lint_file(&path, &content, repo_root, errors);
                crate::pipeline::doctest::check_doctests(&path, &content, errors);
            }
        }
    }
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
        lint_last_updated_vs_git(path, content, repo_root, errors);
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
        let t = line.trim();
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

fn git_last_commit_date(repo_root: &Path, rel_file: &str) -> Option<NaiveDate> {
    let out = // vox-arch-check: allow git-exec
        Command::new("git")
        .current_dir(repo_root)
        .args(["log", "-1", "--format=%cs", "--", rel_file])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()
}

fn lint_last_updated_vs_git(
    path: &Path,
    content: &str,
    repo_root: &Path,
    errors: &mut Vec<LintError>,
) {
    let Some(after_open) = content.strip_prefix("---") else {
        return;
    };
    let Some(end) = after_open.find("---") else {
        return;
    };
    let yaml = &after_open[..end];
    let training = yaml.contains("\ntraining_eligible: true")
        || yaml.contains("\ntraining_eligible: \"true\"");
    if !training {
        return;
    }
    let mut declared: Option<NaiveDate> = None;
    for raw_line in yaml.lines() {
        let line = raw_line.trim();
        if let Some(value) = line.strip_prefix("last_updated:") {
            let value = value.trim().trim_matches(|c| c == '"' || c == '\'');
            declared = NaiveDate::parse_from_str(value, "%Y-%m-%d").ok();
            break;
        }
    }
    let Some(decl) = declared else {
        return;
    };
    let Ok(rel) = path.strip_prefix(repo_root) else {
        return;
    };
    let rel_str = rel.to_string_lossy().replace('\\', "/");
    let Some(git_tip) = git_last_commit_date(repo_root, rel_str.trim_start_matches("./")) else {
        return;
    };
    let delta = (decl - git_tip).num_days().abs();
    if delta > 30 {
        errors.push(LintError {
            file: path.to_owned(),
            line: 1,
            kind: LintKind::LastUpdatedStale {
                declared: decl.to_string(),
                git_tip: git_tip.to_string(),
                delta_days: delta,
            },
        });
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
            const VALID_SCHEMA_TYPES: &[&str] =
                &["HowTo", "FAQPage", "TechArticle", "SoftwareSourceCode"];
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
        let content = "---\ntitle: Doc\ncategory: reference\n---\n\n## Section\n\n---\n\n```vox\n@table type Task {\n    title: str\n}\n```\n";
        lint_file(md_path, content, repo, &mut errs);
        assert!(
            !errs
                .iter()
                .any(|e| matches!(e.kind, LintKind::DuplicateFrontmatter { .. }))
        );
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
}
