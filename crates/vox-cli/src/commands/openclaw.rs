//! `vox openclaw` — OpenClaw / ClawHub gateway integration, skill import, and approval management.
//!
//! **Terminology SSOT:** [`docs/src/explanation/expl-openclaw-analysis.md`](../../../../docs/src/explanation/expl-openclaw-analysis.md) (what OpenClaw and ClawHub are; Vox is a consumer, not the platform vendor).
//!
//! CLI responsibilities:
//! - Connect to a live OpenClaw or ClawHub-compatible gateway
//! - Import skills into the Vox ARS registry
//! - List and resolve pending approval requests from the approval broker

use clap::Subcommand;
use crate::utils::install_policy::OPENCLAW_SIDECAR_BIN_BASENAME;
use vox_openclaw_runtime::{
    DefaultOpenClawRuntimeAdapter, OpenClawClient, OpenClawConnectionOverrides,
    OpenClawDiscoveryOverrides, OpenClawRemoteConfig, OpenClawRuntimeAdapter,
    connect_runtime_adapter_with_overrides, resolve_openclaw_endpoints,
};
use vox_skills::new_registry_arc;

// ── Subcommand Enum ────────────────────────────────────────────────────────────

/// Subcommands for `vox openclaw`.
#[derive(Subcommand)]
pub enum OpenClawAction {
    /// Import a skill from a remote OpenClaw gateway or ClawHub registry by slug.
    ///
    /// Example: `vox openclaw import author/my-skill`
    Import {
        /// Skill slug to import (e.g. "author/my-skill")
        #[arg(required = true, value_name = "SLUG")]
        slug: String,
        /// OpenClaw gateway or ClawHub URL (overrides VOX_OPENCLAW_URL env var)
        #[arg(long, value_name = "URL")]
        gateway: Option<String>,
        /// Bearer token for authentication (overrides VOX_OPENCLAW_TOKEN env var)
        #[arg(long, value_name = "TOKEN")]
        token: Option<String>,
        /// Install the skill into the local ARS registry after importing
        #[arg(long, default_value = "true")]
        install: bool,
        /// Output result as JSON
        #[arg(long, default_value = "false")]
        json: bool,
    },
    /// List all skills available on the remote OpenClaw instance.
    ///
    /// Example: `vox openclaw list-remote`
    ListRemote {
        /// OpenClaw gateway URL (overrides VOX_OPENCLAW_URL env var)
        #[arg(long, value_name = "URL")]
        gateway: Option<String>,
        /// Bearer token for authentication (overrides VOX_OPENCLAW_TOKEN env var)
        #[arg(long, value_name = "TOKEN")]
        token: Option<String>,
        /// Output result as JSON
        #[arg(long, default_value = "false")]
        json: bool,
    },
    /// Search remote OpenClaw skills by keyword.
    ///
    /// Example: `vox openclaw search-remote compiler`
    SearchRemote {
        /// Search query matched against skill name + description.
        #[arg(required = true, value_name = "QUERY")]
        query: String,
        /// OpenClaw gateway URL (overrides VOX_OPENCLAW_URL env var)
        #[arg(long, value_name = "URL")]
        gateway: Option<String>,
        /// Bearer token for authentication (overrides VOX_OPENCLAW_TOKEN env var)
        #[arg(long, value_name = "TOKEN")]
        token: Option<String>,
        /// Output result as JSON
        #[arg(long, default_value = "false")]
        json: bool,
    },
    /// Show or validate the current OpenClaw gateway connection settings.
    ///
    /// Example: `vox openclaw config`
    Config {
        /// OpenClaw gateway URL to validate (overrides VOX_OPENCLAW_URL env var)
        #[arg(long, value_name = "URL")]
        gateway: Option<String>,
        /// OpenClaw Gateway WS URL to validate (overrides VOX_OPENCLAW_WS_URL env var)
        #[arg(long, value_name = "WS_URL")]
        ws_url: Option<String>,
        /// Output as JSON
        #[arg(long, default_value = "false")]
        json: bool,
    },
    /// List all pending tool-approval requests (approval broker).
    ///
    /// Example: `vox openclaw approvals`
    Approvals {
        /// MCP server URL to query (default: http://localhost:3847)
        #[arg(long, value_name = "URL", default_value = "http://localhost:3847")]
        mcp_url: String,
        /// Output as JSON
        #[arg(long, default_value = "false")]
        json: bool,
    },
    /// Approve a pending tool-approval request by its ID.
    ///
    /// Example: `vox openclaw approve abc-123`
    Approve {
        /// Approval ID (from `vox openclaw approvals`)
        #[arg(required = true, value_name = "APPROVAL_ID")]
        id: String,
        /// Optional reason for audit log
        #[arg(long, value_name = "REASON")]
        reason: Option<String>,
        /// MCP server URL (default: http://localhost:3847)
        #[arg(long, value_name = "URL", default_value = "http://localhost:3847")]
        mcp_url: String,
    },
    /// Deny a pending tool-approval request by its ID.
    ///
    /// Example: `vox openclaw deny abc-123`
    Deny {
        /// Approval ID (from `vox openclaw approvals`)
        #[arg(required = true, value_name = "APPROVAL_ID")]
        id: String,
        /// Optional reason for audit log
        #[arg(long, value_name = "REASON")]
        reason: Option<String>,
        /// MCP server URL (default: http://localhost:3847)
        #[arg(long, value_name = "URL", default_value = "http://localhost:3847")]
        mcp_url: String,
    },
    /// Start the OpenClaw gateway HTTP service (vox-gateway).
    ///
    /// Example: `vox openclaw serve --port 3850`
    Serve {
        /// TCP port to listen on
        #[arg(long, default_value_t = 3850)]
        port: u16,
        /// Bind address
        #[arg(long, default_value = "127.0.0.1")]
        addr: String,
    },
    /// Subscribe an agent to a semantic domain (e.g. "vox.web").
    Subscribe {
        /// Semantic domain slug
        #[arg(required = true)]
        domain: String,
        /// Gateway URL (default: http://localhost:3850)
        #[arg(long, default_value = "http://localhost:3850")]
        gateway: String,
        /// Output as JSON
        #[arg(long, default_value = "false")]
        json: bool,
    },
    /// Unsubscribe an agent from a semantic domain.
    Unsubscribe {
        /// Semantic domain slug
        #[arg(required = true)]
        domain: String,
        /// Gateway URL (default: http://localhost:3850)
        #[arg(long, default_value = "http://localhost:3850")]
        gateway: String,
        /// Output as JSON
        #[arg(long, default_value = "false")]
        json: bool,
    },
    /// List all active semantic domain subscriptions on the gateway.
    Subscriptions {
        /// Gateway URL (default: http://localhost:3850)
        #[arg(long, default_value = "http://localhost:3850")]
        gateway: String,
        /// Output as JSON
        #[arg(long, default_value = "false")]
        json: bool,
    },
    /// Trigger a domain-change notification to all subscribed agents.
    Notify {
        /// Semantic domain slug (e.g. "vox.typeck")
        #[arg(required = true)]
        domain: String,
        /// Notification message or reason
        #[arg(required = true)]
        message: String,
        /// Gateway URL (default: http://localhost:3850)
        #[arg(long, default_value = "http://localhost:3850")]
        gateway: String,
        /// Output as JSON
        #[arg(long, default_value = "false")]
        json: bool,
    },
    /// Call an arbitrary OpenClaw gateway WS method.
    GatewayCall {
        /// Gateway method name (e.g. "subscriptions.list")
        #[arg(long, required = true)]
        method: String,
        /// JSON object string passed as request params
        #[arg(long, default_value = "{}")]
        params_json: String,
        /// Gateway WS URL (default: VOX_OPENCLAW_WS_URL or ws://127.0.0.1:18789)
        #[arg(long)]
        ws_url: Option<String>,
        /// Bearer token (overrides VOX_OPENCLAW_TOKEN)
        #[arg(long)]
        token: Option<String>,
        /// Output as JSON
        #[arg(long, default_value = "true")]
        json: bool,
    },
    /// Diagnose OpenClaw connectivity and optionally auto-start sidecar binary.
    Doctor {
        /// OpenClaw gateway URL override.
        #[arg(long, value_name = "URL")]
        gateway: Option<String>,
        /// OpenClaw Gateway WS URL override.
        #[arg(long, value_name = "WS_URL")]
        ws_url: Option<String>,
        /// Bearer token for authentication (overrides VOX_OPENCLAW_TOKEN env var)
        #[arg(long, value_name = "TOKEN")]
        token: Option<String>,
        /// Attempt to auto-start sidecar binary when gateway is unreachable.
        #[arg(long, default_value = "true")]
        auto_start: bool,
        /// Output as JSON
        #[arg(long, default_value = "false")]
        json: bool,
    },
    /// Managed sidecar lifecycle commands (status/start/stop).
    Sidecar {
        /// Sidecar action.
        #[command(subcommand)]
        action: OpenClawSidecarAction,
    },
}

