//! Canonical plugin **target-triple keys** and **artifact-filename derivation** — the single
//! source of truth for both, so the host loader, the CI gates, and every `Plugin.toml`
//! agree by construction rather than by hand-copied literals.

/// Every platform key a `Plugin.toml` `[artifacts]` map (or `[code.artifacts]`) may use.
/// `<os>-<arch>`.
pub const PLUGIN_TARGET_TRIPLES: &[&str] = &[
    "windows-x86_64",
    "windows-aarch64",
    "linux-x86_64",
    "linux-aarch64",
    "macos-x86_64",
    "macos-aarch64",
];

/// The platform key for the crate currently being compiled, or `None` on a platform Vox
/// plugins don't target. The returned value (when `Some`) is always in
/// [`PLUGIN_TARGET_TRIPLES`].
pub fn current_target_triple() -> Option<&'static str> {
    if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        Some("windows-x86_64")
    } else if cfg!(all(target_os = "windows", target_arch = "aarch64")) {
        Some("windows-aarch64")
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Some("linux-x86_64")
    } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        Some("linux-aarch64")
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        Some("macos-x86_64")
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        Some("macos-aarch64")
    } else {
        None
    }
}

/// The cdylib filename a plugin crate produces for `triple`.
///
/// Rule: a `crate-type = ["cdylib"]` lib named `vox-plugin-foo` builds
/// `vox_plugin_foo.dll` (Windows), `libvox_plugin_foo.so` (Linux), or
/// `libvox_plugin_foo.dylib` (macOS). `crate_name` is the crate directory / package name
/// (e.g. `"vox-plugin-nvml-probe"`); hyphens become underscores. Returns `None` for a
/// triple outside [`PLUGIN_TARGET_TRIPLES`].
pub fn plugin_artifact_filename(crate_name: &str, triple: &str) -> Option<String> {
    let stem = crate_name.replace('-', "_");
    let (prefix, ext) = match triple {
        // vox-arch-check: allow dynlib-ext
        "windows-x86_64" | "windows-aarch64" => ("", ".dll"),
        // vox-arch-check: allow dynlib-ext
        "linux-x86_64" | "linux-aarch64" => ("lib", ".so"),
        // vox-arch-check: allow dynlib-ext
        "macos-x86_64" | "macos-aarch64" => ("lib", ".dylib"),
        _ => return None,
    };
    Some(format!("{prefix}{stem}{ext}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_triple_is_in_the_canonical_set() {
        // On any platform CI runs (win/linux/mac x64/arm64), the current triple is listed.
        let cur = current_target_triple().expect("a supported plugin target");
        assert!(PLUGIN_TARGET_TRIPLES.contains(&cur));
    }

    #[test]
    fn artifact_names_follow_the_cdylib_rule() {
        assert_eq!(
            plugin_artifact_filename("vox-plugin-nvml-probe", "windows-x86_64").as_deref(),
            // vox-arch-check: allow dynlib-ext
            Some("vox_plugin_nvml_probe.dll")
        );
        assert_eq!(
            plugin_artifact_filename("vox-plugin-nvml-probe", "linux-x86_64").as_deref(),
            // vox-arch-check: allow dynlib-ext
            Some("libvox_plugin_nvml_probe.so")
        );
        assert_eq!(
            plugin_artifact_filename("vox-plugin-speech", "macos-aarch64").as_deref(),
            // vox-arch-check: allow dynlib-ext
            Some("libvox_plugin_speech.dylib")
        );
        assert_eq!(
            plugin_artifact_filename("vox-plugin-x", "solaris-sparc"),
            None
        );
    }
}
