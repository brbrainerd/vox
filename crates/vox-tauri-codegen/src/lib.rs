//! Tauri 2 packaging hints for `vox compile`.
//!
//! Emits a **`tauri.conf.json`** fragment compatible with Tauri CLI v2 (`$schema` URL). Full
//! `src-tauri` Rust crate wiring is completed by `cargo tauri init` / upstream templates on
//! first clone; this crate keeps the **identifier**, **window**, and **frontendDist** SSOT-aligned
//! with `[bundle]` in `Vox.toml`.
//!
//! When a workspace root containing [`RUNTIME_CAPABILITIES_REL`] is supplied, also emits
//! **`runtime-capabilities.projection.json`** — a merge-friendly projection of the capability →
//! Tauri / Android / iOS packaging map (see `docs/src/adr/036-webir-hir-unification-compare-both.md`).

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Relative path from repository root to the runtime capability packaging SSOT.
pub const RUNTIME_CAPABILITIES_REL: &str = "contracts/capability/runtime-capabilities.v1.yaml";

#[derive(Debug, Clone)]
pub struct TauriEmitParams<'a> {
    pub identifier: &'a str,
    pub display_name: &'a str,
    pub frontend_dist_relative: &'a str,
}

#[derive(Serialize)]
struct TauriConfigV2 {
    #[serde(rename = "$schema")]
    schema: &'static str,
    product_name: String,
    identifier: String,
    build: TauriBuild,
    app: TauriApp,
}

#[derive(Serialize)]
struct TauriBuild {
    frontend_dist: String,
}

#[derive(Serialize)]
struct TauriApp {
    windows: Vec<TauriWindow>,
}

#[derive(Serialize)]
struct TauriWindow {
    /// Stable window label referenced from `capabilities/*.json`.
    label: String,
    title: String,
    width: u32,
    height: u32,
}

/// Build the desktop `tauri.conf.json` document (Tauri CLI v2).
#[must_use]
pub fn tauri_desktop_config_value(params: &TauriEmitParams<'_>) -> serde_json::Value {
    let cfg = TauriConfigV2 {
        schema: "https://schema.tauri.app/config/2",
        product_name: params.display_name.to_string(),
        identifier: params.identifier.to_string(),
        build: TauriBuild {
            frontend_dist: params.frontend_dist_relative.to_string(),
        },
        app: TauriApp {
            windows: vec![TauriWindow {
                label: "main".to_string(),
                title: params.display_name.to_string(),
                width: 1280,
                height: 800,
            }],
        },
    };
    serde_json::to_value(&cfg).expect("TauriConfigV2 serializes to JSON")
}

/// Pretty-printed `tauri.conf.json` body for embedding into generated `src-tauri/`.
pub fn serialize_tauri_desktop_config(params: &TauriEmitParams<'_>) -> Result<String> {
    let v = tauri_desktop_config_value(params);
    serde_json::to_string_pretty(&v).context("serialize tauri.conf.json")
}

/// Write a V2 `tauri.conf.json` (overwrites). Used by `vox compile` to refresh bundle ids after codegen.
pub fn write_tauri_desktop_config(path: &Path, params: &TauriEmitParams<'_>) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("mkdir {}", parent.display()))?;
    }
    let raw = serialize_tauri_desktop_config(params)?;
    fs::write(path, raw).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

/// Walk parents of `start` until `contracts/capability/runtime-capabilities.v1.yaml` exists.
#[must_use]
pub fn find_contracts_repo_root(start: &Path) -> Option<PathBuf> {
    let mut p = start.to_path_buf();
    loop {
        if p.join(RUNTIME_CAPABILITIES_REL).is_file() {
            return Some(p);
        }
        if !p.pop() {
            break;
        }
    }
    None
}

#[derive(Debug, Deserialize)]
struct RuntimeCapabilitiesYaml {
    #[serde(default)]
    schema_version: Option<u32>,
    #[serde(default)]
    capabilities: Vec<RuntimeCapabilityYamlRow>,
}

#[derive(Debug, Deserialize)]
struct RuntimeCapabilityYamlRow {
    id: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    tauri_permission: Vec<String>,
    #[serde(default)]
    android_uses_permission: Vec<String>,
    #[serde(default)]
    ios_info_plist_keys: Vec<String>,
    #[serde(default)]
    web_csp_note: Option<String>,
}

#[derive(Debug, Serialize)]
struct RuntimeCapabilitiesProjection {
    /// Mirror of `contracts/capability/runtime-capabilities.v1.yaml` top-level schema_version when present.
    schema_version: u32,
    source: String,
    capabilities: Vec<RuntimeCapabilityProjectionRow>,
    /// Sorted unique Tauri v2 permission identifiers referenced by any row (merge hint for `capabilities/`).
    tauri_permission_allow_list: Vec<String>,
}