#[derive(Subcommand)]
pub enum OpenClawSidecarAction {
    /// Show sidecar state, pid, and liveness.
    Status {
        /// Output as JSON.
        #[arg(long, default_value = "false")]
        json: bool,
    },
    /// Start sidecar if not already running.
    Start {
        /// Output as JSON.
        #[arg(long, default_value = "false")]
        json: bool,
    },
    /// Stop sidecar when tracked PID is running.
    Stop {
        /// Output as JSON.
        #[arg(long, default_value = "false")]
        json: bool,
    },
}

// ── Dispatch ───────────────────────────────────────────────────────────────────

/// Dispatch an [`OpenClawAction`] to its implementation.
pub async fn run(action: OpenClawAction, json_output: bool) -> anyhow::Result<()> {
    match action {
        OpenClawAction::Import {
            slug,
            gateway,
            token,
            install,
            json,
        } => cmd_import(slug, gateway, token, install, json || json_output).await,
        OpenClawAction::ListRemote {
            gateway,
            token,
            json,
        } => cmd_list_remote(gateway, token, json || json_output).await,
        OpenClawAction::SearchRemote {
            query,
            gateway,
            token,
            json,
        } => cmd_search_remote(query, gateway, token, json || json_output).await,
        OpenClawAction::Config {
            gateway,
            ws_url,
            json,
        } => cmd_config(gateway, ws_url, json || json_output).await,
        OpenClawAction::Approvals { mcp_url, json } => {
            cmd_approvals(mcp_url, json || json_output).await
        }
        OpenClawAction::Approve {
            id,
            reason,
            mcp_url,
        } => cmd_resolve(id, true, reason, mcp_url).await,
        OpenClawAction::Deny {
            id,
            reason,
            mcp_url,
        } => cmd_resolve(id, false, reason, mcp_url).await,
        OpenClawAction::Serve { port, addr } => cmd_serve(port, addr).await,
        OpenClawAction::Subscribe {
            domain,
            gateway,
            json,
        } => cmd_subscribe(domain, gateway, json || json_output).await,
        OpenClawAction::Unsubscribe {
            domain,
            gateway,
            json,
        } => cmd_unsubscribe(domain, gateway, json || json_output).await,
        OpenClawAction::Subscriptions { gateway, json } => {
            cmd_subscriptions(gateway, json || json_output).await
        }
        OpenClawAction::Notify {
            domain,
            message,
            gateway,
            json,
        } => cmd_notify(domain, message, gateway, json || json_output).await,
        OpenClawAction::GatewayCall {
            method,
            params_json,
            ws_url,
            token,
            json,
        } => cmd_gateway_call(method, params_json, ws_url, token, json || json_output).await,
        OpenClawAction::Doctor {
            gateway,
            ws_url,
            token,
            auto_start,
            json,
        } => cmd_doctor(gateway, ws_url, token, auto_start, json || json_output).await,
        OpenClawAction::Sidecar { action } => match action {
            OpenClawSidecarAction::Status { json } => cmd_sidecar_status(json || json_output),
            OpenClawSidecarAction::Start { json } => cmd_sidecar_start(json || json_output),
            OpenClawSidecarAction::Stop { json } => cmd_sidecar_stop(json || json_output),
        },
    }
}

