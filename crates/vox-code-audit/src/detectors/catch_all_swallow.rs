//! Catch-all-swallow detector.
//!
//! Flags a `match` whose wildcard arm (`_ => …`) returns a **neutral / empty** value
//! (`None`, `Vec::new()`, `Default::default()`, `0`, `false`, `""`, `()`, `{}`) while one or
//! more *explicit* arms do real work. This is the silent-drop shape behind the headline
//! pipeline bug (`let` → `Decl::Const` → catch-all → the value vanishes): the unmatched cases
//! are swallowed into nothing instead of being handled or errored.
//!
//! Deliberately NARROW (Info severity, high precision): it does NOT flag wildcard arms that
//!   - return a real/non-empty value (`_ => read_only` is a legitimate default classification),
//!   - error or diverge (`_ => return Err(..)`, `panic!`, `unreachable!`, `todo!`), or
//!   - re-use the matched value.
//! Those are intentional. Only the "value vanishes" case is reported.

use crate::analysis::RustFileContext;
use crate::diagnostics::catalog;
use crate::rules::{DetectionRule, Finding, FindingConfidence, Language, Severity, SourceFile};
use quote::ToTokens;
use syn::visit::Visit;

/// Detector for catch-all arms that swallow unmatched cases into a neutral value.
pub struct CatchAllSwallowDetector;

impl CatchAllSwallowDetector {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CatchAllSwallowDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Skip `#[test]` / `#[cfg(test)]` code (matches `test` as a whole token, not a substring).
fn has_test_attr(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|a| {
        let p = a.path();
        if p.is_ident("test") {
            return true;
        }
        p.is_ident("cfg")
            && a.to_token_stream()
                .to_string()
                .split(|c: char| !c.is_alphanumeric() && c != '_')
                .any(|w| w == "test")
    })
}

/// True if `expr` is a neutral/empty value — i.e. the wildcard arm produces "nothing".
fn is_neutral_expr(expr: &syn::Expr) -> bool {
    match expr {
        // `None`
        syn::Expr::Path(p) => p.path.is_ident("None"),
        // `0`, `0.0`, `false`, `""`, `''` (and empty char/byte strings)
        syn::Expr::Lit(lit) => match &lit.lit {
            syn::Lit::Int(i) => i.base10_digits() == "0",
            syn::Lit::Float(f) => f.base10_digits().chars().all(|c| c == '0' || c == '.'),
            syn::Lit::Bool(b) => !b.value,
            syn::Lit::Str(s) => s.value().is_empty(),
            _ => false,
        },
        // `Default::default()` / `T::default()`, and `<empty-container>::new()` — no args.
        // `new` is restricted to known-empty std containers: an arbitrary `Foo::new()` returns a
        // meaningful value, not "nothing", so it must NOT be treated as a swallow.
        syn::Expr::Call(call) => {
            if !call.args.is_empty() {
                return false;
            }
            let syn::Expr::Path(p) = call.func.as_ref() else {
                return false;
            };
            let segs = &p.path.segments;
            match segs.last().map(|s| s.ident.to_string()).as_deref() {
                Some("default") => true,
                Some("new") => {
                    const EMPTY_CONTAINERS: &[&str] = &[
                        "Vec",
                        "String",
                        "HashMap",
                        "BTreeMap",
                        "HashSet",
                        "BTreeSet",
                        "VecDeque",
                        "BinaryHeap",
                        "LinkedList",
                        "OsString",
                        "PathBuf",
                    ];
                    // The receiver type is the segment before `new` (`Vec::new`, `HashMap::new`,
                    // `std::collections::HashMap::new`).
                    segs.iter()
                        .rev()
                        .nth(1)
                        .map(|s| s.ident.to_string())
                        .is_some_and(|t| EMPTY_CONTAINERS.contains(&t.as_str()))
                }
                _ => false,
            }
        }
        // `vec![]`, `Default::default()`-style macros with empty bodies
        syn::Expr::Macro(m) => {
            let name = m.mac.path.segments.last().map(|s| s.ident.to_string());
            matches!(name.as_deref(), Some("vec")) && m.mac.tokens.is_empty()
        }
        // empty `()` and empty `[]`
        syn::Expr::Tuple(t) => t.elems.is_empty(),
        syn::Expr::Array(a) => a.elems.is_empty(),
        // `{ <neutral> }` or `{}` — unwrap a trivial block
        syn::Expr::Block(b) => match b.block.stmts.as_slice() {
            [] => true,
            [syn::Stmt::Expr(inner, _)] => is_neutral_expr(inner),
            _ => false,
        },
        // `(neutral)`
        syn::Expr::Paren(p) => is_neutral_expr(&p.expr),
        _ => false,
    }
}

