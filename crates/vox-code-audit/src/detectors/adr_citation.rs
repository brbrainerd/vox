use crate::diagnostics::catalog;
use crate::rules::{DetectionRule, Finding, FindingConfidence, Language, Severity, SourceFile};
use regex::Regex;

/// Detects public functions in critical crates that lack ADR/TASK citations in their doc comments.
pub struct AdrCitationDetector {
    pub_fn_pattern: Regex,
    adr_task_pattern: Regex,
    t_number_pattern: Regex,
    supported_langs: Vec<Language>,
}

impl Default for AdrCitationDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl AdrCitationDetector {
    pub fn new() -> Self {
        Self {
            pub_fn_pattern: Regex::new(
                r"^\s*pub\s+(?:(?:async|unsafe|extern\s+\S+)\s+)?fn\s+(\w+)",
            )
            .expect("valid regex"),
            adr_task_pattern: Regex::new(r"ADR-\d+|TASK-\d+\.\d+|Phase\s+\d+")
                .expect("valid regex"),
            t_number_pattern: Regex::new(r"\bT\d{3,}\b").expect("valid regex"),
            supported_langs: vec![Language::Rust],
        }
    }

    fn is_critical_crate(file: &SourceFile) -> bool {
        let path_str = file.path.to_string_lossy();
        path_str.contains("vox-runtime")
            || path_str.contains("vox-orchestrator")
            || path_str.contains("vox-compiler")
    }
}

