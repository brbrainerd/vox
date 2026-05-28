// TODO(VUV-10+): currently this rule only enforces consistency for
// `RenameKind::Primitive`. Decorator and Kwarg consistency rules
// belong in their respective registries (when those rename kinds
// are first used by future VUV phases). EnumValue and Type
// consistency requires reflective access to those name sets which
// is not yet exposed; add when the first such rename lands.

//! VUV-9 Task 8: every `from` of kind `Primitive` in the rename registry must
//! NOT be a current canonical primitive name. The registry alone resolves old
//! names; the parser must not still recognize them.
//!
//! Today: registry is empty — test passes trivially.
//! Tomorrow: when we deprecate a primitive, the rename entry MUST be added in
//! the same commit that removes the primitive from the canonical set.

use std::collections::HashSet;

use vox_rename_registry::primitive_tags;
use vox_rename_registry::renames::{RenameKind, RenameRegistry};

#[test]
fn registry_from_names_are_not_canonical_primitives() {
    let registry = RenameRegistry::load_canonical().expect("load canonical registry");

    let canonical: HashSet<&'static str> =
        primitive_tags::all_primitives().iter().copied().collect();

    let mut violations = Vec::new();
    for entry in registry.entries() {
        if matches!(entry.kind, RenameKind::Primitive) && canonical.contains(entry.from.as_str()) {
            violations.push(format!(
                "`{}` is in the rename registry (kind: primitive) but still a canonical primitive",
                entry.from
            ));
        }
    }

    assert!(
        violations.is_empty(),
        "rename registry contains primitives that are still canonical:\n  - {}\n\
         Either remove the primitive from the canonical set, or remove its entry from \
         contracts/naming/renames.v1.json.",
        violations.join("\n  - ")
    );
}

#[test]
fn synthetic_violation_would_fail() {
    // Negative test: build a registry that violates the invariant in-memory and
    // confirm the same logic would flag it.
    let canonical: Vec<&'static str> = primitive_tags::all_primitives().to_vec();
    let pick = canonical
        .first()
        .expect("at least one canonical primitive must exist");

    let bad_json = format!(
        r#"{{
            "version": 1,
            "entries": [
              {{ "from": "{}", "to": "FAKE_REPLACEMENT", "kind": "primitive", "since": "0.5.0" }}
            ]
        }}"#,
        pick
    );
    let bad_registry = RenameRegistry::parse_json(&bad_json).expect("parse bad registry");

    let canonical_set: HashSet<&str> = canonical.iter().copied().collect();

    let mut violations = 0;
    for entry in bad_registry.entries() {
        if matches!(entry.kind, RenameKind::Primitive)
            && canonical_set.contains(entry.from.as_str())
        {
            violations += 1;
        }
    }
    assert_eq!(
        violations, 1,
        "synthetic violation must be detected by the same logic that protects the real registry"
    );
}
