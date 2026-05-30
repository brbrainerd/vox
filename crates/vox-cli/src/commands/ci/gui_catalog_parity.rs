use anyhow::{Context, Result};
use serde_json::json;
use serde_yaml::Value;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Default)]
struct OperationMeta {
    id: String,
    title: Option<String>,
    description: Option<String>,
    status: Option<String>,
    product_lane: Option<String>,
    side_effect_class: Option<String>,
    feature_gate: Option<String>,
    scope_kind: Option<String>,
    reversible: Option<bool>,
    requires_repo: Option<bool>,
    capability_id: Option<String>,
    cli_path: Option<Vec<String>>,
    mcp_name: Option<String>,
}

fn read_operation_catalog(repo_root: &PathBuf) -> Result<Vec<OperationMeta>> {
    let catalog_path = repo_root.join("contracts/operations/catalog.v1.yaml");
    let raw = fs::read_to_string(&catalog_path).context("read operations catalog")?;
    let parsed: Value = serde_yaml::from_str(&raw).context("parse operations catalog")?;
    let mut out = Vec::new();
    let Some(ops) = parsed.get("operations").and_then(Value::as_sequence) else {
        return Ok(out);
    };
    for op in ops {
        let id = op
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if id.is_empty() {
            continue;
        }
        let cli_path = op
            .get("cli")
            .and_then(Value::as_mapping)
            .and_then(|cli| cli.get(&Value::String("path".to_string())))
            .and_then(Value::as_sequence)
            .map(|parts| {
                parts
                    .iter()
                    .filter_map(Value::as_str)
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
            })
            .filter(|v| !v.is_empty());
        let status = op
            .get("cli")
            .and_then(Value::as_mapping)
            .and_then(|cli| cli.get(&Value::String("status".to_string())))
            .and_then(Value::as_str)
            .map(ToString::to_string);
        let mcp_name = op
            .get("mcp")
            .and_then(Value::as_mapping)
            .and_then(|mcp| mcp.get(&Value::String("name".to_string())))
            .and_then(Value::as_str)
            .map(ToString::to_string);
        out.push(OperationMeta {
            id,
            title: op
                .get("title")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            description: op
                .get("description")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            status,
            product_lane: op
                .get("product_lane")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            side_effect_class: op
                .get("side_effect_class")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            feature_gate: op
                .get("cli")
                .and_then(Value::as_mapping)
                .and_then(|cli| cli.get(&Value::String("feature_gate".to_string())))
                .and_then(Value::as_str)
                .map(ToString::to_string),
            scope_kind: op
                .get("scope_kind")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            reversible: op.get("reversible").and_then(Value::as_bool),
            requires_repo: op.get("requires_repo").and_then(Value::as_bool),
            capability_id: op
                .get("capability_id")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            cli_path,
            mcp_name,
        });
    }
    Ok(out)
}

fn to_safety_class(side_effect_class: Option<&str>) -> &'static str {
    match side_effect_class {
        Some("none") | Some("read_only") => "read_only",
        Some("destructive") => "destructive",
        Some(_) => "mutating",
        None => "unknown",
    }
}

fn to_output_kind(path: &[String]) -> &'static str {
    if path.first().is_some_and(|p| p == "commands") {
        "json"
    } else if path.first().is_some_and(|p| p == "ci") {
        "text"
    } else {
        "mixed"
    }
}

fn confirmation_policy_from_safety(safety: &str) -> &'static str {
    match safety {
        "destructive" => "required",
        "mutating" => "recommended",
        _ => "none",
    }
}

