//! # vox-plugin-runtime-wasm
//!
//! Skill-runtime plugin: wasmtime-based WASI sandbox.
//!
//! This is the **default** sandbox for pure-compute skills (no subprocess, no GPU).
//! Implements [`vox_skill_runtime::SkillRuntime`] via a wasmtime engine.
//!
//! # Why WASM?
//! - Cold start: ~µs (vs seconds for Docker)
//! - Footprint: ~5MB embedded (vs 200MB+ Docker daemon)
//! - No external daemon required
//! - Pure-Rust (wasmtime, Bytecode Alliance)
//! - Capability-bound by default (WASI preopens, no ambient authority)
//!
//! # Status: SCAFFOLD
//! The engine and module loading work. Full WASI preopen plumbing, fuel-based
//! timeout enforcement, and wasi-http are TODO. See runtime.rs for details.
//!
//! Install: `vox plugin install runtime-wasm`

use abi_stable::{export_root_module, prefix_type::PrefixTypeTrait, sabi_extern_fn, std_types::*};
use vox_plugin_api::VOX_PLUGIN_ABI_VERSION;
use vox_plugin_api::abi::{VoxPlugin, VoxPlugin_TO, VoxPluginRef, VoxPluginRoot, VoxPluginRootRef};
use vox_plugin_api::host::VoxHost_TO;

pub mod runtime;

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
    RString::from(r#"{"id":"runtime-wasm","version":"0.1.0"}"#)
}

#[sabi_extern_fn]
fn init(_host: VoxHost_TO<'static, RBox<()>>) -> RResult<VoxPluginRef, RBoxError> {
    let plugin = RuntimeWasmPlugin;
    let to = VoxPlugin_TO::from_value(plugin, abi_stable::erased_types::TD_Opaque);
    RResult::ROk(to)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_advertises_runtime_wasm_id() {
        assert!(manifest_json().as_str().contains("\"runtime-wasm\""));
        assert_eq!(RuntimeWasmPlugin.id().as_str(), "runtime-wasm");
    }
}

struct RuntimeWasmPlugin;

impl VoxPlugin for RuntimeWasmPlugin {
    fn id(&self) -> RString {
        RString::from("runtime-wasm")
    }

    fn shutdown(&self) -> RResult<(), RBoxError> {
        RResult::ROk(())
    }
}
