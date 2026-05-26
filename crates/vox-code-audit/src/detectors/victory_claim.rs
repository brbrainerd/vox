use crate::embedded_rules::embedded_pack;
use crate::rules::{DetectionRule, Finding, Language, Severity, SourceFile};
use vox_rule_pack::CompiledRule;

/// Detects suspicious "victory claim" comments near incomplete or provisional code.
///
/// Patterns are loaded from `contracts/code-audit/rules.v1.yaml` at startup.
pub struct VictoryClaimDetector {
    rules: Vec<&'static CompiledRule>,
}

impl Default for VictoryClaimDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl VictoryClaimDetector {
    pub fn new() -> Self {
        let pack = embedded_pack();
        let mut rules = Vec::with_capacity(4);
        for id in [
            "victory-claim/premature",
            "victory-claim/todo-leftover",
            "victory-claim/fixme",
            "victory-claim/hack",
        ] {
            rules.push(
                pack.rule(id)
                    .unwrap_or_else(|| panic!("vox-rule-pack: required rule missing: {id}")),
            );
        }
        Self { rules }
    }
}

impl DetectionRule for VictoryClaimDetector {
    fn id(&self) -> &'static str {
        "victory-claim"
    }
    fn name(&self) -> &'static str {
        "Victory Claim / Leftover Marker Detector"
    }
    fn description(&self) -> &'static str {
        "Detects premature 'Done!' comments, TODO/FIXME/HACK markers left behind"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
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

    fn minimal_repro(&self) -> Option<&'static str> {
        Some(
            "// VIOLATION — TODO/FIXME markers and premature completion comment\n\
             fn apply_discount(price: f64, user: &User) -> f64 {\n\
             \x20   // TODO: apply loyalty tier discount\n\
             \x20   // HACK: just return full price for now\n\
             \x20   price  // Done! (not actually done)\n\
             }\n\
             \n\
             // FIX — implement fully and remove markers\n\
             fn apply_discount(price: f64, user: &User) -> f64 {\n\
             \x20   let rate = user.loyalty_tier.discount_rate();\n\
             \x20   price * (1.0 - rate)\n\
             }",
        )
    }

    fn detect(
        &self,
        file: &SourceFile,
        _rust: Option<&crate::analysis::RustFileContext>,
    ) -> Vec<Finding> {
        let mut findings = Vec::new();

        for (i, line) in file.lines.iter().enumerate() {
            let line_num = i + 1;
            let tri = line.trim_start();
            if tri.starts_with("///") || tri.starts_with("//!") {
                continue;
            }

            for rule in &self.rules {
                if rule.matches_line(line) {
                    let context_radius = if rule.id == "victory-claim/premature" {
                        2
                    } else {
                        1
                    };
                    findings.push(Finding {
                        rule_id: rule.id.clone(),
                        rule_name: self.name().to_string(),
                        severity: rule.severity.into(),
                        file: file.path.clone(),
                        line: line_num,
                        column: 0,
                        message: rule.message.clone(),
                        suggestion: rule.suggestion.clone(),
                        diagnostic_id: None,
                        alternatives: vec![],
                        rationale: None,
                        context: file.context_around(line_num, context_radius),
                        confidence: None,
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

    fn source(ext: &str, code: &str) -> SourceFile {
        SourceFile::new(PathBuf::from(format!("test.{}", ext)), code.to_string())
    }

    #[test]
    fn detects_victory_comment() {
        let d = VictoryClaimDetector::new();
        let snippet = concat!("// ", "D", "one!", " Implementation complete\nfn foo() {}");
        let f = source("rs", snippet);
        let findings = d.detect(&f, None);
        assert!(
            findings
                .iter()
                .any(|f| f.rule_id == "victory-claim/premature"),
            "should detect victory claim"
        );
    }

    #[test]
    fn detects_todo_leftover() {
        let d = VictoryClaimDetector::new();
        let py = concat!("# TO", "DO: implement later", "\ndef foo():\n    pass");
        let f = source("py", py);
        let findings = d.detect(&f, None);
        assert!(
            findings
                .iter()
                .any(|f| f.rule_id == "victory-claim/todo-leftover"),
            "should detect TODO leftover"
        );
    }

    #[test]
    fn detects_fixme() {
        let d = VictoryClaimDetector::new();
        let snippet = concat!("// ", "FIX", "ME this is broken\nconst x = 1;");
        let f = source("ts", snippet);
        let findings = d.detect(&f, None);
        assert!(
            findings.iter().any(|f| f.rule_id == "victory-claim/fixme"),
            "should detect FIXME"
        );
    }

    #[test]
    fn clean_code_no_findings() {
        let d = VictoryClaimDetector::new();
        let f = source(
            "rs",
            "/// Adds two numbers.\nfn add(a: i32, b: i32) -> i32 {\n    a + b\n}",
        );
        let findings = d.detect(&f, None);
        assert!(findings.is_empty());
    }
}
