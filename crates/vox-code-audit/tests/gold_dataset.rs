//! Gold dataset harness: schema-validated cases run against TOESTUB detectors.

use std::path::PathBuf;

use serde::Deserialize;
use vox_code_audit::analysis::RustFileContext;
use vox_code_audit::detectors;
use vox_code_audit::rules::{DetectionRule, SourceFile};

#[derive(Debug, Deserialize)]
struct GoldFile {
    version: u32,
    cases: Vec<GoldCase>,
}

#[derive(Debug, Deserialize)]
struct GoldCase {
    id: String,
    label: String,
    rule_id_prefix: String,
    path: String,
    line: usize,
    #[serde(default)]
    snippet: Option<String>,
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn validate_against_schema(instance: &serde_json::Value) {
    let root = repo_root();
    let schema_path = root.join("contracts/toestub/gold-dataset.v1.schema.json");
    let schema_val: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&schema_path).unwrap_or_else(|e| {
            panic!("read gold schema {}: {e}", schema_path.display());
        }))
        .expect("parse gold schema");
    let validator = vox_jsonschema_util::compile_validator(&schema_val, schema_path.display())
        .expect("compile gold schema");
    vox_jsonschema_util::validate(instance, &validator, "gold dataset")
        .expect("gold dataset must match schema");
}

fn load_gold() -> GoldFile {
    let root = repo_root();
    let path = root.join("contracts/toestub/gold-dataset.v1.json");
    let raw =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let v: serde_json::Value = serde_json::from_str(&raw).expect("parse gold json");
    validate_against_schema(&v);
    serde_json::from_value(v).expect("deserialize gold file")
}

fn rule_matches_gold_prefix(rule: &dyn DetectionRule, prefix: &str) -> bool {
    if rule.id() == prefix || rule.id().starts_with(prefix) || prefix.starts_with(rule.id()) {
        return true;
    }
    // Sub-findings use hierarchical ids (e.g. `stub/todo`) while detector id may differ.
    match rule.id() {
        "arch/stub" => prefix.starts_with("stub/"),
        "scaling/surfaces" => prefix.starts_with("scaling/"),
        "unresolved-ref" => prefix.starts_with("unresolved-ref"),
        _ => false,
    }
}

fn rule_for_matching_prefix<'a>(
    rules: &'a [Box<dyn DetectionRule>],
    prefix: &str,
) -> Option<&'a dyn DetectionRule> {
    rules
        .iter()
        .find(|r| rule_matches_gold_prefix(r.as_ref(), prefix))
        .map(|b| b.as_ref())
}

#[test]
fn gold_cases_precision_harness() {
    let gold = load_gold();
    assert_eq!(gold.version, 1);
    let rules = detectors::all_rules(None);

    for case in &gold.cases {
        let Some(snippet) = &case.snippet else {
            continue;
        };
        let path = PathBuf::from(&case.path);
        let file = SourceFile::new(path, snippet.clone());
        let rust_ctx = if file.language == vox_code_audit::rules::Language::Rust {
            Some(RustFileContext::parse(snippet))
        } else {
            None
        };
        let rule = rule_for_matching_prefix(&rules, &case.rule_id_prefix).unwrap_or_else(|| {
            panic!(
                "gold case {}: no rule for prefix {}",
                case.id, case.rule_id_prefix
            );
        });

        let findings = rule.detect(&file, rust_ctx.as_ref());
        let matches_line: Vec<_> = findings
            .iter()
            .filter(|f| f.line == case.line && f.rule_id.starts_with(&case.rule_id_prefix))
            .collect();

        match case.label.as_str() {
            "true_positive" => assert!(
                !matches_line.is_empty(),
                "case {}: expected finding at line {} for prefix {}",
                case.id,
                case.line,
                case.rule_id_prefix
            ),
            "false_positive" => assert!(
                matches_line.is_empty(),
                "case {}: expected no finding at line {} for prefix {} (got {:?})",
                case.id,
                case.line,
                case.rule_id_prefix,
                matches_line
            ),
            "false_negative" => {
                // Reserved for future FN fixtures; skip until harness models expected gaps.
            }
            other => panic!("case {}: unknown label {}", case.id, other),
        }
    }
}

#[test]
fn gold_eval_precision_recall_by_rule_prefix() {
    let gold = load_gold();
    let rules = detectors::all_rules(None);
    let mut tp = 0usize;
    let mut fp = 0usize;
    let mut fn_ct = 0usize;

    for case in &gold.cases {
        let Some(snippet) = &case.snippet else {
            continue;
        };
        let path = PathBuf::from(&case.path);
        let file = SourceFile::new(path, snippet.clone());
        let rust_ctx = if file.language == vox_code_audit::rules::Language::Rust {
            Some(RustFileContext::parse(snippet))
        } else {
            None
        };
        let rule = rule_for_matching_prefix(&rules, &case.rule_id_prefix).unwrap_or_else(|| {
            panic!(
                "gold case {}: no rule for prefix {}",
                case.id, case.rule_id_prefix
            );
        });
        let findings = rule.detect(&file, rust_ctx.as_ref());
        let matches_line: Vec<_> = findings
            .iter()
            .filter(|f| f.line == case.line && f.rule_id.starts_with(&case.rule_id_prefix))
            .collect();

        match (case.label.as_str(), matches_line.is_empty()) {
            ("true_positive", true) => fn_ct += 1,
            ("true_positive", false) => tp += 1,
            ("false_positive", false) => fp += 1,
            _ => {}
        }
    }

    let prec = if tp + fp > 0 {
        tp as f64 / (tp + fp) as f64
    } else {
        1.0
    };
    let rec = if tp + fn_ct > 0 {
        tp as f64 / (tp + fn_ct) as f64
    } else {
        1.0
    };
    assert!(
        prec >= 0.5,
        "gold eval: precision {:.3} below sanity floor 0.5 (tp={tp} fp={fp})",
        prec
    );
    assert!(
        rec >= 0.5,
        "gold eval: recall {:.3} below sanity floor 0.5 (tp={tp} fn={fn_ct})",
        rec
    );
}
