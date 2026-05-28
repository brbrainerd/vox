//! LF line endings for scanned source languages (parity with `vox ci line-endings` policy).

use crate::rules::{DetectionRule, Finding, Language, Severity, SourceFile};

/// Warns when a scanned source file contains CR (`\r`), e.g. CRLF line endings.
pub struct LineEndingDetector;

impl Default for LineEndingDetector {
    fn default() -> Self {
        Self
    }
}

impl LineEndingDetector {
    /// Constructs a line-ending policy detector.
    pub fn new() -> Self {
        Self
    }

    fn first_cr_line(content: &str) -> usize {
        let mut line = 1usize;
        for ch in content.chars() {
            if ch == '\r' {
                return line;
            }
            if ch == '\n' {
                line += 1;
            }
        }
        1
    }
}

impl DetectionRule for LineEndingDetector {
    fn id(&self) -> &'static str {
        "cross-platform/line-endings"
    }

    fn name(&self) -> &'static str {
        "Line ending (LF) policy"
    }

    fn description(&self) -> &'static str {
        "Flags CR / CRLF in source files; align with `.gitattributes` and `vox ci line-endings`"
    }

    fn severity(&self) -> Severity {
        Severity::Warning
    }

    fn minimal_repro(&self) -> Option<&'static str> {
        Some(
            "# VIOLATION — file has Windows CRLF line endings (\\r\\n)\n\
             # Each line ends with \\r\\n instead of \\n\n\
             # This causes diff noise and may break Unix tooling\n\
             \n\
             # FIX — configure your editor/git to use LF endings\n\
             # .gitattributes:\n\
             * text=auto eol=lf\n\
             # Or convert with: git add --renormalize .",
        )
    }

    fn languages(&self) -> &[Language] {
        &[
            Language::Rust,
            Language::TypeScript,
            Language::Python,
            Language::GDScript,
            Language::Vox,
        ]
    }

    fn detect(
        &self,
        file: &SourceFile,
        _rust: Option<&crate::analysis::RustFileContext>,
    ) -> Vec<Finding> {
        if !file.content.contains('\r') {
            return Vec::new();
        }
        let line = Self::first_cr_line(&file.content);
        vec![Finding {
            rule_id: "cross-platform/crlf".to_string(),
            diagnostic_id: None,
            rule_name: self.name().to_string(),
            severity: self.severity(),
            file: file.path.clone(),
            line,
            column: 0,
            message: "Carriage return (`\\r`) detected — prefer LF-only line endings for cross-platform CI"
                .to_string(),
            suggestion: Some(
                "Re-save as LF, or run `vox ci line-endings` on your changes; see `.editorconfig`."
                    .to_string(),
            ),
            alternatives: vec![],
            rationale: None,
            context: String::new(),
            confidence: None,
            evidence: None,
        }]
    }
}
