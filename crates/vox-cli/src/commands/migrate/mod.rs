//! `vox migrate` — incremental source migrations for web/React interop.
//!
//! Reporting uses the compiler AST lints (`lint_ast_declarations`) so diagnostics stay aligned with
//! `vox_compiler::typeck::ast_decl_lints`.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::Subcommand;
use regex::Regex;
use serde::Serialize;
use walkdir::WalkDir;

/// `@component fn` → `component` (one step toward Path C; may still need manual `view:` / return-type edits).
fn patch_legacy_component_fn_keyword(src: &str) -> Option<String> {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"@component\s+fn\b").expect("regex"));
    if !re.is_match(src) {
        return None;
    }
    Some(re.replace_all(src, "component").to_string())
}

/// `@hook fn` → `fn` (retired decorator surface; authors still add `view:` / hook body manually).
fn patch_legacy_hook_fn_keyword(src: &str) -> Option<String> {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"@hook\s+fn\b").expect("regex"));
    if !re.is_match(src) {
        return None;
    }
    Some(re.replace_all(src, "fn").to_string())
}

/// Parsed CLI for `vox migrate …`.
#[derive(Subcommand, Debug, Clone)]
pub enum MigrateCmd {
    /// Scan `.vox` files for React interop migration findings (Path C, retired decorators).
    Web(WebMigrateArgs),
    /// Rewrite a .vox corpus to canonical names from `contracts/naming/renames.v1.json`.
    ///
    /// Walks all `.vox` files under ROOT (default: current directory), rewrites any
    /// identifier that appears as a `from` key in the rename registry to its canonical
    /// `to` name, and writes the result back in-place (unless `--dry-run` is given).
    ///
    /// The token-based rewrite logic is a pass-through stub in Task 5; Task 6 implements
    /// the full rewrite.
    Names(NamesArgs),
}

#[derive(clap::Args, Debug, Clone)]
pub struct WebMigrateArgs {
    /// Root directory to scan (recursive).
    #[arg(default_value = ".")]
    pub path: PathBuf,
    /// Emit JSON instead of plain text.
    #[arg(long)]
    pub json: bool,
    /// Apply deterministic filesystem patches (`@component fn`, `@hook fn`, …).
    #[arg(long)]
    pub write: bool,
    /// Exit with failure if any migration findings (or parse errors) remain after the run (for CI).
    #[arg(long)]
    pub check: bool,
}

/// Arguments for `vox migrate names`.
#[derive(clap::Args, Debug, Clone)]
pub struct NamesArgs {
    /// Root directory of .vox sources to rewrite. Defaults to the current working directory.
    #[arg(default_value = ".")]
    pub root: PathBuf,

    /// Print what would change without writing files.
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Serialize)]
struct Finding {
    path: String,
    code: String,
    message: String,
    line: Option<u32>,
}

#[derive(Serialize)]
struct WebReport {
    files_scanned: usize,
    parse_errors: usize,
    files_written: usize,
    legacy_component_fn_replacements: usize,
    legacy_hook_fn_replacements: usize,
    findings: Vec<Finding>,
}

/// Run a `vox migrate` subcommand.
pub fn run(cmd: MigrateCmd) -> Result<()> {
    match cmd {
        MigrateCmd::Web(args) => run_web(args),
        MigrateCmd::Names(args) => run_names(args),
    }
}

fn run_names(args: NamesArgs) -> Result<()> {
    let registry = vox_compiler::parser::renames::RenameRegistry::load_canonical()
        .map_err(|e| anyhow::anyhow!("loading rename registry: {}", e))?;
    let files = collect_vox_files(&args.root)?;
    let mut total = 0usize;
    for path in &files {
        let before =
            std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        let after = rewrite(&before, &registry);
        if before != after {
            total += 1;
            if !args.dry_run {
                std::fs::write(path, &after)
                    .with_context(|| format!("write {}", path.display()))?;
            }
            println!(
                "{}: {}",
                if args.dry_run {
                    "would update"
                } else {
                    "updated"
                },
                path.display()
            );
        }
    }
    println!(
        "{} file(s) {}",
        total,
        if args.dry_run {
            "would be updated"
        } else {
            "updated"
        }
    );
    Ok(())
}

fn collect_vox_files(root: &std::path::Path) -> std::io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    walk_for_names(root, &mut out)?;
    Ok(out)
}

fn walk_for_names(dir: &std::path::Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().unwrap_or_default();
            if name == "target" || name == "node_modules" || name == ".git" {
                continue;
            }
            walk_for_names(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "vox") {
            out.push(path);
        }
    }
    Ok(())
}