fn generated_manifest_payload(repo_root: &PathBuf) -> Result<serde_json::Value> {
    let catalog = crate::command_catalog::build_catalog();
    let operations = read_operation_catalog(repo_root)?;
    let mut by_cli_path: HashMap<String, OperationMeta> = HashMap::new();
    let mut mcp_only = Vec::new();
    for op in operations {
        if let Some(path) = &op.cli_path {
            by_cli_path.insert(path.join(" "), op);
        } else if op.mcp_name.is_some() {
            mcp_only.push(op);
        }
    }

    let mut actions = Vec::new();
    for cmd in catalog.entries {
        let key = cmd.path.join(" ");
        let op = by_cli_path.get(&key);
        let safety_class = to_safety_class(op.and_then(|m| m.side_effect_class.as_deref()));
        actions.push(json!({
            "id": op.map(|m| m.id.clone()).unwrap_or_else(|| key.replace(' ', ".")),
            "title": op.and_then(|m| m.title.clone()).unwrap_or_else(|| format!("vox {}", key)),
            "description": op.and_then(|m| m.description.clone()).unwrap_or_else(|| cmd.about.clone()),
            "handler_kind": "cli",
            "cli_path": cmd.path,
            "mcp_name": op.and_then(|m| m.mcp_name.clone()),
            "command": cmd.command,
            "safety_class": safety_class,
            "feature_gate": op.and_then(|m| m.feature_gate.clone()),
            "capability_id": op.and_then(|m| m.capability_id.clone()).unwrap_or_else(|| key.replace(' ', ".")),
            "scope_kind": op.and_then(|m| m.scope_kind.clone()).unwrap_or_else(|| "workspace".to_string()),
            "requires_repo": op.and_then(|m| m.requires_repo).unwrap_or(false),
            "reversible": op.and_then(|m| m.reversible).unwrap_or(false),
            "confirmation_policy": confirmation_policy_from_safety(safety_class),
            "execution_mode": "sync",
            "output_kind": to_output_kind(&cmd.path),
            "status": op.and_then(|m| m.status.clone()).unwrap_or_else(|| "active".to_string()),
            "product_lane": op.and_then(|m| m.product_lane.clone()),
            "platform": { "desktop": true, "mobile": true },
            "arguments": cmd.arguments,
        }));
    }

    for op in mcp_only {
        let safety_class = to_safety_class(op.side_effect_class.as_deref());
        actions.push(json!({
            "id": op.id,
            "title": op.title.unwrap_or_else(|| "MCP operation".to_string()),
            "description": op.description.unwrap_or_else(|| "MCP-backed operation".to_string()),
            "handler_kind": "mcp",
            "mcp_name": op.mcp_name,
            "safety_class": safety_class,
            "feature_gate": op.feature_gate,
            "capability_id": op.capability_id.unwrap_or_else(|| "mcp.capability.unknown".to_string()),
            "scope_kind": op.scope_kind.unwrap_or_else(|| "workspace".to_string()),
            "requires_repo": op.requires_repo.unwrap_or(false),
            "reversible": op.reversible.unwrap_or(false),
            "confirmation_policy": confirmation_policy_from_safety(safety_class),
            "execution_mode": "async",
            "output_kind": "json",
            "status": op.status.unwrap_or_else(|| "active".to_string()),
            "product_lane": op.product_lane,
            "platform": { "desktop": true, "mobile": false },
            "arguments": [{
                "name": "payload",
                "short": null,
                "long": "payload",
                "help": "JSON payload for MCP tool invocation",
                "required": false,
                "takes_value": true
            }],
        }));
    }
    actions.sort_by_key(|a| {
        a.get("id")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string()
    });
    Ok(json!({
        "x_vox_version": 2,
        "schema_version": 1,
        "generated_from": "clap_catalog+operations_catalog",
        "actions": actions,
    }))
}

