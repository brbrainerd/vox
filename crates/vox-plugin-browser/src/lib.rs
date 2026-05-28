//! vox-plugin-browser — chromiumoxide CDP browser automation plugin.
//!
//! Exposes the full `vox-browser` `BrowserEngine` API surface behind the
//! `BrowserAutomation` sabi_trait extension point. Code ported from
//! `crates/vox-browser/src/engine.rs`; the original crate remains in-tree
//! for transitional consumers that haven't yet migrated to the plugin host.

mod browser;
mod engine;

use abi_stable::{
    erased_types::TD_Opaque, export_root_module, prefix_type::PrefixTypeTrait, sabi_extern_fn,
    std_types::*,
};
use vox_plugin_api::VOX_PLUGIN_ABI_VERSION;
use vox_plugin_api::abi::{VoxPlugin, VoxPlugin_TO, VoxPluginRef, VoxPluginRoot, VoxPluginRootRef};
use vox_plugin_api::extensions::browser_automation::BrowserAutomation_TO;
use vox_plugin_api::host::VoxHost_TO;

#[export_root_module]
fn root_module() -> VoxPluginRootRef {
    VoxPluginRoot {
        abi_version: VOX_PLUGIN_ABI_VERSION,
        manifest_json,
        init,
    }
    .leak_into_prefix()
}

#[sabi_extern_fn]
fn manifest_json() -> RString {
    RString::from(r#"{"id":"browser","version":"0.1.0"}"#)
}

#[sabi_extern_fn]
fn init(_host: VoxHost_TO<'static, RBox<()>>) -> RResult<VoxPluginRef, RBoxError> {
    let plugin = BrowserPluginHost;
    let to = VoxPlugin_TO::from_value(plugin, TD_Opaque);
    RResult::ROk(to)
}

#[derive(Clone)]
struct BrowserPluginHost;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_advertises_browser_id() {
        let json = manifest_json();
        assert!(json.as_str().contains("\"browser\""));
        let plugin = BrowserPluginHost;
        assert_eq!(plugin.id().as_str(), "browser");
    }
}

impl VoxPlugin for BrowserPluginHost {
    fn id(&self) -> RString {
        RString::from("browser")
    }

    fn shutdown(&self) -> RResult<(), RBoxError> {
        RResult::ROk(())
    }

    fn as_browser_automation(&self) -> ROption<BrowserAutomation_TO<'static, RBox<()>>> {
        let plugin = browser::BrowserPlugin::new();
        ROption::RSome(BrowserAutomation_TO::from_value(plugin, TD_Opaque))
    }
}
