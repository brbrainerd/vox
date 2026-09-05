#![cfg_attr(test, allow(unsafe_code))] // test-only std::env::set_var (unsafe on edition 2024)
//! Vox plugin host: discovery, loading, registry.
//!
//! See: docs/src/architecture/plugin-system-redesign-2026.md

#![allow(clippy::result_large_err)]

pub mod capability;
pub mod discover;
pub mod errors;
pub mod external_skills;
pub mod host_impl;
pub mod loader;
pub mod registry;
pub mod skill_author;
pub mod skill_bundle;
pub mod skill_manifest;
pub mod skill_parser;
pub mod skill_registry;
pub mod telemetry;
pub mod user_install;

pub use capability::{CapabilitySet, probe};
pub use discover::discover;
pub use errors::{AbiMismatchError, LoadError, PluginMissingError, SkillNotInstalledError};
pub use host_impl::DefaultVoxHost;
pub use loader::{LoadedCodePlugin, Loader};
pub use registry::{PluginEntry, Registry};
pub use skill_author::author_skill_md;
pub use skill_bundle::{SkillBundle, SkillBundleError, VoxSkillBundle};
pub use skill_manifest::{SkillCategory, SkillManifest, SkillPermission};
pub use skill_parser::{ParseSkillError, parse_skill_md};
pub use skill_registry::{
    BundleInstallError, HydrateError, InstallResult, RegisteredSkill, SkillRegistry, SkillSource,
    UninstallError, UninstallResult, new_registry_arc,
};
pub use user_install::{InstalledUserSkill, install_to_user_root};
pub use vox_plugin_api::VOX_PLUGIN_ABI_VERSION;

/// Resolve the plugin install root, respecting `$VOX_PLUGINS_DIR` if set.
/// Falls back to the platform's local data directory under `vox/plugins`.
pub fn resolve_plugins_root() -> std::path::PathBuf {
    if let Ok(p) = std::env::var("VOX_PLUGINS_DIR") {
        return std::path::PathBuf::from(p);
    }
    dirs::data_local_dir()
        .map(|p| p.join("vox").join("plugins"))
        .unwrap_or_else(|| std::path::PathBuf::from("./vox-plugins"))
}

/// Format the canonical multi-line install hint for a missing plugin.
///
/// Always includes the catalog command (`vox plugin install <id>`). When the
/// caller is running from a Vox workspace checkout that contains the plugin
/// source under `crates/vox-plugin-<id>/`, also appends the local-path
/// variant so contributors get a copy-pasteable command pointing at their
/// own checkout (no GitHub fetch needed).
///
/// Optionally appends a "Build with cargo:" line when the caller knows
/// the plugin is enabled behind a cargo feature flag on a vox crate (e.g.
/// vox-ml-cli's `gpu` / `mens-candle-cuda`). Pass `Some(cargo_hint)` with
/// the command body — `cargo build -p vox-ml-cli --release --features ...`.
#[must_use]
pub fn format_install_hint(plugin_id: &str, cargo_feature_hint: Option<&str>) -> String {
    let mut out = format!("  vox plugin install {plugin_id}");
    if let Some(local) = workspace_local_plugin_source(plugin_id) {
        out.push_str(&format!(
            "\n\nor, from this workspace checkout (faster, no GitHub fetch):\n\n  vox plugin install --path {} --yes",
            local.display()
        ));
    }
    if let Some(cargo_hint) = cargo_feature_hint {
        out.push_str(&format!(
            "\n\nif the missing capability is a cargo-feature gate (not a runtime plugin), rebuild with:\n\n  {cargo_hint}"
        ));
    }
    out.push_str("\n\nSee: docs/src/reference/plugins.md");
    out
}

