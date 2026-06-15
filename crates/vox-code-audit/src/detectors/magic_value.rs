use crate::rule_pack_detector::pack_rule;
use crate::rules::{DetectionRule, Finding, Language, Severity, SourceFile};
use regex::Regex;
use vox_rule_pack::CompiledRule;

/// Detects hardcoded magic values: ports, IPs, filesystem paths, connection strings.
///
/// Detection patterns sourced from embedded rule pack (`magic-value/{port,ip,path,db-conn}`).
/// Skip logic (comment/const/test filters) kept in Rust.
///
/// Enforces AGENTS.md line 138:
/// > "No magic values: Never hardcode ports, database paths, or file system paths."
pub struct MagicValueDetector {
    port_rule: &'static CompiledRule,
    ip_rule: &'static CompiledRule,
    path_rule: &'static CompiledRule,
    db_conn_rule: &'static CompiledRule,
    /// Lines that are clearly comments or docs (to skip).
    comment_re: Regex,
    /// Lines that define constants (acceptable).
    const_def_re: Regex,
    /// Lines inside test modules (acceptable).
    test_attr_re: Regex,
}

impl Default for MagicValueDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl MagicValueDetector {
    pub fn new() -> Self {
        Self {
            port_rule: pack_rule("magic-value/port"),
            ip_rule: pack_rule("magic-value/ip"),
            path_rule: pack_rule("magic-value/path"),
            db_conn_rule: pack_rule("magic-value/db-conn"),
            comment_re: Regex::new(r"^\s*(?://|#|/\*|\*|--|\s*\*)").expect("valid"),
            const_def_re: Regex::new(
                r"(?:const |static |pub const |pub static |DEFAULT_|ENV_|CONFIG_)",
            )
            .expect("valid"),
            test_attr_re: Regex::new(r"#\[(?:cfg\(test\)|test)\]").expect("valid"),
        }
    }

    fn should_skip_line(&self, line: &str) -> bool {
        let trimmed = line.trim();
        if self.comment_re.is_match(trimmed) {
            return true;
        }
        if self.const_def_re.is_match(trimmed) {
            return true;
        }
        if trimmed.starts_with("use ") || trimmed.starts_with("import ") {
            return true;
        }
        false
    }

    fn make_finding(
        rule: &'static CompiledRule,
        file: &SourceFile,
        line_num: usize,
        message: &str,
        suggestion: &str,
        severity: Severity,
    ) -> Finding {
        Finding {
            rule_id: rule.id.clone(),
            rule_name: rule.name.clone(),
            severity,
            file: file.path.clone(),
            line: line_num,
            column: 0,
            message: message.to_string(),
            suggestion: Some(suggestion.to_string()),
            diagnostic_id: None,
            alternatives: vec![],
            rationale: None,
            context: file.context_around(line_num, 1),
            confidence: rule.confidence.map(Into::into),
            evidence: None,
        }
    }
}

impl DetectionRule for MagicValueDetector {
    fn id(&self) -> &'static str {
        "magic-value"
    }
    fn name(&self) -> &'static str {
        "Magic Value Detector"
    }
    fn description(&self) -> &'static str {
        "Detects hardcoded ports, IPs, file paths, and database connection strings"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn minimal_repro(&self) -> Option<&'static str> {
        Some(
            "// VIOLATION — hardcoded port and database path\n\
             let server = Server::bind(\"0.0.0.0:8080\");\n\
             let db = Database::open(\"/var/data/app.db\");\n\
             \n\
             // FIX — read from config or environment\n\
             let port: u16 = config.server_port;        // from Config struct\n\
             let server = Server::bind((\"0.0.0.0\", port));\n\
             let db_path = std::env::var(\"DB_PATH\")?;  // from environment\n\
             let db = Database::open(&db_path);",
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
        let mut findings = Vec::new();
        let mut in_test_block = false;

        for (i, line) in file.lines.iter().enumerate() {
            let line_num = i + 1;

            if self.test_attr_re.is_match(line) {
                in_test_block = true;
            }
            // Rough heuristic: test blocks end at next non-indented `fn` or `mod`.
            if in_test_block {
                let trimmed = line.trim();
                if (trimmed.starts_with("fn ") || trimmed.starts_with("mod "))
                    && !trimmed.contains("test")
                    && !line.starts_with(char::is_whitespace)
                {
                    in_test_block = false;
                }
            }

            if self.should_skip_line(line) || in_test_block {
                continue;
            }

            if self.port_rule.regex().is_match(line)
                && !line.contains("127.0.0.1:0")
                && !line.contains("0.0.0.0:0")
                && !line.contains("localhost:0")
            {
                findings.push(Self::make_finding(
                    self.port_rule,
                    file,
                    line_num,
                    "Hardcoded port/address — use an environment variable or constant",
                    "Extract to a named constant or read from `std::env::var(\"PORT\")`.",
                    self.severity(),
                ));
            }

            if self.ip_rule.regex().is_match(line)
                && !(line.contains("host ==") || line.contains("host=="))
            {
                findings.push(Self::make_finding(
                    self.ip_rule,
                    file,
                    line_num,
                    "Hardcoded IP address — use a configuration variable",
                    "Move to a config file or environment variable.",
                    self.severity(),
                ));
            }

            if self.path_rule.regex().is_match(line)
                // vox-arch-check: allow abs-path
                && !(line.contains("starts_with(\"/usr/")
                    || line.contains("starts_with(\"/bin/")
                    || line.contains("starts_with('/usr/")
                    || line.contains("starts_with('/bin/"))
            {
                findings.push(Self::make_finding(
                    self.path_rule,
                    file,
                    line_num,
                    "Hardcoded filesystem path — use a config or env variable",
                    "Replace with a configurable path or use `dirs` / `std::env` for dynamic resolution.",
                    self.severity(),
                ));
            }

            if self.db_conn_rule.regex().is_match(line) {
                findings.push(Self::make_finding(
                    self.db_conn_rule,
                    file,
                    line_num,
                    "Hardcoded database connection string — use an env variable",
                    "Move to `.env` and read via `std::env::var(\"DATABASE_URL\")`.",
                    Severity::Error,
                ));
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
    fn detects_hardcoded_port() {
        let d = MagicValueDetector::new();
        let f = source("rs", r#"let addr = "127.0.0.1:3000";"#);
        let findings = d.detect(&f, None);
        assert!(!findings.is_empty(), "should detect hardcoded port");
        assert_eq!(findings[0].rule_id, "magic-value/port");
    }

    #[test]
    fn detects_db_connection_string() {
        let d = MagicValueDetector::new();
        let f = source("py", r#"conn = "postgres://user:pass@localhost/db""#);
        let findings = d.detect(&f, None);
        assert!(
            findings.iter().any(|f| f.rule_id == "magic-value/db-conn"),
            "should detect DB connection string"
        );
    }

    #[test]
    fn skips_const_definitions() {
        let d = MagicValueDetector::new();
        let f = source("rs", r#"pub const DEFAULT_PORT: &str = "127.0.0.1:3000";"#);
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "const definitions should be allowed");
    }

    #[test]
    fn skips_comments() {
        let d = MagicValueDetector::new();
        let f = source("rs", r#"// connect to "127.0.0.1:5432""#);
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "comments should be skipped");
    }
}
