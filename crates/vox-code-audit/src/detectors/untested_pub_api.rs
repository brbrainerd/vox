use quote::ToTokens;
use syn::visit::Visit;

use crate::rules::{DetectionRule, Finding, FindingConfidence, Language, Severity, SourceFile};

/// Flags Rust library files that expose `pub fn` declarations but have zero
/// `#[test]` blocks — meaning the public API has no inline unit test coverage.
///
/// Severity: Warning. The default CI mode (`legacy`) only blocks on Errors,
/// so this surfaces in reports without breaking the workspace. The local
/// `tdd-guard` pre-commit hook (lefthook, mode `enforce-strict`) does block
/// new violations. Binary entry-points (`main.rs`, `bin/`), test helpers,
/// and intentionally-thin passthrough files (< 30 non-blank lines) are excluded.
pub struct UntestedPubApiDetector;

impl Default for UntestedPubApiDetector {
    fn default() -> Self {
        Self
    }
}

impl UntestedPubApiDetector {
    pub fn new() -> Self {
        Self
    }

    fn is_binary_path(file: &SourceFile) -> bool {
        let path_str = file.path.to_string_lossy();
        let file_name = file.path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        // vox-arch-check: allow abs-path
        file_name == "main.rs"
            || path_str.contains("/bin/")
            || path_str.contains("\\bin\\")
            || path_str.contains("/build.rs")
            || file_name == "build.rs"
    }

    fn is_test_file(file: &SourceFile) -> bool {
        let path_str = file.path.to_string_lossy();
        path_str.contains("/tests/")
            || path_str.contains("\\tests\\")
            || path_str.starts_with("tests/")
            || path_str.starts_with("tests\\")
            || path_str.contains("test_helpers")
            || path_str.contains("test_harness")
            || file
                .path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("test_") || n.ends_with("_test.rs"))
                .unwrap_or(false)
    }

    fn non_blank_line_count(file: &SourceFile) -> usize {
        file.lines.iter().filter(|l| !l.trim().is_empty()).count()
    }
}

