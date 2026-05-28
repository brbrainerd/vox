//! CloudSync implementation for HF Hub / S3 model artifact sync.
//!
//! SP7 scaffold: all non-trivial methods return "not yet implemented".
//! Actual extraction from cloud integration code is deferred to a follow-up SP.
//! TODO(SP7-followup): wire HF Hub and S3 clients for upload/download/list.

use abi_stable::{erased_types::TD_Opaque, std_types::*};
use vox_plugin_api::abi::{VoxPlugin, VoxPlugin_TO, VoxPluginRef};
use vox_plugin_api::extensions::cloud_sync::{CloudSync, CloudSync_TO};
use vox_plugin_api::host::VoxHost_TO;

#[derive(Clone)]
pub(crate) struct CloudPlugin;

impl VoxPlugin for CloudPlugin {
    fn id(&self) -> RString {
        RString::from("cloud")
    }

    fn shutdown(&self) -> RResult<(), RBoxError> {
        RResult::ROk(())
    }

    fn as_cloud_sync(&self) -> ROption<CloudSync_TO<'static, RBox<()>>> {
        ROption::RSome(CloudSync_TO::from_value(self.clone(), TD_Opaque))
    }
}

impl CloudSync for CloudPlugin {
    fn provider_id(&self) -> RString {
        RString::from("cloud")
    }

    fn upload(&self, _local_path: RStr<'_>, _remote_uri: RStr<'_>) -> RResult<(), RBoxError> {
        RResult::RErr(RBoxError::new(std::io::Error::other(
            "not yet implemented; SP7 scaffold",
        )))
    }

    fn download(&self, _remote_uri: RStr<'_>, _local_path: RStr<'_>) -> RResult<(), RBoxError> {
        RResult::RErr(RBoxError::new(std::io::Error::other(
            "not yet implemented; SP7 scaffold",
        )))
    }

    fn list_remote_json(&self, _remote_prefix: RStr<'_>) -> RResult<RString, RBoxError> {
        // Match sibling methods (upload, download) by returning an explicit
        // not-implemented error instead of silently returning "[]". An empty
        // array is indistinguishable from "no remote artifacts" at this layer
        // and would mislead callers about feature availability.
        // Refs: docs/src/architecture/semantic-gap-audit-2026.md F7.
        RResult::RErr(RBoxError::new(std::io::Error::other(
            "not yet implemented; SP7 scaffold",
        )))
    }
}

pub(crate) fn make_plugin(
    _host: VoxHost_TO<'static, RBox<()>>,
) -> RResult<VoxPluginRef, RBoxError> {
    let plugin = CloudPlugin;
    let to = VoxPlugin_TO::from_value(plugin, TD_Opaque);
    RResult::ROk(to)
}
