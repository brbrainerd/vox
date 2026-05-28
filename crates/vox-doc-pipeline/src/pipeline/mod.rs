//! Documentation linter for `docs/src/`. Checks frontmatter, code fences,
//! training rationale, and embedded Vox doctests.

pub mod doctest;
mod lint;
pub mod types;

use std::fs;
use std::path::{Path, PathBuf};

use lint::{collect_lint_errors, collect_lint_errors_target};
use types::{LintError, LintKind};

fn parse_paths_arg(args: &[String], docs_src: &Path) -> Vec<PathBuf> {
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
    out
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

    let lint_targets = parse_paths_arg(&args, docs_src);
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
                        "  ERROR  {} — unknown category {:?}; use the canonical docs vocabulary",
                        rel.display(),
                        value
                    );
                }
                LintKind::UnknownStatus { value } => {
                    eprintln!(
                        "  ERROR  {} — unknown status {:?}; use current|experimental|legacy|research|roadmap|deprecated",
                        rel.display(),
                        value
                    );
                }
                LintKind::UnknownSchemaType { value } => {
                    eprintln!(
                        "  ERROR  {} — unknown schema_type {:?}; use HowTo|FAQPage|TechArticle|SoftwareSourceCode",
                        rel.display(),
                        value
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
                LintKind::LastUpdatedStale {
                    declared,
                    git_tip,
                    delta_days,
                } => {
                    eprintln!(
                        "  WARN   {} — frontmatter/last-updated-stale: last_updated={} vs git tip {} (Δ{} days)",
                        rel.display(),
                        declared,
                        git_tip,
                        delta_days,
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
                        | LintKind::DocTestFailed { .. }
                        | LintKind::DuplicateFrontmatter { .. }
                )
            })
            .count();
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