/// Detect the in-tree source directory for a plugin id when running from a
/// Vox workspace checkout. Used by `load_code_plugin` to make the "plugin
/// not installed" error message actionable for contributors and by
/// `vox plugin install <id>` (catalog path) to prefer the local checkout
/// over fetching a release tarball.
///
/// Walks up from CWD looking for a `crates/vox-plugin-<id>/Plugin.toml`.
/// Returns `Some(path-to-crate-dir)` on the first hit, `None` otherwise.
/// Honors `VOX_WORKSPACE_ROOT` as an explicit override.
#[must_use]
pub fn workspace_local_plugin_source(plugin_id: &str) -> Option<std::path::PathBuf> {
    let candidates_root = if let Ok(root) = std::env::var("VOX_WORKSPACE_ROOT") {
        vec![std::path::PathBuf::from(root)]
    } else if let Ok(cwd) = std::env::current_dir() {
        // Walk up at most 8 levels — covers both repo-root invocations and
        // common nested layouts (e.g. .claude/worktrees/<name>/...).
        let mut hops = Vec::new();
        let mut cur: &std::path::Path = &cwd;
        for _ in 0..8 {
            hops.push(cur.to_path_buf());
            match cur.parent() {
                Some(p) => cur = p,
                None => break,
            }
        }
        hops
    } else {
        return None;
    };
    let crate_dir_name = format!("vox-plugin-{plugin_id}");
    for root in candidates_root {
        let candidate = root.join("crates").join(&crate_dir_name);
        if candidate.join("Plugin.toml").is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Return the target-triple key used in `[plugin.payload.artifacts]` for the current build.
///
/// The format is `"<os>-<arch>"` where `os` is `"windows"`, `"linux"`, or `"macos"` and
/// `arch` is `"x86_64"` or `"aarch64"`.  This matches the keys emitted by the Plugin.toml
/// generator and by `vox plugin install`.
pub fn current_target_triple_key() -> &'static str {
    // Canonical detection lives in vox-plugin-types (the SSOT shared with the CI gates).
    vox_plugin_types::current_target_triple().unwrap_or("unknown")
}

/// Convenience wrapper: discover the plugin install root, build the registry, and load a
/// code plugin by id in a single call.
///
/// For one-off dispatches from async contexts, wrap in `tokio::task::spawn_blocking`.
pub fn load_code_plugin_by_id(plugin_id: &str) -> Result<LoadedCodePlugin, errors::LoadError> {
    let install_root = resolve_plugins_root();
    let registry = discover(&install_root)?;
    load_code_plugin(&registry, plugin_id)
}

/// Discover the given plugin in `registry`, resolve the dylib path for the current target
/// triple, and load it via [`Loader`].
///
/// This is the preferred one-shot entry point for code-payload plugins.  Callers can then
/// call `.plugin.as_ml_backend()` (or the relevant extension point accessor) on the
/// returned [`LoadedCodePlugin`].
pub fn load_code_plugin(
    registry: &Registry,
    plugin_id: &str,
) -> Result<LoadedCodePlugin, errors::LoadError> {
    use vox_plugin_api::manifest::PluginPayload;

    let entry = registry.get_full_entry(plugin_id).ok_or_else(|| {
        errors::LoadError::InitFailed(format!(
            "plugin '{plugin_id}' is not installed.\n\nTo install it, run:\n\n{}",
            format_install_hint(plugin_id, None)
        ))
    })?;

    // Spec §4.2(d): refuse to load a plugin built for a different core version
    // before touching its dylib. The artifact name already encodes the version
    // (`{id}-v{version}-{triple}.zip`), so this is a plain string comparison
    // against the manifest's own `version` field, not new metadata. Matches
    // the `env!("CARGO_PKG_VERSION")` precedent in
    // `install_first_party_plugin` (crates/vox-cli/src/commands/plugin/install.rs).
    let host_version = env!("CARGO_PKG_VERSION");
    if entry.version != host_version {
        return Err(errors::LoadError::VersionMismatch {
            plugin_id: plugin_id.to_string(),
            expected: host_version.to_string(),
            found: entry.version.clone(),
        });
    }

    let triple = current_target_triple_key();
    let artifacts = match &entry.payload {
        PluginPayload::Code(c) => &c.artifacts,
        PluginPayload::Composite(c) => &c.code.artifacts,
        PluginPayload::Skill(_) => {
            return Err(errors::LoadError::InitFailed(format!(
                "plugin '{plugin_id}' is a skill-only plugin and cannot be loaded as a code plugin"
            )));
        }
    };

    let filename = artifacts.get(triple).ok_or_else(|| {
        errors::LoadError::InitFailed(format!(
            "plugin '{plugin_id}' has no artifact for target triple '{triple}' \
             (available: {:?})",
            artifacts.keys().collect::<Vec<_>>()
        ))
    })?;

    let dylib_path = entry.install_dir.join(filename);
    Loader::load(&entry.id, &entry.version, &dylib_path)
}

/// A first-party candidate plugin known to implement a given extension point,
/// paired with the host capability tag (mirroring `catalog.toml`'s
/// `requires-tag`) it needs to be selectable.
///
/// `vox-plugin-host` is deliberately dependency-free (see the note on
/// `user_install.rs` in `layers.toml`), so it does not read `catalog.toml`
/// itself — callers pass in the small, stable candidate list for the
/// extension point they care about (see [`resolve_extension_point`]).
#[derive(Debug, Clone, Copy)]
pub struct ExtensionCandidate {
    pub plugin_id: &'static str,
    pub requires_tag: Option<&'static str>,
}

/// Pick which of `candidates` should service `extension_point` on this host.
///
/// Returns the first candidate whose `requires_tag` is satisfied by `caps`
/// (typically [`probe`]). This is pure selection with no I/O — callers still
/// load the winner via [`cached_code_plugin`].
///
/// On no match, the error names every candidate considered (and the tag it
/// needed) plus the host's actual capabilities, so the failure is
/// diagnosable rather than a bare "not found".
pub fn resolve_extension_point(
    extension_point: &str,
    candidates: &[ExtensionCandidate],
    caps: &CapabilitySet,
) -> Result<&'static str, errors::LoadError> {
    candidates
        .iter()
        .find(|c| caps.satisfies(c.requires_tag))
        .map(|c| c.plugin_id)
        .ok_or_else(|| {
            let wanted: Vec<String> = candidates
                .iter()
                .map(|c| {
                    format!(
                        "{} (requires-tag: {})",
                        c.plugin_id,
                        c.requires_tag.unwrap_or("<none>")
                    )
                })
                .collect();
            errors::LoadError::InitFailed(format!(
                "no candidate plugin for extension point '{extension_point}' matches this host.\n\
                 candidates considered:\n  {}\n\
                 host capabilities: {caps:?}",
                wanted.join("\n  ")
            ))
        })
}

