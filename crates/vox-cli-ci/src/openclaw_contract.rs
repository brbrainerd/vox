//! `vox ci openclaw-contract` — validate OpenClaw protocol and discovery fixture shapes.

use anyhow::{Result, anyhow};
use serde_json::Value;
use std::path::Path;

const PROTOCOL_FIXTURE_DIR: &str = "contracts/openclaw/protocol";
const DISCOVERY_FIXTURE_DIR: &str = "contracts/openclaw/discovery";
const REQUIRED_PROTOCOL_FIXTURES: &[&str] = &[
    "connect.challenge.json",
    "connect.request.operator.json",
    "connect.hello-ok.json",
    "subscriptions.list.response.json",
];
const REQUIRED_DISCOVERY_FIXTURES: &[&str] =
    &["well-known.response.json", "well-known.minimal.json"];

/// Validate committed OpenClaw contract fixtures under `contracts/openclaw/`.
pub fn run(root: &Path) -> Result<()> {
    let protocol_base = root.join(PROTOCOL_FIXTURE_DIR);
    for fixture in REQUIRED_PROTOCOL_FIXTURES {
        let path = protocol_base.join(fixture);
        if !path.is_file() {
            return Err(anyhow!("missing OpenClaw fixture: {}", path.display()));
        }
        let raw = std::fs::read_to_string(&path)?;
        let json: Value = serde_json::from_str(&raw)
            .map_err(|e| anyhow!("invalid JSON fixture {}: {e}", path.display()))?;
        validate_fixture_shape(fixture, &json)?;
    }

    let discovery_base = root.join(DISCOVERY_FIXTURE_DIR);
    for fixture in REQUIRED_DISCOVERY_FIXTURES {
        let path = discovery_base.join(fixture);
        if !path.is_file() {
            return Err(anyhow!(
                "missing OpenClaw discovery fixture: {}",
                path.display()
            ));
        }
        let raw = std::fs::read_to_string(&path)?;
        let json: Value = serde_json::from_str(&raw)
            .map_err(|e| anyhow!("invalid JSON discovery fixture {}: {e}", path.display()))?;
        validate_discovery_fixture_shape(fixture, &json)?;
    }
    println!(
        "OpenClaw contract fixtures validated ({} files).",
        REQUIRED_PROTOCOL_FIXTURES.len() + REQUIRED_DISCOVERY_FIXTURES.len()
    );
    Ok(())
}