/// Codemod: rewrite identifier tokens that match a registry `from` name to their
/// canonical `to` names. Operates on lexer tokens, not text patterns, so:
///
/// - String literal contents are **never** touched (they are a distinct token kind).
/// - Substring matches inside unrelated identifiers (e.g. `MyBox`, `Boxes`) are
///   never rewritten — only exact whole-identifier tokens are matched.
/// - Comments and whitespace are preserved byte-for-byte (gaps between token
///   spans are copied verbatim; comments are emitted via `lex_preserving`).
/// - On lex failure the function is not applicable: the logos lexer is infallible
///   (it skips unknown characters rather than returning an error), so in practice
///   the source is always tokenised. Any unrecognised bytes are faithfully copied
///   via the gap-fill logic.
///
/// ## Lexer API shape (actual, as of 2026-05)
///
/// ```text
/// vox_compiler::lexer::lex_preserving(source: &str) -> Vec<Spanned>
/// Spanned { token: Token, span: std::ops::Range<usize> }
/// Token::Ident(String)      — lowercase identifiers
/// Token::TypeIdent(String)  — upper-case / type identifiers
/// ```
///
/// Horizontal whitespace is *skipped* by the logos lexer (not emitted as tokens);
/// the inter-token byte ranges are recovered by copying `source[cursor..span.start]`
/// before each token. Comments ARE emitted as `Token::Comment` by `lex_preserving`
/// (unlike the standard `lex()` which strips them).
///
/// Exposed as `pub` for testing from the integration suite.
pub fn rewrite(source: &str, registry: &vox_compiler::parser::renames::RenameRegistry) -> String {
    use vox_compiler::lexer::lex_preserving;
    use vox_compiler::lexer::token::Token;

    let tokens = lex_preserving(source);

    let mut out = String::with_capacity(source.len());
    let mut cursor = 0usize;

    for spanned in &tokens {
        let span_start = spanned.span.start;
        let span_end = spanned.span.end;

        // For Eof the span is empty (source.len()..source.len()); no gap or text to emit.
        // We just flush any tail bytes that weren't covered by a real token.
        if matches!(spanned.token, Token::Eof) {
            break;
        }

        // Copy any bytes between the previous emit point and this token's start.
        // This recovers horizontal whitespace (skipped by logos) and any unrecognised
        // bytes (logos emits Err for them; we filtered those out in lex_preserving,
        // so they land here as part of the inter-token gap).
        if span_start > cursor {
            out.push_str(&source[cursor..span_start]);
        }

        match &spanned.token {
            Token::Ident(name) | Token::TypeIdent(name) => {
                if let Some(entry) = registry.resolve(name) {
                    // VUV-9 codemod scope: only primitive renames are token-level safe.
                    // Kwarg/Decorator/EnumValue/Type renames need AST-aware rewriting
                    // (kwargs need argument-position context; types need type-position
                    // context). Future phases lift this restriction.
                    if matches!(
                        entry.kind,
                        vox_compiler::parser::renames::RenameKind::Primitive
                    ) {
                        out.push_str(&entry.to);
                    } else {
                        out.push_str(name);
                    }
                } else {
                    out.push_str(&source[span_start..span_end]);
                }
            }
            _ => {
                out.push_str(&source[span_start..span_end]);
            }
        }

        cursor = span_end;
    }

    // Flush any trailing bytes after the last real token (trailing whitespace,
    // newlines, unrecognised chars after the last identifier, etc.).
    if cursor < source.len() {
        out.push_str(&source[cursor..]);
    }

    out
}

/// Thin wrapper over [`rewrite`] for integration tests that live outside this
/// crate (and therefore outside `#[cfg(test)]`).
pub fn rewrite_for_test(
    source: &str,
    registry: &vox_compiler::parser::renames::RenameRegistry,
) -> String {
    rewrite(source, registry)
}

fn run_web(args: WebMigrateArgs) -> Result<()> {
    let root = args
        .path
        .canonicalize()
        .with_context(|| format!("canonicalize migrate root {}", args.path.display()))?;

    let mut report = WebReport {
        files_scanned: 0,
        parse_errors: 0,
        files_written: 0,
        legacy_component_fn_replacements: 0,
        legacy_hook_fn_replacements: 0,
        findings: Vec::new(),
    };

    for entry in WalkDir::new(&root).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        let p = entry.path();
        if p.extension().is_none_or(|e| e != "vox") {
            continue;
        }
        report.files_scanned += 1;
        if args.write {
            if let Some((comp_n, hook_n, new_src)) = maybe_patch_vox_file(p)? {
                report.files_written += 1;
                report.legacy_component_fn_replacements += comp_n;
                report.legacy_hook_fn_replacements += hook_n;
                scan_vox_source(p, &new_src, &mut report);
                continue;
            }
        }
        scan_vox_file(p, &mut report);
    }

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).context("serialize migrate report")?
        );
    } else {
        println!(
            "vox migrate web: scanned {} file(s), {} parse error(s), {} lint finding(s){}",
            report.files_scanned,
            report.parse_errors,
            report.findings.len(),
            if args.write {
                format!(
                    ", wrote {} file(s), {} @component and {} @hook keyword replacement(s)",
                    report.files_written,
                    report.legacy_component_fn_replacements,
                    report.legacy_hook_fn_replacements
                )
            } else {
                String::new()
            }
        );
        for f in &report.findings {
            let loc = f
                .line
                .map(|l| format!("{}:{}:", f.path, l))
                .unwrap_or_else(|| format!("{}:", f.path));
            println!("  {loc} [{}] {}", f.code, f.message);
        }
    }

    if args.check && (report.parse_errors > 0 || !report.findings.is_empty()) {
        bail!(
            "migrate web --check: {} parse error(s), {} finding(s)",
            report.parse_errors,
            report.findings.len()
        );
    }

    Ok(())
}

