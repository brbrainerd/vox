//! Track 5: the catalog exposes optional companion-binary *components* (e.g. the
//! Tauri GUI) — distinct from cdylib plugins and from distribution bundles.

#[test]
fn catalog_exposes_gui_component() {
    let components = vox_plugin_catalog::all_components();
    let gui = components
        .iter()
        .find(|c| c.id == "gui")
        .expect("catalog should declare a 'gui' component");
    assert_eq!(gui.binary, "vox-gui");
    assert!(!gui.description.is_empty());
    // Desktop platform matrix should cover the three desktop OSes.
    assert!(gui.requires.os.iter().any(|o| o == "windows"));
    assert!(gui.requires.os.iter().any(|o| o == "macos"));
    assert!(gui.requires.os.iter().any(|o| o == "linux"));
}

#[test]
fn gui_component_is_not_a_plugin() {
    // Components are a separate concept from plugins: 'gui' must not be a plugin id.
    assert!(
        vox_plugin_catalog::all_plugins()
            .iter()
            .all(|p| p.id != "gui"),
        "'gui' is a component, not a cdylib plugin"
    );
}
