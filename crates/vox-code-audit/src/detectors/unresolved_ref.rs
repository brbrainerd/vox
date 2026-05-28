use std::collections::HashSet;
use std::path::Path;

use crate::rules::{
    DetectionRule, Finding, FindingConfidence, Language, Severity, SourceFile,
    rust_byte_is_non_code,
};
use regex::Regex;
use serde_json::json;

#[path = "unresolved_ast.rs"]
mod unresolved_ast;

/// Detects references to symbols (functions, types, modules) that appear to
/// be undefined within the file's scope.
///
/// Phase 1: Single-file heuristic — `use` sites, local `fn` defs, wildcards (`prelude::*`, `defaults::*`),
/// and embedded SQL/schema modules. Not semantic resolution; see `scaling-toestub-rules.md` (limitations).
pub struct UnresolvedRefDetector {
    rust_fn_call: Regex,
    rust_fn_def: Regex,
}

impl Default for UnresolvedRefDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl UnresolvedRefDetector {
    /// Sets up single-file call vs `fn` definition regexes (stdlib names filtered in `is_well_known_fn`).
    pub fn new() -> Self {
        Self {
            rust_fn_call: Regex::new(r"\b([a-z_]\w*)\s*\(").expect("valid regex"),
            rust_fn_def: Regex::new(r"(?:pub\s+)?(?:async\s+)?fn\s+([a-z_]\w*)\s*[<(]")
                .expect("valid regex"),
        }
    }

    /// `pub const SCHEMA_* : &str = "…"` modules are almost always embedded SQL; the fn-call
    /// heuristic matches SQL like `datetime('now')` and produces thousands of false positives.
    fn is_embedded_schema_only_module(content: &str) -> bool {
        if !content.contains("CREATE TABLE") {
            return false;
        }
        if !content.contains("pub const ") {
            return false;
        }
        if !(content.contains("SCHEMA_") || content.contains("SCHEMA ")) {
            return false;
        }
        !content.lines().any(|l| {
            let t = l.trim_start();
            t.starts_with("fn ")
                || t.starts_with("pub fn ")
                || t.starts_with("pub(crate) fn ")
                || t.starts_with("async fn ")
                || t.starts_with("pub async fn ")
        })
    }

    /// Lines that are clearly SQL/DDL, still inside a Rust source file (e.g. string literals).
    /// `use …::defaults::*` brings in `default_*` fns without naming them on a `use` line.
    fn file_imports_defaults_glob(file: &SourceFile) -> bool {
        file.lines.iter().any(|l| {
            let t = l.trim();
            t.starts_with("use ") && t.contains("defaults") && t.contains("::*")
        })
    }

    /// Wildcard imports that typically re-export many callables without listing each symbol on a `use` line.
    ///
    /// **Intentionally not** every `use …::*` — blanket treatment would hide bogus calls in normal modules
    /// (see `docs/src/architecture/scaling-toestub-rules.md` → programmatic audit limitations).
    fn file_has_high_fanout_glob_use(file: &SourceFile) -> bool {
        file.lines.iter().any(|l| {
            let t = l.trim_start();
            if !t.starts_with("use ") || !t.contains("::*") {
                return false;
            }
            let tl = t.to_ascii_lowercase();
            tl.contains("prelude") || tl.contains("defaults")
        })
    }

    fn line_looks_like_sql(line: &str) -> bool {
        let u = line.to_uppercase();
        u.contains("CREATE TABLE")
            || u.contains("CREATE INDEX")
            || u.contains("CREATE UNIQUE INDEX")
            || u.contains("DROP TABLE")
            || u.contains("ALTER TABLE")
            || u.contains("INSERT INTO")
            || u.contains("DELETE FROM")
            || u.contains("UPDATE ")
            || u.contains(" NOT NULL")
            || u.contains("PRIMARY KEY")
            || u.contains("FOREIGN KEY")
            || u.contains("REFERENCES ")
            || u.contains("DEFAULT (")
            || u.contains("AUTOINCREMENT")
            || u.contains("WITHOUT ROWID")
    }

