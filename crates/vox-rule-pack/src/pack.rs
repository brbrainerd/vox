//! Compiled, runtime-ready container of rules. Built from a `RuleFile`.

use std::collections::HashMap;
use std::path::Path;

use regex::Regex;

use crate::error::{RulePackError, RulePackResult};
use crate::schema::{MatchKind, MatchSpec, RuleFile, RuleSpec, SkipScope};
use crate::types::{RuleConfidence, RuleLanguage, RuleSeverity};

/// A rule with its regex pre-compiled and metadata ready for runtime use.
#[derive(Debug)]
pub struct CompiledRule {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub description: String,
    pub severity: RuleSeverity,
    pub confidence: Option<RuleConfidence>,
    pub languages: Vec<RuleLanguage>,
    pub message: String,
    pub suggestion: Option<String>,
    pub skip_in: Vec<SkipScope>,
    pub kind: MatchKind,
    regex: Regex,
}

impl CompiledRule {
    /// Returns true if the compiled regex matches the given line.
    pub fn matches_line(&self, line: &str) -> bool {
        self.regex.is_match(line)
    }

    /// For `MultilineRegex` rules: returns 1-indexed line numbers of every match in `content`.
    /// Returns an empty vec for `LineRegex` and `Substring` kinds (use `matches_line` instead).
    pub fn matches_in_content(&self, content: &str) -> Vec<usize> {
        if self.kind != MatchKind::MultilineRegex {
            return Vec::new();
        }
        self.regex
            .find_iter(content)
            .map(|m| content[..m.start()].lines().count() + 1)
            .collect()
    }

    /// Returns a reference to the pre-compiled regex.
    pub fn regex(&self) -> &Regex {
        &self.regex
    }
}

/// Loaded, compiled rule pack. Build with [`RulePack::load_from_str`] or
/// [`RulePack::load_from_path`]. Cheap to clone via `Arc<RulePack>`.
#[derive(Debug)]
pub struct RulePack {
    rules: Vec<CompiledRule>,
    by_id: HashMap<String, usize>,
}

impl RulePack {
    /// Parse and compile a rule pack from a YAML string.
    pub fn load_from_str(yaml: &str) -> RulePackResult<Self> {
        let file: RuleFile = serde_yaml::from_str(yaml)?;
        if file.version != 1 {
            return Err(RulePackError::UnsupportedVersion(file.version));
        }
        let mut rules = Vec::with_capacity(file.rules.len());
        let mut by_id: HashMap<String, usize> = HashMap::new();
        for spec in file.rules {
            if by_id.contains_key(&spec.id) {
                return Err(RulePackError::DuplicateId(spec.id));
            }
            let compiled = compile(spec)?;
            by_id.insert(compiled.id.clone(), rules.len());
            rules.push(compiled);
        }
        Ok(Self { rules, by_id })
    }

    /// Read a YAML file from disk and compile it.
    pub fn load_from_path(path: &Path) -> RulePackResult<Self> {
        let yaml = std::fs::read_to_string(path).map_err(|source| RulePackError::Io {
            path: path.display().to_string(),
            source,
        })?;
        Self::load_from_str(&yaml)
    }

    /// Number of compiled rules in this pack.
    pub fn len(&self) -> usize {
        self.rules.len()
    }

    /// Returns true if there are no rules.
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Look up a rule by its exact ID.
    pub fn rule(&self, id: &str) -> Option<&CompiledRule> {
        self.by_id.get(id).map(|&i| &self.rules[i])
    }

    /// Iterate all rules in declaration order.
    pub fn rules(&self) -> &[CompiledRule] {
        &self.rules
    }

    /// Iterate only the rules that apply to the given language.
    pub fn rules_for_language(&self, lang: RuleLanguage) -> impl Iterator<Item = &CompiledRule> {
        self.rules
            .iter()
            .filter(move |r| r.languages.contains(&lang))
    }
}

fn compile(spec: RuleSpec) -> RulePackResult<CompiledRule> {
    let regex = build_regex(&spec.id, &spec.match_spec)?;
    Ok(CompiledRule {
        id: spec.id,
        parent_id: spec.parent_id,
        name: spec.name,
        description: spec.description,
        severity: spec.severity,
        confidence: spec.confidence,
        languages: spec.languages,
        message: spec.message,
        suggestion: spec.suggestion,
        skip_in: spec.match_spec.skip_in,
        kind: spec.match_spec.kind,
        regex,
    })
}

