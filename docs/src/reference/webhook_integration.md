---
title: "Vox Webhook Integration"
description: "Lightweight HTTP gateway for receiving events from external services and routing into the orchestrator with HMAC verification."
category: "Language Reference"
status: "current"
last_updated: "2026-04-05"
training_eligible: true

schema_type: "TechArticle"
---

# Vox Webhook Integration

The `vox-webhook` crate provides a lightweight HTTP gateway for receiving events from external services and routing them into the orchestrator.

## Architecture

```text
External Service → HTTPS POST → vox-webhook server → OrchestratorEvent → Agent
```

The webhook server runs as a standalone Axum HTTP service. Payloads are HMAC-verified before being processed.

## Supported Channels

| Channel | Description |
|---------|-------------|
| `github` | GitHub webhook events (push, PR, issue) |
| `slack` | Slack slash commands and event subscriptions |
| `discord` | Discord bot interactions |
| `generic` | Any JSON payload with custom routing |

## Configuration

```toml
[webhook]
port = 9090
secret = "your-hmac-secret"
allowed_channels = ["github", "slack"]
```

## API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| POST | `/webhook/{channel}` | Receive a webhook event from a channel |
| GET | `/webhook/health` | Health check endpoint |

## HMAC Signature Verification

All incoming payloads are verified using HMAC-SHA256:

```text
X-Hub-Signature-256: sha256=<hex_signature>
```

The webhook server computes the HMAC of the raw body using the configured secret and rejects mismatched signatures.

## Event Routing

When a verified payload arrives, it is converted to an `OrchestratorTask` and submitted to the orchestrator:

- GitHub push → `"Process new commit {sha}"` task
- Slack command → `"Handle slash command: {command}"` task
- Custom → as-is description from payload

## Cross-Channel Notifications

The `ChannelManager` can broadcast messages across multiple channels simultaneously using the `Channel` trait:

```rust
manager.send_all("Build failed on main branch").await;
```


