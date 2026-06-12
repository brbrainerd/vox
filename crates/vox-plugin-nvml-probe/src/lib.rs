//! NVML hardware probe plugin.
//!
//! Exports a `VoxPluginRoot` that constructs an `NvmlProbePlugin`, which
//! implements both `VoxPlugin` (id + shutdown) and `HardwareProbe`
//! (probe_summary_json + device_metrics_json). The host obtains the
//! HardwareProbe interface via `VoxPlugin::as_hardware_probe()`.

mod probe;

use abi_stable::erased_types::TD_Opaque;
use vox_plugin_api::extensions::hardware_probe::{HardwareProbe, HardwareProbe_TO};
use vox_plugin_sdk::prelude::*;

// Dylib export glue (root_module / manifest_json / init), stamped with the current ABI
// version. Byte-identical to the previous hand-written block.
vox_plugin_sdk::declare_plugin! {
    init: |_host| ROk(wrap(NvmlProbePlugin)),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_reports_nvml_probe_id() {
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
