//! Serde model for [`contracts/cli/command-registry.yaml`](../../../contracts/cli/command-registry.yaml).
//! Shared by `command-compliance`, `command-contract` metadata, and `command-sync`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RegistryFile {
    pub schema_version: u32,
    pub operations: Vec<RegistryOperation>,
    #[serde(default)]
    pub script_duals: Vec<ScriptDual>,
    #[serde(default)]
    pub env_var_ssot_index: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RegistryOperation {
    pub surface: String,
    pub path: Vec<String>,
    #[serde(default = "default_status")]
    pub status: String,
    #[serde(default)]
    pub latin_ns: Option<String>,
    /// Product lane for bell-curve planning and catalog grouping (`app`, `workflow`, `ai`, `interop`, `data`, `platform`).
    #[serde(default)]
    pub product_lane: Option<String>,
    /// Cargo feature expression (`|` = alternatives, `+` = all required) documented in CLI inventory.
    #[serde(default)]
    pub feature_gate: Option<String>,
    /// UX grouping for `vox commands` when it differs from `latin_ns` (e.g. `oratio` lane).
    #[serde(default)]
    pub catalog_group: Option<String>,
    #[serde(default = "vox_config::serde_defaults::default_true")]
    pub ref_cli_required: bool,
    #[serde(default)]
    pub reachability_required: Option<bool>,
    #[serde(default)]
    pub handler_rust: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ScriptDual {
    pub script_glob: String,
    pub canonical_cli: String,
}

fn default_status() -> String {
    "active".to_string()
}
