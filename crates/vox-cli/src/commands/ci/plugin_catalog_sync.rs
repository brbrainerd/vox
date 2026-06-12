//! `vox ci plugin-catalog-sync [--write]`
//!
//! Derives the per-plugin rows of `crates/vox-plugin-catalog/catalog.toml` from the
//! authoritative per-plugin `Plugin.toml` manifests. The catalog's manifest-echo fields
//! (`description`, `status`, `payload-kind`, `extension-points`, `exposes-tools`) become
//! generated; the catalog keeps its hand-authored fields (`requires-tag`, `default-source`,
//! `bundled-in`) and the `[[bundle]]` / `[[component]]` definitions, which have no
//! per-plugin equivalent.
//!
//! Only catalog rows whose `id` has an in-tree `crates/vox-plugin-*/Plugin.toml` are
//! synced; `github:`-sourced plugins with no in-tree manifest are left untouched. Updates
//! are surgical (via `toml_edit`) so comments and structure are preserved.

use anyhow::{Context, Result, bail};
use std::collections::BTreeMap;
use std::path::Path;

const REL_CATALOG: &str = "crates/vox-plugin-catalog/catalog.toml";

struct Derived {
    payload_kind: String,
    description: String,
    status: Option<String>,
    /// For code/composite payloads (the `provides.extension-points`).
    extension_points: Option<Vec<String>>,
    /// For skill/composite payloads (the `tools.exposes`).
    exposes_tools: Option<Vec<String>>,
}

fn str_array(v: Option<&toml::Value>) -> Vec<String> {
    v.and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|e| e.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// Extract the catalog-derivable fields from every in-tree plugin manifest, keyed by id.
fn scan_manifests(repo_root: &Path) -> Result<BTreeMap<String, Derived>> {
    let crates_root = repo_root.join("crates");
    let mut out = BTreeMap::new();
    if !crates_root.is_dir() {
        return Ok(out);
    }
    for entry in walkdir::WalkDir::new(&crates_root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name() == "Plugin.toml")
    {
        let path = entry.path();
        if path.components().any(|c| c.as_os_str() == "tests") {
            continue;
        }
        let raw =
            std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        let Ok(val) = raw.parse::<toml::Value>() else {
            continue;
        };
        let Some(plugin) = val.get("plugin") else {
            continue;
        };
        let Some(id) = plugin.get("id").and_then(|v| v.as_str()) else {
            continue;
        };
        let Some(payload) = plugin.get("payload") else {
            continue;
        };
        let payload_kind = payload
            .get("kind")
            .and_then(|v| v.as_str())
            .unwrap_or("code")
            .to_string();
        let description = plugin
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let status = plugin
            .get("status")
            .and_then(|v| v.as_str())
            .map(str::to_string);

        // code → payload.* ; composite → payload.code.* / payload.skill.*
        let code = payload.get("code").unwrap_or(payload);
        let extension_points = match payload_kind.as_str() {
            "code" | "composite" => Some(str_array(
                code.get("provides").and_then(|p| p.get("extension-points")),
            )),
            _ => None,
        };
        let skill_tools = payload
            .get("tools")
            .or_else(|| payload.get("skill").and_then(|s| s.get("tools")));
        let exposes_tools = match payload_kind.as_str() {
            "skill" | "composite" => Some(str_array(skill_tools.and_then(|t| t.get("exposes")))),
            _ => None,
        };

        out.insert(
            id.to_string(),
            Derived {
                payload_kind,
                description,
                status,
                extension_points,
                exposes_tools,
            },
        );
    }
    Ok(out)
}

fn string_array_item(items: &[String]) -> toml_edit::Item {
    let mut arr = toml_edit::Array::new();
    for s in items {
        arr.push(s.as_str());
    }
    toml_edit::value(arr)
}

/// Apply the derived fields onto the catalog document (surgical; preserves everything else).
/// Returns the rendered catalog string.
fn render(catalog_src: &str, derived: &BTreeMap<String, Derived>) -> Result<String> {
    let mut doc = catalog_src
        .parse::<toml_edit::DocumentMut>()
        .context("parse catalog.toml")?;
    if let Some(plugins) = doc
        .get_mut("plugin")
        .and_then(|i| i.as_array_of_tables_mut())
    {
        for tbl in plugins.iter_mut() {
            let id = tbl
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let Some(d) = derived.get(&id) else {
                continue; // no in-tree manifest (e.g. github-sourced) — leave as-authored
            };
            tbl["payload-kind"] = toml_edit::value(d.payload_kind.as_str());
            tbl["description"] = toml_edit::value(d.description.as_str());
            if let Some(status) = &d.status {
                tbl["status"] = toml_edit::value(status.as_str());
            }
            if let Some(eps) = &d.extension_points {
                tbl["extension-points"] = string_array_item(eps);
            }
            if let Some(tools) = &d.exposes_tools {
                tbl["exposes-tools"] = string_array_item(tools);
            }
        }
    }
    Ok(doc.to_string())
}

/// Run the gate. `write` regenerates the catalog; otherwise verify it is in sync.
pub fn run(repo_root: &Path, write: bool) -> Result<()> {
    let derived = scan_manifests(repo_root)?;
    let path = repo_root.join(REL_CATALOG);
    let original =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let rendered = render(&original, &derived)?;

    if write {
        std::fs::write(&path, &rendered).with_context(|| format!("write {}", path.display()))?;
        println!(
            "plugin-catalog-sync: wrote {REL_CATALOG} ({} in-tree plugin row(s) synced)",
            derived.len()
        );
        return Ok(());
    }
    if normalize(&original) != normalize(&rendered) {
        bail!(
            "{REL_CATALOG} is stale vs the plugin manifests; run `vox ci plugin-catalog-sync --write`"
        );
    }
    println!(
        "plugin-catalog-sync OK ({} in-tree plugin row(s) match their manifests)",
        derived.len()
    );
    Ok(())
}

fn normalize(s: &str) -> String {
    s.replace("\r\n", "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("repo root")
            .to_path_buf()
    }

    #[test]
    fn committed_catalog_is_in_sync() {
        run(&repo_root(), false).expect("catalog.toml is in sync with plugin manifests");
    }
}