fn build_regex(rule_id: &str, m: &MatchSpec) -> RulePackResult<Regex> {
    let pattern = match m.kind {
        MatchKind::LineRegex | MatchKind::MultilineRegex => m.pattern.clone(),
        MatchKind::Substring => regex::escape(&m.pattern),
    };
    Regex::new(&pattern).map_err(|source| RulePackError::InvalidRegex {
        rule_id: rule_id.to_string(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
version: 1
rules:
  - id: test/foo
    name: "Foo"
    description: "Test rule"
    severity: warning
    confidence: medium
    languages: [rust]
    match: { kind: line-regex, pattern: "foo\\d+" }
    message: "matched"
"#;

    #[test]
    fn loads_from_str() {
        let pack = RulePack::load_from_str(SAMPLE).unwrap();
        assert_eq!(pack.len(), 1);
        let rule = pack.rule("test/foo").unwrap();
        assert_eq!(rule.severity, RuleSeverity::Warning);
        assert!(rule.matches_line("foo123"));
        assert!(!rule.matches_line("bar123"));
    }

    #[test]
    fn rejects_invalid_regex() {
        let bad = SAMPLE.replace("foo\\\\d+", "(unclosed");
        let err = RulePack::load_from_str(&bad).unwrap_err();
        assert!(
            matches!(err, RulePackError::InvalidRegex { .. }),
            "expected InvalidRegex, got: {err}"
        );
    }

    #[test]
    fn rejects_duplicate_id() {
        let dup = format!(
            "{}\n  - id: test/foo\n    name: dup\n    description: dup\n    severity: warning\n    languages: [rust]\n    match: {{kind: line-regex, pattern: \"x\"}}\n    message: m\n",
            SAMPLE
        );
        let err = RulePack::load_from_str(&dup).unwrap_err();
        assert!(
            matches!(err, RulePackError::DuplicateId(_)),
            "expected DuplicateId, got: {err}"
        );
    }

    #[test]
    fn iterates_by_language() {
        let pack = RulePack::load_from_str(SAMPLE).unwrap();
        assert_eq!(pack.rules_for_language(RuleLanguage::Rust).count(), 1);
        assert_eq!(pack.rules_for_language(RuleLanguage::Python).count(), 0);
    }

    #[test]
    fn substring_kind_escapes_pattern() {
        let yaml = r#"
version: 1
rules:
  - id: test/sub
    name: Sub
    description: Sub
    severity: info
    languages: [rust]
    match: { kind: substring, pattern: "foo.bar" }
    message: m
"#;
        let pack = RulePack::load_from_str(yaml).unwrap();
        let rule = pack.rule("test/sub").unwrap();
        // literal "foo.bar" must match but "fooXbar" must not (dot is escaped)
        assert!(rule.matches_line("prefix foo.bar suffix"));
        assert!(!rule.matches_line("prefix fooXbar suffix"));
    }
}

#[cfg(test)]
mod semcov_wave5_tests {
    use super::*;

    #[test]
    fn matches_in_content_returns_empty_for_line_regex_kind() {
        let yaml = r#"
version: 1
rules:
  - id: test/lr
    name: LR
    description: LR
    severity: warning
    languages: [rust]
    match: { kind: line-regex, pattern: "foo" }
    message: m
"#;
        let pack = RulePack::load_from_str(yaml).unwrap();
        let rule = pack.rule("test/lr").unwrap();
        // LineRegex kind must return empty vec regardless of content.
        let result = rule.matches_in_content("foo\nfoo\nfoo");
        assert_eq!(result, Vec::<usize>::new());
    }

    #[test]
    fn matches_in_content_returns_empty_for_substring_kind() {
        let yaml = r#"
version: 1
rules:
  - id: test/sub
    name: Sub
    description: Sub
    severity: warning
    languages: [rust]
    match: { kind: substring, pattern: "foo" }
    message: m
"#;
        let pack = RulePack::load_from_str(yaml).unwrap();
        let rule = pack.rule("test/sub").unwrap();
        let result = rule.matches_in_content("foo\nfoo");
        assert_eq!(result, Vec::<usize>::new());
    }

    #[test]
    fn matches_in_content_finds_correct_1indexed_lines_for_multiline_regex() {
        let yaml = r#"
version: 1
rules:
  - id: test/ml
    name: ML
    description: ML
    severity: warning
    languages: [rust]
    match: { kind: multiline-regex, pattern: "TODO" }
    message: m
"#;
        let pack = RulePack::load_from_str(yaml).unwrap();
        let rule = pack.rule("test/ml").unwrap();
        // Content: line 1 clean, line 2 matches, line 4 matches.
        let content = "clean\nTODO fix this\nstill clean\nTODO another";
        let mut lines = rule.matches_in_content(content);
        lines.sort();
        assert_eq!(lines, vec![2, 4], "expected matches on lines 2 and 4");
    }

    #[test]
    fn matches_in_content_returns_empty_when_no_match_multiline() {
        let yaml = r#"
version: 1
rules:
  - id: test/ml2
    name: ML2
    description: ML2
    severity: warning
    languages: [rust]
    match: { kind: multiline-regex, pattern: "FIXME" }
    message: m
"#;
        let pack = RulePack::load_from_str(yaml).unwrap();
        let rule = pack.rule("test/ml2").unwrap();
        let result = rule.matches_in_content("no match here\nnor here");
        assert_eq!(result, Vec::<usize>::new());
    }

    #[test]
    fn matches_in_content_first_line_match_reports_line_1() {
        let yaml = r#"
version: 1
rules:
  - id: test/ml3
    name: ML3
    description: ML3
    severity: warning
    languages: [rust]
    match: { kind: multiline-regex, pattern: "^START" }
    message: m
"#;
        let pack = RulePack::load_from_str(yaml).unwrap();
        let rule = pack.rule("test/ml3").unwrap();
        let content = "START here\nsecond line";
        let lines = rule.matches_in_content(content);
        assert_eq!(lines, vec![1]);
    }
}
