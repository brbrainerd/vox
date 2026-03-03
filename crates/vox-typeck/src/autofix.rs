use crate::diagnostics::Diagnostic;
use serde::{Deserialize, Serialize};

/// A suggested fix for a diagnostic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixSuggestion {
    pub message: String,
    pub diff: String,
    pub explanation: String,
}

pub trait AutoFixer {
    fn suggest_fixes(&self, source: &str, diagnostics: &[Diagnostic]) -> Vec<FixSuggestion>;
}

/// Default AutoFixer implementation: one fix per diagnostic, using suggestion/context when present.
/// Used by `vox check --force` to apply the first applicable fix.
pub struct StubAutoFixer;

impl AutoFixer for StubAutoFixer {
    fn suggest_fixes(&self, _source: &str, diagnostics: &[Diagnostic]) -> Vec<FixSuggestion> {
        diagnostics
            .iter()
            .map(|d| {
                let suggestion_text = d.suggestions.first().map_or("", String::as_str);
                let diff = if !suggestion_text.is_empty() {
                    let old_code = if let Some(ctx) = &d.context {
                        ctx.split('\n')
                            .map(|l| format!("-{l}"))
                            .collect::<Vec<_>>()
                            .join("\n")
                    } else {
                        "".to_string()
                    };
                    format!("{}\n+{}", old_code, suggestion_text)
                } else {
                    "// No automated fix available for this diagnostic.".to_string()
                };

                FixSuggestion {
                    message: format!("Fix for: {}", d.message),
                    diff,
                    explanation: if suggestion_text.is_empty() {
                        "Manual review required for this issue.".to_string()
                    } else {
                        format!("Applying suggested fix: {}", suggestion_text)
                    },
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::Severity;
    use vox_ast::span::Span;

    #[test]
    fn stub_autofixer_one_fix_per_diagnostic() {
        let fixer = StubAutoFixer;
        let diags = vec![Diagnostic {
            severity: Severity::Error,
            message: "Undefined x".to_string(),
            span: Span { start: 0, end: 1 },
            expected_type: None,
            found_type: None,
            context: Some("let x = 1".to_string()),
            suggestions: vec!["let x: int = 1".to_string()],
        }];
        let fixes = fixer.suggest_fixes("let x = 1", &diags);
        assert_eq!(fixes.len(), 1);
        assert!(fixes[0].diff.contains("-let x = 1"));
        assert!(fixes[0].diff.contains("+let x: int = 1"));
    }
}
