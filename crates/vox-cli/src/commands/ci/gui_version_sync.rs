use anyhow::{Context, Result, anyhow};
use serde_json::Value as JsonValue;
use std::fs;
use std::path::Path;

const GUI_TAURI_CONF: &str = "crates/vox-gui/tauri.conf.json";
const GUI_UI_PACKAGE: &str = "crates/vox-gui/ui/package.json";
const RUNTIME_TYPES_PACKAGE: &str = "clients/runtime-types/package.json";
const RUNTIME_WEB_PACKAGE: &str = "clients/runtime-web/package.json";

fn workspace_version(repo_root: &Path) -> Result<String> {
    let cargo_toml =
        fs::read_to_string(repo_root.join("Cargo.toml")).context("read root Cargo.toml")?;
    let mut in_workspace_package = false;
    for line in cargo_toml.lines() {
        let t = line.trim();
        if t.starts_with('[') && t.ends_with(']') {
            in_workspace_package = t == "[workspace.package]";
            continue;
        }
        if in_workspace_package && t.starts_with("version = \"") {
            let q1 = t.find('"').context("version quote start")?;
            let q2 = t[q1 + 1..]
                .find('"')
                .map(|i| q1 + 1 + i)
                .context("version quote end")?;
            return Ok(t[q1 + 1..q2].to_string());
        }
    }
    Err(anyhow!(
        "find [workspace.package] version in root Cargo.toml"
    ))
}

fn sync_json_version(path: &Path, expected: &str, write: bool) -> Result<()> {
    let raw = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let mut json: JsonValue =
        serde_json::from_str(&raw).with_context(|| format!("parse {}", path.display()))?;
    let current = json
        .get("version")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("{} missing string `version` field", path.display()))?
        .to_string();
    if current == expected {
        return Ok(());
    }
    if !write {
        return Err(anyhow!(
            "{} version drift: expected `{expected}` but found `{current}`",
            path.display()
        ));
    }
    if let Some(obj) = json.as_object_mut() {
        obj.insert(
            "version".to_string(),
            JsonValue::String(expected.to_string()),
        );
    }
    let serialized = serde_json::to_string_pretty(&json)
        .with_context(|| format!("serialize {}", path.display()))?;
    fs::write(path, format!("{serialized}\n")).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

fn sync_runtime_web_dep(path: &Path, expected: &str, write: bool) -> Result<()> {
    let raw = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let mut json: JsonValue =
        serde_json::from_str(&raw).with_context(|| format!("parse {}", path.display()))?;
    let dep_path = ["dependencies", "@vox/runtime-types"];
    let current = json
        .get(dep_path[0])
        .and_then(|deps| deps.get(dep_path[1]))
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("{} missing dependencies.@vox/runtime-types", path.display()))?
        .to_string();
    if current == expected {
        return Ok(());
    }
    if !write {
        return Err(anyhow!(
            "{} dependency drift: dependencies.@vox/runtime-types expected `{expected}` but found `{current}`",
            path.display()
        ));
    }
    let deps = json
        .get_mut(dep_path[0])
        .and_then(|v| v.as_object_mut())
        .ok_or_else(|| anyhow!("{} dependencies is not an object", path.display()))?;
    deps.insert(
        dep_path[1].to_string(),
        JsonValue::String(expected.to_string()),
    );
    let serialized = serde_json::to_string_pretty(&json)
        .with_context(|| format!("serialize {}", path.display()))?;
    fs::write(path, format!("{serialized}\n")).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

pub fn run(repo_root: &Path, write: bool) -> Result<()> {
    let version = workspace_version(repo_root)?;

    let tauri_conf = repo_root.join(GUI_TAURI_CONF);
    let gui_ui_pkg = repo_root.join(GUI_UI_PACKAGE);
    let runtime_types_pkg = repo_root.join(RUNTIME_TYPES_PACKAGE);
    let runtime_web_pkg = repo_root.join(RUNTIME_WEB_PACKAGE);

    sync_json_version(&tauri_conf, &version, write)?;
    sync_json_version(&gui_ui_pkg, &version, write)?;
    sync_json_version(&runtime_types_pkg, &version, write)?;
    sync_json_version(&runtime_web_pkg, &version, write)?;
    sync_runtime_web_dep(&runtime_web_pkg, &version, write)?;

    if write {
        println!("gui-version-sync: wrote GUI/runtime package versions to {version}");
    } else {
        println!("gui-version-sync: versions match workspace {version}");
    }
    Ok(())
}
