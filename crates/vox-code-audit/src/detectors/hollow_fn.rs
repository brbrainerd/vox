use crate::rules::{DetectionRule, Finding, FindingConfidence, Language, Severity, SourceFile};
use quote::ToTokens;
use regex::Regex;
use syn::visit::Visit;

/// Detects functions whose bodies contain only a trivially-default return value,
/// making them structural skeletons that compile but do nothing meaningful.
pub struct HollowFnDetector {
    ts_fn_re: Regex,
    ts_hollow_ui_re: Regex,
}

impl Default for HollowFnDetector {
    fn default() -> Self {
        Self::new()
    }
}

const ALLOWED_FN_NAMES: &[&str] = &[
    "default", "new", "main", "noop", "no_op", "stub", "drop", "fmt", "clone", "deref", "as_ref",
    "as_mut", "from", "into", "try_from", "try_into",
];

impl HollowFnDetector {
    pub fn new() -> Self {
        Self {
            ts_fn_re: Regex::new(
                r"(?:function|async function)\s+(\w+)\s*\([^)]*\)[^{]*\{([^}]*)\}",
            )
            .expect("valid regex"),
            ts_hollow_ui_re: Regex::new(r"return\s*<[a-zA-Z0-9_]+>(?:\s*)</[a-zA-Z0-9_]+>;?")
                .expect("valid regex"),
        }
    }

    fn is_allowed_name(name: &str) -> bool {
        if name.starts_with("default_") {
            return true;
        }
        ALLOWED_FN_NAMES.contains(&name)
    }

    fn is_hollow_expr_ast(expr: &str) -> bool {
        let e = expr.replace(" ", "");
        if [
            "Ok(())",
            "Ok(Default::default())",
            "Default::default()",
            "true",
            "false",
            "0",
            "0.0",
            "\"\"",
            "String::new()",
            "Vec::new()",
            "vec![]",
            "HashMap::new()",
            "HashSet::new()",
            "BTreeMap::new()",
            "BTreeSet::new()",
            "None",
            "()",
            "Ok(String::new())",
            "Ok(Vec::new())",
            "Ok(0)",
            "Ok(false)",
            "Ok(true)",
            "Err(())",
            "Box::new()",
        ]
        .contains(&e.as_str())
        {
            return true;
        }

        if (e.contains("Default::default()") || e.contains("..Default::default()"))
            && e.contains('{')
        {
            return false;
        }

        if e.ends_with("::default()")
            && !e.starts_with("Self")
            && e.chars().next().is_some_and(|c| c.is_ascii_uppercase())
        {
            return true;
        }

        false
    }

    /// `if` / `match` / loops as expression statements carry real control flow; the tail may be a
    /// literal (`false`) without the function being a stub (TOESTUB false positive otherwise).
    fn expr_carries_substantive_control_flow(expr: &syn::Expr) -> bool {
        matches!(
            expr,
            syn::Expr::If(_)
                | syn::Expr::Match(_)
                | syn::Expr::While(_)
                | syn::Expr::ForLoop(_)
                | syn::Expr::Loop(_)
                | syn::Expr::Block(_)
                | syn::Expr::TryBlock(_)
                | syn::Expr::Async(_)
        )
    }

    fn has_test_attr(attrs: &[syn::Attribute]) -> bool {
        for attr in attrs {
            let path = attr.path().to_token_stream().to_string().replace(" ", "");
            if path == "test" || path == "cfg(test)" {
                return true;
            }
            if let syn::Meta::List(list) = &attr.meta
                && list.path.to_token_stream().to_string().replace(" ", "") == "cfg"
                && list.tokens.to_token_stream().to_string().contains("test")
            {
                return true;
            }
        }
        false
    }

    fn detect_rust(
        &self,
        file: &SourceFile,
        rust_ctx: Option<&crate::analysis::RustFileContext>,
    ) -> Vec<Finding> {
        let mut findings = Vec::new();
        let Some(ctx) = rust_ctx else {
            return vec![];
        };
        let Ok(ast) = &ctx.ast else {
            return vec![];
        };

        struct Visitor<'a> {
            file: &'a SourceFile,
            findings: &'a mut Vec<Finding>,
        }

        impl<'a, 'ast> Visit<'ast> for Visitor<'a> {
            fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
                if HollowFnDetector::has_test_attr(&node.attrs) {
                    return;
                }
                syn::visit::visit_item_mod(self, node);
            }

            fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
                if HollowFnDetector::has_test_attr(&node.attrs) {
                    return;
                }
                let name = node.sig.ident.to_string();
                let line = node.sig.ident.span().start().line;

