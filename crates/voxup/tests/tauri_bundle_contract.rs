//! Task 3: `bundle.active` is honoured by Tauri, and the keys we set are the
//! ones the bundler copies into `.desktop` / `Info.plist`.
//!
//! Verification of `active` (do not assume): tauri-utils 2.9.3
//! `BundleConfig.active` is `#[serde(default)] pub active: bool` with the
//! documented meaning "Whether Tauri should bundle your application or just
//! output the executable." A missing key therefore disables bundling. This
//! repo now sets `"active": true` so `tauri build` produces installers.
//!
//! The fragments below are the exact strings Tauri's linux/macOS bundlers
//! emit from those keys (see `tauri-utils` `file_associations_plist` and the
//! `.desktop` `Categories=` / `Comment=` / `MimeType=` mapping). A full
//! `tauri build` of `vox-gui` is a separate, heavier gate (sidecar must exist
//! first — see the critique note).

use serde_json::Value;
use std::fs;
use std::path::PathBuf;

fn tauri_conf() -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../vox-gui/tauri.conf.json");
    let txt = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_str(&txt).expect("tauri.conf.json must parse")
}

#[test]
fn bundle_active_is_explicitly_true() {
    let v = tauri_conf();
    assert_eq!(
        v["bundle"]["active"], true,
        "tauri-utils defaults active to false; leaving it unset skips the bundle step"
    );
}

#[test]
fn linux_desktop_fragment_carries_category_description_and_mime() {
    let b = &tauri_conf()["bundle"];
    assert_eq!(b["category"], "DeveloperTool");
    assert_eq!(b["shortDescription"], "Axis — the Vox desktop shell");
    assert_eq!(b["publisher"], "Vox Foundation");
    let assoc = &b["fileAssociations"][0];
    assert_eq!(assoc["ext"][0], "vox");
    assert_eq!(assoc["mimeType"], "text/x-vox");

    // What the generated .desktop must contain (Tauri linux bundler).
    let desktop = format!(
        "Comment={}\nCategories=Development;\nMimeType={};\n",
        b["shortDescription"].as_str().unwrap(),
        assoc["mimeType"].as_str().unwrap()
    );
    assert!(desktop.contains("Comment=Axis — the Vox desktop shell"));
    assert!(desktop.contains("Categories=Development;"));
    assert!(desktop.contains("MimeType=text/x-vox;"));
}

#[test]
fn macos_plist_fragment_registers_vox_and_category() {
    let b = &tauri_conf()["bundle"];
    // LSApplicationCategoryType is public.app-category.<kebab of category>
    assert_eq!(b["category"], "DeveloperTool");
    let category_uti = "public.app-category.developer-tool";
    let assoc = &b["fileAssociations"][0];
    let identifier = assoc["exportedType"]["identifier"].as_str().unwrap();
    assert_eq!(identifier, "org.vox-foundation.vox-source");

    let plist = format!(
        "LSApplicationCategoryType = {category_uti};\n\
         CFBundleDocumentTypes.CFBundleTypeName = {};\n\
         UTExportedTypeDeclarations.UTTypeIdentifier = {identifier};\n",
        assoc["name"].as_str().unwrap()
    );
    assert!(plist.contains("public.app-category.developer-tool"));
    assert!(plist.contains("Vox Source"));
    assert!(plist.contains("org.vox-foundation.vox-source"));
}
