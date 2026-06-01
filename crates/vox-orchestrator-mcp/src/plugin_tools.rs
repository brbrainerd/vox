//! MCP tools for the vox plugin host + first-party plugin marketplace.
//!
//! Mirrors `skills_tools.rs`. Read tools (`list`/`catalog`/`info`) read from the
//! live plugin-host [`Registry`](vox_plugin_host::registry::Registry) kept on
//! [`ServerState`] and from the static [`vox_plugin_catalog`]. Mutating tools
//! (`install`/`remove`) touch the install dir on disk, then re-discover so both
//! the plugin registry and the skill registry stay fresh. They reach the daemon's
//! HITL gate automatically because the GUI invokes them via `orch.tool_call`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::params::ToolResult;
use crate::server_state::ServerState;

const REM_PLUGIN_ID: &str =
    "Run `vox_plugin_list` / `vox_plugin_catalog` and pass a known plugin `id`.";
const REM_PLUGIN_INSTALL: &str = "Provide a catalog `id`, or a local `path` to a directory containing Plugin.toml. Check disk permissions.";

// ---------------------------------------------------------------------------
// Parameters
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PluginIdParams {
    pub id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PluginInstallParams {
    /// Catalog plugin id (resolves `default-source`, preferring a local workspace checkout).
    #[serde(default)]
    pub id: Option<String>,
    /// Local directory containing `Plugin.toml` + siblings to copy into the install root.
    #[serde(default)]
    pub path: Option<String>,
}

// ---------------------------------------------------------------------------
// Response shapes
// ---------------------------------------------------------------------------

/// One installed plugin row (from the live plugin-host registry).
#[derive(Debug, Serialize, Deserialize)]
pub struct PluginRow {
    pub id: String,
    pub version: String,
    pub payload_kind: String,
    pub install_dir: String,
}

/// Marketplace listing: first-party catalog plugins + bundles.
#[derive(Debug, Serialize)]
pub struct CatalogListing {
    pub plugins: Vec<&'static vox_plugin_catalog::schema::PluginCatalogEntry>,
    pub bundles: Vec<&'static vox_plugin_catalog::schema::BundleEntry>,
}

fn payload_kind_str(payload: &vox_plugin_types::plugin_manifest::PluginPayload) -> &'static str {
    use vox_plugin_types::plugin_manifest::PluginPayload;
    match payload {
        PluginPayload::Code(_) => "code",
        PluginPayload::Skill(_) => "skill",
        PluginPayload::Composite(_) => "composite",
    }
}

fn entry_to_row(entry: &vox_plugin_host::registry::PluginEntry) -> PluginRow {
    PluginRow {
        id: entry.id.clone(),
        version: entry.version.clone(),
        payload_kind: payload_kind_str(&entry.payload).to_string(),
        install_dir: entry.install_dir.display().to_string(),
    }
}

// ---------------------------------------------------------------------------
// Read handlers
// ---------------------------------------------------------------------------

pub async fn plugin_list(state: &ServerState) -> String {
    let reg = state.plugin_registry.read().await;
    let rows: Vec<PluginRow> = reg
        .list_ids()
        .into_iter()
        .filter_map(|id| reg.get_full_entry(&id))
        .map(|e| entry_to_row(&e))
        .collect();
    ToolResult::ok(rows).to_json()
}

pub fn plugin_catalog() -> String {
    let listing = CatalogListing {
        plugins: vox_plugin_catalog::all_plugins().iter().collect(),
        bundles: vox_plugin_catalog::all_bundles().iter().collect(),
    };
    ToolResult::ok(listing).to_json()
}

pub async fn plugin_info(state: &ServerState, params: PluginIdParams) -> String {
    let reg = state.plugin_registry.read().await;
    if let Some(entry) = reg.get_full_entry(&params.id) {
        // Full installed entry, including the typed payload.
        let body = serde_json::json!({
            "id": entry.id,
            "version": entry.version,
            "payload_kind": payload_kind_str(&entry.payload),
            "install_dir": entry.install_dir.display().to_string(),
            "payload": entry.payload,
            "installed": true,
        });
        return ToolResult::ok(body).to_json();
    }
    // Fall back to the marketplace catalog entry (not installed).
    if let Some(cat) = vox_plugin_catalog::all_plugins()
        .iter()
        .find(|p| p.id == params.id)
    {
        let body = serde_json::json!({
            "id": cat.id,
            "payload_kind": format!("{:?}", cat.payload_kind).to_lowercase(),
            "description": cat.description,
            "status": format!("{:?}", cat.status).to_lowercase(),
            "default_source": cat.default_source,
            "exposes_tools": cat.exposes_tools,
            "extension_points": cat.extension_points,
            "bundled_in": cat.bundled_in,
            "installed": false,
        });
        return ToolResult::ok(body).to_json();
    }
    ToolResult::<String>::err_with_remediation(
        format!("Plugin '{}' not installed and not in catalog.", params.id),
        REM_PLUGIN_ID,
    )
    .to_json()
}

// ---------------------------------------------------------------------------
// Mutating handlers
// ---------------------------------------------------------------------------