// ── Implementations ────────────────────────────────────────────────────────────

fn resolve_openclaw_token(override_token: Option<String>) -> Option<String> {
    override_token.or_else(|| {
        vox_secrets::resolve_secret(vox_secrets::SecretId::OpenClawToken)
            .expose()
            .map(std::string::ToString::to_string)
    })
}

async fn make_client(
    gateway: Option<String>,
    ws_url: Option<String>,
    token: Option<String>,
) -> Result<OpenClawClient, anyhow::Error> {
    let resolved = resolve_openclaw_endpoints(OpenClawDiscoveryOverrides {
        explicit_http_gateway_url: gateway,
        explicit_ws_gateway_url: ws_url,
        explicit_well_known_url: None,
    })
    .await;
    let auth_token = resolve_openclaw_token(token);
    let cfg = OpenClawRemoteConfig {
        gateway_url: resolved.http_gateway_url,
        auth_token,
        verify_tls: true,
    };
    OpenClawClient::new(cfg).map_err(|e| anyhow::anyhow!("OpenClaw client error: {e}"))
}

async fn make_adapter(
    gateway: Option<String>,
    ws_url: Option<String>,
    token: Option<String>,
) -> Result<DefaultOpenClawRuntimeAdapter, anyhow::Error> {
    connect_runtime_adapter_with_overrides(OpenClawConnectionOverrides {
        http_gateway_url: gateway,
        ws_gateway_url: ws_url,
        well_known_url: None,
        explicit_token: resolve_openclaw_token(token),
    })
    .await
    .map_err(|e| anyhow::anyhow!("OpenClaw adapter connect failed: {e}"))
}

