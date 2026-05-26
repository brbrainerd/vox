use crate::diagnostics::catalog;
use crate::rules::{DetectionRule, Finding, FindingConfidence, Language, Severity, SourceFile};
use regex::Regex;

/// Detects panicking builtins inside actor handlers, workflow activities, and similar
/// fault-critical contexts in Vox and Rust files.
pub struct PanickingBuiltinDetector {
    /// Matches decorator or block-start lines that open a handler/activity/actor/workflow context
    context_marker: Regex,
    /// Matches panicking builtins in Rust
    rust_panicking: Regex,
    /// Matches panicking builtins in Vox
    vox_panicking: Regex,
    supported_langs: Vec<Language>,
}

impl Default for PanickingBuiltinDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl PanickingBuiltinDetector {
    pub fn new() -> Self {
        Self {
            // Matches @handler, @activity, or block keywords actor/workflow on a line
            context_marker: Regex::new(r"^\s*(?:@(?:handler|activity)|(?:actor|workflow)\s+\w+)\b")
                .expect("valid regex"),
            rust_panicking: Regex::new(
                r"\.unwrap\(\)|\.expect\s*\(|(?:unwrap|unreachable|todo|panic)!",
            )
            .expect("valid regex"),
            vox_panicking: Regex::new(r"\bstd\.(?:unwrap|expect|panic|unreachable|todo)\s*\(")
                .expect("valid regex"),
            supported_langs: vec![Language::Vox, Language::Rust],
        }
    }
}

