use crate::rules::{DetectionRule, Finding, Language, Severity, SourceFile};
use regex::Regex;

/// Detects hardcoded magic values: ports, IPs, filesystem paths, connection strings.
///
/// Enforces AGENTS.md line 138:
/// > "No magic values: Never hardcode ports, database paths, or file system paths."
pub struct MagicValueDetector {
    /// Common hardcoded port numbers.
    port_re: Regex,
    /// `localhost` or IP addresses.
    ip_localhost_re: Regex,
    /// Absolute file paths (Windows and Unix).
    abs_path_re: Regex,
    /// Database connection strings.
    db_conn_re: Regex,
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
            port_re: Regex::new(
                r#"(?:"|')\s*(?:127\.0\.0\.1|0\.0\.0\.0|localhost)\s*:\s*\d+"#,
            )
            .expect("valid"),
            ip_localhost_re: Regex::new(
                r#"(?:"|')(?:127\.0\.0\.1|0\.0\.0\.0|192\.168\.\d+\.\d+|10\.\d+\.\d+\.\d+)(?:"|')"#,
            )
            .expect("valid"),
            abs_path_re: Regex::new(
                r#"(?:"|')(?:C:\\|D:\\|/home/|/tmp/|/var/|/usr/|/etc/)[^"']*(?:"|')"#,
            )
            .expect("valid"),
            db_conn_re: Regex::new(
                r#"(?:"|')(?:postgres(?:ql)?://|mysql://|mongodb://|redis://|sqlite:)[^"']*(?:"|')"#,
            )
            .expect("valid"),
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
        // Skip comments and documentation
        if self.comment_re.is_match(trimmed) {
            return true;
        }
        // Skip constant definitions (that's where magic values *should* be)
        if self.const_def_re.is_match(trimmed) {
            return true;
        }
        // Skip `use` and `import` statements
        if trimmed.starts_with("use ") || trimmed.starts_with("import ") {
            return true;
        }
        false
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
    fn languages(&self) -> &[Language] {
        &[
            Language::Rust,
            Language::TypeScript,
            Language::Python,
            Language::GDScript,
            Language::Vox,
        ]
    }

    fn detect(&self, file: &SourceFile) -> Vec<Finding> {
        let mut findings = Vec::new();
        let mut in_test_block = false;

        for (i, line) in file.lines.iter().enumerate() {
            let line_num = i + 1;

            if self.test_attr_re.is_match(line) {
                in_test_block = true;
            }
            // Rough heuristic: test blocks end at next non-indented `fn` or `mod`
            if in_test_block {
                let trimmed = line.trim();
                if (trimmed.starts_with("fn ") || trimmed.starts_with("mod "))
                    && !trimmed.contains("test")
                    && !line.starts_with(char::is_whitespace)
                {
                    in_test_block = false;
                }
            }

            // Skip comments, consts, test blocks
            if self.should_skip_line(line) || in_test_block {
                continue;
            }

            // --- Port / localhost detection ---
            if self.port_re.is_match(line) {
                findings.push(Finding {
                    rule_id: "magic-value/port".to_string(),
                    rule_name: self.name().to_string(),
                    severity: self.severity(),
                    file: file.path.clone(),
                    line: line_num,
                    column: 0,
                    message: "Hardcoded port/address — use an environment variable or constant"
                        .to_string(),
                    suggestion: Some(
                        "Extract to a named constant or read from `std::env::var(\"PORT\")`."
                            .to_string(),
                    ),
                    context: file.context_around(line_num, 1),
                });
            }

            // --- IP address detection ---
            if self.ip_localhost_re.is_match(line) {
                findings.push(Finding {
                    rule_id: "magic-value/ip".to_string(),
                    rule_name: self.name().to_string(),
                    severity: self.severity(),
                    file: file.path.clone(),
                    line: line_num,
                    column: 0,
                    message: "Hardcoded IP address — use a configuration variable".to_string(),
                    suggestion: Some("Move to a config file or environment variable.".to_string()),
                    context: file.context_around(line_num, 1),
                });
            }

            // --- Absolute path detection ---
            if self.abs_path_re.is_match(line) {
                findings.push(Finding {
                    rule_id: "magic-value/path".to_string(),
                    rule_name: self.name().to_string(),
                    severity: self.severity(),
                    file: file.path.clone(),
                    line: line_num,
                    column: 0,
                    message: "Hardcoded filesystem path — use a config or env variable".to_string(),
                    suggestion: Some(
                        "Replace with a configurable path or use `dirs` / `std::env` for dynamic resolution."
                            .to_string(),
                    ),
                    context: file.context_around(line_num, 1),
                });
            }

            // --- Database connection string detection ---
            if self.db_conn_re.is_match(line) {
                findings.push(Finding {
                    rule_id: "magic-value/db-conn".to_string(),
                    rule_name: self.name().to_string(),
                    severity: Severity::Error,
                    file: file.path.clone(),
                    line: line_num,
                    column: 0,
                    message: "Hardcoded database connection string — use an env variable"
                        .to_string(),
                    suggestion: Some(
                        "Move to `.env` and read via `std::env::var(\"DATABASE_URL\")`."
                            .to_string(),
                    ),
                    context: file.context_around(line_num, 1),
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

    fn source(ext: &str, code: &str) -> SourceFile {
        SourceFile::new(PathBuf::from(format!("test.{}", ext)), code.to_string())
    }

    #[test]
    fn detects_hardcoded_port() {
        let d = MagicValueDetector::new();
        let f = source("rs", r#"let addr = "127.0.0.1:3000";"#);
        let findings = d.detect(&f);
        assert!(!findings.is_empty(), "should detect hardcoded port");
    }

    #[test]
    fn detects_db_connection_string() {
        let d = MagicValueDetector::new();
        let f = source("py", r#"conn = "postgres://user:pass@localhost/db""#);
        let findings = d.detect(&f);
        assert!(
            findings.iter().any(|f| f.rule_id == "magic-value/db-conn"),
            "should detect DB connection string"
        );
    }

    #[test]
    fn skips_const_definitions() {
        let d = MagicValueDetector::new();
        let f = source("rs", r#"pub const DEFAULT_PORT: &str = "127.0.0.1:3000";"#);
        let findings = d.detect(&f);
        assert!(findings.is_empty(), "const definitions should be allowed");
    }

    #[test]
    fn skips_comments() {
        let d = MagicValueDetector::new();
        let f = source("rs", r#"// connect to "127.0.0.1:5432""#);
        let findings = d.detect(&f);
        assert!(findings.is_empty(), "comments should be skipped");
    }
}
