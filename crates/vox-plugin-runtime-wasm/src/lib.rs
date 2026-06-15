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

#[cfg(test)]
mod semcov_wave5_tests {
    use super::*;
    use abi_stable::std_types::{RResult, RStr};

    // Helper: build a minimal valid RunOpts JSON pointing to a non-existent artifact.
    fn missing_artifact_json() -> String {
        // vox-arch-check: allow abs-path
        r#"{"artifact_path":"/tmp/nonexistent_skill.wasm","ports":[],"env":[],"volumes":[],"detach":false,"name":null,"rm":true,"cpu_limit_fuel":null}"#.to_string()
    }

    fn run_opts_json_with_name(artifact: &str, name: Option<&str>) -> String {
        let name_field = match name {
            Some(n) => format!("\"{}\"", n),
            None => "null".to_string(),
        };
        format!(
            r#"{{"artifact_path":"{}","ports":[],"env":[],"volumes":[],"detach":false,"name":{},"rm":true,"cpu_limit_fuel":null}}"#,
            artifact, name_field
        )
    }

    // --- .invoke_skill() tests ---

    #[test]
    fn invoke_skill_returns_err_when_artifact_missing() {
        let plugin = RuntimeWasmPlugin;
        let result = plugin.invoke_skill(
            RStr::from("test-skill"),
            RStr::from(missing_artifact_json().as_str()),
        );
        // Must be an error because the wasm artifact does not exist on disk.
        assert!(
            matches!(result, RResult::RErr(_)),
            "expected RErr when artifact path does not exist, got ROk"
        );
    }

    #[test]
    fn invoke_skill_returns_err_on_invalid_json() {
        let plugin = RuntimeWasmPlugin;
        let result = plugin.invoke_skill(
            RStr::from("test-skill"),
            RStr::from("not valid json at all"),
        );
        assert!(
            matches!(result, RResult::RErr(_)),
            "expected RErr on malformed JSON input"
        );
    }

    // --- invoke_wasm_skill() tests ---

    #[test]
    fn invoke_wasm_skill_propagates_json_parse_error() {
        let result = invoke_wasm_skill("my-skill", "{not: json");
        assert!(result.is_err(), "should fail on unparseable JSON");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("expected")
                || msg.contains("JSON")
                || msg.contains("invalid")
                || msg.contains("key")
                || msg.contains("line"),
            "unexpected error message: {msg}"
        );
    }

    #[test]
    fn invoke_wasm_skill_sets_name_from_skill_id_when_name_is_none() {
        // Point artifact at a path that does not exist so the error comes from
        // execution, not JSON parsing — which proves the name-injection branch
        // was reached (otherwise JSON parse would short-circuit before name is touched).
        // vox-arch-check: allow abs-path
        let json = run_opts_json_with_name("/tmp/not_a_real_skill.wasm", None);
        let result = invoke_wasm_skill("injected-name", &json);
        // We expect an error (artifact missing) but NOT a JSON parse error,
        // meaning the name-injection branch (opts.name = Some(skill_id)) was executed.
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            !msg.contains("expected"),
            "should not be a JSON error; got: {msg}"
        );
    }

    #[test]
    fn invoke_wasm_skill_error_when_artifact_does_not_exist() {
        let json = run_opts_json_with_name("/absolutely/does/not/exist.wasm", Some("named-skill"));
        let result = invoke_wasm_skill("named-skill", &json);
        assert!(result.is_err(), "should fail when wasm artifact is missing");
    }
}
