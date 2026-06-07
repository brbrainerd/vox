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
use vox_plugin_api::extensions::skill_runtime::{SkillRuntime as ExtSkillRuntime, SkillRuntime_TO};
use vox_plugin_api::host::VoxHost_TO;
use vox_skill_runtime::{RunOpts as SkillRunOpts, SkillRuntime};

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

#[derive(Clone)]
struct RuntimeWasmPlugin;

impl VoxPlugin for RuntimeWasmPlugin {
    fn id(&self) -> RString {
        RString::from("runtime-wasm")
    }

    fn shutdown(&self) -> RResult<(), RBoxError> {
        RResult::ROk(())
    }

    fn as_skill_runtime(&self) -> ROption<SkillRuntime_TO<'static, RBox<()>>> {
        ROption::RSome(SkillRuntime_TO::from_value(
            self.clone(),
            abi_stable::erased_types::TD_Opaque,
        ))
    }
}

/// Bridge the native wasmtime [`runtime::WasmRuntime`] onto the ABI-stable `SkillRuntime`
/// extension. `invoke_skill`'s `input_json` is a [`SkillRunOpts`] JSON object; the returned
/// string is a `RunOutcome` JSON object. `skill_id` labels the run when `name` is omitted.
impl ExtSkillRuntime for RuntimeWasmPlugin {
    fn invoke_skill(
        &self,
        skill_id: RStr<'_>,
        input_json: RStr<'_>,
    ) -> RResult<RString, RBoxError> {
        match invoke_wasm_skill(skill_id.as_str(), input_json.as_str()) {
            Ok(json) => RResult::ROk(RString::from(json)),
            Err(e) => RResult::RErr(RBoxError::new(std::io::Error::other(e.to_string()))),
        }
    }
}

fn invoke_wasm_skill(skill_id: &str, input_json: &str) -> anyhow::Result<String> {
    let mut opts: SkillRunOpts = serde_json::from_str(input_json)?;
    if opts.name.is_none() {
        opts.name = Some(skill_id.to_string());
    }
    let rt = runtime::WasmRuntime::new()?;
    let outcome = SkillRuntime::run(&rt, &opts)?;
    Ok(serde_json::to_string(&outcome)?)
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
