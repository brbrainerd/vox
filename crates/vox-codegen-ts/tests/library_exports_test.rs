/// Task 8H (reverse exports): component names must appear in the library
/// `package.json` `exports` map so consumers can import them by path.
use vox_codegen_ts::library_package_emit::{LibraryPackageConfig, emit_library_package_json};

#[test]
fn component_names_appear_in_exports() {
    let cfg = LibraryPackageConfig {
        has_vox_client: false,
        has_types: false,
        has_schemas: false,
        has_openapi: false,
        has_schema_ts: false,
        component_names: vec!["Button".to_string(), "Dialog".to_string()],
    };
    let pkg = emit_library_package_json(cfg);
    assert!(
        pkg.contains("./components/Button"),
        "Button export entry missing\ngot: {pkg}"
    );
    assert!(
        pkg.contains("./components/Dialog"),
        "Dialog export entry missing\ngot: {pkg}"
    );
    assert!(
        pkg.contains("Button.tsx"),
        "Button.tsx path missing\ngot: {pkg}"
    );
}

#[test]
fn no_components_no_export_entry() {
    let cfg = LibraryPackageConfig {
        has_vox_client: true,
        has_types: false,
        has_schemas: false,
        has_openapi: false,
        has_schema_ts: false,
        component_names: vec![],
    };
    let pkg = emit_library_package_json(cfg);
    assert!(
        !pkg.contains("./components/"),
        "unexpected components/ entry when no components\ngot: {pkg}"
    );
}

#[test]
fn any_export_true_when_has_components() {
    let cfg = LibraryPackageConfig {
        has_vox_client: false,
        has_types: false,
        has_schemas: false,
        has_openapi: false,
        has_schema_ts: false,
        component_names: vec!["Card".to_string()],
    };
    assert!(cfg.any_export(), "any_export must be true when component_names is non-empty");
}
