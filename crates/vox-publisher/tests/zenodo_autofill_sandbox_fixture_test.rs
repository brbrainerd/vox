//! Zenodo archive autofill round-trip against the sandbox fixture manifest.
//!
//! Exercises completion-report `required_missing` → autofill → provenance →
//! Zenodo deposition body (no live network).

use vox_publisher::publication::PublicationManifest;
use vox_publisher::scientia_autofill::{apply_autofill, compute_autofill};
use vox_publisher::scientia_discovery::manifest_completion_report;
use vox_publisher::zenodo_metadata::zenodo_deposition_create_body;

fn fixture_manifest() -> PublicationManifest {
    let raw = include_str!("fixtures/zenodo_sandbox_manifest.v1.json");
    serde_json::from_str(raw).expect("sandbox fixture parses")
}

#[test]
fn sandbox_fixture_autofill_fills_zenodo_required_fields_with_provenance() {
    let mut manifest = fixture_manifest();
    let before = manifest_completion_report(&manifest);
    for field in [
        "publication_date",
        "keywords",
        "version",
        "related_identifiers",
    ] {
        assert!(
            before.required_missing.contains(&field.to_string()),
            "fixture should start missing {field}: {:?}",
            before.required_missing
        );
    }

    let plan = compute_autofill(
        &manifest,
        None,
        Some("MIT"),
        Some("https://github.com/vox-foundation/vox"),
        Some("0.6.0"),
    );
    assert!(
        plan.fills.iter().any(|f| f.field == "publication_date"),
        "plan must fill publication_date: {:?}",
        plan.fills
    );
    assert!(
        plan.fills.iter().all(|f| f.origin.starts_with("autofill:")),
        "every fill must carry autofill provenance"
    );

    let new_meta = apply_autofill(
        manifest.metadata_json.as_deref(),
        &mut manifest.abstract_text,
        &plan,
    )
    .expect("apply autofill");
    manifest.metadata_json = Some(new_meta);

    let after = manifest_completion_report(&manifest);
    assert!(
        !after
            .required_missing
            .contains(&"publication_date".to_string()),
        "after autofill: {:?}",
        after.required_missing
    );
    assert!(
        !after.required_missing.contains(&"keywords".to_string()),
        "after autofill: {:?}",
        after.required_missing
    );
    assert!(
        !after.required_missing.contains(&"version".to_string()),
        "after autofill: {:?}",
        after.required_missing
    );
    assert!(
        !after
            .required_missing
            .contains(&"related_identifiers".to_string()),
        "after autofill: {:?}",
        after.required_missing
    );
    assert!(
        after
            .field_provenance
            .iter()
            .any(|p| p.field == "related_identifiers"),
        "related_identifiers provenance: {:?}",
        after.field_provenance
    );

    let body = zenodo_deposition_create_body(&manifest);
    assert!(
        body.metadata
            .publication_date
            .as_deref()
            .is_some_and(|d| d.len() == 10),
        "Zenodo body must carry publication_date"
    );
    assert!(!body.metadata.keywords.is_empty());
    assert_eq!(body.metadata.version.as_deref(), Some("0.6.0"));
    assert!(
        body.metadata
            .related_identifiers
            .iter()
            .any(|r| r.identifier.contains("github.com")),
        "Zenodo body must include code repo related identifier"
    );
}
