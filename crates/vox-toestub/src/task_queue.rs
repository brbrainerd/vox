use serde::{Deserialize, Serialize};

use crate::rules::{Finding, Severity};

/// Priority for a fix suggestion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    /// Must be fixed immediately (Critical/Error findings).
    Immediate,
    /// Should be fixed in the next working session.
    NextSession,
    /// Can be addressed later.
    Backlog,
}

/// A suggested fix action with a prompt suitable for sending to an AI assistant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixSuggestion {
    /// The original finding this fix addresses.
    pub rule_id: String,
    /// File and line reference.
    pub location: String,
    /// A self-contained prompt an AI assistant can use to fix this issue.
    pub prompt: String,
    /// Whether this could potentially be auto-fixed without human review.
    pub auto_fixable: bool,
    /// Priority level.
    pub priority: Priority,
}

/// Queue of remaining work items derived from TOESTUB findings.
///
/// Designed to integrate with task tracking systems:
/// - Generates markdown checklists for task.md
/// - Creates self-contained AI prompts for follow-up sessions
/// - Tracks progress across sessions via JSON state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskQueue {
    /// Total number of findings.
    pub total_findings: usize,
    /// Fix suggestions with AI prompts.
    pub fix_suggestions: Vec<FixSuggestion>,
}

impl TaskQueue {
    /// Create an empty task queue.
    pub fn empty() -> Self {
        Self {
            total_findings: 0,
            fix_suggestions: Vec::new(),
        }
    }

    /// Generate a task queue from a set of findings.
    pub fn from_findings(findings: &[Finding]) -> Self {
        let fix_suggestions: Vec<FixSuggestion> = findings
            .iter()
            .map(|f| {
                let priority = match f.severity {
                    Severity::Critical | Severity::Error => Priority::Immediate,
                    Severity::Warning => Priority::NextSession,
                    Severity::Info => Priority::Backlog,
                };

                let auto_fixable = matches!(
                    f.rule_id.as_str(),
                    "magic-value/port" | "magic-value/ip" | "magic-value/path"
                );

                let prompt = Self::generate_prompt(f);

                FixSuggestion {
                    rule_id: f.rule_id.clone(),
                    location: format!("{}:{}", f.file.display(), f.line),
                    prompt,
                    auto_fixable,
                    priority,
                }
            })
            .collect();

        Self {
            total_findings: findings.len(),
            fix_suggestions,
        }
    }

    /// Generate a self-contained AI prompt for fixing a specific finding.
    fn generate_prompt(finding: &Finding) -> String {
        let base = format!(
            "In file `{}` at line {}: {}.",
            finding.file.display(),
            finding.line,
            finding.message,
        );

        let action = match finding.rule_id.as_str() {
            "stub/todo" | "stub/unimplemented" => {
                "Replace the stub macro with the actual implementation. \
                 Consider the function signature, surrounding context, and any \
                 related tests to determine the correct behavior."
            }
            "stub/pass" | "stub/gdscript-pass" => {
                "Implement the function body. The current body is just `pass` \
                 (a no-op stub). Review the function name and parameters to \
                 determine the expected behavior."
            }
            "empty-body" => {
                "This function has an empty body and does nothing. Either implement \
                 it or remove it if it's unused."
            }
            id if id.starts_with("magic-value") => {
                "Extract the hardcoded value into a named constant or environment \
                 variable. Follow the pattern: `const NAME: Type = value;` or \
                 `std::env::var(\"ENV_NAME\").unwrap_or_else(|_| default.to_string())`."
            }
            "victory-claim/premature" => {
                "Verify that the implementation is truly complete. Remove the \
                 'Done!' comment if it is, or fix the remaining issues if not."
            }
            id if id.starts_with("victory-claim") => {
                "Address the TODO/FIXME/HACK marker by either completing the work \
                 or creating a tracked task for it."
            }
            "unwired/module" => {
                "This module is declared but never used. Either add `use` statements \
                 to import its contents where needed, or remove the module declaration."
            }
            "dry-violation/duplicate-block" => {
                "Extract the duplicated logic into a shared helper function. Both \
                 call sites should use the shared function instead of duplicating code."
            }
            _ => "Review and fix this issue. Check the suggestion for guidance.",
        };

        format!("{} {}", base, action)
    }

    /// Serialize the task queue to JSON for persistent state tracking.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }

    /// Generate a markdown checklist from the task queue.
    pub fn to_markdown_checklist(&self) -> String {
        if self.fix_suggestions.is_empty() {
            return "No outstanding tasks.\n".to_string();
        }

        let mut out = String::new();
        out.push_str("## TOESTUB Task Queue\n\n");

        // Group by priority
        for (priority, label) in &[
            (Priority::Immediate, "🔴 Immediate"),
            (Priority::NextSession, "🟡 Next Session"),
            (Priority::Backlog, "🔵 Backlog"),
        ] {
            let items: Vec<&FixSuggestion> = self
                .fix_suggestions
                .iter()
                .filter(|s| s.priority == *priority)
                .collect();

            if items.is_empty() {
                continue;
            }

            out.push_str(&format!("### {}\n\n", label));
            for item in &items {
                let auto_badge = if item.auto_fixable { " 🤖" } else { "" };
                out.push_str(&format!(
                    "- [ ] **{}** `{}`{}\n",
                    item.rule_id, item.location, auto_badge,
                ));
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

    fn sample_finding(rule_id: &str, severity: Severity) -> Finding {
        Finding {
            rule_id: rule_id.to_string(),
            rule_name: "Test".to_string(),
            severity,
            file: PathBuf::from("src/lib.rs"),
            line: 10,
            column: 0,
            message: "test issue".to_string(),
            suggestion: None,
            context: String::new(),
        }
    }

    #[test]
    fn from_findings_creates_queue() {
        let findings = vec![
            sample_finding("stub/todo", Severity::Error),
            sample_finding("magic-value/port", Severity::Warning),
        ];
        let queue = TaskQueue::from_findings(&findings);
        assert_eq!(queue.total_findings, 2);
        assert_eq!(queue.fix_suggestions.len(), 2);
        assert_eq!(queue.fix_suggestions[0].priority, Priority::Immediate);
        assert_eq!(queue.fix_suggestions[1].priority, Priority::NextSession);
    }

    #[test]
    fn json_roundtrip() {
        let findings = vec![sample_finding("stub/todo", Severity::Error)];
        let queue = TaskQueue::from_findings(&findings);
        let json = queue.to_json();
        let parsed: TaskQueue = serde_json::from_str(&json).expect("valid JSON");
        assert_eq!(parsed.total_findings, 1);
    }

    #[test]
    fn markdown_checklist_output() {
        let findings = vec![
            sample_finding("stub/todo", Severity::Error),
            sample_finding("victory-claim/fixme", Severity::Warning),
        ];
        let queue = TaskQueue::from_findings(&findings);
        let md = queue.to_markdown_checklist();
        assert!(md.contains("- [ ]"));
        assert!(md.contains("Immediate"));
    }

    #[test]
    fn empty_queue_message() {
        let queue = TaskQueue::empty();
        let md = queue.to_markdown_checklist();
        assert!(md.contains("No outstanding tasks"));
    }
}