async fn cmd_import(
    slug: String,
    gateway: Option<String>,
    token: Option<String>,
    install: bool,
    json: bool,
) -> anyhow::Result<()> {
    let mut adapter = make_adapter(gateway.clone(), None, token.clone()).await?;
    let skill: vox_openclaw_runtime::ArsSkill = adapter
        .import_skill(&slug)
        .await
        .map_err(|e| anyhow::anyhow!("Import failed: {e}"))?;

    if install {
        let client = make_client(gateway, None, token).await?;
        let registry = new_registry_arc();
        match client.import_and_install(&slug, &registry).await {
            Ok(result) => {
                if json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "status": "installed",
                            "skill_id": skill.id,
                            "name": skill.name,
                            "version": skill.version,
                            "result": format!("{result:?}"),
                        }))?
                    );
                } else {
                    println!(
                        "✓ Imported and installed: {} v{} (id: {})",
                        skill.name, skill.version, skill.id
                    );
                }
            }
            Err(e) => {
                if json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "status": "import_ok_install_failed",
                            "skill_id": skill.id,
                            "error": e.to_string(),
                        }))?
                    );
                } else {
                    eprintln!("⚠ Imported but install failed: {e}");
                    println!(
                        "Skill: {} v{} (id: {})",
                        skill.name, skill.version, skill.id
                    );
                }
            }
        }
    } else {
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "status": "fetched",
                    "skill_id": skill.id,
                    "name": skill.name,
                    "version": skill.version,
                }))?
            );
        } else {
            println!(
                "✓ Fetched (not installed): {} v{} (id: {})",
                skill.name, skill.version, skill.id
            );
        }
    }
    Ok(())
}

async fn cmd_list_remote(
    gateway: Option<String>,
    token: Option<String>,
    json: bool,
) -> anyhow::Result<()> {
    let mut adapter = make_adapter(gateway, None, token).await?;
    let skills: Vec<vox_openclaw_runtime::OpenClawSkillSpec> =
        adapter
            .list_remote_skills()
            .await
            .map_err(|e| anyhow::anyhow!("List failed: {e}"))?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "count": skills.len(),
                "skills": skills,
            }))?
        );
    } else {
        println!("Remote OpenClaw Skills ({} available):", skills.len());
        for s in &skills {
            println!(
                "  {:<40} v{:<10}  {}",
                s.name,
                s.version,
                s.description.as_deref().unwrap_or("")
            );
        }
    }
    Ok(())
}