/// Re-discover the install dir and rebuild both the plugin registry and the
/// skill registry. Called after every successful install/remove.
async fn refresh_registries(state: &ServerState) {
    let install_dir = state.plugins_dir.as_ref().clone();
    match vox_plugin_host::discover(&install_dir) {
        Ok(fresh) => {
            *state.plugin_registry.write().await = fresh;
        }
        Err(e) => {
            tracing::warn!("plugin re-discover failed at {install_dir:?}: {e}");
        }
    }
    crate::plugin_skills_bridge::install_discovered_skills(&state.skill_registry, &install_dir)
        .await;
}

pub async fn plugin_install(state: &ServerState, params: PluginInstallParams) -> String {
    let install_root = state.plugins_dir.as_ref().clone();

    // Resolve a source directory containing Plugin.toml.
    let src_dir: std::path::PathBuf = match (&params.path, &params.id) {
        (Some(path), _) => std::path::PathBuf::from(path),
        (None, Some(id)) => {
            // Catalog install: prefer a local workspace checkout, else require `local:` source.
            if let Some(local) = vox_plugin_host::workspace_local_plugin_source(id) {
                local
            } else if let Some(cat) = vox_plugin_catalog::all_plugins()
                .iter()
                .find(|p| &p.id == id)
            {
                match cat.default_source.strip_prefix("local:") {
                    Some(rel) => std::path::PathBuf::from(rel),
                    None => {
                        return ToolResult::<String>::err_with_remediation(
                            format!(
                                "Plugin '{id}' default-source '{}' is not a local path; install it with the `vox plugin install` CLI (github/url fetch) first.",
                                cat.default_source
                            ),
                            REM_PLUGIN_INSTALL,
                        )
                        .to_json();
                    }
                }
            } else {
                return ToolResult::<String>::err_with_remediation(
                    format!("Plugin '{id}' not found in catalog."),
                    REM_PLUGIN_ID,
                )
                .to_json();
            }
        }
        (None, None) => {
            return ToolResult::<String>::err_with_remediation(
                "Specify a catalog `id` or a local `path`.",
                REM_PLUGIN_INSTALL,
            )
            .to_json();
        }
    };

    match copy_plugin_into_root(&src_dir, &install_root) {
        Ok((id, version, files)) => {
            refresh_registries(state).await;
            ToolResult::ok(format!(
                "Installed plugin '{id}' v{version} ({files} files) into {}",
                install_root.display()
            ))
            .to_json()
        }
        Err(e) => ToolResult::<String>::err_with_remediation(e, REM_PLUGIN_INSTALL).to_json(),
    }
}

pub async fn plugin_remove(state: &ServerState, params: PluginIdParams) -> String {
    let install_root = state.plugins_dir.as_ref().clone();
    let id_dir = install_root.join(&params.id);
    if !id_dir.exists() {
        return ToolResult::ok(format!(
            "Plugin '{}' was not installed (no dir at {}).",
            params.id,
            id_dir.display()
        ))
        .to_json();
    }
    match std::fs::remove_dir_all(&id_dir) {
        Ok(()) => {
            refresh_registries(state).await;
            ToolResult::ok(format!("Removed plugin '{}'.", params.id)).to_json()
        }
        Err(e) => ToolResult::<String>::err_with_remediation(
            format!("Failed to remove '{}': {e}", params.id),
            REM_PLUGIN_ID,
        )
        .to_json(),
    }
}

/// Copy `src_dir` (must contain `Plugin.toml`) into `<root>/<id>/<version>/`.
/// Returns `(id, version, files_copied)`. Mirrors the CLI `install_from_path`.
fn copy_plugin_into_root(
    src_dir: &std::path::Path,
    root: &std::path::Path,
) -> Result<(String, String, usize), String> {
    let plugin_toml = src_dir.join("Plugin.toml");
    if !plugin_toml.exists() {
        return Err(format!("No Plugin.toml found in {}", src_dir.display()));
    }
    let raw = std::fs::read_to_string(&plugin_toml)
        .map_err(|e| format!("reading {}: {e}", plugin_toml.display()))?;
    let head: PluginHead =
        toml::from_str(&raw).map_err(|e| format!("parsing {}: {e}", plugin_toml.display()))?;
    let dest = root.join(&head.plugin.id).join(&head.plugin.version);
    std::fs::create_dir_all(&dest).map_err(|e| format!("creating {}: {e}", dest.display()))?;

    let mut copied = 0usize;
    let entries = std::fs::read_dir(src_dir).map_err(|e| format!("reading src dir: {e}"))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("dir entry: {e}"))?;
        let from = entry.path();
        if from.is_file() {
            let to = dest.join(entry.file_name());
            std::fs::copy(&from, &to)
                .map_err(|e| format!("copying {} -> {}: {e}", from.display(), to.display()))?;
            copied += 1;
        }
    }
    Ok((head.plugin.id, head.plugin.version, copied))
}

#[derive(Deserialize)]
struct PluginHead {
    plugin: PluginMeta,
}

#[derive(Deserialize)]
struct PluginMeta {
    id: String,
    version: String,
}
