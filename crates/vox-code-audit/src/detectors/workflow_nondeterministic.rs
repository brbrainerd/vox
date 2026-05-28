use crate::diagnostics::catalog;
use crate::rules::{DetectionRule, Finding, FindingConfidence, Language, Severity, SourceFile};
use regex::Regex;

/// Detects calls to non-deterministic builtins inside `workflow { }` or `workflow fn` bodies
/// in Vox source files.
pub struct WorkflowNondeterministicDetector {
    /// Matches lines that begin or declare a workflow context
    workflow_marker: Regex,
    /// Matches non-deterministic builtin calls
    nondeterministic: Regex,
    supported_langs: Vec<Language>,
}

impl Default for WorkflowNondeterministicDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkflowNondeterministicDetector {
    pub fn new() -> Self {
        Self {
            workflow_marker: Regex::new(
                r"\bworkflow\b",
            )
            .expect("valid regex"),
            nondeterministic: Regex::new(
                r"\b(?:time\.now\s*\(|time\.utc\s*\(|random\.int\s*\(|random\.float\s*\(|random\.uuid\s*\(|uuid\s*\(|Date\.now\s*\(|crypto\.random_bytes\s*\()",
            )
            .expect("valid regex"),
            supported_langs: vec![Language::Vox],
        }
    }
}

