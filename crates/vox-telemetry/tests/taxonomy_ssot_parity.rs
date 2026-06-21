//! Privacy parity gate: the collection-taxonomy SSOT must be parseable and must contain
//! no free-form string/free fields (spec §3.2 — only enum|int|bool|hash allowed).

use serde_json::Value;

const TAXONOMY_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../contracts/telemetry/collection-taxonomy.v1.json"
);

/// Required category names from spec §5. If any are absent the test fails.
const REQUIRED_CATEGORIES: &[&str] = &[
    "command_usage",
    "skill_activation",
    "edit_pattern",
    "harness_usage",
    "error_surface",
    "default_decision",
    "model_prompt",
];

#[test]
fn taxonomy_parses_and_has_no_freeform_string_fields() {
    let txt = std::fs::read_to_string(TAXONOMY_PATH)
        .expect("contracts/telemetry/collection-taxonomy.v1.json must exist");
    let v: Value = serde_json::from_str(&txt).expect("taxonomy JSON must parse");

    let cats = v["categories"]
        .as_array()
        .expect("taxonomy must have a 'categories' array");

    for cat in cats {
        let cat_name = cat["name"]
            .as_str()
            .expect("each category must have a 'name'");
        let fields = cat["fields"]
            .as_array()
            .unwrap_or_else(|| panic!("category '{cat_name}' must have a 'fields' array"));

        for field in fields {
            let field_name = field["name"]
                .as_str()
                .unwrap_or_else(|| panic!("field in category '{cat_name}' missing 'name'"));
            let ty = field["type"]
                .as_str()
                .unwrap_or_else(|| panic!("field '{field_name}' in '{cat_name}' missing 'type'"));

            assert!(
                matches!(ty, "enum" | "int" | "bool" | "hash"),
                "field '{field_name}' in category '{cat_name}' has type '{ty}' — \
                 only enum|int|bool|hash are allowed (spec §3.2, privacy invariant #2). \
                 Free-form strings must never appear in the taxonomy allowlist."
            );

            // enum fields must have a non-empty 'allowed' list
            if ty == "enum" {
                let allowed = field["allowed"].as_array().unwrap_or_else(|| {
                    panic!("enum field '{field_name}' in '{cat_name}' must have an 'allowed' list")
                });
                assert!(
                    !allowed.is_empty(),
                    "enum field '{field_name}' in '{cat_name}' has an empty 'allowed' list"
                );
            }
        }
    }
}

#[test]
fn taxonomy_contains_all_required_categories() {
    let txt = std::fs::read_to_string(TAXONOMY_PATH).expect("taxonomy must exist");
    let v: Value = serde_json::from_str(&txt).expect("taxonomy must parse");

    let cats = v["categories"].as_array().expect("must have 'categories'");
    let present: std::collections::HashSet<&str> =
        cats.iter().filter_map(|c| c["name"].as_str()).collect();

    for required in REQUIRED_CATEGORIES {
        assert!(
            present.contains(required),
            "required category '{required}' is missing from the taxonomy (spec §5)"
        );
    }
}

#[test]
fn taxonomy_version_is_1() {
    let txt = std::fs::read_to_string(TAXONOMY_PATH).expect("taxonomy must exist");
    let v: Value = serde_json::from_str(&txt).expect("taxonomy must parse");
    assert_eq!(v["version"].as_u64(), Some(1), "taxonomy version must be 1");
}

#[test]
fn taxonomy_k_anonymity_floor_is_at_least_20() {
    let txt = std::fs::read_to_string(TAXONOMY_PATH).expect("taxonomy must exist");
    let v: Value = serde_json::from_str(&txt).expect("taxonomy must parse");
    let k = v["k_anonymity"]
        .as_u64()
        .expect("taxonomy must have a numeric 'k_anonymity' field");
    assert!(k >= 20, "k_anonymity must be ≥ 20 (spec §3.7); got {k}");
}
