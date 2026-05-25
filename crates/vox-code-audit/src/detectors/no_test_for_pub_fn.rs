use regex::Regex;

use crate::rules::{DetectionRule, Finding, FindingConfidence, Language, Severity, SourceFile};

/// Detects exported `fn` declarations in `examples/golden/` Vox files that are not
/// exercised by any `@test` block in the same file.
///
/// Vox golden examples feed the MENS training corpus. A callable function with no
/// `@test` calling it produces a weaker training signal — the model learns the
/// output without learning the intention. This rule enforces the @test-first gate
/// described in `docs/src/contributors/contribution-loop.md`.
pub struct NoTestForPubFnDetector {
    fn_decl_re: Regex,
    test_call_re: Regex,
}

impl Default for NoTestForPubFnDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl NoTestForPubFnDetector {
    pub fn new() -> Self {
        Self {
            // Matches bare `fn name(` in Vox (not preceded by @test on the same line)
            fn_decl_re: Regex::new(r"^\s*fn\s+([a-z_][a-zA-Z0-9_]*)\s*[(\{]").expect("valid"),
            // Matches any call expression `name(` inside @test blocks
            test_call_re: Regex::new(r"\b([a-z_][a-zA-Z0-9_]*)\s*\(").expect("valid"),
        }
    }

    fn is_golden_path(file: &SourceFile) -> bool {
        file.path.components().any(|c| c.as_os_str() == "golden")
            && file.path.components().any(|c| c.as_os_str() == "examples")
    }
}

