//! Syn visitor and shared helpers for [`super::ScalingSurfacesDetector`].

use std::collections::HashMap;

use regex::Regex;
use serde_json::json;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{Expr, ExprPath, ImplItemFn, Item, ItemFn, ItemMod, Meta};

use crate::analysis::RustFileContext;
use crate::rules::{
    Finding, FindingConfidence, Language, Severity, SourceFile, rust_byte_is_non_code,
};

pub(super) fn recent_line_starts_for_loop(lines: &[String], idx: usize, window: usize) -> bool {
    let start = idx.saturating_sub(window);
    for line in lines.iter().take(idx).skip(start) {
        let t = line.trim_start();
        if t.starts_with("//") {
            continue;
        }
        if t.starts_with("for ") {
            return true;
        }
    }
    false
}

pub(super) fn parse_rust_usize_literal(s: &str) -> Option<u64> {
    let clean: String = s.chars().filter(|c| *c != '_').collect();
    clean.parse().ok()
}

/// Strip `//` comments and normal `"` / `'` literals so SQL keywords inside examples don't trip heuristics.
pub(super) fn sql_line_for_keyword_scan(line: &str) -> String {
    let no_line_comment = line.split_once("//").map(|(a, _)| a).unwrap_or(line);
    let mut out = String::with_capacity(no_line_comment.len());
    let mut it = no_line_comment.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '"' | '\'' => {
                let quote = c;
                out.push(' ');
                while let Some(nc) = it.next() {
                    if nc == '\\' {
                        let _ = it.next();
                        continue;
                    }
                    if nc == quote {
                        break;
                    }
                }
            }
            _ => out.push(c),
        }
    }
    out
}

fn path_expr_is_fs_unbounded_read(p: &ExprPath) -> bool {
    let segs: Vec<String> = p
        .path
        .segments
        .iter()
        .map(|s| s.ident.to_string())
        .collect();
    if segs.is_empty() {
        return false;
    }
    let last = segs.last().map(String::as_str).unwrap_or("");
    if last != "read_to_string" && last != "read" {
        return false;
    }
    segs.iter().any(|s| s == "fs")
}

/// AST-backed `std::fs` unbounded reads (`read_to_string` / `read`). Empty when parse fails.
pub(super) fn fs_unbounded_read_findings(file: &SourceFile) -> Vec<Finding> {
    if file.language != Language::Rust {
        return Vec::new();
    }
    let Ok(ast) = syn::parse_file(&file.content) else {
        return Vec::new();
    };
    struct FsReadVisitor<'a> {
        file: &'a SourceFile,
        out: Vec<Finding>,
    }
    impl<'ast> Visit<'ast> for FsReadVisitor<'_> {
        fn visit_expr_call(&mut self, c: &'ast syn::ExprCall) {
            if let Expr::Path(p) = c.func.as_ref()
                && path_expr_is_fs_unbounded_read(p)
            {
                let span = c.span();
                let line = span.start().line;
                let column = span.start().column;
                self.out.push(Finding {
                        rule_id: "scaling/unbounded-read".to_string(),
                        diagnostic_id: None,
                        rule_name: "Scaling — fs read_to_string".to_string(),
                        severity: Severity::Info,
                        file: self.file.path.clone(),
                        line,
                        column,
                        message: "Unbounded `std::fs` read — consider size cap / streaming / `tokio::fs` in async contexts"
                            .to_string(),
                        suggestion: None,
                        alternatives: vec![],
                        rationale: None,
                        context: self.file.context_around(line, 1),
                        confidence: Some(FindingConfidence::High),
                        evidence: Some(json!({
                            "why": "syn ExprCall to fs::read / read_to_string",
                            "evidence": ["ast"]
                        })),
                    });
            }
            visit::visit_expr_call(self, c);
        }
    }
    let mut v = FsReadVisitor {
        file,
        out: Vec::new(),
    };
    v.visit_file(&ast);
    v.out
}

