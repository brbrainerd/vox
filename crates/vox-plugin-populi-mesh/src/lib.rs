//! vox-plugin-populi-mesh — Populi mesh transport composite plugin.
//!
//! Code side (cdylib): MeshDriver impl. Skill side: populi.skill.md
//! describing the mesh tools to agents.
//!
//! Transport code ported from vox-populi's `transport` feature (~4,990 LOC).
//! The plugin owns the HTTP control plane; vox-cli and others dispatch through
//! the MeshDriver trait rather than calling vox-populi::transport directly.

// Transport infrastructure ported from vox-populi; not all paths are exercised
// through the FFI entry points yet. Suppress until mesh integration is complete.
#![allow(dead_code)]

pub(crate) mod http_client;
mod mesh;
pub(crate) mod transport;

// Re-export vox_populi root items that the ported transport code references
// via `crate::`. This lets the copied files compile unchanged (they use
// `crate::NodeRecord`, `crate::now_ms()`, etc.).
pub(crate) use vox_populi::{
    MAX_MAINTENANCE_FOR_MS, NodeRecord, PopuliRegistryError, PopuliRegistryFile,
    node_maintenance_blocks_new_work, populi_env, populi_scope_id_from_env,
    sweep_expired_maintenance_on_nodes,
};
// node_record_for_current_process and local_registry_path are used by transport code
pub(crate) use vox_populi::{local_registry_path, node_record_for_current_process};

/// `now_ms` is pub(crate) in vox_populi; re-implement locally.
#[inline]
pub(crate) fn now_ms() -> u64 {
    vox_populi::wall_clock_unix_ms()
}

use abi_stable::{
    erased_types::TD_Opaque, export_root_module, prefix_type::PrefixTypeTrait, sabi_extern_fn,
    std_types::*,
};
use vox_plugin_api::VOX_PLUGIN_ABI_VERSION;
use vox_plugin_api::abi::{VoxPlugin_TO, VoxPluginRef, VoxPluginRoot, VoxPluginRootRef};
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
    RString::from(r#"{"id":"populi-mesh","version":"0.1.0"}"#)
}

#[sabi_extern_fn]
fn init(_host: VoxHost_TO<'static, RBox<()>>) -> RResult<VoxPluginRef, RBoxError> {
    let plugin = mesh::PopuliMeshPlugin::new();
    let to = VoxPlugin_TO::from_value(plugin, TD_Opaque);
    RResult::ROk(to)
}