/// Cached singleton: load a code plugin once and reuse the handle process-wide.
/// First call: discover + dlopen (tens of ms). Subsequent calls: O(1) HashMap lookup.
///
/// The plugin is leaked for the process lifetime (`Box::leak`) — code plugins are
/// designed to never unload while the host is running. Designed for plugins called
/// repeatedly (browser, mesh, ml backends).
pub fn cached_code_plugin(
    plugin_id: &'static str,
) -> Result<&'static LoadedCodePlugin, errors::LoadError> {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    type Cache = Mutex<HashMap<&'static str, &'static LoadedCodePlugin>>;
    static CACHE: OnceLock<Cache> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = cache.lock().unwrap();
    if let Some(p) = guard.get(plugin_id) {
        return Ok(p);
    }
    let loaded = load_code_plugin_by_id(plugin_id)?;
    let leaked: &'static LoadedCodePlugin = Box::leak(Box::new(loaded));
    guard.insert(plugin_id, leaked);
    Ok(leaked)
}

#[cfg(test)]
mod load_code_plugin_version_gate_tests {
    //! Spec §4.2(d): a plugin whose manifest `version` disagrees with the
    //! running core's own `CARGO_PKG_VERSION` must be refused before any
    //! attempt to resolve or dlopen its dylib.
    use super::*;
    use registry::{PluginEntry, Registry};
    use vox_plugin_api::manifest::{CodePayload, PluginPayload};

