//! `contracts/code-audit/rules.v1.yaml` rule ids must not equal compiler diagnostic `code` strings.

use std::collections::HashSet;
use std::path::PathBuf;

fn audit_rule_ids(yaml: &str) -> Vec<String> {
    yaml.lines()
        .filter_map(|line| {
            let s = line.trim_start();
            let rest = s.strip_prefix("- id:")?;
            Some(rest.trim().to_string())
        })
        .collect()
}

#[test]
fn no_audit_rule_collides_with_compiler_diagnostic_code() {
    let rules_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../contracts/code-audit/rules.v1.yaml");
    let raw = std::fs::read_to_string(&rules_path).expect("read rules.v1.yaml");
    let ids = audit_rule_ids(&raw);

    let compiler: HashSet<&str> =
        vox_compiler::typeck::diagnostics::codes::ALL_COMPILER_DIAGNOSTIC_CODES
            .iter()
            .copied()
            .collect();

    for id in ids {
        assert!(
            !compiler.contains(id.as_str()),
            "audit rule id `{id}` collides with a compiler diagnostic code"
        );
    }
}
