use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Default, Serialize, Deserialize)]
struct Overrides {
    entries: BTreeMap<String, PolicyOverride>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct PolicyOverride {
    pub enabled: Option<bool>,
    pub title: Option<String>,
    pub description: Option<String>,
}

fn path(root: &Path) -> std::path::PathBuf {
    root.join(".vox").join("policy-overrides.json")
}

fn load(root: &Path) -> std::io::Result<Overrides> {
    match std::fs::read(path(root)) {
        Ok(b) => Ok(serde_json::from_slice(&b).unwrap_or_default()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Overrides::default()),
        Err(e) => Err(e),
    }
}

fn store(root: &Path, ov: &Overrides) -> std::io::Result<()> {
    let p = path(root);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(p, serde_json::to_vec_pretty(ov)?)
}

pub fn get_entry(root: &Path, id: &str) -> std::io::Result<Option<PolicyOverride>> {
    Ok(load(root)?.entries.get(id).cloned())
}

pub fn get_override(root: &Path, id: &str) -> std::io::Result<Option<bool>> {
    Ok(load(root)?.entries.get(id).and_then(|o| o.enabled))
}

pub fn set_enabled(root: &Path, id: &str, enabled: bool) -> std::io::Result<()> {
    let mut ov = load(root)?;
    ov.entries.entry(id.to_string()).or_default().enabled = Some(enabled);
    store(root, &ov)
}

pub fn set_fields(
    root: &Path,
    id: &str,
    title: Option<String>,
    description: Option<String>,
) -> std::io::Result<()> {
    let mut ov = load(root)?;
    let e = ov.entries.entry(id.to_string()).or_default();
    if title.is_some() {
        e.title = title;
    }
    if description.is_some() {
        e.description = description;
    }
    store(root, &ov)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn set_then_get_roundtrips_in_tempdir() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        assert_eq!(get_override(root, "code-audit/stub/todo").unwrap(), None);
        set_enabled(root, "code-audit/stub/todo", false).unwrap();
        assert_eq!(
            get_override(root, "code-audit/stub/todo").unwrap(),
            Some(false)
        );
        set_enabled(root, "code-audit/stub/todo", true).unwrap();
        assert_eq!(
            get_override(root, "code-audit/stub/todo").unwrap(),
            Some(true)
        );
    }
}