impl DetectionRule for PanickingBuiltinDetector {
    fn id(&self) -> &'static str {
        "handler/panicking-builtin"
    }

    fn name(&self) -> &'static str {
        "Panicking Builtin in Handler Detector"
    }

    fn description(&self) -> &'static str {
        "Detects panicking builtins inside actor message handlers or workflow activities, \
        where a panic would crash the entire actor or workflow."
    }

    fn severity(&self) -> Severity {
        Severity::Warning
    }

    fn languages(&self) -> &[Language] {
        &self.supported_langs
    }

    fn diagnostic_id(&self) -> Option<&'static str> {
        Some(catalog::HANDLER_PANICKING_BUILTIN)
    }

    fn explain(&self) -> &'static str {
        "Actor message handlers and workflow activities must never panic — a panic crashes \
        the entire actor or workflow. Use Result-returning alternatives and propagate errors.\n\n\
        Bad (in @handler):  let x = value.unwrap();\n\
        Good:               let x = value?;  // or value.map_err(|e| ...)?;"
    }

    fn minimal_repro(&self) -> Option<&'static str> {
        Some(
            "// VIOLATION — .unwrap() inside @handler crashes the whole actor on None/Err\n\
             @handler\n\
             fn on_payment(msg: PaymentMsg) -> Result<(), Err> {\n\
             \x20   let record = db.find(msg.id).unwrap();  // panics if not found → kills actor\n\
             \x20   Ok(())\n\
             }\n\
             \n\
             // FIX — propagate with ? so the handler returns Err gracefully\n\
             @handler\n\
             fn on_payment(msg: PaymentMsg) -> Result<(), DbError> {\n\
             \x20   let record = db.find(msg.id)?;\n\
             \x20   Ok(())\n\
             }",
        )
    }

    fn detect(
        &self,
        file: &SourceFile,
        _rust_ctx: Option<&crate::analysis::RustFileContext>,
    ) -> Vec<Finding> {
        if !matches!(file.language, Language::Vox | Language::Rust) {
            return vec![];
        }

        let panicking_re = match file.language {
            Language::Vox => &self.vox_panicking,
            Language::Rust => &self.rust_panicking,
            _ => return vec![],
        };

        let mut findings = Vec::new();
        let lines = &file.lines;
        let n = lines.len();

        // Strategy: for each line that contains a panicking builtin, look back up to 50 lines
        // for a context marker. This avoids complex stateful brace tracking.
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

            let Some(m) = panicking_re.find(line) else {
                continue;
            };

            // Look back up to 50 lines for a context marker.
            // Stop early only if we see a top-level non-fn item (struct/impl/mod/use)
            // at column 0, which would mean we've left the handler's scope.
            let look_back_start = i.saturating_sub(50);
            let mut in_context = false;

            for j in (look_back_start..i).rev() {
                if self.context_marker.is_match(&lines[j]) {
                    in_context = true;
                    break;
                }
                // Stop looking back if we hit a non-handler top-level declaration at col 0
                // that definitively closes the previous handler scope.
                let t = lines[j].trim();
                let at_col0 = !lines[j].starts_with(' ')
                    && !lines[j].starts_with('\t')
                    && !lines[j].is_empty();
                if at_col0
                    && (t.starts_with("struct ")
                        || t.starts_with("impl ")
                        || t.starts_with("mod ")
                        || t.starts_with("use ")
                        || t.starts_with("pub struct ")
                        || t.starts_with("pub impl ")
                        || t.starts_with("pub mod "))
                {
                    break;
                }
            }

            if in_context {
                let call = m.as_str().to_string();
                findings.push(Finding {
                    rule_id: self.id().to_string(),
                    diagnostic_id: self.diagnostic_id().map(str::to_string),
                    rule_name: self.name().to_string(),
                    severity: Severity::Warning,
                    file: file.path.clone(),
                    line: line_num,
                    column: m.start() + 1,
                    message: format!(
                        "Panicking builtin `{call}` inside an actor handler/activity — use `Result` instead."
                    ),
                    suggestion: Some(format!(
                        "Replace `{call}` with a `Result`-returning alternative. Panics in handlers crash the actor; `Result` allows graceful error reporting."
                    )),
                    alternatives: vec![],
                    rationale: Some(
                        "Actor message handlers and workflow activities must never panic — \
                        a panic crashes the entire actor or workflow. Use Result-returning \
                        alternatives and propagate errors.".into(),
                    ),
                    context: file.context_around(line_num, 2),
                    confidence: Some(FindingConfidence::Medium),
                    evidence: None,
                });
            }
        }

        let _ = n;
        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn source(lang: &str, code: &str) -> SourceFile {
        SourceFile::new(PathBuf::from(format!("test.{lang}")), code.to_string())
    }

    #[test]
    fn fires_on_unwrap_in_handler() {
        let d = PanickingBuiltinDetector::new();
        let code = "@handler\nfn on_message(msg: Msg) {\n    let val = get_data().unwrap();\n}";
        let f = source("rs", code);
        let findings = d.detect(&f, None);
        assert!(!findings.is_empty(), "should fire on .unwrap() in @handler");
        assert!(findings[0].message.contains("unwrap"));
    }

    #[test]
    fn does_not_fire_outside_handler() {
        let d = PanickingBuiltinDetector::new();
        let code = "fn utility_fn() {\n    let val = get_data().unwrap();\n}";
        let f = source("rs", code);
        let findings = d.detect(&f, None);
        assert!(
            findings.is_empty(),
            "unwrap outside handler context should not fire"
        );
    }

    #[test]
    fn fires_on_panic_macro_in_activity() {
        let d = PanickingBuiltinDetector::new();
        let code = "@activity\nfn do_work() {\n    panic!(\"something went wrong\");\n}";
        let f = source("rs", code);
        let findings = d.detect(&f, None);
        assert!(!findings.is_empty(), "should fire on panic! in @activity");
    }

    #[test]
    fn fires_on_todo_in_actor() {
        let d = PanickingBuiltinDetector::new();
        let code = "actor MyActor {\n    fn handle(msg: Msg) {\n        todo!(\"implement me\");\n    }\n}";
        let f = source("rs", code);
        let findings = d.detect(&f, None);
        assert!(
            !findings.is_empty(),
            "should fire on todo! inside actor block"
        );
    }

    #[test]
    fn fires_on_vox_std_unwrap_in_handler() {
        let d = PanickingBuiltinDetector::new();
        let code = "@handler\nfn on_event(e) {\n    let x = std.unwrap(compute());\n}";
        let f = source("vox", code);
        let findings = d.detect(&f, None);
        assert!(
            !findings.is_empty(),
            "should fire on std.unwrap( in vox @handler"
        );
    }

    #[test]
    fn does_not_fire_on_result_propagation_in_handler() {
        let d = PanickingBuiltinDetector::new();
        let code =
            "@handler\nfn on_message(msg: Msg) {\n    let val = get_data()?;\n    Ok(val)\n}";
        let f = source("rs", code);
        let findings = d.detect(&f, None);
        assert!(
            findings.is_empty(),
            "Result propagation with ? should not fire"
        );
    }
}
