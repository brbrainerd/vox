//! Documentation linter for `docs/src/`. Checks frontmatter, code fences,
//! training rationale, and embedded Vox doctests.

pub mod anchors;
pub mod doctest;
mod lint;
pub mod types;

use std::fs;
use std::path::{Path, PathBuf};

use lint::{collect_lint_errors, collect_lint_errors_target};
use types::{LintError, LintKind};

/// Render a `" Did you mean \"X\"?"` fragment (note the leading space) when a close
/// valid value exists, or an empty string otherwise. Lets a rejected frontmatter
/// value be fixed in one pass instead of guessing case/punctuation.
fn suggestion_hint(value: &str, candidates: &[&str]) -> String {
    match lint::suggest(value, candidates) {
        Some(best) => format!(" Did you mean {best:?}?"),
        None => String::new(),
    }
}

/// One-line remediation hint per error kind. Printed in the grouped summary so a
/// human — or a fan-out of fix agents — knows the canonical workflow for each class
/// of failure without spelunking the linter source.
fn workflow_for_kind(kind: &LintKind) -> &'static str {
    match kind {
        LintKind::UnknownCategory { .. } => {
            "set `category:` to a canonical label (SSOT: contracts/documentation/docs-sidebar-section-order.v1.json)"
        }
        LintKind::UnknownStatus { .. } => "set `status:` to a canonical lifecycle value",
        LintKind::UnknownSchemaType { .. } => "set `schema_type:` to a supported schema.org type",
        LintKind::MissingFrontmatter | LintKind::MissingCategory => {
            "add the YAML frontmatter block (title/description/category) — see AGENTS.md §Authored Markdown Frontmatter"
        }
        LintKind::MissingTrainingRationale => {
            "add `training_rationale:` next to `training_eligible: true`"
        }
        LintKind::GenericDescription => {
            "replace the template `description:` with a hand-written one"
        }
        LintKind::UnclosedCodeFence | LintKind::ShortCodeFence { .. } => {
            "balance the ``` code fences (mdBook requires exactly 3 backticks)"
        }
        LintKind::UnlabeledCodeFence { .. } => "add a language tag to the ``` fence (e.g. ```bash)",
        LintKind::DuplicateFrontmatter { .. } => {
            "delete the second `---` frontmatter block (usually a merge accident)"
        }
        LintKind::BrokenIncludeFile { .. } | LintKind::BrokenIncludeAnchor { .. } => {
            "fix the `{{#include}}` target path/anchor"
        }
        LintKind::WholeFileIncludeHasTrainingHeader { .. } => {
            "use `{{#include file:display}}` to strip the `// ---` training header"
        }
        LintKind::DocTestFailed { .. } => {
            "make the ```vox block compile, or annotate it `// vox:skip` with a reason"
        }
        LintKind::HandAuthoredLastUpdated => "remove the hand-authored last_updated key",
        LintKind::ReadmeSyncDrift { .. } => {
            "re-sync the block with README.md, or fix the ANCHOR if content genuinely changed"
        }
        LintKind::ReadmeSyncMissingBlock { .. } => {
            "add the matching SYNC-FROM-README block to index.mdx"
        }
        LintKind::ReadmeSyncMissingAnchor { .. } => "add the matching ANCHOR block to README.md",
        LintKind::ReadmeSyncSourceMissing { .. } => {
            "restore README.md and docs/src/index.mdx — the sync check can't run without both"
        }
    }
}

