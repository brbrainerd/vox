use crate::analysis::RustFileContext;
use crate::rules::{DetectionRule, Finding, Language, Severity, SourceFile};
use quote::ToTokens;
use syn::visit::Visit;

#[derive(Default)]
pub struct ReachabilityDetector {}

impl ReachabilityDetector {
    pub fn new() -> Self {
        Self {}
    }

    fn detect_rust<'a>(
        &self,
        file: &'a SourceFile,
        rust_ctx: Option<&'a RustFileContext>,
    ) -> Vec<Finding> {
        let mut findings = Vec::new();
        let Some(ctx) = rust_ctx else {
            return findings;
        };
        let ast = match &ctx.ast {
            Ok(ast) => ast,
            Err(_) => return findings,
        };

        let mut visitor = ReachVisitor {
            findings: &mut findings,
            file,
            content: &file.content,
        };
        visitor.visit_file(ast);

        findings
    }
}

const TRIVIAL_RETURNS: &[&str] = &[
    "Ok(())",
    "Default::default()",
    "true",
    "false",
    "0",
    "0.0",
    "\"\"",
    "String::new()",
    "Vec::new()",
    "None",
    "()",
    "Ok(true)",
];

struct ReachVisitor<'a, 'b> {
    findings: &'a mut Vec<Finding>,
    file: &'b SourceFile,
    content: &'b str,
}

impl<'a, 'b> ReachVisitor<'a, 'b> {
    fn check_trivial_reachability(&mut self, sig: &'a syn::Signature, block: &'a syn::Block) {
        let name = sig.ident.to_string();
        if name.starts_with("default_") || name == "new" {
            return;
        }

        let is_trivial = if block.stmts.is_empty() {
            true
        } else if block.stmts.len() == 1 {
            let stmt = &block.stmts[0];
            let s = stmt.to_token_stream().to_string().replace(" ", "");
            if s.contains("todo!(") || s.contains("unimplemented!(") {
                true
            } else {
                TRIVIAL_RETURNS.iter().any(|r| s == *r)
            }
        } else {
            false
        };

        if is_trivial {
            let count_in_file = self.content.matches(&name).count();
            if count_in_file <= 1
                && !crate::run_context::workspace_crate_contains_word(&self.file.path, &name)
            {
                self.findings.push(Finding {
                    rule_id: "skeleton/declared-not-called".to_string(),
                    diagnostic_id: None,
                    rule_name: "Integration Graph Reachability Detector".to_string(),
                    severity: Severity::Warning, // Tier B
                    file: self.file.path.clone(),
                    line: sig.ident.span().start().line,
                    column: sig.ident.span().start().column,
                    message: format!("Public function {} returns a trivial default and is never called in this workspace crate.", name),
                    suggestion: Some("Implement the function, write a test that exercises it, or remove it.".to_string()),
                    alternatives: vec![],
                    rationale: None,
                    context: self.file.context_around(sig.ident.span().start().line, 1),
                    confidence: None,
                    evidence: None,
                });
            }
        }
    }
}

impl<'a, 'b> syn::visit::Visit<'a> for ReachVisitor<'a, 'b> {
    fn visit_item_fn(&mut self, i: &'a syn::ItemFn) {
        if matches!(i.vis, syn::Visibility::Public(_)) {
            self.check_trivial_reachability(&i.sig, &i.block);
        }
        syn::visit::visit_item_fn(self, i);
    }

    fn visit_item_impl(&mut self, i: &'a syn::ItemImpl) {
        // If it's a trait impl, it is wired by the interface, so its fns are reached via polymorph.
        if i.trait_.is_some() {
            return;
        }
        syn::visit::visit_item_impl(self, i);
    }

    fn visit_impl_item_fn(&mut self, i: &'a syn::ImplItemFn) {
        if matches!(i.vis, syn::Visibility::Public(_)) {
            self.check_trivial_reachability(&i.sig, &i.block);
        }
        syn::visit::visit_impl_item_fn(self, i);
    }
}

impl DetectionRule for ReachabilityDetector {
    fn id(&self) -> &'static str {
        "skeleton/declared-not-called"
    }
    fn name(&self) -> &'static str {
        "Integration Graph Reachability Detector"
    }
    fn description(&self) -> &'static str {
        "Detects fake wiring: pub fns that return trivial defaults and are not called from tests or entry points."
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn minimal_repro(&self) -> Option<&'static str> {
        Some(
            "// VIOLATION — pub fn declared but returns trivial default and is never called\n\
             pub fn process_events(events: Vec<Event>) -> Vec<Result<(), Error>> {\n\
             \x20   Vec::new()  // trivial default: not actually processing anything\n\
             }\n\
             // (no test or entry point ever calls process_events)\n\
             \n\
             // FIX — implement the function and wire it into a test\n\
             pub fn process_events(events: Vec<Event>) -> Vec<Result<(), Error>> {\n\
             \x20   events.into_iter().map(|e| handle_event(e)).collect()\n\
             }\n\
             #[test]\n\
             fn test_process_events() { assert!(!process_events(sample()).is_empty()); }",
        )
    }
    fn languages(&self) -> &[Language] {
        &[Language::Rust]
    }
    fn detect(
        &self,
        file: &SourceFile,
        rust: Option<&crate::analysis::RustFileContext>,
    ) -> Vec<Finding> {
        match file.language {
            Language::Rust => self.detect_rust(file, rust),
            _ => Vec::new(),
        }
    }
}