async fn cmd_config(
    gateway: Option<String>,
    ws_url: Option<String>,
    json: bool,
) -> anyhow::Result<()> {
    let resolved = resolve_openclaw_endpoints(OpenClawDiscoveryOverrides {
        explicit_http_gateway_url: gateway,
        explicit_ws_gateway_url: ws_url,
        explicit_well_known_url: None,
    })
    .await;
    let token_set = vox_secrets::resolve_secret(vox_secrets::SecretId::OpenClawToken).is_present();

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "gateway_url": resolved.http_gateway_url,
                "ws_gateway_url": resolved.ws_gateway_url,
                "catalog_list_url": resolved.catalog_list_url,
                "catalog_search_url": resolved.catalog_search_url,
                "discovery_source": resolved.discovery_source,
                "token_configured": token_set,
            }))?
        );
    } else {
        println!("OpenClaw Configuration:");
        println!("  Gateway URL    : {}", resolved.http_gateway_url);
        println!("  Gateway WS URL : {}", resolved.ws_gateway_url);
        if let Some(list) = resolved.catalog_list_url {
            println!("  Catalog List   : {list}");
        }
        if let Some(search) = resolved.catalog_search_url {
            println!("  Catalog Search : {search}");
        }
        println!("  Discovery      : {}", resolved.discovery_source);
        println!(
            "  Token set      : {}",
            if token_set {
                "yes"
            } else {
                "no (set VOX_OPENCLAW_TOKEN)"
            }
        );
    }
    Ok(())
}

async fn cmd_approvals(mcp_url: String, json: bool) -> anyhow::Result<()> {
    // Call vox_approval_list via HTTP against the running MCP server.
    let url = format!("{mcp_url}/mcp");
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "vox_approval_list",
            "arguments": {}
        }
    });
    let client = vox_http_client::client_builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()?;
    let resp = client.post(&url).json(&body).send().await;
    match resp {
        Ok(r) if r.status().is_success() => {
            let text = r.text().await.unwrap_or_default();
            if json {
                println!("{text}");
            } else {
                println!("Pending Approvals:\n{text}");
            }
            Ok(())
        }
        Ok(r) => anyhow::bail!("MCP server returned HTTP {}", r.status()),
        Err(e) => {
            anyhow::bail!("Cannot reach MCP server at {mcp_url}: {e}\nIs `vox mcp-server` running?")
        }
    }
}

async fn cmd_resolve(
    id: String,
    approved: bool,
    reason: Option<String>,
    mcp_url: String,
) -> anyhow::Result<()> {
    let url = format!("{mcp_url}/mcp");
    let mut args = serde_json::json!({
        "approval_id": id,
        "approved": approved,
    });
    if let Some(ref r) = reason {
        args["reason"] = serde_json::Value::String(r.clone());
    }
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "vox_approval_resolve",
            "arguments": args
        }
    });
    let client = vox_http_client::client_builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()?;
    match client.post(&url).json(&body).send().await {
        Ok(r) if r.status().is_success() => {
            let verb = if approved { "Approved" } else { "Denied" };
            println!("✓ {verb} approval request: {id}");
            Ok(())
        }
        Ok(r) => anyhow::bail!("MCP server returned HTTP {}", r.status()),
        Err(e) => anyhow::bail!("Cannot reach MCP server at {mcp_url}: {e}"),
    }
}

async fn cmd_serve(port: u16, addr: String) -> anyhow::Result<()> {
    let full_addr = format!("{addr}:{port}");
    println!("Starting vox-gateway on http://{full_addr}...");
    let gateway_path = crate::process_supervision::resolve_managed_binary_path("vox-gateway");

    // Launch the vox-gateway binary since it's a separate crate.
    let mut child = std::process::Command::new(&gateway_path)
        .env("VOX_GATEWAY_ADDR", &full_addr)
        .spawn()
        .map_err(|e| anyhow::anyhow!("Failed to launch {}. Error: {e}", gateway_path.display()))?;

    let status = child.wait()?;
    if !status.success() {
        anyhow::bail!("vox-gateway exited with error: {status}");
    }
    Ok(())
}

async fn cmd_subscribe(domain: String, gateway: String, json_output: bool) -> anyhow::Result<()> {
    let mut adapter = make_adapter(Some(gateway), None, None).await?;
    let payload = adapter
        .subscribe_domain(&domain)
        .await
        .map_err(|e| anyhow::anyhow!("Subscribe failed: {e}"))?;
    if json_output {
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        println!(
            "✓ Subscribed to domain: {domain}\n{}",
            serde_json::to_string_pretty(&payload)?
        );
    }
    Ok(())
}