/// Stable short label for grouping errors by class in the summary.
fn kind_label(kind: &LintKind) -> &'static str {
    match kind {
        LintKind::UnclosedCodeFence => "unclosed-code-fence",
        LintKind::ShortCodeFence { .. } => "short-code-fence",
        LintKind::GenericDescription => "generic-description",
        LintKind::MissingFrontmatter => "missing-frontmatter",
        LintKind::MissingCategory => "missing-category",
        LintKind::MissingTrainingRationale => "missing-training-rationale",
        LintKind::UnknownCategory { .. } => "unknown-category",
        LintKind::UnknownStatus { .. } => "unknown-status",
        LintKind::UnknownSchemaType { .. } => "unknown-schema-type",
        LintKind::BrokenIncludeAnchor { .. } => "broken-include-anchor",
        LintKind::BrokenIncludeFile { .. } => "broken-include-file",
        LintKind::WholeFileIncludeHasTrainingHeader { .. } => "whole-file-include-training-header",
        LintKind::DocTestFailed { .. } => "doctest-failed",
        LintKind::UnlabeledCodeFence { .. } => "unlabeled-code-fence",
        LintKind::DuplicateFrontmatter { .. } => "duplicate-frontmatter",
        LintKind::HandAuthoredLastUpdated => "hand-authored-last-updated",
        LintKind::ReadmeSyncDrift { .. } => "readme-sync-drift",
        LintKind::ReadmeSyncMissingBlock { .. } => "readme-sync-missing-block",
        LintKind::ReadmeSyncMissingAnchor { .. } => "readme-sync-missing-anchor",
        LintKind::ReadmeSyncSourceMissing { .. } => "readme-sync-source-missing",
    }
}

/// Print a grouped, deduplicated summary of every error class with a remediation
/// hint, so all failures can be fanned out and fixed in parallel.
fn print_grouped_summary(errors: &[LintError]) {
    use std::collections::BTreeMap;
    let mut groups: BTreeMap<&'static str, (usize, &LintKind)> = BTreeMap::new();
    for e in errors {
        let entry = groups.entry(kind_label(&e.kind)).or_insert((0, &e.kind));
        entry.0 += 1;
    }
    eprintln!(
        "\n── summary: {} error(s) across {} class(es) ──",
        errors.len(),
        groups.len()
    );
    for (label, (count, sample)) in &groups {
        eprintln!("  {count:>3}× {label:<34} → {}", workflow_for_kind(sample));
    }
}

/// Resolve `--paths` / `--paths=` arguments relative to `docs_src`.
///
/// Returns `Ok(empty)` when the flag is absent (lint the whole tree).
/// Returns `Err(missing)` when any resolved path does not exist — a silent empty
/// match used to print a false green (e.g. `--paths docs/src/archive/foo.md`
/// joined onto `docs/src/` → `docs/src/docs/src/archive/foo.md`).
fn parse_paths_arg(args: &[String], docs_src: &Path) -> Result<Vec<PathBuf>, Vec<PathBuf>> {
    let mut out = Vec::new();
    let mut i = 0_usize;
    while i < args.len() {
        let arg = &args[i];
        let value_opt = if let Some(v) = arg.strip_prefix("--paths=") {
            Some(v.to_string())
        } else if arg == "--paths" {
            args.get(i + 1).cloned()
        } else {
            None
        };
        if let Some(value) = value_opt {
            for raw in value.split(',') {
                let trimmed = raw.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let p = PathBuf::from(trimmed);
                let resolved = if p.is_absolute() { p } else { docs_src.join(p) };
                out.push(resolved);
            }
        }
        i += 1;
    }
    let missing: Vec<PathBuf> = out.iter().filter(|p| !p.exists()).cloned().collect();
    if missing.is_empty() {
        Ok(out)
    } else {
        Err(missing)
    }
}

fn try_autofix_status_draft(path: &Path) -> bool {
    if !path.is_file() || path.extension().and_then(|e| e.to_str()) != Some("md") {
        return false;
    }
    let Ok(raw) = vox_bounded_fs::read_utf8_path_capped(path) else {
        return false;
    };
    let Some(after_open) = raw.strip_prefix("---\n") else {
        return false;
    };
    let Some(end) = after_open.find("\n---") else {
        return false;
    };
    let frontmatter = &after_open[..end];
    if !frontmatter.contains("status: \"draft\"") && !frontmatter.contains("status: draft") {
        return false;
    }
    let updated = raw
        .replace("status: \"draft\"", "status: \"roadmap\"")
        .replace("status: draft", "status: roadmap");
    if updated == raw {
        return false;
    }
    fs::write(path, updated).is_ok()
}