    fn code_entry(id: &str, version: &str) -> PluginEntry {
        PluginEntry {
            id: id.to_string(),
            version: version.to_string(),
            // Deliberately nonexistent: a version mismatch must be caught
            // before this path is ever touched, so it need not resolve.
            install_dir: std::path::PathBuf::from("/nonexistent/does-not-matter"),
            payload: PluginPayload::Code(CodePayload {
                abi_version: VOX_PLUGIN_ABI_VERSION,
                provides: Default::default(),
                requires: Default::default(),
                artifacts: Default::default(),
            }),
        }
    }

    #[test]
    fn mismatched_version_is_refused_before_dlopen() {
        let registry = Registry::new();
        let host_version = env!("CARGO_PKG_VERSION");
        let bogus_version = format!("{host_version}-definitely-not-installed");
        registry.record(code_entry("versioned-plugin", &bogus_version));

        let err = match load_code_plugin(&registry, "versioned-plugin") {
            Ok(_) => panic!("expected VersionMismatch, got Ok"),
            Err(e) => e,
        };
        match err {
            LoadError::VersionMismatch {
                plugin_id,
                expected,
                found,
            } => {
                assert_eq!(plugin_id, "versioned-plugin");
                assert_eq!(expected, host_version);
                assert_eq!(found, bogus_version);
            }
            other => panic!("expected VersionMismatch, got: {other:?}"),
        }
    }

    #[test]
    fn matching_version_proceeds_past_the_version_gate() {
        // A matching version must not be rejected as a VersionMismatch. It will
        // still fail past the gate (no real artifact for this triple exists),
        // which proves the gate let it through rather than swallowing the error.
        let registry = Registry::new();
        let host_version = env!("CARGO_PKG_VERSION");
        registry.record(code_entry("versioned-plugin-ok", host_version));

        let err = match load_code_plugin(&registry, "versioned-plugin-ok") {
            Ok(_) => panic!("expected an error past the version gate (no real artifact exists)"),
            Err(e) => e,
        };
        assert!(
            !matches!(err, LoadError::VersionMismatch { .. }),
            "matching version must not be rejected as a mismatch, got: {err:?}"
        );
    }
}

#[cfg(test)]
mod resolve_extension_point_tests {
    use super::*;

    // Fake catalog: mirrors catalog.toml's mens-candle-cuda/mens-candle-metal
    // `MlBackend` entries (id + requires-tag) without depending on the
    // vox-plugin-catalog crate.
    const ML_BACKEND_CANDIDATES: &[ExtensionCandidate] = &[
        ExtensionCandidate {
            plugin_id: "mens-candle-cuda",
            requires_tag: Some("nvidia-gpu"),
        },
        ExtensionCandidate {
            plugin_id: "mens-candle-metal",
            requires_tag: Some("apple-silicon"),
        },
    ];

    #[test]
    fn picks_metal_when_only_apple_silicon_present() {
        let caps = CapabilitySet::from_tags(["cpu-only", "apple-silicon", "metal"]);
        let id = resolve_extension_point("MlBackend", ML_BACKEND_CANDIDATES, &caps).unwrap();
        assert_eq!(id, "mens-candle-metal");
    }

    #[test]
    fn picks_cuda_when_only_nvidia_gpu_present() {
        let caps = CapabilitySet::from_tags(["cpu-only", "nvidia-gpu"]);
        let id = resolve_extension_point("MlBackend", ML_BACKEND_CANDIDATES, &caps).unwrap();
        assert_eq!(id, "mens-candle-cuda");
    }

    #[test]
    fn errors_diagnosably_when_neither_tag_present() {
        let caps = CapabilitySet::from_tags(["cpu-only"]);
        let err = resolve_extension_point("MlBackend", ML_BACKEND_CANDIDATES, &caps).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("MlBackend"), "msg: {msg}");
        assert!(msg.contains("nvidia-gpu"), "msg: {msg}");
        assert!(msg.contains("apple-silicon"), "msg: {msg}");
        assert!(msg.contains("mens-candle-cuda"), "msg: {msg}");
        assert!(msg.contains("mens-candle-metal"), "msg: {msg}");
    }
}

