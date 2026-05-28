//! Code-plugin loader. Loads a cdylib via abi_stable's RootModule machinery,
//! asserts ABI version matches the host's expectation, and obtains a
//! VoxPluginRef trait object by calling the plugin's exported `init`
//! function with a fresh DefaultVoxHost.
//!
//! abi_stable leaks the underlying library (by design, for safe 'static
//! symbol access), so LoadedCodePlugin carries only the VoxPluginRef.
//!
//! Skill-payload plugins do NOT use this loader — they're parsed at
//! discover() time and stored in the SkillRegistry directly.

#![allow(clippy::result_large_err)]

use crate::errors::{AbiMismatchError, LoadError};
use crate::host_impl::DefaultVoxHost;
use crate::telemetry;
use abi_stable::library::RootModule;
use abi_stable::std_types::*;
use std::path::Path;
use std::time::Instant;
use vox_plugin_api::VOX_PLUGIN_ABI_VERSION;
use vox_plugin_api::abi::{VoxPluginRef, VoxPluginRootRef};
use vox_plugin_api::host::VoxHost_TO;

pub struct Loader;

pub struct LoadedCodePlugin {
    pub plugin: VoxPluginRef,
}

impl Loader {
    pub fn load(
        plugin_id: &str,
        version: &str,
        dylib_path: &Path,
    ) -> Result<LoadedCodePlugin, LoadError> {
        let started = Instant::now();

        let root_ref: VoxPluginRootRef =
            VoxPluginRootRef::load_from_file(dylib_path).map_err(|e| {
                telemetry::load_failed(plugin_id, version, "root_module");
                LoadError::InitFailed(format!("loading root module: {e}"))
            })?;

        let plugin_abi = root_ref.abi_version();
        if plugin_abi != VOX_PLUGIN_ABI_VERSION {
            telemetry::abi_mismatch(plugin_id, plugin_abi, VOX_PLUGIN_ABI_VERSION);
            return Err(LoadError::AbiMismatch(AbiMismatchError {
                id: plugin_id.to_string(),
                plugin_abi,
                host_abi: VOX_PLUGIN_ABI_VERSION,
            }));
        }

        let host = DefaultVoxHost::new();
        let host_to: VoxHost_TO<'static, RBox<()>> =
            VoxHost_TO::from_value(host, abi_stable::erased_types::TD_Opaque);

        let plugin_ref = (root_ref.init())(host_to).into_result().map_err(|e| {
            telemetry::load_failed(plugin_id, version, "init");
            LoadError::InitFailed(e.to_string())
        })?;

        telemetry::loaded(plugin_id, version, "code", started.elapsed().as_millis());
        Ok(LoadedCodePlugin { plugin: plugin_ref })
    }
}