impl DetectionRule for AdrCitationDetector {
    fn id(&self) -> &'static str {
        "doc/missing-adr-citation"
    }

    fn name(&self) -> &'static str {
        "Missing ADR Citation Detector"
    }

    fn description(&self) -> &'static str {
        "Detects public functions in critical crates whose doc comments lack an ADR-NNN or TASK-N.M citation."
    }

    fn severity(&self) -> Severity {
        Severity::Info
    }

    fn languages(&self) -> &[Language] {
        &self.supported_langs
    }

    fn diagnostic_id(&self) -> Option<&'static str> {
        Some(catalog::DOC_MISSING_ADR_CITATION)
    }

    fn explain(&self) -> &'static str {
        "Public APIs in critical crates (vox-runtime, vox-orchestrator, vox-compiler) must cite \
        the ADR or task that motivated their shape. This makes automated refactors traceable and \
        prevents LLMs from re-inventing design that was already decided.\n\n\
        Bad:  /// Processes a workflow step.\n      pub fn process_step(...) {}\n\n\
        Good: /// ADR-042: Processes a workflow step per the orchestration spec.\n      pub fn process_step(...) {}"
    }

    fn minimal_repro(&self) -> Option<&'static str> {
        Some(
            "// VIOLATION — pub fn in critical crate with no ADR/TASK citation\n\
             /// Executes a workflow activity step.\n\
             pub fn execute_step(ctx: &Context, step: Step) -> Result<Output, Error> {\n\
             \x20   // ...\n\
             }\n\
             \n\
             // FIX — cite the ADR that governs this function's interface\n\
             /// ADR-042: Executes a workflow activity step per the orchestration spec.\n\
             /// See also: TASK-3.2 (activity execution contract).\n\
             pub fn execute_step(ctx: &Context, step: Step) -> Result<Output, Error> {\n\
             \x20   // ...\n\
             }",
        )
    }

    fn detect(
        &self,
        file: &SourceFile,
        _rust_ctx: Option<&crate::analysis::RustFileContext>,
    ) -> Vec<Finding> {
        if file.language != Language::Rust {
            return vec![];
        }

        let is_critical = Self::is_critical_crate(file);
        let mut findings = Vec::new();

        for (i, line) in file.lines.iter().enumerate() {
            let line_num = i + 1;

            // Check for a public fn declaration
            if let Some(caps) = self.pub_fn_pattern.captures(line) {
                let fn_name = caps.get(1).map_or("", |m| m.as_str());

                // Collect the preceding ≤ 10 lines as the potential doc comment block
                let look_back_start = i.saturating_sub(10);
                let preceding: String = file.lines[look_back_start..i].join("\n");

                // Check for wrong citation scheme (T-numbers)
                if let Some(t_match) = self.t_number_pattern.find(&preceding) {
                    let t_ref = t_match.as_str().to_string();
                    findings.push(Finding {
                        rule_id: self.id().to_string(),
                        diagnostic_id: self.diagnostic_id().map(str::to_string),
                        rule_name: self.name().to_string(),
                        severity: Severity::Warning,
                        file: file.path.clone(),
                        line: line_num,
                        column: 0,
                        message: format!(
                            "T-number citation (`{t_ref}`) found — Vox uses `ADR-NNN` / `TASK-N.M` scheme, not T-numbers."
                        ),
                        suggestion: Some(
                            "Add `/// ADR-NNN` or `/// TASK-N.M` to the doc comment. See docs/src/adr/ for the ADR index.".into(),
                        ),
                        alternatives: vec![],
                        rationale: Some(
                            "Public APIs in critical crates must cite the ADR or task that motivated their shape. \
                            This makes automated refactors traceable and prevents LLMs from re-inventing design that was already decided.".into(),
                        ),
                        context: file.context_around(line_num, 2),
                        confidence: Some(FindingConfidence::High),
                        evidence: None,
                    });
                    continue;
                }

                // Check for missing citation
                if !self.adr_task_pattern.is_match(&preceding) {
                    let severity = if is_critical {
                        Severity::Warning
                    } else {
                        Severity::Info
                    };

                    findings.push(Finding {
                        rule_id: self.id().to_string(),
                        diagnostic_id: self.diagnostic_id().map(str::to_string),
                        rule_name: self.name().to_string(),
                        severity,
                        file: file.path.clone(),
                        line: line_num,
                        column: 0,
                        message: format!(
                            "Public fn `{fn_name}` in a critical crate has no ADR/TASK citation in its doc comment."
                        ),
                        suggestion: Some(
                            "Add `/// ADR-NNN` or `/// TASK-N.M` to the doc comment. See docs/src/adr/ for the ADR index.".into(),
                        ),
                        alternatives: vec![],
                        rationale: Some(
                            "Public APIs in critical crates must cite the ADR or task that motivated their shape. \
                            This makes automated refactors traceable and prevents LLMs from re-inventing design that was already decided.".into(),
                        ),
                        context: file.context_around(line_num, 2),
                        confidence: Some(FindingConfidence::Medium),
                        evidence: None,
                    });
                }
            }
        }

        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn source_critical(code: &str) -> SourceFile {
        SourceFile::new(
            PathBuf::from("crates/vox-runtime/src/lib.rs"),
            code.to_string(),
        )
    }

    fn source_normal(code: &str) -> SourceFile {
        SourceFile::new(
            PathBuf::from("crates/vox-utils/src/lib.rs"),
            code.to_string(),
        )
    }

    #[test]
    fn fires_on_pub_fn_without_adr_in_critical_crate() {
        let d = AdrCitationDetector::new();
        let code = "/// Does something important.\npub fn process_step() {}";
        let f = source_critical(code);
        let findings = d.detect(&f, None);
        assert!(!findings.is_empty(), "should fire for missing ADR citation");
        assert_eq!(findings[0].severity, Severity::Warning);
        assert!(findings[0].message.contains("process_step"));
    }

    #[test]
    fn does_not_fire_when_adr_present() {
        let d = AdrCitationDetector::new();
        let code = "/// ADR-042: Processes a workflow step.\npub fn process_step() {}";
        let f = source_critical(code);
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "should not fire when ADR is cited");
    }

    #[test]
    fn fires_info_on_non_critical_crate() {
        let d = AdrCitationDetector::new();
        let code = "/// Helper utility.\npub fn helper() {}";
        let f = source_normal(code);
        let findings = d.detect(&f, None);
        assert!(
            !findings.is_empty(),
            "should fire even in non-critical crates"
        );
        assert_eq!(
            findings[0].severity,
            Severity::Info,
            "non-critical should be Info"
        );
    }

    #[test]
    fn fires_warning_on_t_number_citation() {
        let d = AdrCitationDetector::new();
        let code = "/// T1234: Does something.\npub fn do_thing() {}";
        let f = source_critical(code);
        let findings = d.detect(&f, None);
        assert!(!findings.is_empty(), "should fire for T-number citation");
        assert_eq!(findings[0].severity, Severity::Warning);
        assert!(findings[0].message.contains("T-number"));
    }

    #[test]
    fn does_not_fire_for_task_citation() {
        let d = AdrCitationDetector::new();
        let code = "/// TASK-3.7: Implements the activation protocol.\npub fn activate() {}";
        let f = source_critical(code);
        let findings = d.detect(&f, None);
        assert!(
            findings.is_empty(),
            "TASK-N.M citation should satisfy the rule"
        );
    }
}