/// Returns `(@component fixes, @hook fixes, new_source)` when anything changed.
fn maybe_patch_vox_file(path: &Path) -> Result<Option<(usize, usize, String)>> {
    let src = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    static RE_COMP: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    static RE_HOOK: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let re_comp = RE_COMP.get_or_init(|| Regex::new(r"@component\s+fn\b").expect("regex"));
    let re_hook = RE_HOOK.get_or_init(|| Regex::new(r"@hook\s+fn\b").expect("regex"));
    let comp_before = re_comp.find_iter(&src).count();
    let hook_before = re_hook.find_iter(&src).count();
    if comp_before == 0 && hook_before == 0 {
        return Ok(None);
    }
    let mut fixed = src;
    if let Some(s) = patch_legacy_component_fn_keyword(&fixed) {
        fixed = s;
    }
    if let Some(s) = patch_legacy_hook_fn_keyword(&fixed) {
        fixed = s;
    }
    let comp_after = re_comp.find_iter(&fixed).count();
    let hook_after = re_hook.find_iter(&fixed).count();
    let comp_n = comp_before.saturating_sub(comp_after);
    let hook_n = hook_before.saturating_sub(hook_after);
    if comp_n == 0 && hook_n == 0 {
        return Ok(None);
    }
    std::fs::write(path, &fixed).with_context(|| format!("write patched {}", path.display()))?;
    Ok(Some((comp_n, hook_n, fixed)))
}

fn scan_vox_source(path: &Path, src: &str, report: &mut WebReport) {
    let rel = path.display().to_string();
    let tokens = vox_compiler::lexer::cursor::lex(src);
    let module = match vox_compiler::parser::parse(tokens) {
        Ok(m) => m,
        Err(errs) => {
            report.parse_errors += 1;
            for e in errs {
                let off = e.span.start;
                report.findings.push(Finding {
                    path: rel.clone(),
                    code: "migrate.parse".into(),
                    message: e.message,
                    line: line_for_offset(src, off),
                });
            }
            return;
        }
    };

    let diags = vox_compiler::typeck::typecheck_module(&module, src);
    for d in diags {
        let code = d.code.clone().unwrap_or_else(|| "lint.unknown".into());
        if !is_migration_lint(&code) {
            continue;
        }
        let line = line_for_offset(src, d.span.start);
        report.findings.push(Finding {
            path: rel.clone(),
            code,
            message: d.message,
            line,
        });
    }
}

fn scan_vox_file(path: &Path, report: &mut WebReport) {
    let rel = path.display().to_string();
    let src = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            report.findings.push(Finding {
                path: rel,
                code: "migrate.io".into(),
                message: format!("read failed: {e}"),
                line: None,
            });
            return;
        }
    };
    scan_vox_source(path, &src, report);
}

fn is_migration_lint(code: &str) -> bool {
    matches!(
        code,
        "lint.legacy_component_fn"
            | "lint.retired_context"
            | "lint.retired_hook_fn"
            | "lint.retired_provider_fn"
            | "lint.retired_page_decl"
            | "lint.component_react_hook"
    ) || code.starts_with("lint.retired_")
}

fn line_for_offset(src: &str, byte_off: usize) -> Option<u32> {
    if byte_off > src.len() {
        return None;
    }
    let prefix = &src[..byte_off];
    Some(prefix.bytes().filter(|&b| b == b'\n').count() as u32 + 1)
}

#[cfg(test)]
mod report_shape_tests {
    use super::WebReport;

    #[test]
    fn migrate_web_report_json_includes_write_counters() {
        let r = WebReport {
            files_scanned: 1,
            parse_errors: 0,
            files_written: 0,
            legacy_component_fn_replacements: 0,
            legacy_hook_fn_replacements: 0,
            findings: vec![],
        };
        let v = serde_json::to_value(&r).expect("serde");
        assert!(v.get("files_written").is_some());
        assert!(v.get("legacy_component_fn_replacements").is_some());
        assert!(v.get("legacy_hook_fn_replacements").is_some());
        let mut keys: Vec<_> = v.as_object().expect("object").keys().cloned().collect();
        keys.sort();
        assert_eq!(
            keys,
            vec![
                "files_scanned",
                "files_written",
                "findings",
                "legacy_component_fn_replacements",
                "legacy_hook_fn_replacements",
                "parse_errors",
            ],
            "JSON must expose a stable key set for migrate --json consumers"
        );
    }
}
