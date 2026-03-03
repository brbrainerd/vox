use crate::rules::{DetectionRule, Finding, Language, Severity, SourceFile};
use regex::Regex;

/// Detects modules/files that are declared but never imported or referenced.
///
/// Catches the classic AI pattern: create a helper module, forget to wire it in.
#[allow(dead_code)]
pub struct UnwiredModuleDetector {
    rust_mod_decl: Regex,
    rust_use_stmt: Regex,
    ts_export_re: Regex,
}

impl Default for UnwiredModuleDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl UnwiredModuleDetector {
    pub fn new() -> Self {
        Self {
            rust_mod_decl: Regex::new(r"^\s*(?:pub\s+)?mod\s+(\w+)\s*;").expect("valid regex"),
            rust_use_stmt: Regex::new(r"\buse\s+(?:crate|super|self)::(\w+)").expect("valid regex"),
            ts_export_re: Regex::new(
                r"export\s+(?:default\s+)?(?:function|class|const|let|type|interface|enum)\s+(\w+)",
            )
            .expect("valid regex"),
        }
    }

    fn detect_rust(&self, file: &SourceFile) -> Vec<Finding> {
        let mut findings = Vec::new();

        // Collect all `mod name;` declarations
        let mut declared_mods: Vec<(String, usize)> = Vec::new();
        // Collect all `use crate::name` / `use super::name` references
        let mut used_mods: Vec<String> = Vec::new();

        for (i, line) in file.lines.iter().enumerate() {
            if let Some(caps) = self.rust_mod_decl.captures(line) {
                if let Some(name) = caps.get(1) {
                    declared_mods.push((name.as_str().to_string(), i + 1));
                }
            }
            for caps in self.rust_use_stmt.captures_iter(line) {
                if let Some(name) = caps.get(1) {
                    used_mods.push(name.as_str().to_string());
                }
            }
        }

        // Also check for inline `name::` usage patterns
        for (mod_name, line_num) in &declared_mods {
            // Check if `mod_name` appears anywhere in the file as `mod_name::` or in a `use`
            let is_used = used_mods.contains(mod_name)
                || file.lines.iter().enumerate().any(|(j, line)| {
                    j + 1 != *line_num // skip the declaration line itself
                    && (line.contains(&format!("{}::", mod_name))
                        || line.contains(&format!("use {}", mod_name)))
                });

            if !is_used {
                findings.push(Finding {
                    rule_id: "unwired/module".to_string(),
                    rule_name: self.name().to_string(),
                    severity: self.severity(),
                    file: file.path.clone(),
                    line: *line_num,
                    column: 0,
                    message: format!(
                        "Module `{}` is declared but never referenced in this file",
                        mod_name
                    ),
                    suggestion: Some(format!(
                        "Add `use crate::{}::...;` or remove the module declaration if unused.",
                        mod_name
                    )),
                    context: file.context_around(*line_num, 1),
                });
            }
        }

        findings
    }
}

impl DetectionRule for UnwiredModuleDetector {
    fn id(&self) -> &'static str {
        "arch/unwired"
    }
    fn name(&self) -> &'static str {
        "Unwired Module Detector"
    }
    fn description(&self) -> &'static str {
        "Detects modules declared but never imported or referenced"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn languages(&self) -> &[Language] {
        &[Language::Rust]
    }

    fn detect(&self, file: &SourceFile) -> Vec<Finding> {
        match file.language {
            Language::Rust => self.detect_rust(file),
            _ => Vec::new(),
        }
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
    fn detects_unwired_mod() {
        let d = UnwiredModuleDetector::new();
        let f = source(
            "rs",
            "mod helpers;\nmod utils;\n\nfn main() {\n    helpers::do_thing();\n}",
        );
        let findings = d.detect(&f);
        // `utils` is declared but never used, `helpers` is used via `helpers::do_thing()`
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("utils"));
    }

    #[test]
    fn no_findings_when_all_used() {
        let d = UnwiredModuleDetector::new();
        let f = source(
            "rs",
            "mod engine;\nmod rules;\n\nuse crate::engine::Engine;\nuse crate::rules::Rule;\n",
        );
        let findings = d.detect(&f);
        assert!(findings.is_empty());
    }
}
