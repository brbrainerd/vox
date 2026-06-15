//! Shared timing constants for the Vox GUI Tauri backend.
//! Also re-exports the generated GUI config field catalog (a VIEW over CONFIG_KEYS).

/// Scientia queue watcher poll interval (seconds).
pub const SCIENTIA_QUEUE_POLL_SECS: u64 = 3;

/// Orchestrator status stream channel capacity.
pub const ORCH_STATUS_CHANNEL_CAP: usize = 64;

/// Agent events stream channel capacity.
pub const AGENT_EVENTS_CHANNEL_CAP: usize = 256;

/// One field entry in the generated settings catalog.
/// Regenerate with: `vox ci config-gui-codegen --fields`
#[derive(Debug, Clone, Copy)]
pub struct GeneratedField {
    pub key: &'static str,
    pub label: &'static str,
    pub hint: &'static str,
    /// Kebab section id (e.g. "runtime", "tuning").
    pub section: &'static str,
    /// GUI kind string: "string" | "int" | "float" | "bool" | "path" | "enum".
    pub kind: &'static str,
    /// Allowed values when `kind == "enum"`.
    pub options: &'static [&'static str],
    pub default: &'static str,
}

/// The generated settings catalog — a static view over CONFIG_KEYS.
/// DO NOT EDIT: regenerate with `vox ci config-gui-codegen --fields`
pub const GENERATED_FIELDS: &[GeneratedField] = include!("generated_fields.rs");