    /// Well-known Rust standard library and common crate functions to exclude
    /// from false-positive detection.
    fn is_well_known_fn(name: &str) -> bool {
        matches!(
            name,
            "println"
                | "print"
                | "eprintln"
                | "eprint"
                | "format"
                | "write"
                | "writeln"
                | "dbg"
                | "vec"
                | "panic"
                | "assert"
                | "assert_eq"
                | "assert_ne"
                | "debug_assert"
                | "debug_assert_eq"
                | "debug_assert_ne"
                | "cfg"
                | "include"
                | "include_str"
                | "include_bytes"
                | "env"
                | "option_env"
                | "concat"
                | "line"
                | "file"
                | "column"
                | "stringify"
                | "todo"
                | "unimplemented"
                | "unreachable"
                | "compile_error"
                | "matches"
                | "if"
                | "for"
                | "while"
                | "match"
                | "loop"
                | "return"
                | "break"
                | "continue"
                | "Some"
                | "None"
                | "Ok"
                | "Err"
                | "Box"
                | "Rc"
                | "Arc"
                | "main"
                | "new"
                | "default"
                | "from"
                | "into"
                | "as_ref"
                | "as_mut"
                | "push"
                | "pop"
                | "get"
                | "set"
                | "len"
                | "is_empty"
                | "iter"
                | "map"
                | "filter"
                | "collect"
                | "unwrap"
                | "expect"
                | "clone"
                | "to_string"
                | "to_owned"
                | "contains"
                | "starts_with"
                | "ends_with"
                | "trim"
                | "split"
                | "join"
                | "insert"
                | "remove"
                | "extend"
                | "retain"
                | "sort"
                | "sort_by"
                | "with_capacity"
                // SQLite / SQL builtins often appear inside embedded schema strings.
                | "datetime"
                | "strftime"
                | "ifnull"
                | "coalesce"
                | "nullif"
                | "random"
                | "randomblob"
                | "zeroblob"
                | "typeof"
                | "unicode"
                | "quote"
                | "unhex"
                | "iif"
                | "instr"
                | "substr"
                | "lower"
                | "upper"
                | "abs"
                | "round"
                | "like"
                | "glob"
                // Rust syntax / prelude — often matched as `word (` by the call regex.
                | "let"
                | "pub"
                | "drop"
                | "allow"
        ) || crate::run_context::prelude_allowlist_contains(name)
    }

    fn looks_generated_or_build_output(path: &Path, content: &str) -> bool {
        let p = path
            .to_string_lossy()
            .replace('\\', "/")
            .to_ascii_lowercase();
        if p.contains("/target/") {
            return true;
        }
        for line in content.lines().take(48) {
            let t = line.trim();
            let u = t.to_ascii_lowercase();
            if u.contains("@generated") || u.contains("generated by the protocol buffer compiler") {
                return true;
            }
            if u.contains("do not edit") && (u.contains("generated") || u.contains("protobuf")) {
                return true;
            }
        }
        false
    }

    fn import_line_candidates(file: &SourceFile, name: &str) -> Vec<String> {
        let mut out = Vec::new();
        for l in &file.lines {
            let t = l.trim();
            if t.starts_with("use ") && t.contains(name) && out.len() < 8 {
                out.push(t.chars().take(120).collect());
            }
        }
        out
    }