pub(super) fn env_unwrap_or_duplicate_findings(
    file: &SourceFile,
    re: &Regex,
    rust_ctx: Option<&RustFileContext>,
) -> Vec<Finding> {
    let mut out = Vec::new();
    let mut map: HashMap<String, Vec<usize>> = HashMap::new();
    let mut in_test_block = false;
    let test_attr = Regex::new(r"#\[(?:cfg\(test\)|test)\]").expect("valid");

    for (i, line) in file.lines.iter().enumerate() {
        let line_num = i + 1;
        if test_attr.is_match(line) {
            in_test_block = true;
        }
        if in_test_block {
            let trimmed = line.trim();
            if (trimmed.starts_with("fn ") || trimmed.starts_with("mod "))
                && !trimmed.contains("test")
                && !line.starts_with(char::is_whitespace)
            {
                in_test_block = false;
            }
        }
        if in_test_block || line.trim_start().starts_with("//") {
            continue;
        }
        if !line.contains("std::env::var") {
            continue;
        }
        if let Some(c) = re.captures(line) {
            let full = c.get(0).expect("regex full match");
            if rust_byte_is_non_code(file, line_num, full.start(), rust_ctx) {
                continue;
            }
            let lit = c.get(1).map(|m| m.as_str()).unwrap_or("");
            if lit.len() < 4 {
                continue;
            }
            map.entry(lit.to_string()).or_default().push(line_num);
        }
    }

    for (lit, lines) in map {
        if lines.len() < 2 {
            continue;
        }
        for &ln in &lines[1..] {
            out.push(Finding {
                rule_id: "scaling/env-default-duplication".to_string(),
                diagnostic_id: None,
                rule_name: "Scaling — duplicate env string default".to_string(),
                severity: Severity::Info,
                file: file.path.clone(),
                line: ln,
                column: 0,
                message: "Same `unwrap_or(\"…\")` default repeated on multiple `std::env::var` / `vox_secrets::resolve_secret` lines — centralize (const / policy / SSOT)"
                    .to_string(),
                suggestion: Some(format!("Literal default appears {}×: `{lit}`", lines.len())),
                alternatives: vec![],
                rationale: None,
                context: file.context_around(ln, 1),
                confidence: Some(FindingConfidence::Low),
                evidence: None,
            });
        }
    }

    out
}

struct ScalingSynVisitor<'a> {
    file: &'a SourceFile,
    findings: Vec<Finding>,
    in_async: bool,
    in_test_module: bool,
    depth_test_mod: usize,
    crate_allow_blocking_fs: bool,
}

fn attr_is_cfg_test(a: &syn::Attribute) -> bool {
    match &a.meta {
        Meta::List(list) if list.path.is_ident("cfg") => list.tokens.to_string().contains("test"),
        _ => false,
    }
}

impl<'ast> Visit<'ast> for ScalingSynVisitor<'_> {
    fn visit_item_mod(&mut self, node: &'ast ItemMod) {
        let is_test = node.attrs.iter().any(attr_is_cfg_test);
        if is_test {
            self.depth_test_mod += 1;
            self.in_test_module = true;
        }
        visit::visit_item_mod(self, node);
        if is_test {
            self.depth_test_mod -= 1;
            if self.depth_test_mod == 0 {
                self.in_test_module = false;
            }
        }
    }

    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        if is_test_fn(&node.attrs) {
            visit::visit_item_fn(self, node);
            return;
        }
        let was = self.in_async;
        self.in_async = node.sig.asyncness.is_some();
        visit::visit_item_fn(self, node);
        self.in_async = was;
    }

    fn visit_impl_item_fn(&mut self, node: &'ast ImplItemFn) {
        if is_test_fn(&node.attrs) {
            visit::visit_impl_item_fn(self, node);
            return;
        }
        let was = self.in_async;
        self.in_async = node.sig.asyncness.is_some();
        visit::visit_impl_item_fn(self, node);
        self.in_async = was;
    }

    fn visit_item(&mut self, node: &'ast Item) {
        if let Item::Fn(f) = node
            && !is_test_fn(&f.attrs)
        {
            let was = self.in_async;
            self.in_async = f.sig.asyncness.is_some();
            visit::visit_item_fn(self, f);
            self.in_async = was;
            return;
        }
        visit::visit_item(self, node);
    }

    fn visit_expr(&mut self, node: &'ast Expr) {
        if self.in_async
            && !self.crate_allow_blocking_fs
            && !self.in_test_module
            && let Expr::Call(call) = node
        {
            if call_looks_like_std_fs_blocking(&call.func) {
                let span = call.span();
                let line = span.start().line;
                self.findings.push(Finding {
                    rule_id: "scaling/blocking-in-async".to_string(),
                    diagnostic_id: None,
                    rule_name: "Scaling — blocking fs in async".to_string(),
                    severity: Severity::Info,
                    file: self.file.path.clone(),
                    line,
                    column: span.start().column,
                    message: "`std::fs` (or known blocking) call inside `async` — use `tokio::fs` or offload via `spawn_blocking`"
                        .to_string(),
                    suggestion: Some(
                        "Policy: `contracts/scaling/policy.yaml` per-crate overrides if intentional."
                            .to_string(),
                    ),
                    alternatives: vec![],
                    rationale: None,
                    context: self.file.context_around(line, 2),
                    confidence: Some(FindingConfidence::High),
                    evidence: None,
                });
            }
            if call_looks_like_thread_sleep(&call.func) {
                let span = call.span();
                let line = span.start().line;
                self.findings.push(Finding {
                    rule_id: "scaling/thread-sleep-async".to_string(),
                    diagnostic_id: None,
                    rule_name: "Scaling — thread sleep".to_string(),
                    severity: Severity::Info,
                    file: self.file.path.clone(),
                    line,
                    column: span.start().column,
                    message: "`thread::sleep` in async context blocks the executor".to_string(),
                    suggestion: Some("`tokio::time::sleep` or structured backoff".to_string()),
                    alternatives: vec![],
                    rationale: None,
                    context: self.file.context_around(line, 2),
                    confidence: Some(FindingConfidence::High),
                    evidence: None,
                });
            }
        }
        visit::visit_expr(self, node);
    }
}

