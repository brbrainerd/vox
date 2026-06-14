//! Parser-backed AST evaluation for Vox code snippets.
//!
//! Replaces the regex-based `vox_eval::detect_constructs` (research Gap G-15)
//! with a true parse → AST walk that accurately counts language constructs.
//!
//! **Research source:** Compiler Testing §Wave T1, Plan Adequacy §Coverage.

use std::collections::HashMap;

use crate::ast::decl::{Decl, Module};

/// Result of parsing and analysing a Vox code snippet through the real compiler frontend.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AstEvalReport {
    /// Whether the snippet parsed without error.
    pub parse_success: bool,
    /// Total AST declaration count.
    pub node_count: usize,
    /// Per-construct-kind counts (e.g. `"fn" => 3, "actor" => 1`).
    pub construct_histogram: HashMap<String, usize>,
    /// Whether the snippet contains at least one `@test` or `@forall` declaration.
    pub has_tests: bool,
    /// Byte-offset span of the first parse error (when `!parse_success`).
    pub error_span: Option<(usize, usize)>,
}

impl AstEvalReport {
    /// Coverage score in `[0.0, 1.0]` based on the number of distinct construct kinds.
    ///
    /// Saturates at 8 distinct kinds (fn, type, actor, workflow, table, test, server, query).
    #[must_use]
    pub fn coverage_score(&self) -> f64 {
        if !self.parse_success {
            return 0.0;
        }
        (self.construct_histogram.len() as f64 / 8.0).min(1.0)
    }
}

/// Analyse an already-parsed module without re-lexing/re-parsing.
#[must_use]
pub fn eval_module(module: &Module) -> AstEvalReport {
    let counts = count_module_constructs(module);
    AstEvalReport {
        parse_success: true,
        node_count: counts.total,
        construct_histogram: counts.histogram,
        has_tests: counts.has_tests,
        error_span: None,
    }
}

/// Parse a Vox code snippet through the real compiler pipeline and return a structured report.
///
/// This replaces the regex-based `vox_eval::detect_constructs` with a true parser-backed analysis,
/// addressing research Gap G-15 (regex eval misses syntactic context).
#[must_use]
pub fn ast_eval(code: &str) -> AstEvalReport {
    let tokens = crate::lexer::lex(code);
    match crate::parser::parse(tokens) {
        Ok(module) => eval_module(&module),
        Err(errors) => AstEvalReport {
            parse_success: false,
            node_count: 0,
            construct_histogram: HashMap::new(),
            has_tests: false,
            error_span: errors.first().map(|e| (e.span.start, e.span.end)),
        },
    }
}

struct ConstructCounts {
    total: usize,
    histogram: HashMap<String, usize>,
    has_tests: bool,
}

fn count_module_constructs(module: &Module) -> ConstructCounts {
    let mut histogram: HashMap<String, usize> = HashMap::new();
    let mut has_tests = false;
    for decl in &module.declarations {
        let key = match decl {
            Decl::Function(_) => "fn",
            Decl::ReactiveComponent(_) => "component",
            Decl::TypeDef(_) => "type",
            Decl::Import(_) => "import",
            Decl::HttpRoute(_) => "http",
            Decl::McpTool(_) => "mcp_tool",
            Decl::McpResource(_) => "mcp_resource",
            Decl::Test(_) | Decl::Forall(_) => {
                has_tests = true;
                "test"
            }
            Decl::Table(_) => "table",
            Decl::Collection(_) => "collection",
            Decl::Index(_) | Decl::VectorIndex(_) | Decl::SearchIndex(_) => "index",
            Decl::V0Component(_) => "v0",
            Decl::Routes(_) => "routes",
            Decl::Agent(_) | Decl::AgentDef(_) => "agent",
            Decl::Url(_) => "url",
            Decl::StateMachine(_) => "state_machine",
            _ => "other",
        };
        *histogram.entry(key.to_string()).or_insert(0) += 1;
    }
    let total = module.declarations.len();
    ConstructCounts {
        total,
        histogram,
        has_tests,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_function_parses() {
        let report = ast_eval("fn greet(name: str) to str { return \"hello\" }");
        assert!(report.parse_success);
        assert!(report.construct_histogram.contains_key("fn"));
        assert!(report.coverage_score() > 0.0);
    }

    #[test]
    fn invalid_snippet_fails_parse() {
        let report = ast_eval("fn broken( { ");
        assert!(!report.parse_success);
        assert!(report.error_span.is_some());
        assert_eq!(report.coverage_score(), 0.0);
    }

    #[test]
    fn test_declaration_detected() {
        let report = ast_eval("@test fn check() to Unit { assert(true) }");
        assert!(report.parse_success);
        assert!(report.has_tests);
    }

    #[test]
    fn empty_string_is_empty_module() {
        let report = ast_eval("");
        assert_eq!(report.node_count, 0);
    }

    #[test]
    fn multiple_constructs_counted() {
        let code = "fn hello() to str { return \"hi\" }\n\ntype Color = | Red | Blue\n\n@test fn check() to Unit { assert(true) }";
        let report = ast_eval(code);
        assert!(report.parse_success);
        assert!(report.construct_histogram.contains_key("fn"));
        assert!(report.construct_histogram.contains_key("type"));
        assert!(report.construct_histogram.contains_key("test"));
        assert!(report.has_tests);
        assert!(report.coverage_score() >= 0.25);
    }
}

#[cfg(test)]
mod semcov_wave1d_tests {
    #![allow(unused_imports)]
    use super::*;

    #[test]
    fn eval_module_empty_module_is_success_with_no_constructs() {
        let module = crate::ast::decl::Module {
            declarations: vec![],
            span: vox_ast::span::Span::new(0, 0),
        };
        let report = eval_module(&module);
        assert!(report.parse_success);
        assert_eq!(report.error_span, None);
        assert_eq!(report.node_count, 0);
        assert!(report.construct_histogram.is_empty());
        assert!(!report.has_tests);
        // parse_success is true but zero distinct kinds => 0/8 = 0.0
        assert!((report.coverage_score() - 0.0).abs() < 1e-9);
    }

    #[test]
    fn count_module_constructs_empty_module_is_zeroed() {
        let module = crate::ast::decl::Module {
            declarations: Vec::new(),
            span: crate::ast::span::Span::new(0, 0),
        };
        let counts = count_module_constructs(&module);
        assert_eq!(counts.total, 0);
        assert!(counts.histogram.is_empty());
        assert!(!counts.has_tests);
    }
}
