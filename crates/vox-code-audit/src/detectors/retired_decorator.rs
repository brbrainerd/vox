use crate::diagnostics::catalog;
use crate::rules::{DetectionRule, Finding, FindingConfidence, Language, Severity, SourceFile};
use regex::Regex;

/// Detects retired Vox decorator and import forms per
/// [`AGENTS.md` §Retired Surfaces (LLM Guard)](../../../../../AGENTS.md).
///
/// Each pattern has a canonical replacement that the agent should use instead.
/// This detector is the first slice of the CR-L6 retirement-guard parity gate
/// (council ratified 2026-05-15, D6/D25 in
/// [`v1-llm-target-implementation-plan-2026.md`](../../../../../docs/src/architecture/v1-llm-target-implementation-plan-2026.md)
/// §8.1). The remaining retirement-guard rules (`recall()`, `@capacitor/*`,
/// `axum::serve` in generated apps, `rust-embed` in generated apps,
/// `vox-sherpa-transcribe`) land in implementation-plan P1.4.
///
/// Patterns covered by this detector:
///
/// | Retired form                       | Canonical replacement                  |
/// |------------------------------------|----------------------------------------|
/// | `@component fn Name()`             | `component Name() { ... }`             |
/// | `@endpoint(kind: server) fn ...`   | `@server fn ...`                       |
/// | `@endpoint(kind: query) fn ...`    | `@query fn ...`                        |
/// | `@endpoint(kind: mutation) fn ...` | `@mutation fn ...`                     |
/// | `@py.import ...`                   | (removed; Python interop retired)      |
///
/// **2026-05-23 direction flip:** an earlier version of this detector had
/// the endpoint direction backwards (treating `@server`/`@query`/`@mutation`
/// as retired). Phase B of the decorator audit
/// ([vox-stdlib-gap-audit-2026-05-23.md](../../../../../docs/src/architecture/vox-stdlib-gap-audit-2026-05-23.md)
/// §11.2 / commit f8dc41a9f1) introduced the three bare-form decorators as
/// the canonical surface and retired `@endpoint(kind: ...)` to the
/// deprecation queue (Phase H). The corpus migration in that same commit
/// rewrote 76 of 76 call sites. This detector was updated 2026-05-24 to
/// match the post-Phase-B reality.
///
/// Severity is `Warning` at land; the [vox-language-rules Phase 2 plan](../../../../../docs/src/architecture/vox-language-rules-phase2-lint-extension-2026.md)
/// describes the escalation path to `Error` after one minor version.
pub struct RetiredDecoratorDetector {
    component_fn: Regex,
    endpoint_kind: Regex,
    py_import: Regex,
    supported_langs: Vec<Language>,
}

impl Default for RetiredDecoratorDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl RetiredDecoratorDetector {
    pub fn new() -> Self {
        Self {
            component_fn: Regex::new(r"@component\s+fn\b").expect("valid regex"),
            // Match `@endpoint(kind: server|query|mutation)`. Whitespace is
            // tolerated around the colon. Captures the kind for the
            // suggestion message.
            endpoint_kind: Regex::new(r"@endpoint\s*\(\s*kind\s*:\s*(server|query|mutation)\s*\)")
                .expect("valid regex"),
            py_import: Regex::new(r"@py\.import\b").expect("valid regex"),
            supported_langs: vec![Language::Vox],
        }
    }

    fn build_finding(
        &self,
        file: &SourceFile,
        line: usize,
        column: usize,
        message: String,
        suggestion: String,
        rationale: &'static str,
    ) -> Finding {
        Finding {
            rule_id: self.id().to_string(),
            diagnostic_id: self.diagnostic_id().map(str::to_string),
            rule_name: self.name().to_string(),
            severity: Severity::Warning,
            file: file.path.clone(),
            line,
            column,
            message,
            suggestion: Some(suggestion),
            alternatives: vec![],
            rationale: Some(rationale.to_string()),
            context: file.context_around(line, 2),
            confidence: Some(FindingConfidence::High),
            evidence: None,
        }
    }
}