fn validate_fixture_shape(name: &str, value: &Value) -> Result<()> {
    let frame_type = value
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("{name}: missing string field `type`"))?;
    match name {
        "connect.challenge.json" => {
            if frame_type != "event" {
                return Err(anyhow!("{name}: expected type=event"));
            }
            let event = value
                .get("event")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("{name}: missing event"))?;
            if event != "connect.challenge" {
                return Err(anyhow!("{name}: expected event=connect.challenge"));
            }
        }
        "connect.request.operator.json" => {
            if frame_type != "req" {
                return Err(anyhow!("{name}: expected type=req"));
            }
            let method = value
                .get("method")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("{name}: missing method"))?;
            if method != "connect" {
                return Err(anyhow!("{name}: expected method=connect"));
            }
            let params = value
                .get("params")
                .ok_or_else(|| anyhow!("{name}: missing params object"))?;
            let min_protocol = params
                .get("minProtocol")
                .and_then(Value::as_u64)
                .ok_or_else(|| anyhow!("{name}: missing minProtocol"))?;
            let max_protocol = params
                .get("maxProtocol")
                .and_then(Value::as_u64)
                .ok_or_else(|| anyhow!("{name}: missing maxProtocol"))?;
            if min_protocol == 0 || max_protocol == 0 || min_protocol > max_protocol {
                return Err(anyhow!("{name}: invalid protocol bounds"));
            }
            let role = params
                .get("role")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("{name}: missing role"))?;
            if role != "operator" && role != "node" {
                return Err(anyhow!("{name}: unexpected role `{role}`"));
            }
        }
        "connect.hello-ok.json" | "subscriptions.list.response.json" => {
            if frame_type != "res" {
                return Err(anyhow!("{name}: expected type=res"));
            }
            let ok = value
                .get("ok")
                .and_then(Value::as_bool)
                .ok_or_else(|| anyhow!("{name}: missing boolean ok"))?;
            if !ok {
                return Err(anyhow!("{name}: expected ok=true"));
            }
            if name == "connect.hello-ok.json" {
                let payload_type = value
                    .get("payload")
                    .and_then(|p| p.get("type"))
                    .and_then(Value::as_str)
                    .ok_or_else(|| anyhow!("{name}: missing payload.type"))?;
                if payload_type != "hello-ok" {
                    return Err(anyhow!("{name}: expected payload.type=hello-ok"));
                }
            }
            if name == "subscriptions.list.response.json" {
                let subscriptions = value
                    .get("payload")
                    .and_then(|p| p.get("subscriptions"))
                    .and_then(Value::as_array)
                    .ok_or_else(|| anyhow!("{name}: missing payload.subscriptions array"))?;
                for (idx, entry) in subscriptions.iter().enumerate() {
                    let domain = entry
                        .get("domain")
                        .and_then(Value::as_str)
                        .ok_or_else(|| anyhow!("{name}: subscriptions[{idx}] missing domain"))?;
                    if domain.trim().is_empty() {
                        return Err(anyhow!("{name}: subscriptions[{idx}] empty domain"));
                    }
                    entry
                        .get("count")
                        .and_then(Value::as_u64)
                        .ok_or_else(|| anyhow!("{name}: subscriptions[{idx}] missing count"))?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_discovery_fixture_shape(name: &str, value: &Value) -> Result<()> {
    let gateway = value
        .get("gateway")
        .ok_or_else(|| anyhow!("{name}: missing gateway object"))?;
    let http_url = gateway
        .get("httpUrl")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("{name}: missing gateway.httpUrl"))?;
    let ws_url = gateway
        .get("wsUrl")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("{name}: missing gateway.wsUrl"))?;
    validate_scheme(name, "gateway.httpUrl", http_url, &["http://", "https://"])?;
    validate_scheme(name, "gateway.wsUrl", ws_url, &["ws://", "wss://"])?;

    if let Some(catalog) = value.get("catalog") {
        if let Some(list_url) = catalog.get("listUrl").and_then(Value::as_str) {
            validate_scheme(name, "catalog.listUrl", list_url, &["http://", "https://"])?;
        }
        if let Some(search_url) = catalog.get("searchUrl").and_then(Value::as_str) {
            validate_scheme(
                name,
                "catalog.searchUrl",
                search_url,
                &["http://", "https://"],
            )?;
        }
    }

    if let Some(protocol) = value.get("protocol") {
        let min_protocol = protocol
            .get("minVersion")
            .and_then(Value::as_u64)
            .ok_or_else(|| anyhow!("{name}: protocol.minVersion must be an integer"))?;
        let max_protocol = protocol
            .get("maxVersion")
            .and_then(Value::as_u64)
            .ok_or_else(|| anyhow!("{name}: protocol.maxVersion must be an integer"))?;
        if min_protocol == 0 || max_protocol == 0 || min_protocol > max_protocol {
            return Err(anyhow!("{name}: invalid protocol bounds"));
        }
    }

    let ttl_seconds = value
        .get("cacheTtlSeconds")
        .and_then(Value::as_u64)
        .ok_or_else(|| anyhow!("{name}: missing cacheTtlSeconds integer"))?;
    if !(30..=86_400).contains(&ttl_seconds) {
        return Err(anyhow!(
            "{name}: cacheTtlSeconds out of range (expected 30..=86400)"
        ));
    }
    Ok(())
}

fn validate_scheme(name: &str, field: &str, value: &str, allowed_prefixes: &[&str]) -> Result<()> {
    if allowed_prefixes
        .iter()
        .any(|prefix| value.starts_with(prefix))
    {
        return Ok(());
    }
    Err(anyhow!(
        "{name}: invalid {field} `{value}` (expected prefixes: {})",
        allowed_prefixes.join(", ")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn validate_fixture_shape_accepts_connect_challenge() {
        let value = json!({
            "type": "event",
            "event": "connect.challenge"
        });
        validate_fixture_shape("connect.challenge.json", &value).unwrap();
    }

    #[test]
    fn validate_fixture_shape_rejects_wrong_event_type() {
        let value = json!({
            "type": "req",
            "event": "connect.challenge"
        });
        let err = validate_fixture_shape("connect.challenge.json", &value).unwrap_err();
        assert!(err.to_string().contains("expected type=event"));
    }

    #[test]
    fn validate_discovery_accepts_minimal_well_known() {
        let value = json!({
            "gateway": {
                "httpUrl": "https://gateway.example/openclaw",
                "wsUrl": "wss://gateway.example/openclaw/ws"
            },
            "cacheTtlSeconds": 300
        });
        validate_discovery_fixture_shape("well-known.minimal.json", &value).unwrap();
    }

    #[test]
    fn validate_discovery_rejects_invalid_ws_scheme() {
        let value = json!({
            "gateway": {
                "httpUrl": "https://gateway.example/openclaw",
                "wsUrl": "http://gateway.example/ws"
            },
            "cacheTtlSeconds": 300
        });
        let err = validate_discovery_fixture_shape("well-known.minimal.json", &value).unwrap_err();
        assert!(err.to_string().contains("gateway.wsUrl"));
    }
}