    fn detect_rust(
        &self,
        file: &SourceFile,
        rust_ctx: Option<&crate::analysis::RustFileContext>,
    ) -> Vec<Finding> {
        if Self::is_embedded_schema_only_module(&file.content) {
            return Vec::new();
        }
        if crate::run_context::skip_unresolved_for_tests_path(&file.path) {
            return Vec::new();
        }
        if Self::looks_generated_or_build_output(&file.path, &file.content) {
            return Vec::new();
        }

        let ast_hints = if crate::run_context::enhanced_unresolved_for_path(&file.path) {
            rust_ctx
                .and_then(|ctx| ctx.ast.as_ref().ok())
                .map(unresolved_ast::analyze_rust_ast)
        } else {
            None
        };

        // 1. Collect all local `fn` definitions (regex ∪ syn)
        let mut defined_fns: HashSet<String> = HashSet::new();
        for line in &file.lines {
            for caps in self.rust_fn_def.captures_iter(line) {
                if let Some(name) = caps.get(1) {
                    defined_fns.insert(name.as_str().to_string());
                }
            }
        }
        if let Some(h) = &ast_hints {
            defined_fns.extend(h.defined_fns.iter().cloned());
        }

        // 2. Collect all function calls and check if they resolve
        let mut findings = Vec::new();
        for (i, line) in file.lines.iter().enumerate() {
            let trimmed = line.trim();
            // Skip comments, use statements, mod declarations, macro definitions
            if trimmed.starts_with("//")
                || trimmed.starts_with("/*")
                || trimmed.starts_with("* ")
                || trimmed.starts_with("use ")
                || trimmed.starts_with("mod ")
                || trimmed.starts_with("macro_rules!")
                || trimmed.starts_with("#[")
                || trimmed.starts_with("fn ")
                || trimmed.starts_with("pub fn ")
                || trimmed.starts_with("pub(crate) fn ")
                || trimmed.starts_with("async fn ")
                || trimmed.starts_with("pub async fn ")
            {
                continue;
            }

            if Self::line_looks_like_sql(line) {
                continue;
            }

            // This detector is deliberately conservative — only flag standalone
            // function calls (not method calls like `x.foo()` or qualified
            // paths like `module::foo()`). This avoids a flood of false positives.
            // We skip any call that contains `::` or `.` before the `(`.
            for caps in self.rust_fn_call.captures_iter(line) {
                if let Some(name_match) = caps.get(1) {
                    let name = name_match.as_str();
                    let start = name_match.start();

                    // Skip if preceded by `.` or `::`
                    if start > 0 {
                        let before = &line[..start];
                        if before.ends_with('.') || before.ends_with("::") {
                            continue;
                        }
                    }

                    if rust_byte_is_non_code(file, i + 1, start, rust_ctx) {
                        continue;
                    }

                    // Skip well-known functions, macros (ending with `!`), and locals
                    if Self::is_well_known_fn(name)
                        || defined_fns.contains(name)
                        || name.starts_with('_')
                    {
                        continue;
                    }

                    // Skip short names (likely closures or variables)
                    if name.len() < 3 {
                        continue;
                    }

                    if Self::file_imports_defaults_glob(file) && name.starts_with("default_") {
                        continue;
                    }

                    // When syn succeeded, only flag calls the AST actually recorded as `ExprCall`.
                    // Cuts regex-only matches (macros, odd spans, doc examples) that slip past token maps.
                    if let Some(h) = &ast_hints
                        && !h.call_sites.contains(&(i + 1, name.to_string()))
                        && !crate::run_context::feature_enabled("unresolved-regex-fallback")
                    {
                        continue;
                    }

                    // Imports: syn `UseTree` ∪ legacy substring scan (parse-fallback / edge cases).
                    let has_import = ast_hints
                        .as_ref()
                        .is_some_and(|h| h.use_imports.contains(name))
                        || file.lines.iter().any(|l| {
                            let t = l.trim();
                            t.starts_with("use ") && t.contains(name)
                        });

                    if !has_import && Self::file_has_high_fanout_glob_use(file) {
                        continue;
                    }

                    if !has_import {
                        let import_candidates = Self::import_line_candidates(file, name);
                        let confidence = ast_hints.as_ref().map(|h| {
                            if h.call_sites.contains(&(i + 1, name.to_string())) {
                                FindingConfidence::High
                            } else {
                                FindingConfidence::Medium
                            }
                        });
                        let evidence = Some(json!({
                            "importCandidates": import_candidates,
                            "symbolCandidates": [name],
                            "resolutionAttempt": "single-file heuristic + optional AST import map / call sites",
                        }));
                        crate::run_context::record_unresolved_callee(name);
                        findings.push(Finding {
                            rule_id: "unresolved-ref/fn-call".to_string(),
                            diagnostic_id: None,
                            rule_name: self.name().to_string(),
                            severity: Severity::Info, // conservative — might be from a wildcard import
                            file: file.path.clone(),
                            line: i + 1,
                            column: start,
                            message: format!(
                                "Function `{}` called but not defined or imported in this file",
                                name,
                            ),
                            suggestion: Some(format!(
                                "Add `use crate::some_module::{};` or verify the function exists.",
                                name,
                            )),
                            alternatives: vec![],
                            rationale: None,
                            context: file.context_around(i + 1, 1),
                            confidence,
                            evidence,
                        });
                    }
                }
            }
        }

        findings
    }
}