impl DetectionRule for RetiredDecoratorDetector {
    fn id(&self) -> &'static str {
        "retired/decorator-usage"
    }

    fn name(&self) -> &'static str {
        "Retired Decorator Usage Detector"
    }

    fn description(&self) -> &'static str {
        "Detects decorator and import forms retired per AGENTS.md §Retired Surfaces (LLM Guard)."
    }

    fn severity(&self) -> Severity {
        Severity::Warning
    }

    fn languages(&self) -> &[Language] {
        &self.supported_langs
    }

    fn diagnostic_id(&self) -> Option<&'static str> {
        Some(catalog::RETIRED_DECORATOR_USAGE)
    }

    fn explain(&self) -> &'static str {
        "AGENTS.md §Retired Surfaces lists decorator and import forms retired in favor of \
canonical alternatives. LLMs trained on pre-2026 corpora may emit these; this lint \
catches them at audit time so the agent can rewrite to the canonical form.\n\n\
Retired → Canonical:\n\
  @component fn Name() {...}        →  component Name() {...}\n\
  @endpoint(kind: server) fn ...    →  @server fn ...\n\
  @endpoint(kind: query) fn ...     →  @query fn ...\n\
  @endpoint(kind: mutation) fn ...  →  @mutation fn ...\n\
  @py.import ...                    →  Python interop retired; use Vox-native or external HTTP.\n\n\
This detector is part of the CR-L6 retirement-guard parity gate. The \
endpoint-direction was flipped 2026-05-24 to match Phase B (commit \
f8dc41a9f1) which introduced the bare-form decorators as canonical. \
Severity escalates to Error one minor version after Phase H @endpoint \
retirement lands."
    }

    fn detect(
        &self,
        file: &SourceFile,
        _rust_ctx: Option<&crate::analysis::RustFileContext>,
    ) -> Vec<Finding> {
        if file.language != Language::Vox {
            return vec![];
        }

        let mut findings = Vec::new();

        for (i, line) in file.lines.iter().enumerate() {
            let line_num = i + 1;
            let trimmed = line.trim();

            // Skip comment-style lines (vox uses `//` and `/*`; some glue scripts use `#`).
            if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('#') {
                continue;
            }

            if let Some(m) = self.component_fn.find(line) {
                findings.push(self.build_finding(
                    file,
                    line_num,
                    m.start() + 1,
                    "Retired form `@component fn` — use the bare `component` keyword instead."
                        .to_string(),
                    "Replace `@component fn Name()` with `component Name() { ... }`. The bare \
                     `component` keyword is canonical per AGENTS.md §Grammar Unification."
                        .to_string(),
                    "AGENTS.md §Retired Surfaces: `@component fn` was retired during the 2026-Q1 \
                     grammar unification. The bare `component` keyword opens its own scope with \
                     component-specific rules and replaces the decorator+fn pair.",
                ));
            }

            if let Some(caps) = self.endpoint_kind.captures(line) {
                let kind = caps
                    .get(1)
                    .map(|m| m.as_str())
                    .expect("regex group 1 always present");
                let full = caps
                    .get(0)
                    .expect("regex group 0 always present");
                findings.push(self.build_finding(
                    file,
                    line_num,
                    full.start() + 1,
                    format!(
                        "Retired form `@endpoint(kind: {kind})` — use bare `@{kind}` decorator instead."
                    ),
                    format!(
                        "Replace `@endpoint(kind: {kind})` with `@{kind}`. The bare-form \
                         decorators introduced in Phase B (audit doc §11.2) are the canonical \
                         surface; `@endpoint(kind: ...)` is queued for retirement in Phase H."
                    ),
                    "AGENTS.md §Retired Surfaces: `@endpoint(kind: server|query|mutation)` was \
                     superseded by the bare `@server` / `@query` / `@mutation` decorators in \
                     Phase B (2026-05-23, commit f8dc41a9f1). The bare forms produce the same \
                     `EndpointDecl` AST node — pure grammar simplification, no behavior change.",
                ));
            }

            if let Some(m) = self.py_import.find(line) {
                findings.push(self.build_finding(
                    file,
                    line_num,
                    m.start() + 1,
                    "Retired form `@py.import` — Python interop has been removed.".to_string(),
                    "Replace with a Vox-native equivalent or call the upstream service via HTTP. \
                     If Python automation glue is needed, port the script to `.vox` per AGENTS.md \
                     §VoxScript-First Glue Code."
                        .to_string(),
                    "AGENTS.md §Retired Surfaces + §VoxScript-First Glue Code: Python is no \
                     longer a Vox glue surface. `@py.import` directives leak Python-side state \
                     into the Vox compiler and cannot be analyzed by the effect system.",
                ));
            }
        }

        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn source(code: &str) -> SourceFile {
        SourceFile::new(PathBuf::from("test.vox"), code.to_string())
    }

    #[test]
    fn flags_at_component_fn() {
        let d = RetiredDecoratorDetector::new();
        let f = source("@component fn Dashboard() {}");
        let findings = d.detect(&f, None);
        assert_eq!(findings.len(), 1, "should flag `@component fn`");
        assert!(findings[0].message.contains("@component fn"));
        assert_eq!(findings[0].severity, Severity::Warning);
        assert_eq!(
            findings[0].diagnostic_id.as_deref(),
            Some(catalog::RETIRED_DECORATOR_USAGE)
        );
    }

    #[test]
    fn does_not_flag_bare_component_keyword() {
        let d = RetiredDecoratorDetector::new();
        let f = source("component Dashboard() {}");
        let findings = d.detect(&f, None);
        assert!(
            findings.is_empty(),
            "bare `component` keyword is canonical, not retired"
        );
    }

    // 2026-05-24: detector direction flipped to match Phase B reality.
    // `@server` / `@query` / `@mutation` are canonical; `@endpoint(kind: ...)`
    // is the retired form.

    #[test]
    fn flags_endpoint_kind_server() {
        let d = RetiredDecoratorDetector::new();
        let f = source("@endpoint(kind: server) fn list_items() {}");
        let findings = d.detect(&f, None);
        assert_eq!(findings.len(), 1, "should flag `@endpoint(kind: server)`");
        assert!(findings[0].message.contains("@endpoint(kind: server)"));
        assert!(findings[0].suggestion.as_ref().unwrap().contains("@server"));
    }

    #[test]
    fn flags_endpoint_kind_query() {
        let d = RetiredDecoratorDetector::new();
        let f = source("@endpoint(kind: query) fn list_items() {}");
        let findings = d.detect(&f, None);
        assert_eq!(findings.len(), 1, "should flag `@endpoint(kind: query)`");
        assert!(findings[0].message.contains("@endpoint(kind: query)"));
        assert!(findings[0].suggestion.as_ref().unwrap().contains("@query"));
    }

    #[test]
    fn flags_endpoint_kind_mutation() {
        let d = RetiredDecoratorDetector::new();
        let f = source("@endpoint(kind: mutation) fn add_item() {}");
        let findings = d.detect(&f, None);
        assert_eq!(findings.len(), 1, "should flag `@endpoint(kind: mutation)`");
        assert!(findings[0].message.contains("@endpoint(kind: mutation)"));
        assert!(findings[0].suggestion.as_ref().unwrap().contains("@mutation"));
    }

    #[test]
    fn does_not_flag_bare_decorators() {
        // The bare-form decorators are canonical post-Phase B.
        let d = RetiredDecoratorDetector::new();
        for src in ["@server fn x() {}", "@query fn y() {}", "@mutation fn z() {}"] {
            let f = source(src);
            let findings = d.detect(&f, None);
            assert!(
                findings.is_empty(),
                "bare-form decorator should be canonical, not retired: {src}",
            );
        }
    }

    #[test]
    fn does_not_flag_canonical_bare_decorators() {
        // 2026-05-24 direction flip: bare-form decorators are canonical
        // post-Phase B. The legacy `@endpoint(kind: ...)` form is the
        // retired one (Phase H queue).
        let d = RetiredDecoratorDetector::new();
        for src in [
            "@server fn list_items() {}",
            "@query fn get_count() {}",
            "@mutation fn add_one() {}",
        ] {
            let f = source(src);
            let findings = d.detect(&f, None);
            assert!(
                findings.is_empty(),
                "canonical bare-form decorator should not fire: {src}",
            );
        }
    }

    #[test]
    fn flags_at_py_import() {
        let d = RetiredDecoratorDetector::new();
        let f = source("@py.import pandas as pd");
        let findings = d.detect(&f, None);
        assert_eq!(findings.len(), 1, "should flag `@py.import`");
        assert!(findings[0].message.contains("@py.import"));
    }

    #[test]
    fn ignores_comment_lines() {
        let d = RetiredDecoratorDetector::new();
        let f = source(
            "// @component fn Dashboard() {}\n// @server fn x() {}\n// @py.import pandas",
        );
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "comment lines should be skipped");
    }

    #[test]
    fn ignores_block_comment_lines() {
        let d = RetiredDecoratorDetector::new();
        let f = source("/* @component fn Dashboard() {} */");
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "block-comment lines should be skipped");
    }

    #[test]
    fn does_not_fire_on_rust_files() {
        let d = RetiredDecoratorDetector::new();
        let f = SourceFile::new(
            PathBuf::from("test.rs"),
            "@component fn Dashboard() {}".to_string(),
        );
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "rust files should be ignored");
    }

    #[test]
    fn flags_all_three_endpoint_kinds_independently() {
        let d = RetiredDecoratorDetector::new();
        let f = source(
            "@endpoint(kind: server) fn a() {}\n\
             @endpoint(kind: query) fn b() {}\n\
             @endpoint(kind: mutation) fn c() {}",
        );
        let findings = d.detect(&f, None);
        assert_eq!(
            findings.len(),
            3,
            "should flag all three retired `@endpoint(kind: ...)` forms independently"
        );
    }

    #[test]
    fn flags_mixed_retirement_patterns_in_one_file() {
        let d = RetiredDecoratorDetector::new();
        let f = source(
            "@component fn Dashboard() {}\n\
             @endpoint(kind: server) fn list() {}\n\
             @py.import os",
        );
        let findings = d.detect(&f, None);
        assert_eq!(
            findings.len(),
            3,
            "should flag component + endpoint(kind:server) + py.import on three separate lines"
        );
    }

    #[test]
    fn finding_has_high_confidence() {
        let d = RetiredDecoratorDetector::new();
        let f = source("@component fn Foo() {}");
        let findings = d.detect(&f, None);
        assert_eq!(findings[0].confidence, Some(FindingConfidence::High));
    }
}
