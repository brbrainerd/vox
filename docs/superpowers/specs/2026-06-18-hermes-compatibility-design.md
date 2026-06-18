---
title: "Hermes & OpenClaw Unified Agent Runtime System (ARS) Compatibility"
description: "Specification for generalized agent runtime adapters and compiler built-ins supporting both OpenClaw and Hermes in the Vox ecosystem."
category: "architecture"
status: "approved"
---

# Hermes & OpenClaw Unified ARS Design Spec

This document details the architectural design to generalize the OpenClaw runtime interface into a provider-agnostic Agent Runtime System (ARS) supporting both OpenClaw and Hermes.

## 1. Abstract Abstraction & Adapter Trait

The legacy `OpenClawRuntimeAdapter` will be replaced with a generic `AgentRuntimeAdapter` trait in the `vox-openclaw-runtime` crate.

### Configuration
```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentProvider {
    OpenClaw,
    Hermes,
}

#[derive(Debug, Clone)]
pub struct AgentRuntimeConfig {
    pub provider: AgentProvider,
    pub http_gateway_url: String,       // OpenClaw: ClawHub, Hermes: OpenAI API
    pub ws_gateway_url: Option<String>, // OpenClaw WebSocket, Hermes: None
    pub auth_token: Option<String>,     // OpenClaw Bearer, Hermes: Provider Keys
    pub local_skills_path: Option<std::path::PathBuf>, // Hermes: ~/.hermes/skills/
}
```

### Trait Definition
```rust
#[async_trait]
pub trait AgentRuntimeAdapter: Send {
    async fn list_remote_skills(&mut self) -> Result<Vec<SkillSpec>, AdapterError>;
    async fn import_skill(&mut self, id: &str) -> Result<ArsSkill, AdapterError>;
    async fn list_subscriptions(&mut self) -> Result<Value, AdapterError>;
    async fn subscribe_domain(&mut self, domain: &str) -> Result<Value, AdapterError>;
    async fn unsubscribe_domain(&mut self, domain: &str) -> Result<Value, AdapterError>;
    async fn notify_domain(&mut self, domain: &str, message: &str) -> Result<Value, AdapterError>;
    async fn gateway_call(&mut self, method: &str, params: Value) -> Result<Value, AdapterError>;
}
```

### Adapters
1. **`DefaultOpenClawAdapter`:** The existing HTTP + WS gateway implementation.
2. **`DefaultHermesAdapter`:** 
   - Uses direct local directory scanning under `local_skills_path` for skill indexing and importing.
   - Maps `gateway_call` to an HTTP POST call targeting Hermes's OpenAI-compatible endpoint.
   - Disables WS subscriptions/notifications (returning an error/no-op).

---

## 2. Compiler and Language Integration

Built-ins will be renamed from `openclaw_*` to `agent_*` for unified syntax:

* `vox_agent_list_skills`
* `vox_agent_call`
* `vox_agent_subscribe`
* `vox_agent_unsubscribe`
* `vox_agent_notify`

Legacy `openclaw_*` calls will emit compiler warnings instructing migration to the new symbols, but will map internally to the same implementations.

---

## 3. Operations & Workspace Setup

* Workspace configuration `Vox.toml` and environment variable `VOX_AGENT_PROVIDER` will dictate the default active provider.
* Generic MCP tools (e.g. `vox_agent_list_remote`) will be exposed, while legacy `vox_openclaw_*` tools will act as aliases.
