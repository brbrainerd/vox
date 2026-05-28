//! NVML hardware probe plugin.
//!
//! Exports a `VoxPluginRoot` that constructs an `NvmlProbePlugin`, which
//! implements both `VoxPlugin` (id + shutdown) and `HardwareProbe`
//! (probe_summary_json + device_metrics_json). The host obtains the
//! HardwareProbe interface via `VoxPlugin::as_hardware_probe()`.

mod probe;

use abi_stable::{
    erased_types::TD_Opaque, export_root_module, prefix_type::PrefixTypeTrait, sabi_extern_fn,
    std_types::*,
};
use vox_plugin_api::VOX_PLUGIN_ABI_VERSION;
use vox_plugin_api::abi::{VoxPlugin, VoxPlugin_TO, VoxPluginRef, VoxPluginRoot, VoxPluginRootRef};
use vox_plugin_api::extensions::hardware_probe::{HardwareProbe, HardwareProbe_TO};
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
    RString::from(r#"{"id":"nvml-probe","version":"0.1.0"}"#)
}

#[sabi_extern_fn]
fn init(_host: VoxHost_TO<'static, RBox<()>>) -> RResult<VoxPluginRef, RBoxError> {
    let plugin = NvmlProbePlugin;
    let to = VoxPlugin_TO::from_value(plugin, TD_Opaque);
    RResult::ROk(to)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_advertises_nvml_probe_id() {
        assert!(manifest_json().as_str().contains("\"nvml-probe\""));
        assert_eq!(NvmlProbePlugin.id().as_str(), "nvml-probe");
    }
}

#[derive(Clone)]
struct NvmlProbePlugin;

impl VoxPlugin for NvmlProbePlugin {
    fn id(&self) -> RString {
        RString::from("nvml-probe")
    }

    fn shutdown(&self) -> RResult<(), RBoxError> {
        RResult::ROk(())
    }

    fn as_hardware_probe(&self) -> ROption<HardwareProbe_TO<'static, RBox<()>>> {
        ROption::RSome(HardwareProbe_TO::from_value(self.clone(), TD_Opaque))
    }
}

impl HardwareProbe for NvmlProbePlugin {
    fn probe_summary_json(&self) -> RResult<RString, RBoxError> {
        match probe::probe_summary() {
            Ok(s) => RResult::ROk(RString::from(s)),
            Err(e) => RResult::RErr(RBoxError::new(std::io::Error::other(e.to_string()))),
        }
    }

    fn device_metrics_json(&self) -> RResult<RString, RBoxError> {
        match probe::device_metrics() {
            Ok(s) => RResult::ROk(RString::from(s)),
            Err(e) => RResult::RErr(RBoxError::new(std::io::Error::other(e.to_string()))),
        }
    }
}