#[derive(Debug, Serialize)]
struct RuntimeCapabilityProjectionRow {
    id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    tauri_permission: Vec<String>,
    android_uses_permission: Vec<String>,
    ios_info_plist_keys: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    web_csp_note: Option<String>,
}

fn emit_runtime_capabilities_projection(
    contracts_repo_root: &Path,
    packaging_dir: &Path,
    required_ids: Option<&BTreeSet<String>>,
) -> Result<Option<PathBuf>> {
    let yaml_path = contracts_repo_root.join(RUNTIME_CAPABILITIES_REL);
    if !yaml_path.is_file() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&yaml_path)
        .with_context(|| format!("read {}", yaml_path.display()))?;
    let doc: RuntimeCapabilitiesYaml =
        serde_yaml::from_str(&raw).with_context(|| format!("parse {}", yaml_path.display()))?;

    let mut tauri_all: BTreeSet<String> = BTreeSet::new();
    let mut rows: Vec<RuntimeCapabilityProjectionRow> = Vec::with_capacity(doc.capabilities.len());
    for c in &doc.capabilities {
        if required_ids.is_some_and(|ids| !ids.contains(&c.id)) {
            continue;
        }
        for p in &c.tauri_permission {
            tauri_all.insert(p.clone());
        }
        rows.push(RuntimeCapabilityProjectionRow {
            id: c.id.clone(),
            description: c.description.clone(),
            tauri_permission: c.tauri_permission.clone(),
            android_uses_permission: c.android_uses_permission.clone(),
            ios_info_plist_keys: c.ios_info_plist_keys.clone(),
            web_csp_note: c.web_csp_note.clone(),
        });
    }

    let mut allow: Vec<String> = tauri_all.into_iter().collect();
    allow.sort();

    let projection = RuntimeCapabilitiesProjection {
        schema_version: doc.schema_version.unwrap_or(1),
        source: RUNTIME_CAPABILITIES_REL.to_string(),
        capabilities: rows,
        tauri_permission_allow_list: allow,
    };

    let out_path = packaging_dir.join("runtime-capabilities.projection.json");
    let json = serde_json::to_string_pretty(&projection).context("serialize runtime-capabilities.projection.json")?;
    fs::write(&out_path, json).with_context(|| format!("write {}", out_path.display()))?;
    Ok(Some(out_path))
}

/// Writes `tauri/tauri.conf.json` plus `tauri/README.md` under `out_root`.
///
/// When `contracts_repo_root` is `Some` and contains [`RUNTIME_CAPABILITIES_REL`], also writes
/// `runtime-capabilities.projection.json` beside those files. Pass `required` to include only
/// capability rows whose ids appear in [`vox_compiler::required_capabilities::RequiredRuntimeCapabilities::capability_ids`]
/// (plus a derived `tauri_permission_allow_list`); pass `None` to mirror the full YAML contract.
///
/// Consumers run **`cargo tauri build`** from a `src-tauri` crate that includes or merges this
/// config (see README).
pub fn emit_tauri_packaging_hints(
    out_root: &Path,
    params: &TauriEmitParams<'_>,
    contracts_repo_root: Option<&Path>,
    required: Option<&vox_compiler::required_capabilities::RequiredRuntimeCapabilities>,
) -> Result<PathBuf> {
    let required_ids: Option<BTreeSet<String>> = required
        .map(|c| c.capability_ids.iter().cloned().collect());
    emit_tauri_packaging_hints_inner(
        out_root,
        params,
        contracts_repo_root,
        required_ids.as_ref(),
    )
}

