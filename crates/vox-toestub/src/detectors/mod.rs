//! Registry of all built-in detection rules.

pub mod deprecated_usage;
pub mod dry_violation;
pub mod empty_body;
pub mod magic_value;
pub mod secrets;
pub mod god_object;
pub mod sprawl;
pub mod schema_compliance;
pub mod stub;
pub mod unresolved_ref;
pub mod unwired_module;
pub mod victory_claim;
pub mod file_organization;
pub mod stringly_typed_enum;

use crate::rules::DetectionRule;

/// Returns all built-in detectors.
pub fn all_rules(schema_path: Option<std::path::PathBuf>) -> Vec<Box<dyn DetectionRule>> {
    vec![
        Box::new(stub::StubDetector::new()),
        Box::new(empty_body::EmptyBodyDetector::new()),
        Box::new(magic_value::MagicValueDetector::new()),
        Box::new(victory_claim::VictoryClaimDetector::new()),
        Box::new(unwired_module::UnwiredModuleDetector::new()),
        Box::new(dry_violation::DryViolationDetector::new()),
        Box::new(unresolved_ref::UnresolvedRefDetector::new()),
        Box::new(deprecated_usage::DeprecatedUsageDetector::new()),
        Box::new(secrets::SecretDetector::new()),
        Box::new(god_object::GodObjectDetector::default()),
        Box::new(sprawl::SprawlDetector::default()),
        Box::new(schema_compliance::SchemaComplianceDetector::new(schema_path)),
        Box::new(file_organization::FileOrganizationDetector::default()),
        Box::new(stringly_typed_enum::StringlyTypedEnumDetector::new()),
    ]
}

/// Returns the number of built-in rules.
pub fn rule_count() -> usize {
    14
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_rules_instantiate() {
        let rules = all_rules(None);
        assert_eq!(rules.len(), rule_count());
        // Every rule must have a non-empty ID and name
        for rule in &rules {
            println!("Rule ID: {}", rule.id());
            assert!(!rule.id().is_empty(), "rule ID must not be empty");
            assert!(!rule.name().is_empty(), "rule name must not be empty");
            assert!(
                !rule.languages().is_empty(),
                "rule must support at least one language"
            );
        }
    }
    #[test]
    fn god_object_detector_catches_large_files() {
        use crate::rules::SourceFile;
        use std::path::PathBuf;
        let content = "fn main() {}\n".repeat(600);
        let file = SourceFile::new(PathBuf::from("large.rs"), content);
        let detector = god_object::GodObjectDetector::default();
        let findings = detector.detect(&file);
        assert!(!findings.is_empty());
        assert!(findings[0].message.contains("too large"));
    }

    #[test]
    fn sprawl_detector_catches_forbidden_names() {
        use crate::rules::SourceFile;
        use std::path::PathBuf;
        let file = SourceFile::new(PathBuf::from("utils.rs"), "fn helper() {}".to_string());
        let detector = sprawl::SprawlDetector::default();
        let findings = detector.detect(&file);
        assert!(!findings.is_empty());
        assert!(findings[0].message.contains("forbidden"));
    }

    #[test]
    fn organization_detector_catches_bloated_lib() {
        use crate::rules::SourceFile;
        use std::path::PathBuf;
        let content = "pub struct A; pub struct B; pub struct C; pub struct D;".replace("; ", ";\n");
        let file = SourceFile::new(PathBuf::from("src/lib.rs"), content);
        let detector = file_organization::FileOrganizationDetector::default();
        let findings = detector.detect(&file);
        assert!(!findings.is_empty());
        assert!(findings[0].message.contains("lib.rs contains 4 definitions"));
    }
}
