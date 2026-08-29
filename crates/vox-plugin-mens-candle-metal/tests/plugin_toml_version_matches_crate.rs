//! Plugin.toml is hand-maintained, not Cargo-generated, so nothing else
//! enforces it staying in sync with the crate's own `version.workspace =
//! true`. This is the drift gate: a workspace version bump that forgets to
//! touch Plugin.toml fails here, loudly, instead of shipping a plugin whose
//! declared version disagrees with the release it ships in.

#[test]
fn plugin_toml_version_matches_crate_version() {
    let manifest = include_str!("../Plugin.toml");
    let parsed: toml::Value = manifest.parse().expect("Plugin.toml must be valid TOML");
    let declared = parsed["plugin"]["version"]
        .as_str()
        .expect("Plugin.toml must have [plugin] version as a string");
    assert_eq!(
        declared,
        env!("CARGO_PKG_VERSION"),
        "Plugin.toml's version ({declared}) does not match this crate's \
         Cargo.toml version ({}). Update Plugin.toml's [plugin] version to \
         match -- it is not derived automatically.",
        env!("CARGO_PKG_VERSION")
    );
}