impl DetectionRule for WorkflowNondeterministicDetector {
    fn id(&self) -> &'static str {
        "vox/workflow/non-deterministic-builtin"
    }

    fn name(&self) -> &'static str {
        "Workflow Non-Deterministic Builtin Detector"
    }

    fn description(&self) -> &'static str {
        "Detects calls to non-deterministic builtins inside workflow blocks or workflow functions \
        in Vox source files. Workflows must be deterministic for replay safety."
    }

    fn severity(&self) -> Severity {
        Severity::Error
    }

    fn languages(&self) -> &[Language] {
        &self.supported_langs
    }

    fn diagnostic_id(&self) -> Option<&'static str> {
        Some(catalog::WORKFLOW_NON_DETERMINISTIC_BUILTIN)
    }

    fn explain(&self) -> &'static str {
        "Workflow bodies must be fully deterministic so the runtime can replay them safely after \
        a crash or checkpoint. Calls to time, random, UUID, or crypto-random sources introduce \
        non-determinism that breaks replay.\n\n\
        Bad (in workflow):  let t = time.now();\n\
        Good:               pass `now` as an input parameter, or read from workflow context."
    }

    fn minimal_repro(&self) -> Option<&'static str> {
        Some(
            "// VIOLATION — time.now() inside a @workflow body\n\
             @workflow fn process_order(id: str) {\n\
             \x20   let started_at = time.now()  // non-deterministic!\n\
             }\n\
             \n\
             // FIX — pass timestamp as input parameter\n\
             @workflow fn process_order(id: str, started_at: str) {\n\
             \x20   // use started_at from caller\n\
             }",
        )
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
        let lines = &file.lines;

        // Strategy: for each non-deterministic call, look back up to 100 lines for a `workflow`
        // keyword. Stop looking back early if we hit a top-level `fn` that is NOT a workflow fn.
        for (i, line) in lines.iter().enumerate() {
            let line_num = i + 1;
            let trimmed = line.trim();

            // Skip comment lines
            if trimmed.starts_with("//")
                || trimmed.starts_with('#')
                || trimmed.starts_with('*')
                || trimmed.starts_with("/*")
            {
                continue;
            }

            let Some(m) = self.nondeterministic.find(line) else {
                continue;
            };

            // Look back up to 100 lines for a workflow marker
            let look_back_start = i.saturating_sub(100);
            let mut in_workflow = false;

            for j in (look_back_start..i).rev() {
                let candidate = &lines[j];
                if self.workflow_marker.is_match(candidate) {
                    in_workflow = true;
                    break;
                }
                // Stop if we cross a non-workflow top-level `fn` declaration at column 0
                // that would close the enclosing scope
                let t = candidate.trim();
                let at_col0 = !candidate.starts_with(' ')
                    && !candidate.starts_with('\t')
                    && !candidate.is_empty();
                if at_col0 && t.starts_with("fn ") && !t.contains("workflow") {
                    break;
                }
            }

            if in_workflow {
                let call = m.as_str().trim_end_matches('(').to_string();
                findings.push(Finding {
                    rule_id: self.id().to_string(),
                    diagnostic_id: self.diagnostic_id().map(str::to_string),
                    rule_name: self.name().to_string(),
                    severity: Severity::Error,
                    file: file.path.clone(),
                    line: line_num,
                    column: m.start() + 1,
                    message: format!(
                        "Non-deterministic builtin `{call}(...)` called inside a workflow — workflows must be deterministic for replay safety."
                    ),
                    suggestion: Some(format!(
                        "Pass the value of `{call}(...)` as an input parameter to the workflow, or source it from the workflow context object."
                    )),
                    alternatives: vec![
                        "Inject deterministic values through workflow input parameters.".into(),
                        "Use workflow-provided context for timestamps and IDs.".into(),
                    ],
                    rationale: Some(
                        "Workflow bodies are replayed by the runtime after checkpoints and crashes. \
                        Any non-deterministic call will produce different values on replay, \
                        corrupting state. All sources of randomness and time must come from \
                        deterministic inputs or the workflow context.".into(),
                    ),
                    context: file.context_around(line_num, 2),
                    confidence: Some(FindingConfidence::Medium),
                    evidence: None,
                });
            }
        }

        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn vox_source(code: &str) -> SourceFile {
        SourceFile::new(PathBuf::from("test.vox"), code.to_string())
    }

    #[test]
    fn flags_time_now_inside_workflow_block() {
        let d = WorkflowNondeterministicDetector::new();
        let code = "workflow MyFlow {\n    fn run() {\n        let t = time.now();\n    }\n}";
        let f = vox_source(code);
        let findings = d.detect(&f, None);
        assert!(
            !findings.is_empty(),
            "should flag time.now() inside a workflow block"
        );
        assert!(findings[0].message.contains("time.now"));
    }

    #[test]
    fn ignores_time_now_in_regular_fn() {
        let d = WorkflowNondeterministicDetector::new();
        let code = "fn regular_function() {\n    let t = time.now();\n    return t;\n}";
        let f = vox_source(code);
        let findings = d.detect(&f, None);
        assert!(
            findings.is_empty(),
            "time.now() outside a workflow should not fire"
        );
    }

    #[test]
    fn flags_random_uuid_inside_workflow() {
        let d = WorkflowNondeterministicDetector::new();
        let code = "workflow OrderFlow {\n    fn start(input: OrderInput) {\n        let id = random.uuid();\n        process(id, input);\n    }\n}";
        let f = vox_source(code);
        let findings = d.detect(&f, None);
        assert!(
            !findings.is_empty(),
            "should flag random.uuid() inside a workflow"
        );
        assert!(findings[0].message.contains("random.uuid"));
    }

    #[test]
    fn does_not_flag_non_vox_files() {
        let d = WorkflowNondeterministicDetector::new();
        let code = "workflow MyFlow {\n    let t = time.now();\n}";
        let f = SourceFile::new(PathBuf::from("test.rs"), code.to_string());
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "should not fire on non-Vox files");
    }

    #[test]
    fn flags_date_now_in_workflow_fn() {
        let d = WorkflowNondeterministicDetector::new();
        let code =
            "workflow fn process_order(input) {\n    let ts = Date.now();\n    return ts;\n}";
        let f = vox_source(code);
        let findings = d.detect(&f, None);
        assert!(
            !findings.is_empty(),
            "should flag Date.now() inside a workflow fn"
        );
    }
}
