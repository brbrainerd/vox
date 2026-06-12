//! # Vox Plugin SDK
//!
//! Ergonomic authoring surface for **Vox code plugins**. It re-exports the stable
//! plugin ABI ([`vox_plugin_api`]) plus the [`abi_stable`] types plugin authors need,
//! and provides the [`declare_plugin!`] macro that emits the dylib **export glue**
//! (`root_module` / `manifest_json` / `init`) every plugin otherwise hand-copies.
//!
//! ## Why this exists / how it stays in sync
//!
//! The SDK adds **zero new ABI surface** — `declare_plugin!` expands to exactly the
//! `#[export_root_module]` + `#[sabi_extern_fn]` pattern plugins write by hand today, so a
//! macro-built plugin and a hand-built one produce **byte-identical exports**. Because the
//! macro stamps [`vox_plugin_api::VOX_PLUGIN_ABI_VERSION`] (not a hard-coded number) and
//! plugins implement the [`VoxPlugin`] trait from `vox-plugin-api` directly, new extension
//! points are picked up on recompile without touching the SDK. See
//! `docs/superpowers/plans/2026-06-06-plugin-sdk-and-abi-sync.md`.
//!
//! ## Minimal plugin
//!
//! ```ignore
//! use vox_plugin_sdk::prelude::*;
//!
//! #[derive(Clone)]
//! struct MyPlugin;
//!
//! impl VoxPlugin for MyPlugin {
//!     fn id(&self) -> RString { RString::from("my-plugin") }
//!     fn shutdown(&self) -> RResult<(), RBoxError> { ROk(()) }
//!     // ...optional `as_*` extension accessors...
//! }
//!
//! vox_plugin_sdk::declare_plugin! {
//!     // id + version are read from the crate's Plugin.toml — no duplication.
//!     init: |_host| ROk(wrap(MyPlugin)),
//! }
//! ```

// Re-export the stable API surface so a plugin's only Vox dependency is this SDK.
pub use vox_plugin_api::{self, VOX_PLUGIN_ABI_VERSION, abi, extensions, host};
// Re-export abi_stable so `declare_plugin!` is hermetic (callers need not depend on it
// directly, and the macro can reference `$crate::abi_stable::…`).
pub use abi_stable::{self, std_types};

use vox_plugin_api::abi::{VoxPlugin, VoxPlugin_TO, VoxPluginRef};

/// Wrap a concrete [`VoxPlugin`] value into the stable-ABI trait object the host loads.
///
/// Equivalent to the hand-written `VoxPlugin_TO::from_value(plugin, TD_Opaque)`.
pub fn wrap<P>(plugin: P) -> VoxPluginRef
where
    P: VoxPlugin + 'static,
{
    VoxPlugin_TO::from_value(plugin, abi_stable::erased_types::TD_Opaque)
}

/// Build the `manifest_json` ABI probe (`{"id":..,"version":..}`) from an embedded
/// `Plugin.toml` string. Used by [`declare_plugin!`] so id/version have a single authoring
/// site (the manifest) rather than being re-typed as macro arguments.
///
/// On a parse failure or a missing `[plugin]` id/version, falls back to empty/`0.0.0` — the
/// host reads the real `Plugin.toml` from disk anyway; this probe is advisory.
pub fn manifest_id_version_json(manifest_toml: &str) -> std_types::RString {
    let table = manifest_toml.parse::<toml::Value>().ok();
    let plugin = table.as_ref().and_then(|t| t.get("plugin"));
    let field = |k: &str, default: &str| -> String {
        plugin
            .and_then(|p| p.get(k))
            .and_then(|v| v.as_str())
            .unwrap_or(default)
            // Minimal JSON-string escaping (ids are kebab-case, versions semver — but be safe).
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
    };
    std_types::RString::from(format!(
        r#"{{"id":"{}","version":"{}"}}"#,
        field("id", ""),
        field("version", "0.0.0"),
    ))
}

/// Glob-import surface for plugin authors: `use vox_plugin_sdk::prelude::*;`.
pub mod prelude {
    pub use super::{declare_plugin, wrap};
    pub use vox_plugin_api::VOX_PLUGIN_ABI_VERSION;
    pub use vox_plugin_api::abi::{VoxPlugin, VoxPluginRef};
    pub use vox_plugin_api::host::VoxHost_TO;

    pub use abi_stable::std_types::ROption::{RNone, RSome};
    pub use abi_stable::std_types::RResult::{RErr, ROk};
    pub use abi_stable::std_types::{RBox, RBoxError, ROption, RResult, RSlice, RStr, RString};
}