impl DetectionRule for UntestedPubApiDetector {
    fn id(&self) -> &'static str {
        "skeleton/untested-pub-api"
    }

    fn name(&self) -> &'static str {
        "Untested Public API"
    }

    fn description(&self) -> &'static str {
        "Rust library file has pub fn declarations but no #[test] blocks"
    }

    fn severity(&self) -> Severity {
        Severity::Warning
    }

    fn minimal_repro(&self) -> Option<&'static str> {
        Some(
            "// VIOLATION — pub fn with no #[test] block in this file\n\
             pub fn calculate_tax(amount: f64, rate: f64) -> f64 {\n\
             \x20   amount * rate\n\
             }\n\
             \n\
             // FIX — add an inline test module\n\
             pub fn calculate_tax(amount: f64, rate: f64) -> f64 {\n\
             \x20   amount * rate\n\
             }\n\
             \n\
             #[cfg(test)]\n\
             mod tests {\n\
             \x20   use super::*;\n\
             \x20   #[test]\n\
             \x20   fn test_calculate_tax() {\n\
             \x20       assert_eq!(calculate_tax(100.0, 0.1), 10.0);\n\
             \x20   }\n\
             }",
        )
    }

    fn languages(&self) -> &[Language] {
        &[Language::Rust]
    }

    fn detect(
        &self,
        file: &SourceFile,
        rust_ctx: Option<&crate::analysis::RustFileContext>,
    ) -> Vec<Finding> {
        if file.language != Language::Rust {
            return vec![];
        }
        if Self::is_binary_path(file) || Self::is_test_file(file) {
            return vec![];
        }
        if Self::non_blank_line_count(file) < 30 {
            return vec![];
        }

        let Some(ctx) = rust_ctx else {
            return vec![];
        };
        let Ok(ast) = &ctx.ast else {
            return vec![];
        };

        struct Visitor {
            pub_fns: Vec<(String, usize)>,
            has_test: bool,
        }

        impl<'ast> Visit<'ast> for Visitor {
            fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
                for attr in &node.attrs {
                    let path = attr.path().to_token_stream().to_string().replace(' ', "");
                    if path == "test" || path == "cfg(test)" {
                        self.has_test = true;
                        return;
                    }
                    if let syn::Meta::List(list) = &attr.meta
                        && list.path.to_token_stream().to_string().replace(' ', "") == "cfg"
                        && list.tokens.to_token_stream().to_string().contains("test")
                    {
                        self.has_test = true;
                        return;
                    }
                }
                syn::visit::visit_item_mod(self, node);
            }

            fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
                for attr in &node.attrs {
                    let path = attr.path().to_token_stream().to_string().replace(' ', "");
                    if path == "test" || path == "tokio::test" || path == "async_std::test" {
                        self.has_test = true;
                        return;
                    }
                }
                if matches!(node.vis, syn::Visibility::Public(_)) {
                    let name = node.sig.ident.to_string();
                    let line = node.sig.ident.span().start().line;
                    self.pub_fns.push((name, line));
                }
                syn::visit::visit_item_fn(self, node);
            }

            fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
                for attr in &node.attrs {
                    let path = attr.path().to_token_stream().to_string().replace(' ', "");
                    if path == "test" || path == "tokio::test" {
                        self.has_test = true;
                        return;
                    }
                }
                if matches!(node.vis, syn::Visibility::Public(_)) {
                    let name = node.sig.ident.to_string();
                    let line = node.sig.ident.span().start().line;
                    self.pub_fns.push((name, line));
                }
                syn::visit::visit_impl_item_fn(self, node);
            }
        }

        let mut v = Visitor {
            pub_fns: Vec::new(),
            has_test: false,
        };
        v.visit_file(ast);

        if v.has_test || v.pub_fns.is_empty() {
            return vec![];
        }

        let names: Vec<_> = v.pub_fns.iter().map(|(n, _)| n.as_str()).collect();
        let first_line = v.pub_fns[0].1;
        vec![Finding {
            rule_id: self.id().to_string(),
            diagnostic_id: None,
            rule_name: self.name().to_string(),
            severity: self.severity(),
            file: file.path.clone(),
            line: first_line,
            column: 0,
            message: format!(
                "File has {} public fn(s) ({}) but no #[test] blocks — consider writing tests first (TDD)",
                names.len(),
                names[..names.len().min(3)].join(", ") + if names.len() > 3 { ", …" } else { "" }
            ),
            suggestion: Some(
                "Add a #[cfg(test)] mod tests block with at least one #[test] per public fn. \
                 Write the test before the implementation (see contribution-loop.md §@test-first)."
                    .to_string(),
            ),
            alternatives: vec![],
            rationale: None,
            context: file.context_around(first_line, 2),
            confidence: Some(FindingConfidence::High),
            evidence: Some(serde_json::json!({
                "pub_fn_count": names.len(),
                "first_pub_fn": names[0],
            })),
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn src(path: &str, code: &str) -> SourceFile {
        SourceFile::new(PathBuf::from(path), code.to_string())
    }

    fn detect(file: &SourceFile) -> Vec<Finding> {
        let d = UntestedPubApiDetector::new();
        let rust_ctx = if file.language == Language::Rust {
            Some(crate::analysis::RustFileContext::parse(&file.content))
        } else {
            None
        };
        d.detect(file, rust_ctx.as_ref())
    }

    fn pub_fn_block(n: usize) -> String {
        let lines: Vec<_> = (0..n)
            .map(|i| format!("pub fn func_{i}(x: i32) -> i32 {{ x + {i} }}"))
            .collect();
        lines.join("\n")
    }

    #[test]
    fn flags_pub_fn_with_no_test() {
        let code = pub_fn_block(35);
        let f = src("src/lib.rs", &code);
        let findings = detect(&f);
        assert!(
            findings
                .iter()
                .any(|f| f.rule_id == "skeleton/untested-pub-api"),
            "should flag lib file with pub fns and no tests"
        );
    }

    #[test]
    fn passes_when_test_block_present() {
        let code = format!(
            "{}\n\n#[cfg(test)]\nmod tests {{\n    #[test]\n    fn it_works() {{ assert_eq!(func_0(1), 1); }}\n}}",
            pub_fn_block(35)
        );
        let f = src("src/lib.rs", &code);
        let findings = detect(&f);
        assert!(
            findings.is_empty(),
            "should not flag when #[cfg(test)] mod is present"
        );
    }

    #[test]
    fn passes_when_tokio_test_present() {
        let code = format!(
            "{}\n\n#[tokio::test]\nasync fn async_test() {{ assert_eq!(func_0(2), 2); }}",
            pub_fn_block(35)
        );
        let f = src("src/lib.rs", &code);
        let findings = detect(&f);
        assert!(
            findings.is_empty(),
            "should not flag when #[tokio::test] is present"
        );
    }

    #[test]
    fn skips_main_rs() {
        let code = pub_fn_block(35);
        let f = src("src/main.rs", &code);
        let findings = detect(&f);
        assert!(findings.is_empty(), "should skip main.rs");
    }

    #[test]
    fn skips_bin_directory() {
        let code = pub_fn_block(35);
        let f = src("src/bin/cli.rs", &code);
        let findings = detect(&f);
        assert!(findings.is_empty(), "should skip bin/ files");
    }

    #[test]
    fn skips_tests_directory() {
        let code = pub_fn_block(35);
        let f = src("tests/integration.rs", &code);
        let findings = detect(&f);
        assert!(findings.is_empty(), "should skip tests/ directory");
    }

    #[test]
    fn skips_small_files() {
        // Only 5 pub fns — fewer than 30 non-blank lines
        let code = pub_fn_block(5);
        let f = src("src/lib.rs", &code);
        let findings = detect(&f);
        assert!(
            findings.is_empty(),
            "should skip files with fewer than 30 non-blank lines"
        );
    }

    #[test]
    fn skips_file_with_no_pub_fn() {
        let code = "fn private_helper(x: i32) -> i32 { x }\n".repeat(35);
        let f = src("src/lib.rs", &code);
        let findings = detect(&f);
        assert!(
            findings.is_empty(),
            "should skip files with only private fns"
        );
    }

    #[test]
    fn finding_includes_fn_names_in_message() {
        let code = pub_fn_block(35);
        let f = src("src/lib.rs", &code);
        let findings = detect(&f);
        assert!(!findings.is_empty());
        assert!(
            findings[0].message.contains("func_"),
            "finding message should include function names"
        );
    }
}