pub fn run(repo_root: &PathBuf) -> Result<()> {
    tracing::info!("Running gui-catalog-parity check...");
    crate::commands::ci::gui_version_sync::run(repo_root, false)?;

    let catalog = crate::command_catalog::build_catalog();
    if catalog.entries.is_empty() {
        anyhow::bail!("CommandCatalog has zero entries");
    }

    for entry in &catalog.entries {
        if entry.path.is_empty() {
            anyhow::bail!("CommandCatalog contains entry with empty path");
        }
        if entry.about == "(no description)" {
            anyhow::bail!(
                "Command 'vox {}' has placeholder about string '(no description)'. All commands must have meaningful descriptions.",
                entry.path.join(" ")
            );
        }
    }

    let ts_path = repo_root.join("crates/vox-gui/ui/src/types/catalog.ts");
    if !ts_path.exists() {
        anyhow::bail!("TypeScript catalog types file missing at: {:?}", ts_path);
    }
    let ts_content = fs::read_to_string(&ts_path).context("Failed to read catalog.ts")?;
    if !ts_content.contains("CommandCatalogEntry") {
        anyhow::bail!("CommandCatalogEntry missing from catalog.ts");
    }
    let action_manifest_path = repo_root.join("contracts/gui/action-manifest.v1.yaml");
    if !action_manifest_path.exists() {
        anyhow::bail!(
            "GUI action manifest contract missing at: {:?}",
            action_manifest_path
        );
    }
    let action_manifest = fs::read_to_string(&action_manifest_path)
        .context("Failed to read action-manifest.v1.yaml")?;
    if !action_manifest.contains("handler_kind") {
        anyhow::bail!("action-manifest.v1.yaml must define handler_kind");
    }
    if !action_manifest.contains("schema_version: 1") {
        anyhow::bail!("action-manifest.v1.yaml must declare schema_version: 1");
    }
    let action_manifest_schema_path =
        repo_root.join("contracts/gui/action-manifest.v1.schema.json");
    if !action_manifest_schema_path.exists() {
        anyhow::bail!(
            "GUI action manifest JSON schema missing at: {:?}",
            action_manifest_schema_path
        );
    }
    let schema_raw = fs::read_to_string(&action_manifest_schema_path)
        .context("Failed to read action-manifest.v1.schema.json")?;
    let schema_val: serde_json::Value = serde_json::from_str(&schema_raw)
        .context("Failed to parse action-manifest.v1.schema.json")?;
    let validator =
        vox_jsonschema_util::compile_validator(&schema_val, action_manifest_schema_path.display())
            .context("compile action-manifest schema")?;
    let generated_manifest = generated_manifest_payload(repo_root)?;
    vox_jsonschema_util::validate(
        &generated_manifest,
        &validator,
        "gui action manifest schema",
    )
    .context("validate generated action-manifest against schema")?;

    let runtime_types_path = repo_root.join("clients/runtime-types/src/index.ts");
    if runtime_types_path.exists() {
        let runtime_types = fs::read_to_string(&runtime_types_path)
            .context("Failed to read runtime-types index.ts")?;
        if runtime_types.contains("@tauri-apps/api") {
            anyhow::bail!(
                "runtime boundary guard failed: clients/runtime-types must not import @tauri-apps/api"
            );
        }
    }

    // Dead-surface guard: every command module file should be exported by commands/mod.rs.
    let command_dir = repo_root.join("crates/vox-gui/src/commands");
    let mod_rs_path = command_dir.join("mod.rs");
    let mod_rs = fs::read_to_string(&mod_rs_path).context("Failed to read commands/mod.rs")?;
    for entry in fs::read_dir(&command_dir).context("Failed to read commands dir")? {
        let entry = entry.context("commands dir entry error")?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !name.ends_with(".rs") || name == "mod.rs" {
            continue;
        }
        let module = name.trim_end_matches(".rs");
        let expected = format!("pub mod {module};");
        if !mod_rs.contains(&expected) {
            anyhow::bail!(
                "Dead-surface guard failed: commands/{name} exists but `{expected}` is missing from commands/mod.rs"
            );
        }
    }

    // Coverage hardening guard: metadata fields must be present in generated entries.
    let Some(actions) = generated_manifest.get("actions").and_then(|v| v.as_array()) else {
        anyhow::bail!("generated action-manifest missing `actions` array");
    };
    for action in actions {
        let id = action
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("<unknown>");
        for required in [
            "safety_class",
            "capability_id",
            "scope_kind",
            "requires_repo",
            "reversible",
            "confirmation_policy",
            "execution_mode",
            "output_kind",
            "platform",
        ] {
            if action.get(required).is_none() || action.get(required).is_some_and(|v| v.is_null()) {
                anyhow::bail!(
                    "metadata hardening guard failed: action `{id}` missing `{required}`"
                );
            }
        }
    }

    tracing::info!("gui-catalog-parity check passed.");
    Ok(())
}