async fn cmd_unsubscribe(domain: String, gateway: String, json_output: bool) -> anyhow::Result<()> {
    let mut adapter = make_adapter(Some(gateway), None, None).await?;
    let payload = adapter
        .unsubscribe_domain(&domain)
        .await
        .map_err(|e| anyhow::anyhow!("Unsubscribe failed: {e}"))?;
    if json_output {
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        println!(
            "✓ Unsubscribed from domain: {domain}\n{}",
            serde_json::to_string_pretty(&payload)?
        );
    }
    Ok(())
}

async fn cmd_subscriptions(gateway: String, json: bool) -> anyhow::Result<()> {
    let mut adapter = make_adapter(Some(gateway), None, None).await?;
    let payload = adapter
        .list_subscriptions()
        .await
        .map_err(|e| anyhow::anyhow!("List subscriptions failed: {e}"))?;
    if json {
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        println!(
            "Active Semantic Subscriptions:\n{}",
            serde_json::to_string_pretty(&payload)?
        );
    }
    Ok(())
}

async fn cmd_notify(
    domain: String,
    message: String,
    gateway: String,
    json_output: bool,
) -> anyhow::Result<()> {
    let mut adapter = make_adapter(Some(gateway), None, None).await?;
    let payload = adapter
        .notify_domain(&domain, &message)
        .await
        .map_err(|e| anyhow::anyhow!("Notify failed: {e}"))?;
    if json_output {
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        println!(
            "✓ Notified agents in domain: {domain}\n{}",
            serde_json::to_string_pretty(&payload)?
        );
    }
    Ok(())
}

async fn cmd_gateway_call(
    method: String,
    params_json: String,
    ws_url: Option<String>,
    token: Option<String>,
    json_output: bool,
) -> anyhow::Result<()> {
    let params: serde_json::Value = serde_json::from_str(&params_json)
        .map_err(|e| anyhow::anyhow!("Invalid --params-json payload: {e}"))?;
    let mut adapter = make_adapter(None, ws_url, token).await?;
    let payload = adapter
        .gateway_call(&method, params)
        .await
        .map_err(|e| anyhow::anyhow!("Gateway call failed: {e}"))?;
    if json_output {
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        println!(
            "Gateway call `{method}` result:\n{}",
            serde_json::to_string_pretty(&payload)?
        );
    }
    Ok(())
}

async fn cmd_search_remote(
    query: String,
    gateway: Option<String>,
    token: Option<String>,
    json: bool,
) -> anyhow::Result<()> {
    let mut adapter = make_adapter(gateway, None, token).await?;
    let skills: Vec<vox_openclaw_runtime::OpenClawSkillSpec> =
        adapter
            .list_remote_skills()
            .await
            .map_err(|e| anyhow::anyhow!("Search failed: {e}"))?;
    let q = query.to_lowercase();
    let matches: Vec<vox_openclaw_runtime::OpenClawSkillSpec> = skills
        .into_iter()
        .filter(|s| {
            s.name.to_lowercase().contains(&q)
                || s.description
                    .as_deref()
                    .unwrap_or_default()
                    .to_lowercase()
                    .contains(&q)
        })
        .collect();
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "query": query,
                "count": matches.len(),
                "skills": matches,
            }))?
        );
    } else {
        println!(
            "Remote OpenClaw Search `{query}` ({} matches):",
            matches.len()
        );
        for s in &matches {
            println!(
                "  {:<40} v{:<10}  {}",
                s.name,
                s.version,
                s.description.as_deref().unwrap_or("")
            );
        }
    }
    Ok(())
}

