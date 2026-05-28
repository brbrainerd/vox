use std::collections::HashMap;

use serde::Serialize;
use serde_json::{self, json};

use crate::rules::{Finding, Severity};
use crate::task_queue::TaskQueue;

/// Borrowed snapshot of an engine run (avoids `engine` ↔ `report` cycles).
#[derive(Debug, Clone, Copy)]
pub struct RunSnapshot<'a> {
    pub findings: &'a [Finding],
    pub files_scanned: usize,
    pub rules_applied: usize,
    pub rust_parse_failures: usize,
    pub unresolved_ref_callee_counts: &'a HashMap<String, usize>,
    pub suppressions_applied: usize,
    pub suppression_counts_by_family: &'a HashMap<String, usize>,
}

/// JSON output wrapper (`format: json`) — includes findings and run telemetry (plan: report provenance).
#[derive(Debug, Serialize)]
pub struct ToestubJsonReportV1<'a> {
    pub schema_version: u32,
    pub tool_version: &'static str,
    pub files_scanned: usize,
    pub rules_applied: usize,
    pub rust_parse_failures: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unresolved_ref_hot_callers: Option<serde_json::Value>,
    pub suppressions_applied: usize,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub suppression_counts_by_family: HashMap<String, usize>,
    pub findings: &'a [Finding],
}

/// Output format for the report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// Human-readable colored terminal output for local `vox` runs.
    Terminal,
    /// Machine-readable JSON array of findings (CI parsers, dashboards).
    Json,
    /// LLM-optimized JSON: includes `rationale`, `alternatives`, `minimal_excerpt`,
    /// `confidence`, and `explain_url` for every finding. Use `--format llm-json`.
    LlmJson,
    /// GitHub-flavored Markdown sections per file/rule (reports, PR comments).
    Markdown,
}