fn emit_tauri_packaging_hints_inner(
    out_root: &Path,
    params: &TauriEmitParams<'_>,
    contracts_repo_root: Option<&Path>,
    required_ids: Option<&BTreeSet<String>>,
) -> Result<PathBuf> {
    let dir = out_root.join("tauri-packaging");
    fs::create_dir_all(&dir).with_context(|| format!("mkdir {}", dir.display()))?;

    let json = serialize_tauri_desktop_config(params)?;
    fs::write(dir.join("tauri.conf.json"), json).context("write tauri.conf.json")?;

    if let Some(root) = contracts_repo_root {
        let _ = emit_runtime_capabilities_projection(root, &dir, required_ids)?;
    }

    let readme = format!(
        r#"# Tauri packaging hints (Vox-generated)

This directory contains **`tauri.conf.json`** fields aligned with `Vox.toml` `[bundle]`.

When present, **`runtime-capabilities.projection.json`** is a JSON projection of
`contracts/capability/runtime-capabilities.v1.yaml` (capability ids → Tauri permission IDs,
Android `uses-permission`, iOS Info.plist keys). Merge `tauri_permission_allow_list` into your
Tauri v2 `capabilities` allowlist as needed.

Next steps:

1. Ensure the Vite/app build output exists at **{}** relative to your `src-tauri` crate (adjust `frontendDist` if needed).
2. From your app repo root, use **Tauri CLI v2** (`cargo install tauri-cli --version '^2'`) and either:
   - merge these keys into your existing `src-tauri/tauri.conf.json`, or
   - scaffold with `cargo tauri init` once and replace the generated window/product/identifier fields from this file.
3. Run **`cargo tauri build`** for platform installers (`.msi`, `.dmg`, `.AppImage`, mobile targets when configured).

Identifier: **`{}`**
Display name: **`{}`**
"#,
        params.frontend_dist_relative, params.identifier, params.display_name
    );
    fs::write(dir.join("README.md"), readme).context("write README.md")?;

    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn emits_packaging_hints() {
        let dir = tempfile::tempdir().unwrap();
        let p = TauriEmitParams {
            identifier: "com.example.app",
            display_name: "Example",
            frontend_dist_relative: "../public",
        };
        let out = emit_tauri_packaging_hints(dir.path(), &p, None, None).unwrap();
        assert!(out.join("tauri.conf.json").is_file());
        let raw = std::fs::read_to_string(out.join("tauri.conf.json")).unwrap();
        assert!(raw.contains("com.example.app"));
        assert!(raw.contains("../public"));
        assert!(raw.contains("\"label\": \"main\""));
        assert!(!out.join("runtime-capabilities.projection.json").exists());
    }

    #[test]
    fn emits_runtime_capabilities_projection_when_contract_present() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("repo");
        let cap_dir = repo.join("contracts/capability");
        fs::create_dir_all(&cap_dir).unwrap();
        fs::write(
            cap_dir.join("runtime-capabilities.v1.yaml"),
            r#"schema_version: 1
capabilities:
  - id: net.http
    description: HTTP
    tauri_permission:
      - core:default
      - http:default
    android_uses_permission: []
    ios_info_plist_keys: []
"#,
        )
        .unwrap();

        let p = TauriEmitParams {
            identifier: "com.example.app",
            display_name: "Example",
            frontend_dist_relative: "../public",
        };
        let out = emit_tauri_packaging_hints(tmp.path(), &p, Some(&repo), None).unwrap();
        let proj = out.join("runtime-capabilities.projection.json");
        assert!(proj.is_file(), "expected projection at {}", proj.display());
        let body: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&proj).unwrap()).unwrap();
        assert_eq!(body["schema_version"], 1);
        assert!(body["tauri_permission_allow_list"].as_array().unwrap().len() >= 2);
    }

    #[test]
    fn filters_runtime_capabilities_projection_to_required_ids() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("repo");
        let cap_dir = repo.join("contracts/capability");
        fs::create_dir_all(&cap_dir).unwrap();
        fs::write(
            cap_dir.join("runtime-capabilities.v1.yaml"),
            r#"schema_version: 1
capabilities:
  - id: net.http
    description: HTTP
    tauri_permission:
      - http:default
    android_uses_permission:
      - android.permission.INTERNET
    ios_info_plist_keys: []
  - id: notifications
    description: Push notifications
    tauri_permission:
      - notification:default
    android_uses_permission:
      - android.permission.POST_NOTIFICATIONS
    ios_info_plist_keys:
      - NSUserNotificationsUsageDescription
"#,
        )
        .unwrap();

        let p = TauriEmitParams {
            identifier: "com.example.app",
            display_name: "Example",
            frontend_dist_relative: "../public",
        };
        let required = vox_compiler::required_capabilities::RequiredRuntimeCapabilities {
            schema_version: vox_compiler::required_capabilities::REQUIRED_CAPABILITIES_SCHEMA_VERSION,
            capability_ids: vec!["net.http".to_string()],
        };
        let out =
            emit_tauri_packaging_hints(tmp.path(), &p, Some(&repo), Some(&required)).unwrap();
        let proj = out.join("runtime-capabilities.projection.json");
        let body: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&proj).unwrap()).unwrap();
        let caps = body["capabilities"].as_array().unwrap();
        assert_eq!(caps.len(), 1);
        assert_eq!(caps[0]["id"], "net.http");
        assert_eq!(
            body["tauri_permission_allow_list"],
            serde_json::json!(["http:default"])
        );
        assert!(!body.to_string().contains("notification:default"));
    }

    #[test]
    fn find_contracts_repo_root_walks_up() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("monorepo");
        let cap = repo.join("contracts/capability");
        fs::create_dir_all(&cap).unwrap();
        fs::write(cap.join("runtime-capabilities.v1.yaml"), "capabilities: []\n").unwrap();
        let nested = repo.join("packages/app");
        fs::create_dir_all(&nested).unwrap();
        let got = find_contracts_repo_root(&nested).expect("root");
        assert_eq!(got, repo);
    }
}