impl DetectionRule for NoTestForPubFnDetector {
    fn id(&self) -> &'static str {
        "skeleton/no-test-for-pub-fn"
    }

    fn name(&self) -> &'static str {
        "No Test For Exported Fn"
    }

    fn description(&self) -> &'static str {
        "Exported fn in examples/golden/ Vox file is not called from any @test block"
    }

    fn severity(&self) -> Severity {
        Severity::Warning
    }

    fn languages(&self) -> &[Language] {
        &[Language::Vox]
    }

    fn minimal_repro(&self) -> Option<&'static str> {
        Some(
            "// VIOLATION — public fn with no @test annotation anywhere in the file\n\
             pub fn parse_invoice(raw: str) -> Invoice {\n\
             \x20   Invoice::from_json(raw)\n\
             }\n\
             \n\
             // FIX — add a @test block\n\
             pub fn parse_invoice(raw: str) -> Invoice {\n\
             \x20   Invoice::from_json(raw)\n\
             }\n\
             \n\
             @test\n\
             fn test_parse_invoice_roundtrip() {\n\
             \x20   let inv = parse_invoice('{\"id\":\"1\",\"amount\":42}')\n\
             \x20   assert(inv.amount == 42)\n\
             }",
        )
    }

    fn detect(
        &self,
        file: &SourceFile,
        _rust_ctx: Option<&crate::analysis::RustFileContext>,
    ) -> Vec<Finding> {
        if file.language != Language::Vox || !Self::is_golden_path(file) {
            return vec![];
        }

        // Pass 1: collect non-@test fn declarations (name → line).
        // Pass 2: collect all names called from @test blocks.
        let mut plain_fns: Vec<(String, usize)> = Vec::new();
        let mut in_test_block = false;
        let mut brace_depth: i32 = 0;
        let mut tested_names: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut prev_was_test_attr = false;

        for (i, line) in file.lines.iter().enumerate() {
            let trimmed = line.trim();

            // Detect `@test` attribute line.
            if trimmed == "@test" || trimmed.starts_with("@test ") {
                prev_was_test_attr = true;
                continue;
            }

            if let Some(caps) = self.fn_decl_re.captures(line) {
                let fn_name = caps[1].to_string();
                if prev_was_test_attr {
                    // This fn IS the @test entry point; entering its body.
                    in_test_block = true;
                    brace_depth = 0;
                } else {
                    plain_fns.push((fn_name, i + 1));
                }
                prev_was_test_attr = false;
            } else {
                prev_was_test_attr = false;
            }

            if in_test_block {
                for ch in line.chars() {
                    match ch {
                        '{' => brace_depth += 1,
                        '}' => {
                            brace_depth -= 1;
                            if brace_depth <= 0 {
                                in_test_block = false;
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                for caps in self.test_call_re.captures_iter(line) {
                    tested_names.insert(caps[1].to_string());
                }
            }
        }

        // Emit a finding for every plain fn whose name never appears in a @test block.
        plain_fns
            .into_iter()
            .filter(|(name, _)| {
                if file.path.to_string_lossy().contains("toestub-ignore") {
                    return false;
                }
                // Suppress via inline annotation on the fn's line.
                let line_text = file
                    .lines
                    .get(name.len().saturating_sub(1))
                    .map(String::as_str)
                    .unwrap_or("");
                if line_text.contains("toestub-ignore") {
                    return false;
                }
                !tested_names.contains(name.as_str())
            })
            .map(|(name, line)| Finding {
                rule_id: self.id().to_string(),
                diagnostic_id: None,
                rule_name: self.name().to_string(),
                severity: self.severity(),
                file: file.path.clone(),
                line,
                column: 0,
                message: format!(
                    "fn `{}` has no `@test` block calling it in this golden example file",
                    name
                ),
                suggestion: Some(format!(
                    "Add an `@test` fn that calls `{}(...)` before implementing it (see contribution-loop.md §@test-first).",
                    name
                )),
                alternatives: vec![],
                rationale: None,
                context: file.context_around(line, 2),
                confidence: Some(FindingConfidence::Medium),
                evidence: Some(serde_json::json!({ "fn_name": name })),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn golden_source(code: &str) -> SourceFile {
        SourceFile::new(
            PathBuf::from("examples/golden/my_feature.vox"),
            code.to_string(),
        )
    }

    fn non_golden_source(code: &str) -> SourceFile {
        SourceFile::new(PathBuf::from("scripts/setup.vox"), code.to_string())
    }

    #[test]
    fn flags_fn_with_no_test() {
        let d = NoTestForPubFnDetector::new();
        let src = golden_source("fn greet(name: str) to str {\n    return \"hello\"\n}\n");
        let findings = d.detect(&src, None);
        assert!(
            findings
                .iter()
                .any(|f| f.rule_id == "skeleton/no-test-for-pub-fn"),
            "should flag fn with no @test"
        );
    }

    #[test]
    fn passes_fn_called_from_test() {
        let d = NoTestForPubFnDetector::new();
        let src = golden_source(
            "fn greet(name: str) to str {\n    return \"hello\"\n}\n\n@test\nfn test_greet() {\n    let r = greet(\"world\")\n    assert(r is \"hello\")\n}\n",
        );
        let findings = d.detect(&src, None);
        assert!(
            findings.is_empty(),
            "should not flag fn that is called from @test"
        );
    }

    #[test]
    fn skips_test_fn_itself() {
        let d = NoTestForPubFnDetector::new();
        let src = golden_source("@test\nfn test_standalone() {\n    assert(true)\n}\n");
        let findings = d.detect(&src, None);
        assert!(
            findings.is_empty(),
            "should not flag @test-decorated fn itself"
        );
    }

    #[test]
    fn skips_non_golden_vox_file() {
        let d = NoTestForPubFnDetector::new();
        let src = non_golden_source("fn helper() {\n    return 0\n}\n");
        let findings = d.detect(&src, None);
        assert!(
            findings.is_empty(),
            "should not flag fns outside examples/golden/"
        );
    }

    #[test]
    fn flags_only_untested_fns_in_mixed_file() {
        let d = NoTestForPubFnDetector::new();
        // greet is tested; farewell is not.
        let src = golden_source(
            "fn greet(name: str) to str {\n    return \"hi\"\n}\n\nfn farewell(name: str) to str {\n    return \"bye\"\n}\n\n@test\nfn test_greet() {\n    let r = greet(\"x\")\n    assert(r is \"hi\")\n}\n",
        );
        let findings = d.detect(&src, None);
        assert_eq!(findings.len(), 1, "only farewell should be flagged");
        assert!(findings[0].message.contains("farewell"));
    }
}