impl OutputFormat {
    /// Parses CLI strings: `json`, `llm-json`, `markdown`/`md`, anything else → [`OutputFormat::Terminal`].
    pub fn parse_format(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "json" => OutputFormat::Json,
            "llm-json" | "llm_json" | "for-llm" => OutputFormat::LlmJson,
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
            OutputFormat::LlmJson => Self::format_llm_json(findings),
            OutputFormat::Markdown => Self::format_markdown(findings, task_queue),
        }
    }

    /// Full run formatting (JSON includes telemetry envelope).
    pub fn format_run(
        run: RunSnapshot<'_>,
        format: OutputFormat,
        task_queue: &TaskQueue,
    ) -> String {
        match format {
            OutputFormat::Terminal => Self::format_terminal(run.findings),
            OutputFormat::Json => Self::format_json_run(run),
            OutputFormat::LlmJson => Self::format_llm_json(run.findings),
            OutputFormat::Markdown => Self::format_markdown(run.findings, task_queue),
        }
    }

    /// LLM-optimized JSON output.
    ///
    /// Includes `rationale`, `alternatives`, `confidence`, `explain_url`, and
    /// a 7-line `minimal_excerpt` so an agent can propose a fix without additional
    /// context fetches. Schema: `vox.lint.llm-report.v1`.
    pub fn format_llm_json(findings: &[Finding]) -> String {
        let diagnostics: Vec<serde_json::Value> = findings
            .iter()
            .map(|f| {
                let mut d = json!({
                    "id": f.diagnostic_id.as_deref().unwrap_or(&f.rule_id),
                    "rule_id": f.rule_id,
                    "severity": format!("{}", f.severity).to_lowercase(),
                    "file": f.file.display().to_string(),
                    "line": f.line,
                    "column": f.column,
                    "message": f.message,
                });
                if let Some(ref sug) = f.suggestion {
                    d["suggested_fix"] = json!(sug);
                }
                if !f.alternatives.is_empty() {
                    d["alternatives"] = json!(f.alternatives);
                }
                if let Some(ref rat) = f.rationale {
                    d["rationale"] = json!(rat);
                }
                if let Some(conf) = f.confidence {
                    d["confidence"] = json!(format!("{:?}", conf).to_lowercase());
                }
                if !f.context.is_empty() {
                    d["minimal_excerpt"] = json!(f.context);
                }
                if let Some(id) = f.diagnostic_id.as_deref() {
                    d["explain_url"] = json!(format!("https://voxlang.org/diag/{}", id));
                }
                d
            })
            .collect();

        let report = json!({
            "schema": "vox.lint.llm-report.v1",
            "total": findings.len(),
            "diagnostics": diagnostics,
        });
        serde_json::to_string_pretty(&report).unwrap_or_default()
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

    fn format_json_run(run: RunSnapshot<'_>) -> String {
        let mut pairs: Vec<(String, usize)> = run
            .unresolved_ref_callee_counts
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        pairs.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        let hot: Vec<serde_json::Value> = pairs
            .into_iter()
            .take(25)
            .map(|(callee, count)| json!({ "callee": callee, "count": count }))
            .collect();
        let unresolved_ref_hot_callers = if hot.is_empty() {
            None
        } else {
            Some(serde_json::Value::Array(hot))
        };
        let supfam = run.suppression_counts_by_family.clone();
        let env = ToestubJsonReportV1 {
            schema_version: 1,
            tool_version: env!("CARGO_PKG_VERSION"),
            files_scanned: run.files_scanned,
            rules_applied: run.rules_applied,
            rust_parse_failures: run.rust_parse_failures,
            unresolved_ref_hot_callers,
            suppressions_applied: run.suppressions_applied,
            suppression_counts_by_family: supfam,
            findings: run.findings,
        };
        serde_json::to_string_pretty(&env).unwrap_or_else(|_| "{}".to_string())
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
                diagnostic_id: None,
                rule_name: "Stub Detector".to_string(),
                severity: Severity::Error,
                file: PathBuf::from("src/main.rs"),
                line: 42,
                column: 0,
                message: "todo!() found".to_string(),
                suggestion: Some("Implement the function.".to_string()),
                alternatives: vec![],
                rationale: None,
                context: String::new(),
                confidence: None,
                evidence: None,
            },
            Finding {
                rule_id: "magic-value/port".to_string(),
                diagnostic_id: None,
                rule_name: "Magic Value Detector".to_string(),
                severity: Severity::Warning,
                file: PathBuf::from("src/server.rs"),
                line: 10,
                column: 0,
                message: "Hardcoded port".to_string(),
                suggestion: None,
                alternatives: vec![],
                rationale: None,
                context: String::new(),
                confidence: None,
                evidence: None,
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
    fn json_run_envelope_has_findings_and_telemetry() {
        let findings = sample_findings();
        let hot = HashMap::new();
        let supfam = HashMap::new();
        let out = Reporter::format_run(
            RunSnapshot {
                findings: &findings,
                files_scanned: 3,
                rules_applied: 7,
                rust_parse_failures: 1,
                unresolved_ref_callee_counts: &hot,
                suppressions_applied: 0,
                suppression_counts_by_family: &supfam,
            },
            OutputFormat::Json,
            &TaskQueue::empty(),
        );
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["schema_version"], 1);
        assert_eq!(v["files_scanned"], 3);
        assert_eq!(v["rules_applied"], 7);
        assert_eq!(v["rust_parse_failures"], 1);
        assert_eq!(v["suppressions_applied"], 0);
        assert!(v["findings"].is_array());
        assert_eq!(v["findings"].as_array().unwrap().len(), 2);
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

    #[test]
    fn llm_json_format_has_schema_and_diagnostics() {
        let findings = sample_findings();
        let output = Reporter::format(&findings, OutputFormat::LlmJson, &TaskQueue::empty());
        let v: serde_json::Value = serde_json::from_str(&output).expect("valid JSON");
        assert_eq!(v["schema"], "vox.lint.llm-report.v1");
        assert_eq!(v["total"], 2);
        let diags = v["diagnostics"].as_array().unwrap();
        assert_eq!(diags.len(), 2);
        assert!(diags[0]["rule_id"].is_string());
        assert!(diags[0]["message"].is_string());
    }

    #[test]
    fn parse_format_accepts_llm_json_variants() {
        assert!(matches!(
            OutputFormat::parse_format("llm-json"),
            OutputFormat::LlmJson
        ));
        assert!(matches!(
            OutputFormat::parse_format("llm_json"),
            OutputFormat::LlmJson
        ));
        assert!(matches!(
            OutputFormat::parse_format("for-llm"),
            OutputFormat::LlmJson
        ));
    }
}
