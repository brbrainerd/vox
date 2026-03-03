use serde_json;

use crate::rules::{Finding, Severity};
use crate::task_queue::TaskQueue;

/// Output format for the report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Terminal,
    Json,
    Markdown,
}

impl OutputFormat {
    pub fn parse_format(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "json" => OutputFormat::Json,
            "markdown" | "md" => OutputFormat::Markdown,
            _ => OutputFormat::Terminal,
        }
    }
}

/// Generates formatted output from findings.
pub struct Reporter;

impl Reporter {
    /// Format findings into the specified output format.
    pub fn format(findings: &[Finding], format: OutputFormat, task_queue: &TaskQueue) -> String {
        match format {
            OutputFormat::Terminal => Self::format_terminal(findings),
            OutputFormat::Json => Self::format_json(findings),
            OutputFormat::Markdown => Self::format_markdown(findings, task_queue),
        }
    }

    fn format_terminal(findings: &[Finding]) -> String {
        if findings.is_empty() {
            return "✅ No issues found — code is clean!\n".to_string();
        }

        let mut out = String::new();
        out.push_str(&format!(
            "\n🦶 TOESTUB — {} finding{} detected\n",
            findings.len(),
            if findings.len() == 1 { "" } else { "s" }
        ));
        out.push_str(&"─".repeat(60));
        out.push('\n');

        for finding in findings {
            let icon = match finding.severity {
                Severity::Critical => "🔴",
                Severity::Error => "🟠",
                Severity::Warning => "🟡",
                Severity::Info => "🔵",
            };

            out.push_str(&format!(
                "\n{} [{}] {} ({})\n",
                icon, finding.severity, finding.rule_id, finding.rule_name,
            ));
            out.push_str(&format!(
                "   📁 {}:{}\n",
                finding.file.display(),
                finding.line,
            ));
            out.push_str(&format!("   💬 {}\n", finding.message));

            if let Some(ref suggestion) = finding.suggestion {
                out.push_str(&format!("   💡 {}\n", suggestion));
            }

            if !finding.context.is_empty() {
                out.push_str("   ┌─────────────────────────────\n");
                for ctx_line in finding.context.lines() {
                    out.push_str(&format!("   │ {}\n", ctx_line));
                }
                out.push_str("   └─────────────────────────────\n");
            }
        }

        // Summary
        let critical = findings
            .iter()
            .filter(|f| f.severity == Severity::Critical)
            .count();
        let error = findings
            .iter()
            .filter(|f| f.severity == Severity::Error)
            .count();
        let warning = findings
            .iter()
            .filter(|f| f.severity == Severity::Warning)
            .count();
        let info = findings
            .iter()
            .filter(|f| f.severity == Severity::Info)
            .count();

        out.push_str("\n─");
        out.push_str(&"─".repeat(59));
        out.push_str(&format!(
            "\n📊 Summary: {} critical, {} error, {} warning, {} info\n",
            critical, error, warning, info,
        ));

        out
    }

    fn format_json(findings: &[Finding]) -> String {
        serde_json::to_string_pretty(findings).unwrap_or_else(|_| "[]".to_string())
    }

    fn format_markdown(findings: &[Finding], task_queue: &TaskQueue) -> String {
        let mut out = String::new();
        out.push_str("## TOESTUB Findings\n\n");

        if findings.is_empty() {
            out.push_str("✅ No issues found — code is clean!\n");
            return out;
        }

        out.push_str(&format!("**{}** issues detected.\n\n", findings.len()));

        // Group by severity
        for severity in &[
            Severity::Critical,
            Severity::Error,
            Severity::Warning,
            Severity::Info,
        ] {
            let group: Vec<&Finding> = findings
                .iter()
                .filter(|f| f.severity == *severity)
                .collect();
            if group.is_empty() {
                continue;
            }

            out.push_str(&format!("### {} ({})\n\n", severity, group.len()));
            for f in &group {
                out.push_str(&format!(
                    "- [ ] **{}** `{}:{}` — {}\n",
                    f.rule_id,
                    f.file.display(),
                    f.line,
                    f.message,
                ));
            }
            out.push('\n');
        }

        // Add task queue section if not empty
        if !task_queue.fix_suggestions.is_empty() {
            out.push_str("### Suggested Follow-up Prompts\n\n");
            for suggestion in &task_queue.fix_suggestions {
                out.push_str(&format!("- {}\n", suggestion.prompt));
            }
            out.push('\n');
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sample_findings() -> Vec<Finding> {
        vec![
            Finding {
                rule_id: "stub/todo".to_string(),
                rule_name: "Stub Detector".to_string(),
                severity: Severity::Error,
                file: PathBuf::from("src/main.rs"),
                line: 42,
                column: 0,
                message: "todo!() found".to_string(),
                suggestion: Some("Implement the function.".to_string()),
                context: String::new(),
            },
            Finding {
                rule_id: "magic-value/port".to_string(),
                rule_name: "Magic Value Detector".to_string(),
                severity: Severity::Warning,
                file: PathBuf::from("src/server.rs"),
                line: 10,
                column: 0,
                message: "Hardcoded port".to_string(),
                suggestion: None,
                context: String::new(),
            },
        ]
    }

    #[test]
    fn terminal_format_works() {
        let findings = sample_findings();
        let output = Reporter::format(&findings, OutputFormat::Terminal, &TaskQueue::empty());
        assert!(output.contains("TOESTUB"));
        assert!(output.contains("stub/todo"));
        assert!(output.contains("magic-value/port"));
    }

    #[test]
    fn json_format_works() {
        let findings = sample_findings();
        let output = Reporter::format(&findings, OutputFormat::Json, &TaskQueue::empty());
        let parsed: Vec<Finding> = serde_json::from_str(&output).expect("valid JSON");
        assert_eq!(parsed.len(), 2);
    }

    #[test]
    fn markdown_format_works() {
        let findings = sample_findings();
        let output = Reporter::format(&findings, OutputFormat::Markdown, &TaskQueue::empty());
        assert!(output.contains("## TOESTUB Findings"));
        assert!(output.contains("- [ ]"));
    }

    #[test]
    fn empty_findings_clean_message() {
        let output = Reporter::format(&[], OutputFormat::Terminal, &TaskQueue::empty());
        assert!(output.contains("clean"));
    }
}