#[cfg(test)]
mod semcov_wave3_tests {
    // Rust 2024 made std::env::{set_var,remove_var} unsafe; the env-mutating
    // tests below serialize on ENV_MUTEX so the parallel harness can't interleave
    // their process-wide VOX_PLUGINS_DIR writes.
    #![allow(unused_imports, unsafe_code)]
    use super::*;
    use std::sync::{Mutex, OnceLock};

    static ENV_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();
    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        ENV_MUTEX
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    // ── resolve_plugins_root ──────────────────────────────────────────────────

    #[test]
    fn resolve_plugins_root_env_override() {
        let _guard = env_lock();
        // vox-arch-check: allow abs-path
        unsafe { std::env::set_var("VOX_PLUGINS_DIR", "/tmp/my-plugins") };
        let result = resolve_plugins_root();
        unsafe { std::env::remove_var("VOX_PLUGINS_DIR") };
        // vox-arch-check: allow abs-path
        assert_eq!(result, std::path::PathBuf::from("/tmp/my-plugins"));
    }

    #[test]
    fn resolve_plugins_root_fallback_is_non_empty() {
        let _guard = env_lock();
        // Ensure env var is cleared so we exercise the fallback branch.
        unsafe { std::env::remove_var("VOX_PLUGINS_DIR") };
        let result = resolve_plugins_root();
        // The returned path must be absolute (data_local_dir) or the
        // hardcoded fallback "./vox-plugins".  Either way it must contain
        // "plugins" to confirm we got the right sub-path.
        let s = result.to_string_lossy().to_lowercase();
        assert!(
            s.contains("plugins"),
            "expected 'plugins' in path, got: {s}"
        );
    }

    // ── format_install_hint ───────────────────────────────────────────────────

    #[test]
    fn format_install_hint_basic_contains_plugin_id() {
        let hint = format_install_hint("browser", None);
        assert!(hint.contains("vox plugin install browser"), "hint: {hint}");
        assert!(
            hint.contains("docs/src/reference/plugins.md"),
            "hint: {hint}"
        );
    }

    #[test]
    fn format_install_hint_cargo_feature_appended() {
        let hint = format_install_hint(
            "ml",
            Some("cargo build -p vox-ml-cli --release --features gpu"),
        );
        assert!(hint.contains("cargo build -p vox-ml-cli"), "hint: {hint}");
        assert!(hint.contains("cargo-feature gate"), "hint: {hint}");
    }

    #[test]
    fn format_install_hint_no_cargo_when_none() {
        let hint = format_install_hint("browser", None);
        // When no cargo_hint is provided, the "cargo-feature gate" block must be absent.
        assert!(
            !hint.contains("cargo-feature gate"),
            "unexpected cargo section in hint: {hint}"
        );
    }

    // ── workspace_local_plugin_source ─────────────────────────────────────────

    #[test]
    fn workspace_local_plugin_source_env_override_missing_dir_returns_none() {
        let _guard = env_lock();
        // Point VOX_WORKSPACE_ROOT at a directory that has no crates/ sub-tree.
        let tmp = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("VOX_WORKSPACE_ROOT", tmp.path().to_str().unwrap()) };
        let result = workspace_local_plugin_source("nonexistent-plugin");
        unsafe { std::env::remove_var("VOX_WORKSPACE_ROOT") };
        assert!(
            result.is_none(),
            "expected None for missing plugin dir, got: {result:?}"
        );
    }

    #[test]
    fn workspace_local_plugin_source_env_override_hit() {
        let _guard = env_lock();
        let tmp = tempfile::tempdir().unwrap();
        // Create a fake crates/vox-plugin-myplugin/Plugin.toml
        let crate_dir = tmp.path().join("crates").join("vox-plugin-myplugin");
        std::fs::create_dir_all(&crate_dir).unwrap();
        std::fs::write(
            crate_dir.join("Plugin.toml"),
            "[plugin]\nid = \"myplugin\"\n",
        )
        .unwrap();
        unsafe { std::env::set_var("VOX_WORKSPACE_ROOT", tmp.path().to_str().unwrap()) };
        let result = workspace_local_plugin_source("myplugin");
        unsafe { std::env::remove_var("VOX_WORKSPACE_ROOT") };
        assert_eq!(result, Some(crate_dir));
    }
}
