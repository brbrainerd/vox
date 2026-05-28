//! Rename registry for Vox public identifiers.
//!
//! Loads `contracts/naming/renames.v1.json` and validates it: no duplicate `from`
//! keys, no alias chains, version must be 1. Used by parser alias resolution
//! (Task 4) and the `vox migrate` codemod (Tasks 5-7).
//!
//! See: `docs/src/architecture/vuv-naming-policy-2026.md`

use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RenameKind {
    Primitive,
    Kwarg,
    Decorator,
    EnumValue,
    Type,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RenameEntry {
    pub from: String,
    pub to: String,
    pub kind: RenameKind,
    pub since: String,
    #[serde(default)]
    pub removed_in: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RenameRegistryFile {
    version: u32,
    #[serde(default)]
    entries: Vec<RenameEntry>,
}

#[derive(Debug, Clone)]
pub struct RenameRegistry {
    by_from: HashMap<String, RenameEntry>,
}

#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("registry version {0} is not supported (expected 1)")]
    UnsupportedVersion(u32),
    #[error("duplicate `from` key: {0}")]
    DuplicateFrom(String),
    #[error("alias chain: `from` {0} is also a `to` in another entry")]
    AliasChain(String),
    #[error("rename `from` is empty")]
    EmptyFrom,
    #[error("self-rename: `from` and `to` are both `{0}`")]
    SelfRename(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse: {0}")]
    Parse(#[from] serde_json::Error),
}

impl RenameRegistry {
    pub fn parse_json(json: &str) -> Result<Self, RegistryError> {
        let file: RenameRegistryFile = serde_json::from_str(json)?;
        if file.version != 1 {
            return Err(RegistryError::UnsupportedVersion(file.version));
        }
        let mut by_from: HashMap<String, RenameEntry> = HashMap::new();
        // Per-entry validation: reject empty `from` and self-renames before further checks.
        for entry in &file.entries {
            if entry.from.is_empty() {
                return Err(RegistryError::EmptyFrom);
            }
            if entry.from == entry.to {
                return Err(RegistryError::SelfRename(entry.from.clone()));
            }
        }
        let to_set: std::collections::HashSet<&String> =
            file.entries.iter().map(|e| &e.to).collect();
        for entry in &file.entries {
            if by_from.contains_key(&entry.from) {
                return Err(RegistryError::DuplicateFrom(entry.from.clone()));
            }
            if to_set.contains(&entry.from) {
                return Err(RegistryError::AliasChain(entry.from.clone()));
            }
            by_from.insert(entry.from.clone(), entry.clone());
        }
        Ok(Self { by_from })
    }

    pub fn load_canonical() -> Result<Self, RegistryError> {
        let path = canonical_path();
        let bytes = std::fs::read_to_string(&path)?;
        Self::parse_json(&bytes)
    }

    pub fn entries(&self) -> impl Iterator<Item = &RenameEntry> {
        self.by_from.values()
    }

    /// Resolve an old name to its canonical replacement. Returns `None` if the
    /// name is canonical (no rename applies).
    pub fn resolve(&self, name: &str) -> Option<&RenameEntry> {
        self.by_from.get(name)
    }
}

fn canonical_path() -> PathBuf {
    if let Ok(custom) = std::env::var("VOX_RENAMES_PATH") {
        return PathBuf::from(custom);
    }
    // Walk up from CARGO_MANIFEST_DIR to find the workspace root.
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest
        .ancestors()
        .find(|p| p.join("contracts/naming/renames.v1.json").exists())
        .expect("workspace root with contracts/naming/renames.v1.json must be findable");
    workspace_root.join("contracts/naming/renames.v1.json")
}
