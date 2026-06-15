//! Verifies that the sabi-trait machinery compiles end-to-end. No runtime
//! behavior is asserted here — actual loading is exercised in vox-plugin-host
//! integration tests (SP2 batch 6).

use abi_stable::{erased_types::TD_Opaque, std_types::*};
use vox_plugin_api::abi::{VoxPlugin, VoxPlugin_TO};
use vox_plugin_api::host::{SabiLogLevel, VoxHost, VoxHost_TO};

struct DummyHost;
impl VoxHost for DummyHost {
    fn data_dir(&self) -> RString {
        // vox-arch-check: allow abs-path
        RString::from("/tmp")
    }
    fn log(&self, _level: SabiLogLevel, _msg: RStr<'_>) {}
    fn telemetry_event(&self, _kind: RStr<'_>, _payload: RStr<'_>) {}
}

struct DummyPlugin;
impl VoxPlugin for DummyPlugin {
    fn id(&self) -> RString {
        RString::from("dummy")
    }
    fn shutdown(&self) -> RResult<(), RBoxError> {
        RResult::ROk(())
    }
}

#[test]
fn host_to_constructs() {
    let _: VoxHost_TO<'static, RBox<()>> = VoxHost_TO::from_value(DummyHost, TD_Opaque);
}

#[test]
fn plugin_to_constructs() {
    let _: VoxPlugin_TO<'static, RBox<()>> = VoxPlugin_TO::from_value(DummyPlugin, TD_Opaque);
}
