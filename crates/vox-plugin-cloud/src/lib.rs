//! Vox plugin: cloud
//!
//! Stub plugin that provides the CloudSync extension point for syncing model
//! artifacts with cloud providers (HF Hub, S3, etc.). SP7 scaffold — actual
//! extraction is deferred. See src/sync.rs for TODO(SP7-followup) markers.

mod sync;

use abi_stable::{export_root_module, prefix_type::PrefixTypeTrait, sabi_extern_fn, std_types::*};
use vox_plugin_api::VOX_PLUGIN_ABI_VERSION;
use vox_plugin_api::abi::{VoxPluginRef, VoxPluginRoot, VoxPluginRootRef};
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
    RString::from(r#"{"id":"cloud","version":"0.1.0"}"#)
}

#[sabi_extern_fn]
fn init(host: VoxHost_TO<'static, RBox<()>>) -> RResult<VoxPluginRef, RBoxError> {
    sync::make_plugin(host)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_advertises_cloud_id() {
        let json = manifest_json();
        assert!(json.as_str().contains("\"cloud\""));
    }
}
