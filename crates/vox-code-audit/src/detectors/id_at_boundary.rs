use crate::diagnostics::catalog;
use crate::rules::{DetectionRule, Finding, FindingConfidence, Language, Severity, SourceFile};
use regex::Regex;

/// Detects bare `str`-typed ID parameters at API boundaries in Vox files.
///
/// Functions decorated with `@query`, `@mutation`, `@server`, or `@activity`, or
/// message handlers inside `actor {}` blocks, should use typed ID wrappers like
/// `Id[User]` instead of `str` for parameters whose names end with `_id`.
pub struct IdAtBoundaryDetector {
    /// Matches `@query`, `@mutation`, `@server`, or `@activity` decorators
    boundary_decorator: Regex,
    /// Matches `actor { ... }` block open
    actor_block: Regex,
    /// Matches `fn name(` inside actors (simple heuristic)
    fn_line: Regex,
    /// Matches a parameter like `user_id: str` (bare str for _id-suffixed param)
    bare_id_param: Regex,
    supported_langs: Vec<Language>,
}

impl Default for IdAtBoundaryDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl IdAtBoundaryDetector {
    pub fn new() -> Self {
        Self {
            boundary_decorator: Regex::new(r"^\s*@(?:query|mutation|server|activity)\b").expect("valid regex"),
            actor_block: Regex::new(r"\bactor\s+\w+\s*\{").expect("valid regex"),
            fn_line: Regex::new(r"^\s*fn\s+\w+\s*\(").expect("valid regex"),
            bare_id_param: Regex::new(r"\b(\w+_id)\s*:\s*str\b").expect("valid regex"),
            supported_langs: vec![Language::Vox],
        }
    }
}

impl DetectionRule for IdAtBoundaryDetector {
    fn id(&self) -> &'static str {
        "types/id-required-at-boundary"
    }

    fn name(&self) -> &'static str {
        "ID Required at Boundary Detector"
    }

    fn description(&self) -> &'static str {
        "Detects bare `str` parameters with `_id` suffix names at API boundaries \
        (@query, @mutation, @server, @activity, actor message handlers) where typed ID wrappers should be used."
    }

    fn severity(&self) -> Severity {
        Severity::Warning
    }

    fn languages(&self) -> &[Language] {
        &self.supported_langs
    }

    fn diagnostic_id(&self) -> Option<&'static str> {
        Some(catalog::TYPES_ID_REQUIRED_AT_BOUNDARY)
    }

    fn explain(&self) -> &'static str {
        "ID parameters at API boundaries must use typed wrappers (e.g. `Id[User]`) instead of bare `str` to prevent accidental ID confusion across entity types."
    }

    fn minimal_repro(&self) -> Option<&'static str> {
        Some(
            "// VIOLATION — bare str with _id suffix at an @query boundary\n\
             @query\n\
             fn get_order(order_id: str) -> Order {\n\
             \x20   db.orders.get(order_id)\n\
             }\n\
             \n\
             // FIX — use a typed ID wrapper\n\
             @query\n\
             fn get_order(order_id: Id[Order]) -> Order {\n\
             \x20   db.orders.get(order_id)\n\
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
        let n = lines.len();
        let mut in_actor_block = false;

        for i in 0..n {
            let line = &lines[i];
            let trimmed = line.trim();

            // Skip comment lines
            if trimmed.starts_with("//") || trimmed.starts_with('#') || trimmed.starts_with("/*") {
                continue;
            }

            // Track entry into actor blocks (simple heuristic: actor keyword opens block)
            if self.actor_block.is_match(line) {
                in_actor_block = true;
            }
            // Simple heuristic: if we see a closing brace at column 0, we may have left the actor
            if !line.starts_with(' ') && !line.starts_with('\t') && line.trim() == "}" {
                in_actor_block = false;
            }

            // Determine if this is a boundary line:
            // Either the line itself (or the previous line) is @query/@mutation/@server/@activity,
            // OR we are inside an actor block and this line starts a fn
            let is_decorated_boundary = self.boundary_decorator.is_match(line)
                || (i > 0 && self.boundary_decorator.is_match(&lines[i - 1]));

            let is_actor_fn = in_actor_block && self.fn_line.is_match(line);

            if !is_decorated_boundary && !is_actor_fn {
                continue;
            }

            // Check this line and up to 3 following lines for the bare _id: str pattern
            let end = (i + 4).min(n);
            for (j, sig_line) in lines.iter().enumerate().skip(i).take(end - i) {
                if let Some(cap) = self.bare_id_param.captures(sig_line) {
                    let param_name = cap.get(1).map_or("", |m| m.as_str());
                    let m = self.bare_id_param.find(sig_line).unwrap();
                    findings.push(Finding {
                        rule_id: self.id().to_string(),
                        diagnostic_id: self.diagnostic_id().map(str::to_string),
                        rule_name: self.name().to_string(),
                        severity: Severity::Warning,
                        file: file.path.clone(),
                        line: j + 1,
                        column: m.start() + 1,
                        message: format!(
                            "Parameter `{param_name}: str` at API boundary — use a typed ID wrapper like `Id[{}]` instead.",
                            param_name
                                .trim_end_matches("_id")
                                .split('_')
                                .map(|s| {
                                    let mut c = s.chars();
                                    match c.next() {
                                        None => String::new(),
                                        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                                    }
                                })
                                .collect::<String>()
                        ),
                        suggestion: Some(format!(
                            "Replace `{param_name}: str` with a typed ID wrapper such as `Id[EntityType]` to prevent ID confusion."
                        )),
                        alternatives: vec![],
                        rationale: Some(
                            "Using bare `str` for ID parameters at API boundaries allows callers \
                            to pass IDs of the wrong entity type without a compile-time error. \
                            Typed wrappers like `Id[User]` make this impossible.".into(),
                        ),
                        context: file.context_around(j + 1, 2),
                        confidence: Some(FindingConfidence::Medium),
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

    fn source(code: &str) -> SourceFile {
        SourceFile::new(PathBuf::from("test.vox"), code.to_string())
    }

    #[test]
    fn flags_bare_str_id_under_query() {
        let d = IdAtBoundaryDetector::new();
        let code = "@query\nfn get_user(user_id: str) -> User {}";
        let f = source(code);
        let findings = d.detect(&f, None);
        assert!(
            !findings.is_empty(),
            "should flag user_id: str under @query"
        );
        assert!(findings[0].message.contains("user_id"));
    }

    #[test]
    fn ignores_typed_id_under_query() {
        let d = IdAtBoundaryDetector::new();
        let code = "@query\nfn get_user(user_id: Id[User]) -> User {}";
        let f = source(code);
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "Id[User] typed param should not fire");
    }

    #[test]
    fn ignores_non_id_str_param() {
        let d = IdAtBoundaryDetector::new();
        let code = "@query\nfn compute(value: str) -> str {}";
        let f = source(code);
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "non-_id str param should not fire");
    }

    #[test]
    fn flags_bare_str_id_under_activity() {
        let d = IdAtBoundaryDetector::new();
        let code = "@activity\nfn process_order(order_id: str) {}";
        let f = source(code);
        let findings = d.detect(&f, None);
        assert!(
            !findings.is_empty(),
            "should flag order_id: str under @activity"
        );
    }

    #[test]
    fn does_not_fire_on_rust_files() {
        let d = IdAtBoundaryDetector::new();
        let f = SourceFile::new(
            PathBuf::from("test.rs"),
            "@query\nfn get_user(user_id: str) {}".to_string(),
        );
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "rust files should be ignored");
    }
}