                if self
                    .file
                    .lines
                    .get(line.saturating_sub(1))
                    .is_some_and(|l| l.contains("toestub-ignore"))
                {
                    return;
                }

                self.check_block(&name, &node.block, line);
                syn::visit::visit_item_fn(self, node);
            }

            fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
                if HollowFnDetector::has_test_attr(&node.attrs) {
                    return;
                }
                let name = node.sig.ident.to_string();
                let line = node.sig.ident.span().start().line;

                if self
                    .file
                    .lines
                    .get(line.saturating_sub(1))
                    .is_some_and(|l| l.contains("toestub-ignore"))
                {
                    return;
                }

                self.check_block(&name, &node.block, line);
                syn::visit::visit_impl_item_fn(self, node);
            }
        }

        impl<'a> Visitor<'a> {
            fn check_block(&mut self, name: &str, block: &syn::Block, line: usize) {
                if HollowFnDetector::is_allowed_name(name) {
                    return;
                }

                let mut meaningful_stmts = 0;
                let mut last_expr = None;
                for stmt in &block.stmts {
                    match stmt {
                        syn::Stmt::Expr(expr, semi) => {
                            if let syn::Expr::Macro(mac) = expr {
                                let path =
                                    mac.mac.path.to_token_stream().to_string().replace(" ", "");
                                if path.contains("println")
                                    || path.contains("tracing::")
                                    || path.contains("dbg")
                                    || path.contains("eprintln")
                                {
                                    if semi.is_none() {
                                        last_expr = Some(expr);
                                    }
                                    continue;
                                }
                            }
                            if semi.is_none() {
                                if HollowFnDetector::expr_carries_substantive_control_flow(expr) {
                                    meaningful_stmts += 1;
                                }
                                last_expr = Some(expr);
                            } else {
                                if let syn::Expr::Return(ret) = expr {
                                    if let Some(e) = &ret.expr {
                                        last_expr = Some(&**e);
                                    } else {
                                        last_expr = Some(expr);
                                    }
                                } else {
                                    meaningful_stmts += 1;
                                }
                            }
                        }
                        syn::Stmt::Macro(mac) => {
                            let path = mac.mac.path.to_token_stream().to_string().replace(" ", "");
                            if path.contains("println")
                                || path.contains("tracing::")
                                || path.contains("dbg")
                                || path.contains("eprintln")
                            {
                                continue;
                            }
                            meaningful_stmts += 1;
                        }
                        syn::Stmt::Local(_) | syn::Stmt::Item(_) => {
                            meaningful_stmts += 1;
                        }
                    }
                }

                if meaningful_stmts == 0
                    && let Some(expr) = last_expr
                {
                    let expr_str = expr.to_token_stream().to_string();
                    if HollowFnDetector::is_hollow_expr_ast(&expr_str) {
                        self.findings.push(Finding {
                                rule_id: "skeleton/hollow-fn".to_string(),
                                diagnostic_id: None,
                                rule_name: "Hollow Function Detector".to_string(),
                                severity: Severity::Warning,
                                file: self.file.path.clone(),
                                line,
                                column: 0,
                                message: format!("Function `{}` has a hollow body (returns `{}` without meaningful computation)", name, expr_str.replace(" ", "")),
                                suggestion: Some("Implement the function logic, or if intentionally empty, add a comment explaining why.".to_string()),
                                alternatives: vec![],
                                rationale: None,
                                context: self.file.context_around(line, 2),
                                confidence: Some(FindingConfidence::Medium),
                                evidence: Some(serde_json::json!({
                                    "fn_name": name,
                                    "hollow_expr": expr_str.replace(" ", ""),
                                })),
                            });
                    }
                }
            }
        }

        let mut visitor = Visitor {
            file,
            findings: &mut findings,
        };
        visitor.visit_file(ast);
        findings
    }

    fn detect_typescript(&self, file: &SourceFile) -> Vec<Finding> {
        let mut findings = Vec::new();

        let ts_hollow = [
            "return true",
            "return false",
            "return null",
            "return undefined",
            "return ''",
            "return \"\"",
            "return []",
            "return {}",
            "return 0",
            "return",
            "return { } as any",
            "return <> </>",
        ];

        for (i, line) in file.lines.iter().enumerate() {
            if let Some(caps) = self.ts_fn_re.captures(line)
                && let (Some(name), Some(body)) = (caps.get(1), caps.get(2))
            {
                let body_trimmed = body.as_str().trim().trim_end_matches(';').trim();
                let mut is_hollow = ts_hollow.contains(&body_trimmed);

                if !is_hollow
                    && (body_trimmed == "return <></>" || body_trimmed == "return [] as any")
                {
                    is_hollow = true;
                }
                if !is_hollow && self.ts_hollow_ui_re.is_match(body_trimmed) {
                    is_hollow = true;
                }

                if is_hollow {
                    findings.push(Finding {
                            rule_id: "skeleton/hollow-fn".to_string(),
                            diagnostic_id: None,
                            rule_name: self.name().to_string(),
                            severity: self.severity(),
                            file: file.path.clone(),
                            line: i + 1,
                            column: 0,
                            message: format!(
                                "Function `{}` has a hollow body (returns a trivial default)",
                                name.as_str()
                            ),
                            suggestion: Some(
                                "Implement the function logic or document why it's intentionally empty.".to_string(),
                            ),
                            alternatives: vec![],
                            rationale: None,
                            context: file.context_around(i + 1, 1),
                            confidence: Some(FindingConfidence::Medium),
                            evidence: None,
                        });
                }
            }
        }
        findings
    }
}