fn collect_md_files(target: &Path, out: &mut Vec<PathBuf>) {
    if target.is_file() {
        if target.extension().and_then(|e| e.to_str()) == Some("md") {
            out.push(target.to_path_buf());
        }
        return;
    }
    if !target.is_dir() {
        return;
    }
    if let Ok(entries) = fs::read_dir(target) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                collect_md_files(&p, out);
            } else if p.extension().and_then(|e| e.to_str()) == Some("md") {
                out.push(p);
            }
        }
    }
}

/// Run the doc pipeline: lint source markdown and optionally export corpus.
///
/// SUMMARY.md and feed.xml are no longer generated here — the Starlight site
/// builds the sidebar and RSS from frontmatter directly at Astro build time.
pub fn run() {
    let args: Vec<String> = std::env::args().collect();
    let fix_mode = args.contains(&"--fix".to_string());
    let corpus_mode = args
        .windows(2)
        .any(|w| w[0] == "--mode" && w[1] == "corpus");

    // Legacy flags accepted but ignored (no longer meaningful)
    let _check_mode = args.contains(&"--check".to_string());
    let _lint_only = args.contains(&"--lint-only".to_string());

    let docs_src = Path::new("docs/src");
    if !docs_src.exists() {
        eprintln!("Error: docs/src/ not found. Run from repo root.");
        std::process::exit(1);
    }

    if corpus_mode {
        let mut md_files = Vec::new();
        collect_md_files(docs_src, &mut md_files);
        let mut corpus_output = String::new();
        for f in md_files {
            if let Ok(content) = vox_bounded_fs::read_utf8_path_capped(&f) {
                let item = serde_json::json!({
                    "path": f.to_string_lossy().to_string(),
                    "content": content
                });
                corpus_output.push_str(&item.to_string());
                corpus_output.push('\n');
            }
        }
        let out_path = docs_src.join("corpus.jsonl");
        fs::write(&out_path, corpus_output).expect("Failed to write corpus.jsonl");
        println!("Successfully generated docs/src/corpus.jsonl");
        return;
    }

    let lint_targets = match parse_paths_arg(&args, docs_src) {
        Ok(paths) => paths,
        Err(missing) => {
            for p in &missing {
                eprintln!(
                    "Error: --paths did not match any file or directory: {}",
                    p.display()
                );
            }
            std::process::exit(1);
        }
    };
    if !lint_targets.is_empty() {
        for t in &lint_targets {
            println!("vox-doc-pipeline: linting {}", t.display());
        }
    }
    if fix_mode {
        let mut fixed = 0_usize;
        if lint_targets.is_empty() {
            let mut md_files = Vec::new();
            collect_md_files(docs_src, &mut md_files);
            for f in md_files {
                if try_autofix_status_draft(&f) {
                    fixed += 1;
                }
            }
        } else {
            for t in &lint_targets {
                if t.is_file() {
                    if try_autofix_status_draft(t) {
                        fixed += 1;
                    }
                    continue;
                }
                let mut md_files = Vec::new();
                collect_md_files(t, &mut md_files);
                for f in md_files {
                    if try_autofix_status_draft(&f) {
                        fixed += 1;
                    }
                }
            }
        }
        if fixed > 0 {
            eprintln!("Applied {} frontmatter status auto-fix(es).", fixed);
        }
    }

    let mut lint_errors: Vec<LintError> = Vec::new();
    if lint_targets.is_empty() {
        collect_lint_errors(docs_src, &mut lint_errors);
    } else {
        for target in &lint_targets {
            collect_lint_errors_target(target, &mut lint_errors);
        }
    }
    // Always run, even in scoped mode: it's two cheap file reads (README.md,
    // index.mdx), not a directory walk, so there's no cost reason to gate it
    // behind an unscoped run — and gating it was the reason the local
    // (scoped-by-default) `vox ci pre-push` never caught README<->homepage
    // drift, only CI did.
    lint::lint_readme_sync(&mut lint_errors);

    if !lint_errors.is_empty() {
        eprintln!("\n── vox-doc-pipeline: doc lint errors ──────────────────────────────");
        for e in &lint_errors {
            let rel = e.file.strip_prefix(docs_src).unwrap_or(&e.file);
            match &e.kind {
                LintKind::UnclosedCodeFence => {
                    eprintln!(
                        "  ERROR  {} — unclosed code fence (file ends with open ```)",
                        rel.display()
                    );
                }
                LintKind::ShortCodeFence { backticks, at_line } => {
                    eprintln!(
                        "  ERROR  {}:{} — code fence has only {} backtick(s); mdBook requires 3 (```)",
                        rel.display(),
                        at_line,
                        backticks
                    );
                }
                LintKind::GenericDescription => {
                    eprintln!(
                        "  ERROR  {} — description is the auto-generated template text; replace with a specific, hand-written description",
                        rel.display()
                    );
                }
                LintKind::MissingFrontmatter => {
                    eprintln!(
                        "  WARN   {} — no YAML frontmatter block; add title/description/category",
                        rel.display()
                    );
                }
                LintKind::MissingCategory => {
                    eprintln!(
                        "  WARN   {} — frontmatter is missing `category:`; docs nav will fall back to folder-based placement",
                        rel.display()
                    );
                }
                LintKind::MissingTrainingRationale => {
                    eprintln!(
                        "  ERROR  {} — `training_eligible: true` requires `training_rationale:` frontmatter on research/roadmap pages",
                        rel.display()
                    );
                }
                LintKind::UnknownCategory { value } => {
                    eprintln!(
                        "  ERROR  {} — unknown category {:?}.{} Valid: {}",
                        rel.display(),
                        value,
                        suggestion_hint(value, lint::VALID_CATEGORIES),
                        lint::VALID_CATEGORIES.join(" | "),
                    );
                }
                LintKind::UnknownStatus { value } => {
                    eprintln!(
                        "  ERROR  {} — unknown status {:?}.{} Valid: {}",
                        rel.display(),
                        value,
                        suggestion_hint(value, lint::VALID_STATUS),
                        lint::VALID_STATUS.join(" | "),
                    );
                }
                LintKind::UnknownSchemaType { value } => {
                    eprintln!(
                        "  ERROR  {} — unknown schema_type {:?}.{} Valid: {}",
                        rel.display(),
                        value,
                        suggestion_hint(value, lint::VALID_SCHEMA_TYPES),
                        lint::VALID_SCHEMA_TYPES.join(" | "),
                    );
                }
                LintKind::BrokenIncludeFile { file } => {
                    eprintln!(
                        "  ERROR  {} — `{{{{#include {}}}}}` target file not found. The Starlight build will fail at runtime.",
                        rel.display(),
                        file
                    );
                }
                LintKind::BrokenIncludeAnchor { file, anchor } => {
                    eprintln!(
                        "  ERROR  {} — unresolved anchor `:{}` in `{{{{#include ...}}}}` (target {}). Check if REGION exists in the golden file.",
                        rel.display(),
                        anchor,
                        file
                    );
                }
                LintKind::WholeFileIncludeHasTrainingHeader { file } => {
                    eprintln!(
                        "  ERROR  {} — whole-file include pulls in `// ---` training metadata from {}. Use `{{{{#include {}:display}}}}`.",
                        rel.display(),
                        file,
                        file
                    );
                }
                LintKind::DocTestFailed { msg } => {
                    eprintln!("{}", msg);
                }
                LintKind::UnlabeledCodeFence { at_line } => {
                    eprintln!(
                        "  WARN   {}:{} — code fence has no language tag; add one (e.g. ```bash, ```rust, ```toml)",
                        rel.display(),
                        at_line,
                    );
                }
                LintKind::DuplicateFrontmatter {
                    second_block_start_line,
                } => {
                    eprintln!(
                        "  ERROR  {}:{} — duplicate YAML frontmatter block (frontmatter/duplicate-block)",
                        rel.display(),
                        second_block_start_line,
                    );
                }
                LintKind::HandAuthoredLastUpdated => {
                    eprintln!(
                        "  ERROR  {} — hand-authored `last_updated:` in frontmatter; the pipeline derives this from Git history (documentation-governance.md). Remove the key.",
                        rel.display()
                    );
                }
                LintKind::ReadmeSyncDrift { block } => {
                    eprintln!(
                        "  ERROR  {} — SYNC block '{}' has drifted from its README.md ANCHOR counterpart. Re-sync both, or fix the anchor if content genuinely changed.",
                        rel.display(),
                        block
                    );
                }
                LintKind::ReadmeSyncMissingBlock { block } => {
                    eprintln!(
                        "  ERROR  {} — README.md has ANCHOR '{}' but index.mdx has no matching SYNC-FROM-README block.",
                        rel.display(),
                        block
                    );
                }
                LintKind::ReadmeSyncMissingAnchor { block } => {
                    eprintln!(
                        "  ERROR  {} — index.mdx expects README.md ANCHOR '{}' but it's missing there.",
                        rel.display(),
                        block
                    );
                }
                LintKind::ReadmeSyncSourceMissing { path } => {
                    eprintln!(
                        "  ERROR  {} — README<->index.mdx sync check could not read '{}'. The file is missing or unreadable, so drift can no longer be detected.",
                        rel.display(),
                        path
                    );
                }
            }
        }

        let hard_errors = lint_errors
            .iter()
            .filter(|e| {
                matches!(
                    e.kind,
                    LintKind::UnclosedCodeFence
                        | LintKind::ShortCodeFence { .. }
                        | LintKind::GenericDescription
                        | LintKind::UnknownCategory { .. }
                        | LintKind::UnknownStatus { .. }
                        | LintKind::UnknownSchemaType { .. }
                        | LintKind::BrokenIncludeFile { .. }
                        | LintKind::BrokenIncludeAnchor { .. }
                        | LintKind::WholeFileIncludeHasTrainingHeader { .. }
                        | LintKind::MissingTrainingRationale
                        | LintKind::HandAuthoredLastUpdated
                        | LintKind::DocTestFailed { .. }
                        | LintKind::DuplicateFrontmatter { .. }
                        | LintKind::ReadmeSyncDrift { .. }
                        | LintKind::ReadmeSyncMissingBlock { .. }
                        | LintKind::ReadmeSyncMissingAnchor { .. }
                        | LintKind::ReadmeSyncSourceMissing { .. }
                )
            })
            .count();
        print_grouped_summary(&lint_errors);

        if hard_errors > 0 {
            eprintln!(
                "\n{} hard error(s) — fix before building docs.",
                hard_errors
            );
            std::process::exit(1);
        }
        eprintln!();
    }

    println!("vox-doc-pipeline lint complete — no hard errors.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_paths_arg_nonexistent_is_error() {
        let docs_src = Path::new("/tmp/vox-doc-pipeline-paths-test-docs-src");
        let args = [
            "vox-doc-pipeline".to_string(),
            "--paths".to_string(),
            "does-not-exist.md".to_string(),
        ];
        let err = parse_paths_arg(&args, docs_src)
            .expect_err("nonexistent --paths value must be an error");
        assert_eq!(err.len(), 1);
        assert!(
            err[0].ends_with("does-not-exist.md"),
            "unmatched path should be listed, got {:?}",
            err
        );
        assert!(!err[0].exists());
    }

    #[test]
    fn parse_paths_arg_absent_flag_is_ok_empty() {
        let docs_src = Path::new("/tmp/vox-doc-pipeline-paths-test-docs-src");
        let args = ["vox-doc-pipeline".to_string(), "--lint-only".to_string()];
        let paths = parse_paths_arg(&args, docs_src).expect("no --paths is not an error");
        assert!(paths.is_empty());
    }
}
