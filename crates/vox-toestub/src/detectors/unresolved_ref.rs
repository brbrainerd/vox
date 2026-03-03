use crate::rules::{DetectionRule, Finding, Language, Severity, SourceFile};
use regex::Regex;

/// Detects references to symbols (functions, types, modules) that appear to
/// be undefined within the file's scope.
///
/// Phase 1: Simple heuristic — looks for `use` imports pointing at unknown
/// crate-internal modules and function calls that don't match any `fn` definition
/// in the same file. Full cross-crate resolution is a Phase 2 feature.
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
    pub fn new() -> Self {
        Self {
            rust_fn_call: Regex::new(r"\b([a-z_]\w*)\s*\(").expect("valid regex"),
            rust_fn_def: Regex::new(r"(?:pub\s+)?(?:async\s+)?fn\s+([a-z_]\w*)\s*[<(]")
                .expect("valid regex"),
        }
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
        )
    }

    fn detect_rust(&self, file: &SourceFile) -> Vec<Finding> {
        // 1. Collect all local `fn` definitions
        let mut defined_fns: Vec<String> = Vec::new();
        for line in &file.lines {
            for caps in self.rust_fn_def.captures_iter(line) {
                if let Some(name) = caps.get(1) {
                    defined_fns.push(name.as_str().to_string());
                }
            }
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

                    // Skip well-known functions, macros (ending with `!`), and locals
                    if Self::is_well_known_fn(name)
                        || defined_fns.contains(&name.to_string())
                        || name.starts_with('_')
                    {
                        continue;
                    }

                    // Skip short names (likely closures or variables)
                    if name.len() < 3 {
                        continue;
                    }

                    // Check if there's an import that could provide this function
                    let has_import = file.lines.iter().any(|l| {
                        let t = l.trim();
                        t.starts_with("use ") && t.contains(name)
                    });

                    if !has_import {
                        findings.push(Finding {
                            rule_id: "unresolved-ref/fn-call".to_string(),
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
                            context: file.context_around(i + 1, 1),
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
    fn languages(&self) -> &[Language] {
        &[Language::Rust]
    }

    fn detect(&self, file: &SourceFile) -> Vec<Finding> {
        match file.language {
            Language::Rust => self.detect_rust(file),
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
    fn no_findings_for_local_fns() {
        let d = UnresolvedRefDetector::new();
        let f = source(
            "rs",
            "fn helper() -> i32 { 42 }\n\nfn main() {\n    let x = helper();\n}",
        );
        let findings = d.detect(&f);
        assert!(findings.is_empty(), "local fn calls should resolve");
    }

    #[test]
    fn no_findings_for_std_fns() {
        let d = UnresolvedRefDetector::new();
        let f = source("rs", "fn main() {\n    println!(\"hello\");\n}");
        let findings = d.detect(&f);
        assert!(findings.is_empty(), "std fns should be excluded");
    }
}