async fn cmd_doctor(
    gateway: Option<String>,
    ws_url: Option<String>,
    token: Option<String>,
    auto_start: bool,
    json: bool,
) -> anyhow::Result<()> {
    let resolved = resolve_openclaw_endpoints(OpenClawDiscoveryOverrides {
        explicit_http_gateway_url: gateway.clone(),
        explicit_ws_gateway_url: ws_url.clone(),
        explicit_well_known_url: None,
    })
    .await;
    let resolved_token = resolve_openclaw_token(token);
    let token_set = resolved_token.is_some();
    let http_probe_url = format!(
        "{}/v1/skills",
        resolved.http_gateway_url.trim_end_matches('/')
    );
    let http_status = vox_http_client::client_builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()?
        .get(&http_probe_url)
        .send()
        .await
        .ok()
        .map(|r| r.status().as_u16());
    let http_ok = http_status.is_some();

    let mut ws_ok = make_adapter(gateway.clone(), ws_url.clone(), resolved_token.clone())
        .await
        .is_ok();
    let mut sidecar_started = false;
    let mut sidecar_running_prior = false;
    let mut sidecar_spawn_error: Option<String> = None;
    let mut sidecar_start_attempts: u32 = 0;
    let mut sidecar_pid: Option<u32> = None;
    let mut sidecar_state_file: Option<String> = None;
    let sidecar_base = OPENCLAW_SIDECAR_BIN_BASENAME;
    let sidecar_path = crate::process_supervision::resolve_managed_binary_path(sidecar_base);
    let sidecar_bin = sidecar_path.display().to_string();
    let expected_sidecar_version = std::env::var("VOX_OPENCLAW_SIDECAR_EXPECT_VERSION")
        .ok()
        .and_then(|v| {
            let trimmed = v.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        });
    let detected_sidecar_version = crate::process_supervision::probe_binary_version(sidecar_base);
    let sidecar_version_match = match (&expected_sidecar_version, &detected_sidecar_version) {
        (Some(expected), Some(found)) => found.contains(expected),
        (Some(_), None) => false,
        (None, _) => true,
    };
    if !ws_ok && auto_start {
        let max_attempts = read_u32_env("VOX_OPENCLAW_SIDECAR_START_MAX_ATTEMPTS", 3).clamp(1, 10);
        let mut backoff_ms =
            read_u64_env("VOX_OPENCLAW_SIDECAR_START_BACKOFF_MS", 500).clamp(100, 30_000);
        match crate::process_supervision::ensure_managed_process_running(sidecar_base, &[]) {
            Ok(info) => {
                sidecar_pid = Some(info.pid);
                sidecar_state_file = Some(info.state_file.display().to_string());
                sidecar_started = info.started_now;
                sidecar_running_prior = !info.started_now;
                for attempt in 1..=max_attempts {
                    sidecar_start_attempts = attempt;
                    ws_ok = make_adapter(gateway.clone(), ws_url.clone(), resolved_token.clone())
                        .await
                        .is_ok();
                    if ws_ok {
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
                    backoff_ms = vox_foundation::primitives::backoff::next_backoff_ms_double_clamped(
                        backoff_ms, 100, 30_000,
                    );
                }
            }
            Err(err) => {
                sidecar_spawn_error = Some(err.to_string());
            }
        }
    }

    let ready = http_ok && ws_ok;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "ready": ready,
                "http_ok": http_ok,
                "http_status": http_status,
                "ws_ok": ws_ok,
                "token_configured": token_set,
                "sidecar_auto_start_attempted": auto_start,
                "sidecar_started": sidecar_started,
                "sidecar_running_prior": sidecar_running_prior,
                "sidecar_start_attempts": sidecar_start_attempts,
                "sidecar_binary": sidecar_bin,
                "sidecar_pid": sidecar_pid,
                "sidecar_state_file": sidecar_state_file,
                "sidecar_spawn_error": sidecar_spawn_error,
                "sidecar_version": detected_sidecar_version,
                "sidecar_expected_version": expected_sidecar_version,
                "sidecar_version_match": sidecar_version_match,
                "discovery_source": resolved.discovery_source,
                "http_gateway_url": resolved.http_gateway_url,
                "ws_gateway_url": resolved.ws_gateway_url,
                "catalog_list_url": resolved.catalog_list_url,
                "catalog_search_url": resolved.catalog_search_url,
            }))?
        );
    } else {
        println!("OpenClaw Doctor:");
        println!("  Ready          : {}", if ready { "yes" } else { "no" });
        println!("  HTTP reachable : {}", if http_ok { "yes" } else { "no" });
        if let Some(code) = http_status {
            println!("  HTTP status    : {code}");
        }
        println!("  WS reachable   : {}", if ws_ok { "yes" } else { "no" });
        println!(
            "  Token set      : {}",
            if token_set { "yes" } else { "no" }
        );
        println!("  Sidecar bin    : {sidecar_bin}");
        if let Some(found) = detected_sidecar_version {
            println!("  Sidecar version: {found}");
        }
        if let Some(expected) = expected_sidecar_version {
            println!(
                "  Sidecar target : {} ({})",
                expected,
                if sidecar_version_match {
                    "match"
                } else {
                    "mismatch"
                }
            );
        }
        if auto_start && sidecar_started {
            println!("  Sidecar start  : started");
        }
        if auto_start && sidecar_running_prior {
            println!("  Sidecar start  : already-running");
        }
        if auto_start && sidecar_start_attempts > 0 {
            println!("  Start attempts : {sidecar_start_attempts}");
        }
        if let Some(pid) = sidecar_pid {
            println!("  Sidecar pid    : {pid}");
        }
        if let Some(path) = sidecar_state_file {
            println!("  Sidecar state  : {path}");
        }
        if let Some(err) = sidecar_spawn_error {
            println!("  Sidecar error  : {err}");
        }
        println!("  Discovery      : {}", resolved.discovery_source);
    }
    Ok(())
}