/// Emit the dylib export glue for a Vox code plugin.
///
/// Generates the three functions the host loads — `root_module` (the
/// `#[export_root_module]` entry stamped with [`VOX_PLUGIN_ABI_VERSION`]),
/// `manifest_json` (a minimal `{"id":..,"version":..}` probe whose id/version are read from
/// the crate's `Plugin.toml` via [`manifest_id_version_json`] — one authoring site), and
/// `init` (which runs the supplied closure to produce a [`VoxPluginRef`]).
///
/// `init` is a non-capturing closure `|host| -> RResult<VoxPluginRef, RBoxError>`. Use
/// [`wrap`] for the common "construct a value and erase it" case, or call an existing
/// constructor (e.g. one that needs `host`).
///
/// **Dependency requirement:** the consuming crate must declare `abi_stable` as a direct
/// dependency. This macro is hermetic with respect to the *Vox* surface, but abi_stable's
/// own `#[export_root_module]` / `#[sabi_extern_fn]` attribute macros expand to bare
/// `abi_stable::` paths, which only resolve when `abi_stable` is in the crate's deps.
#[macro_export]
macro_rules! declare_plugin {
    (init: $init:expr $(,)?) => {
        #[$crate::abi_stable::export_root_module]
        fn __vox_plugin_root_module() -> $crate::abi::VoxPluginRootRef {
            use $crate::abi_stable::prefix_type::PrefixTypeTrait;
            $crate::abi::VoxPluginRoot {
                abi_version: $crate::VOX_PLUGIN_ABI_VERSION,
                manifest_json: __vox_plugin_manifest_json,
                init: __vox_plugin_init,
            }
            .leak_into_prefix()
        }

        #[$crate::abi_stable::sabi_extern_fn]
        fn __vox_plugin_manifest_json() -> $crate::std_types::RString {
            // id/version come from the crate's Plugin.toml — the single authoring site —
            // embedded at compile time. `CARGO_MANIFEST_DIR` is the *plugin* crate's dir.
            $crate::manifest_id_version_json(::core::include_str!(::core::concat!(
                ::core::env!("CARGO_MANIFEST_DIR"),
                "/Plugin.toml"
            )))
        }

        #[$crate::abi_stable::sabi_extern_fn]
        fn __vox_plugin_init(
            host: $crate::host::VoxHost_TO<'static, $crate::std_types::RBox<()>>,
        ) -> $crate::std_types::RResult<$crate::abi::VoxPluginRef, $crate::std_types::RBoxError> {
            // Coerce the supplied closure to a plain fn pointer (no captures allowed),
            // then run it. Keeps the macro hygienic and forbids accidental state capture.
            let init_fn: fn(
                $crate::host::VoxHost_TO<'static, $crate::std_types::RBox<()>>,
            ) -> $crate::std_types::RResult<
                $crate::abi::VoxPluginRef,
                $crate::std_types::RBoxError,
            > = $init;
            init_fn(host)
        }
    };
}

#[cfg(test)]
mod tests {
    use super::std_types::{RBoxError, RResult, RString};
    use super::wrap;
    use vox_plugin_api::abi::VoxPlugin;

    #[derive(Clone)]
    struct TestPlugin;

    impl VoxPlugin for TestPlugin {
        fn id(&self) -> RString {
            RString::from("test-plugin")
        }
        fn shutdown(&self) -> RResult<(), RBoxError> {
            RResult::ROk(())
        }
    }

    #[test]
    fn wrap_roundtrips_through_the_stable_trait_object() {
        // `wrap` erases a concrete VoxPlugin into the host-facing trait object;
        // the erased object must still answer `id()` correctly.
        let erased = wrap(TestPlugin);
        assert_eq!(erased.id().as_str(), "test-plugin");
        assert!(erased.shutdown().is_ok());
    }

    #[test]
    fn manifest_json_reads_id_and_version_from_plugin_toml() {
        let manifest = "[plugin]\nid = \"demo\"\nversion = \"1.2.3\"\n\
                        \n[plugin.payload]\nkind = \"code\"\nabi-version = 12\n";
        assert_eq!(
            super::manifest_id_version_json(manifest).as_str(),
            r#"{"id":"demo","version":"1.2.3"}"#
        );
    }

    #[test]
    fn manifest_json_falls_back_when_fields_missing() {
        assert_eq!(
            super::manifest_id_version_json("not = valid = toml").as_str(),
            r#"{"id":"","version":"0.0.0"}"#
        );
    }
}
