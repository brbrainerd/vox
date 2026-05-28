use serde::Serialize;
use serde_yaml::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionHandlerKind {
    Cli,
    Mcp,
    Ipc,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActionPlatform {
    pub desktop: bool,
    pub mobile: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct GuiActionEntry {
    pub id: String,
    pub title: String,
    pub description: String,
    pub handler_kind: ActionHandlerKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cli_path: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    pub safety_class: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feature_gate: Option<String>,
    pub capability_id: String,
    pub scope_kind: String,
    pub requires_repo: bool,
    pub reversible: bool,
    pub confirmation_policy: String,
    pub execution_mode: String,
    pub output_kind: String,
    pub status: String,
    pub product_lane: Option<String>,
    pub platform: ActionPlatform,
    #[serde(default)]
    pub arguments: Vec<vox_cli::command_catalog::CommandCatalogArgument>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GuiActionManifest {
    pub x_vox_version: u8,
    pub schema_version: u8,
    pub generated_from: String,
    pub actions: Vec<GuiActionEntry>,
}

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

fn read_operation_catalog() -> Result<Vec<OperationMeta>, String> {
    let repo_root = vox_repository::resolve_repo_root_for_ci();
    let catalog_path = repo_root.join("contracts/operations/catalog.v1.yaml");
    let raw = std::fs::read_to_string(&catalog_path)
        .map_err(|e| format!("Failed to read operations catalog: {e}"))?;
    let parsed: Value =
        serde_yaml::from_str(&raw).map_err(|e| format!("Failed to parse operations catalog: {e}"))?;
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
            title: op.get("title").and_then(Value::as_str).map(ToString::to_string),
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

fn to_safety_class(side_effect_class: Option<&str>) -> String {
    match side_effect_class {
        Some("none") | Some("read_only") => "read_only".to_string(),
        Some("destructive") => "destructive".to_string(),
        Some(_) => "mutating".to_string(),
        None => "unknown".to_string(),
    }
}

fn to_output_kind(path: &[String]) -> String {
    if path.first().is_some_and(|p| p == "commands") {
        "json".to_string()
    } else if path.first().is_some_and(|p| p == "ci") {
        "text".to_string()
    } else {
        "mixed".to_string()
    }
}

fn confirmation_policy_from_safety(safety: &str) -> String {
    match safety {
        "destructive" => "required".to_string(),
        "mutating" => "recommended".to_string(),
        _ => "none".to_string(),
    }
}

pub fn build_action_manifest() -> Result<GuiActionManifest, String> {
    let catalog = vox_cli::command_catalog::build_catalog();
    let operations = read_operation_catalog()?;

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
        let title = op
            .and_then(|m| m.title.clone())
            .unwrap_or_else(|| format!("vox {}", cmd.path.join(" ")));
        let description = op
            .and_then(|m| m.description.clone())
            .unwrap_or_else(|| cmd.about.clone());
        let status = op
            .and_then(|m| m.status.clone())
            .unwrap_or_else(|| "active".to_string());
        let safety_class = to_safety_class(op.and_then(|m| m.side_effect_class.as_deref()));
        actions.push(GuiActionEntry {
            id: op.map(|m| m.id.clone()).unwrap_or_else(|| key.replace(' ', ".")),
            title,
            description,
            handler_kind: ActionHandlerKind::Cli,
            cli_path: Some(cmd.path.clone()),
            mcp_name: op.and_then(|m| m.mcp_name.clone()),
            command: Some(cmd.command),
            safety_class: safety_class.clone(),
            feature_gate: op.and_then(|m| m.feature_gate.clone()),
            capability_id: op
                .and_then(|m| m.capability_id.clone())
                .unwrap_or_else(|| key.replace(' ', ".")),
            scope_kind: op
                .and_then(|m| m.scope_kind.clone())
                .unwrap_or_else(|| "workspace".to_string()),
            requires_repo: op.and_then(|m| m.requires_repo).unwrap_or(false),
            reversible: op.and_then(|m| m.reversible).unwrap_or(false),
            confirmation_policy: confirmation_policy_from_safety(&safety_class),
            execution_mode: "sync".to_string(),
            output_kind: to_output_kind(&cmd.path),
            status,
            product_lane: op.and_then(|m| m.product_lane.clone()),
            platform: ActionPlatform {
                desktop: true,
                mobile: true,
            },
            arguments: cmd.arguments,
        });
    }

    for op in mcp_only {
        let safety_class = to_safety_class(op.side_effect_class.as_deref());
        actions.push(GuiActionEntry {
            id: op.id,
            title: op.title.unwrap_or_else(|| "MCP operation".to_string()),
            description: op.description.unwrap_or_else(|| "MCP-backed operation".to_string()),
            handler_kind: ActionHandlerKind::Mcp,
            cli_path: None,
            mcp_name: op.mcp_name,
            command: None,
            safety_class: safety_class.clone(),
            feature_gate: op.feature_gate,
            capability_id: op
                .capability_id
                .unwrap_or_else(|| "mcp.capability.unknown".to_string()),
            scope_kind: op.scope_kind.unwrap_or_else(|| "workspace".to_string()),
            requires_repo: op.requires_repo.unwrap_or(false),
            reversible: op.reversible.unwrap_or(false),
            confirmation_policy: confirmation_policy_from_safety(&safety_class),
            execution_mode: "async".to_string(),
            output_kind: "json".to_string(),
            status: op.status.unwrap_or_else(|| "active".to_string()),
            product_lane: op.product_lane,
            platform: ActionPlatform {
                desktop: true,
                mobile: false,
            },
            // Operations catalog currently lacks MCP JSON schemas, so keep a generic
            // payload slot for GUI form generation rather than dropping MCP input entirely.
            arguments: vec![vox_cli::command_catalog::CommandCatalogArgument {
                name: "payload".to_string(),
                short: None,
                long: Some("payload".to_string()),
                help: Some("JSON payload for MCP tool invocation".to_string()),
                required: false,
                takes_value: true,
            }],
        });
    }

    actions.sort_by(|a, b| a.id.cmp(&b.id));

    Ok(GuiActionManifest {
        x_vox_version: 2,
        schema_version: 1,
        generated_from: "clap_catalog+operations_catalog".to_string(),
        actions,
    })
}

#[tauri::command]
pub fn get_action_manifest() -> Result<GuiActionManifest, String> {
    build_action_manifest()
}