fn read_u32_env(name: &str, default: u32) -> u32 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.trim().parse::<u32>().ok())
        .unwrap_or(default)
}

fn read_u64_env(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .unwrap_or(default)
}

fn cmd_sidecar_status(json: bool) -> anyhow::Result<()> {
    let status = crate::process_supervision::managed_process_status(OPENCLAW_SIDECAR_BIN_BASENAME);
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "process_name": status.process_name,
                "pid": status.pid,
                "running": status.running,
                "stale_state": status.stale_state,
                "state_file": status.state_file,
                "binary_path": status.binary_path,
            }))?
        );
    } else {
        println!("OpenClaw Sidecar Status:");
        println!(
            "  Running       : {}",
            if status.running { "yes" } else { "no" }
        );
        if let Some(pid) = status.pid {
            println!("  PID           : {pid}");
        }
        println!(
            "  Stale state   : {}",
            if status.stale_state { "yes" } else { "no" }
        );
        println!("  State file    : {}", status.state_file.display());
        println!("  Binary path   : {}", status.binary_path.display());
    }
    Ok(())
}

fn cmd_sidecar_start(json: bool) -> anyhow::Result<()> {
    let result = crate::process_supervision::ensure_managed_process_running(
        OPENCLAW_SIDECAR_BIN_BASENAME,
        &[],
    )?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "process_name": OPENCLAW_SIDECAR_BIN_BASENAME,
                "pid": result.pid,
                "started_now": result.started_now,
                "state_file": result.state_file,
            }))?
        );
    } else {
        println!(
            "OpenClaw sidecar {} (pid {}).",
            if result.started_now {
                "started"
            } else {
                "already running"
            },
            result.pid
        );
        println!("  State file    : {}", result.state_file.display());
    }
    Ok(())
}

fn cmd_sidecar_stop(json: bool) -> anyhow::Result<()> {
    let result = crate::process_supervision::stop_managed_process(OPENCLAW_SIDECAR_BIN_BASENAME)?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "process_name": result.process_name,
                "pid": result.pid,
                "stopped": result.stopped,
                "state_file": result.state_file,
            }))?
        );
    } else if result.stopped {
        if let Some(pid) = result.pid {
            println!("OpenClaw sidecar stopped (pid {pid}).");
        } else {
            println!("OpenClaw sidecar stopped.");
        }
        println!("  State file    : {}", result.state_file.display());
    } else if result.pid.is_some() {
        println!("OpenClaw sidecar was not running; stale state removed.");
        println!("  State file    : {}", result.state_file.display());
    } else {
        println!("OpenClaw sidecar has no tracked state to stop.");
        println!("  State file    : {}", result.state_file.display());
    }
    Ok(())
}