impl DetectionRule for UnresolvedRefDetector {
    fn id(&self) -> &'static str {
        "unresolved-ref"
    }
    fn name(&self) -> &'static str {
        "Unresolved Reference Detector"
    }
    fn description(&self) -> &'static str {
        "Detects function calls that don't appear to be defined or imported"
    }
    fn severity(&self) -> Severity {
        Severity::Info
    }
    fn minimal_repro(&self) -> Option<&'static str> {
        Some(
            "// VIOLATION — calling a function that is never defined or imported\n\
             fn process(data: Vec<u8>) -> Result<Output, Error> {\n\
             \x20   let parsed = parse_frame(data)?;  // parse_frame not defined here!\n\
             \x20   Ok(transform(parsed))\n\
             }\n\
             \n\
             // FIX — define the function or add the import\n\
             use crate::codec::parse_frame;  // bring it into scope\n\
             fn process(data: Vec<u8>) -> Result<Output, Error> {\n\
             \x20   let parsed = parse_frame(data)?;\n\
             \x20   Ok(transform(parsed))\n\
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
        match file.language {
            Language::Rust => self.detect_rust(file, rust_ctx),
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::RustFileContext;
    use std::path::PathBuf;

    fn source(ext: &str, code: &str) -> SourceFile {
        SourceFile::new(PathBuf::from(format!("test.{}", ext)), code.to_string())
    }

    #[test]
    fn no_findings_for_local_fns() {
        let d = UnresolvedRefDetector::new();
        let f = source(
            "rs",
            "fn helper() -> i32 { 42 }\n\nfn main() {\n    let x = helper();\n}",
        );
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "local fn calls should resolve");
    }

    #[test]
    fn no_findings_for_std_fns() {
        let d = UnresolvedRefDetector::new();
        let f = source("rs", "fn main() {\n    println!(\"hello\");\n}");
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "std fns should be excluded");
    }

    #[test]
    fn no_findings_for_embedded_sql_schema_const() {
        let d = UnresolvedRefDetector::new();
        let f = source(
            "rs",
            "pub const SCHEMA_X: &str = \"\n\
CREATE TABLE t (id INTEGER PRIMARY KEY);\n\
SELECT datetime('now');\n\
\";\n",
        );
        let findings = d.detect(&f, None);
        assert!(
            findings.is_empty(),
            "embedded SCHEMA_* SQL should not trigger fn-call heuristic"
        );
    }

    #[test]
    fn no_findings_for_default_fns_under_defaults_glob() {
        let d = UnresolvedRefDetector::new();
        let f = source(
            "rs",
            "use super::defaults::*;\n\nfn demo() -> u64 {\n    default_heartbeat_interval()\n}\n",
        );
        assert!(d.detect(&f, None).is_empty());
    }

    #[test]
    fn no_findings_when_prelude_glob_may_import() {
        let d = UnresolvedRefDetector::new();
        let f = source(
            "rs",
            "use some_crate::prelude::*;\n\nfn main() {\n    frobnicate_all();\n}\n",
        );
        assert!(
            d.detect(&f, None).is_empty(),
            "prelude::* may provide symbols not listed on a use line"
        );
    }

    #[test]
    fn flags_unknown_call_with_opaque_glob_import() {
        let d = UnresolvedRefDetector::new();
        let f = source(
            "rs",
            "use misc::internal::*;\n\nfn main() {\n    totally_unknown_fn();\n}\n",
        );
        assert!(
            !d.detect(&f, None).is_empty(),
            "arbitrary ::* globs must not suppress unresolved-call findings"
        );
    }

    #[test]
    fn skips_integration_test_tree_paths() {
        let d = UnresolvedRefDetector::new();
        let mut f = source("rs", "fn main() { totally_unknown_fn(); }\n");
        f.path = PathBuf::from("crates/acme/tests/it.rs");
        assert!(d.detect(&f, None).is_empty());
    }

    #[test]
    fn skips_fn_like_tokens_inside_double_quoted_strings() {
        let d = UnresolvedRefDetector::new();
        let f = source(
            "rs",
            r#"fn main() {
    let _ = "Rust compiler (`rustc --version`)";
}"#,
        );
        assert!(
            d.detect(&f, None).is_empty(),
            "prose inside string literals should not be scanned as Rust calls"
        );
    }

    #[test]
    fn skips_let_tuple_binding_open_paren() {
        let d = UnresolvedRefDetector::new();
        let f = source(
            "rs",
            "fn demo() {\n    let (a, b) = (1, 2);\n    let _ = a + b;\n}\n",
        );
        assert!(
            d.detect(&f, None).is_empty(),
            "`let (` tuple patterns are not function calls"
        );
    }

    #[test]
    fn skips_pub_crate_visibility() {
        let d = UnresolvedRefDetector::new();
        let f = source("rs", "pub(crate) fn demo() -> u32 { 0 }\n");
        assert!(
            d.detect(&f, None).is_empty(),
            "`pub(` visibility is not a call to `pub`"
        );
    }

    #[test]
    fn no_findings_for_module_qualified_call() {
        let d = UnresolvedRefDetector::new();
        let f = source(
            "rs",
            "mod m {\n    pub fn foo() {}\n}\nfn main() {\n    m::foo();\n}\n",
        );
        let ctx = RustFileContext::parse(&f.content);
        assert!(
            d.detect(&f, Some(&ctx)).is_empty(),
            "qualified calls `m::foo()` are intentionally out of scope"
        );
    }

    #[test]
    fn canary_scope_skips_ast_confidence_outside_listed_crates() {
        crate::run_context::init(crate::run_context::RunContext {
            canary_crates: Some(vec!["other-crate".into()]),
            ..Default::default()
        });
        let d = UnresolvedRefDetector::new();
        let mut f = source("rs", "fn main() { totally_unknown_z99(); }\n");
        f.path = PathBuf::from("crates/my-crate/src/lib.rs");
        let ctx = RustFileContext::parse(&f.content);
        let findings = d.detect(&f, Some(&ctx));
        assert!(
            findings.iter().all(|x| x.confidence.is_none()),
            "expected no AST confidence outside canary crates: {findings:?}"
        );
        crate::run_context::reset();
    }
}