struct Visitor<'a> {
    file: &'a SourceFile,
    findings: Vec<Finding>,
}

impl<'a> Visitor<'a> {
    fn check_match(&mut self, node: &syn::ExprMatch) {
        // Need a real handler plus a catch-all: at least two arms.
        if node.arms.len() < 2 {
            return;
        }
        // The catch-all must be a bare wildcard `_` (a named binding may use the value).
        let Some(wild) = node
            .arms
            .iter()
            .find(|a| matches!(a.pat, syn::Pat::Wild(_)))
        else {
            return;
        };
        // A guarded wildcard (`_ if cond =>`) is conditional handling, not a blanket swallow.
        if wild.guard.is_some() {
            return;
        }
        if !is_neutral_expr(&wild.body) {
            return;
        }
        // At least one explicit (non-wildcard) arm must do real work.
        if !node
            .arms
            .iter()
            .any(|a| !matches!(a.pat, syn::Pat::Wild(_)))
        {
            return;
        }
        let line = node.match_token.span.start().line;
        if self
            .file
            .lines
            .get(line.saturating_sub(1))
            .is_some_and(|l| l.contains("toestub-ignore"))
        {
            return;
        }
        self.findings.push(Finding {
            rule_id: "vox/catch-all-swallow".to_string(),
            diagnostic_id: Some(catalog::CATCH_ALL_SWALLOW.to_string()),
            rule_name: "Catch-All Swallow Detector".to_string(),
            severity: Severity::Info,
            file: self.file.path.clone(),
            line,
            column: 1,
            message:
                "`match` wildcard arm `_ =>` returns a neutral/empty value while other arms do \
                 real work — unmatched cases are silently swallowed (the value vanishes)."
                    .to_string(),
            suggestion: Some(
                "Handle the remaining cases explicitly, or make the fallback loud: \
                 `_ => return Err(...)` / `unreachable!()` / log. If swallowing is intended, \
                 add `// toestub-ignore` on the `match` line."
                    .to_string(),
            ),
            alternatives: vec![],
            rationale: Some(
                "A wildcard arm that discards into an empty value hides newly-added variants and \
                 unexpected inputs: the case compiles, runs, and produces nothing — the exact \
                 shape of silent-drop pipeline bugs."
                    .to_string(),
            ),
            context: String::new(),
            confidence: Some(FindingConfidence::Medium),
            evidence: None,
        });
    }
}

impl<'a, 'ast> Visit<'ast> for Visitor<'a> {
    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        if has_test_attr(&node.attrs) {
            return;
        }
        syn::visit::visit_item_mod(self, node);
    }
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        if has_test_attr(&node.attrs) {
            return;
        }
        syn::visit::visit_item_fn(self, node);
    }
    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        if has_test_attr(&node.attrs) {
            return;
        }
        syn::visit::visit_impl_item_fn(self, node);
    }
    fn visit_expr_match(&mut self, node: &'ast syn::ExprMatch) {
        self.check_match(node);
        syn::visit::visit_expr_match(self, node);
    }
}