impl DetectionRule for HollowFnDetector {
    fn id(&self) -> &'static str {
        "skeleton/hollow-fn"
    }
    fn name(&self) -> &'static str {
        "Hollow Function Detector"
    }
    fn description(&self) -> &'static str {
        "Detects functions with trivially-default return values (compile but do nothing)"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn languages(&self) -> &[Language] {
        &[Language::Rust, Language::TypeScript]
    }
    fn minimal_repro(&self) -> Option<&'static str> {
        Some(
            "// VIOLATION — hollow function: compiles but does nothing\n\
             fn process_payment(amount: f64, user_id: UserId) -> Result<PaymentResult> {\n\
             \x20   Ok(PaymentResult::default())  // structural skeleton only\n\
             }\n\
             \n\
             // FIX — real implementation\n\
             fn process_payment(amount: f64, user_id: UserId) -> Result<PaymentResult> {\n\
             \x20   let account = accounts.get(user_id)?;\n\
             \x20   account.charge(amount).map(PaymentResult::Success)\n\
             }",
        )
    }

    fn detect(
        &self,
        file: &SourceFile,
        rust_ctx: Option<&crate::analysis::RustFileContext>,
    ) -> Vec<Finding> {
        match file.language {
            Language::Rust => self.detect_rust(file, rust_ctx),
            Language::TypeScript => self.detect_typescript(file),
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn source(ext: &str, code: &str) -> SourceFile {
        SourceFile::new(PathBuf::from(format!("test.{}", ext)), code.to_string())
    }

    #[test]
    fn detects_ok_unit_hollow() {
        let d = HollowFnDetector::new();
        let f = source(
            "rs",
            "fn process_event(e: Event) -> Result<()> {\n    Ok(())\n}",
        );
        let rust_ctx = if f.language == Language::Rust {
            Some(crate::analysis::RustFileContext::parse(&f.content))
        } else {
            None
        };
        let findings = d.detect(&f, rust_ctx.as_ref());
        assert!(
            findings.iter().any(|f| f.rule_id == "skeleton/hollow-fn"),
            "should detect Ok(()) hollow function"
        );
    }

    #[test]
    fn detects_true_hollow() {
        let d = HollowFnDetector::new();
        let f = source("rs", "fn validate(input: &str) -> bool {\n    true\n}");
        let rust_ctx = if f.language == Language::Rust {
            Some(crate::analysis::RustFileContext::parse(&f.content))
        } else {
            None
        };
        let findings = d.detect(&f, rust_ctx.as_ref());
        assert!(
            findings.iter().any(|f| f.rule_id == "skeleton/hollow-fn"),
            "should detect `true` hollow function"
        );
    }

    #[test]
    fn detects_vec_new_hollow() {
        let d = HollowFnDetector::new();
        let f = source("rs", "fn get_items() -> Vec<Item> {\n    Vec::new()\n}");
        let rust_ctx = if f.language == Language::Rust {
            Some(crate::analysis::RustFileContext::parse(&f.content))
        } else {
            None
        };
        let findings = d.detect(&f, rust_ctx.as_ref());
        assert!(
            findings.iter().any(|f| f.rule_id == "skeleton/hollow-fn"),
            "should detect Vec::new() hollow function"
        );
    }

    #[test]
    fn detects_default_default_hollow() {
        let d = HollowFnDetector::new();
        let f = source(
            "rs",
            "fn build_config() -> Config {\n    Default::default()\n}",
        );
        let rust_ctx = if f.language == Language::Rust {
            Some(crate::analysis::RustFileContext::parse(&f.content))
        } else {
            None
        };
        let findings = d.detect(&f, rust_ctx.as_ref());
        assert!(
            findings.iter().any(|f| f.rule_id == "skeleton/hollow-fn"),
            "should detect Default::default() hollow function"
        );
    }

    #[test]
    fn detects_type_default_hollow() {
        let d = HollowFnDetector::new();
        let f = source(
            "rs",
            "fn build_response() -> Response {\n    Response::default()\n}",
        );
        let rust_ctx = if f.language == Language::Rust {
            Some(crate::analysis::RustFileContext::parse(&f.content))
        } else {
            None
        };
        let findings = d.detect(&f, rust_ctx.as_ref());
        assert!(
            findings.iter().any(|f| f.rule_id == "skeleton/hollow-fn"),
            "should detect Response::default() hollow function"
        );
    }

    #[test]
    fn skips_impl_default_fn() {
        let d = HollowFnDetector::new();
        let f = source(
            "rs",
            "impl Default for Config {\n    fn default() -> Self {\n        Default::default()\n    }\n}",
        );
        let rust_ctx = if f.language == Language::Rust {
            Some(crate::analysis::RustFileContext::parse(&f.content))
        } else {
            None
        };
        let findings = d.detect(&f, rust_ctx.as_ref());
        assert!(
            findings.is_empty(),
            "should skip fn default() in Default impls"
        );
    }

    #[test]
    fn skips_new_constructor() {
        let d = HollowFnDetector::new();
        let f = source(
            "rs",
            "impl Config {\n    pub fn new() -> Self {\n        Default::default()\n    }\n}",
        );
        let rust_ctx = if f.language == Language::Rust {
            Some(crate::analysis::RustFileContext::parse(&f.content))
        } else {
            None
        };
        let findings = d.detect(&f, rust_ctx.as_ref());
        assert!(findings.is_empty(), "should skip fn new() constructors");
    }

    #[test]
    fn skips_test_functions() {
        let d = HollowFnDetector::new();
        let f = source(
            "rs",
            "#[cfg(test)]\nmod tests {\n    fn helper() -> bool {\n        true\n    }\n}",
        );
        let rust_ctx = if f.language == Language::Rust {
            Some(crate::analysis::RustFileContext::parse(&f.content))
        } else {
            None
        };
        let findings = d.detect(&f, rust_ctx.as_ref());
        assert!(findings.is_empty(), "should skip functions in test modules");
    }

    #[test]
    fn skips_functions_with_real_logic() {
        let d = HollowFnDetector::new();
        let f = source("rs", "fn add(a: i32, b: i32) -> i32 {\n    a + b\n}");
        let rust_ctx = if f.language == Language::Rust {
            Some(crate::analysis::RustFileContext::parse(&f.content))
        } else {
            None
        };
        let findings = d.detect(&f, rust_ctx.as_ref());
        assert!(findings.is_empty(), "should skip functions with real logic");
    }

    #[test]
    fn skips_suppressed_functions() {
        let d = HollowFnDetector::new();
        let f = source(
            "rs",
            "fn noop_handler(e: Event) -> Result<()> { // toestub-ignore(skeleton)\n    Ok(())\n}",
        );
        let rust_ctx = if f.language == Language::Rust {
            Some(crate::analysis::RustFileContext::parse(&f.content))
        } else {
            None
        };
        let findings = d.detect(&f, rust_ctx.as_ref());
        assert!(findings.is_empty(), "should skip suppressed functions");
    }

    #[test]
    fn detects_ts_hollow() {
        let d = HollowFnDetector::new();
        let f = source("ts", "function validate(input: string) { return true }");
        let rust_ctx = if f.language == Language::Rust {
            Some(crate::analysis::RustFileContext::parse(&f.content))
        } else {
            None
        };
        let findings = d.detect(&f, rust_ctx.as_ref());
        assert!(
            findings.iter().any(|f| f.rule_id == "skeleton/hollow-fn"),
            "should detect TS hollow function"
        );
    }

    #[test]
    fn skips_ts_real_function() {
        let d = HollowFnDetector::new();
        let f = source("ts", "function add(a: number, b: number) { return a + b }");
        let rust_ctx = if f.language == Language::Rust {
            Some(crate::analysis::RustFileContext::parse(&f.content))
        } else {
            None
        };
        let findings = d.detect(&f, rust_ctx.as_ref());
        assert!(
            findings.is_empty(),
            "should skip TS functions with real logic"
        );
    }
}