fn is_test_fn(attrs: &[syn::Attribute]) -> bool {
    attrs
        .iter()
        .any(|a| a.path().is_ident("test") || attr_is_cfg_test(a))
}

fn call_looks_like_std_fs_blocking(func: &Expr) -> bool {
    let Expr::Path(p) = func else {
        return false;
    };
    let segs: Vec<String> = p
        .path
        .segments
        .iter()
        .map(|s| s.ident.to_string())
        .collect();
    let joined = segs.join("::");
    joined.starts_with("std::fs::")
        || joined == "fs::read_to_string"
        || joined == "fs::read"
        || joined == "fs::write"
}

fn call_looks_like_thread_sleep(func: &Expr) -> bool {
    let Expr::Path(p) = func else {
        return false;
    };
    let last = p
        .path
        .segments
        .last()
        .map(|s| s.ident == "sleep")
        .unwrap_or(false);
    let has_thread = p.path.segments.iter().any(|s| s.ident == "thread");
    last && has_thread
}

pub(super) fn detect_rust_syn_blockings(
    file: &SourceFile,
    crate_allow_blocking_fs: bool,
) -> Vec<Finding> {
    if file.language != Language::Rust {
        return Vec::new();
    }
    let Ok(ast) = syn::parse_file(&file.content) else {
        return Vec::new();
    };
    let mut v = ScalingSynVisitor {
        file,
        findings: Vec::new(),
        in_async: false,
        in_test_module: false,
        depth_test_mod: 0,
        crate_allow_blocking_fs,
    };
    v.visit_file(&ast);
    v.findings
}

#[cfg(test)]
mod semcov_behavior_tests {
    use super::*;

    // Catches: usize-literal parser failing to strip `_` digit separators or
    // accepting non-numeric junk as a valid bound.
    #[test]
    fn parse_rust_usize_literal_strips_separators() {
        assert_eq!(parse_rust_usize_literal("1_000"), Some(1000));
        assert_eq!(parse_rust_usize_literal("42"), Some(42));
        assert_eq!(parse_rust_usize_literal("0xFF"), None); // hex not handled
        assert_eq!(parse_rust_usize_literal("abc"), None);
    }

    // Catches: lookback window scanner ignoring `//`-comment skip, or reading
    // outside the [idx-window, idx) range and falsely reporting a `for` loop.
    #[test]
    fn recent_line_starts_for_loop_window_and_comment_skip() {
        let lines = vec![
            "for x in xs {".to_string(),
            "// for show".to_string(),
            "do_thing();".to_string(),
        ];
        // idx=2 (third line), window=5: real `for` on line 0 is in range.
        assert!(recent_line_starts_for_loop(&lines, 2, 5));
        // idx=2, window=1: only line 1 (a comment) is in range => false.
        assert!(!recent_line_starts_for_loop(&lines, 2, 1));
    }
}