impl DetectionRule for CatchAllSwallowDetector {
    fn id(&self) -> &'static str {
        "vox/catch-all-swallow"
    }
    fn name(&self) -> &'static str {
        "Catch-All Swallow Detector"
    }
    fn description(&self) -> &'static str {
        "Flags a match whose wildcard arm returns a neutral/empty value while other arms do real \
         work — unmatched cases are silently swallowed."
    }
    fn severity(&self) -> Severity {
        Severity::Info
    }
    fn languages(&self) -> &[Language] {
        &[Language::Rust]
    }
    fn detect(&self, file: &SourceFile, rust_ctx: Option<&RustFileContext>) -> Vec<Finding> {
        let Some(ctx) = rust_ctx else {
            return Vec::new();
        };
        let Ok(ast) = &ctx.ast else {
            return Vec::new();
        };
        let mut v = Visitor {
            file,
            findings: Vec::new(),
        };
        v.visit_file(ast);
        v.findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(src: &str) -> Vec<Finding> {
        let file = SourceFile::new(
            std::path::PathBuf::from("crates/x/src/a.rs"),
            src.to_string(),
        );
        let ctx = RustFileContext::parse(&file.content);
        CatchAllSwallowDetector::new().detect(&file, Some(&ctx))
    }

    #[test]
    fn flags_wildcard_returning_none() {
        let f = run("fn f(k: Kind) -> Option<i32> { match k { Kind::A => Some(1), _ => None } }");
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].severity, Severity::Info);
    }

    #[test]
    fn flags_wildcard_returning_default_and_vec_new() {
        assert_eq!(
            run("fn f(k: Kind) -> i32 { match k { Kind::A => 7, _ => Default::default() } }").len(),
            1
        );
        assert_eq!(
            run("fn f(k: Kind) -> Vec<u8> { match k { Kind::A => real(), _ => Vec::new() } }")
                .len(),
            1
        );
    }

    #[test]
    fn skips_custom_new_constructor() {
        // `Foo::new()` for a non-container type returns a meaningful value, not "nothing" —
        // must NOT be flagged as a swallow (only empty std containers' new() count).
        assert!(
            run("fn f(k: Kind) -> Perm { match k { Kind::Write => Perm::Write, _ => Permission::new() } }")
                .is_empty()
        );
    }

    #[test]
    fn skips_wildcard_returning_real_value() {
        // `_ => read_only` is a deliberate default classification, not a silent drop.
        assert!(
            run("fn f(k: Kind) -> Perm { match k { Kind::Write => Perm::Write, _ => read_only } }")
                .is_empty()
        );
    }

    #[test]
    fn skips_wildcard_that_errors_or_diverges() {
        assert!(run("fn f(k: Kind) -> Result<i32> { match k { Kind::A => Ok(1), _ => return Err(e()) } }").is_empty());
        assert!(
            run("fn f(k: Kind) -> i32 { match k { Kind::A => 1, _ => unreachable!() } }")
                .is_empty()
        );
        assert!(
            run("fn f(k: Kind) -> i32 { match k { Kind::A => 1, _ => panic!(\"x\") } }").is_empty()
        );
    }

    #[test]
    fn skips_exhaustive_match_no_wildcard() {
        assert!(run("fn f(k: Kind) -> i32 { match k { Kind::A => 1, Kind::B => 2 } }").is_empty());
    }

    #[test]
    fn skips_guarded_wildcard() {
        // `_ if cond => None` is conditional handling, not a blanket swallow.
        assert!(run("fn f(k: i32) -> Option<i32> { match k { 1 => Some(1), _ if k > 9 => None, _ => Some(0) } }").is_empty());
    }

    #[test]
    fn skips_test_functions() {
        assert!(run("#[test]\nfn t() { match k { Kind::A => Some(1), _ => None } }").is_empty());
    }

    #[test]
    fn honors_toestub_ignore() {
        assert!(run("fn f(k: Kind) -> Option<i32> { match k { // toestub-ignore\n Kind::A => Some(1), _ => None } }").is_empty());
    }

    #[test]
    fn skips_single_arm_match() {
        assert!(run("fn f(k: Kind) -> Option<i32> { match k { _ => None } }").is_empty());
    }
}
