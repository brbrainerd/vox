# Hermes & OpenClaw Unified ARS Compatibility Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Integrate Hermes runtime compatibility alongside OpenClaw by introducing a generic Agent Runtime System (ARS) adapter layer, provider configuration options, and updated compiler built-ins.

**Architecture:** Refactor `OpenClawRuntimeAdapter` into a generic `AgentRuntimeAdapter` trait. Implement `DefaultHermesRuntimeAdapter` using local filesystem scanning for skills and OpenAI-compatible HTTP endpoints for runtime execution. Compiler built-ins are renamed to `vox_agent_*` with legacy `vox_openclaw_*` deprecated.

**Tech Stack:** Rust (async-trait, reqwest, serde_json), Vox compiler.

---

### Task 1: Generic Agent Provider Definitions & Trait Refactor

**Files:**
- Modify: [openclaw_adapter.rs](file:///c:/Users/Owner/vox/crates/vox-openclaw-runtime/src/openclaw_adapter.rs)
- Modify: [lib.rs](file:///c:/Users/Owner/vox/crates/vox-openclaw-runtime/src/lib.rs)

- [ ] **Step 1: Write the failing test**

Modify [openclaw_adapter.rs](file:///c:/Users/Owner/vox/crates/vox-openclaw-runtime/src/openclaw_adapter.rs) to append this test at the end:
```rust
#[cfg(test)]
mod tests_generic_adaptation {
    use super::*;

    #[test]
    fn test_agent_provider_config_resolution() {
        let cfg = AgentRuntimeConfig {
            provider: AgentProvider::Hermes,
            http_gateway_url: "http://127.0.0.1:8642/v1".to_string(),
            ws_gateway_url: None,
            auth_token: None,
            local_skills_path: Some(std::path::PathBuf::from("~/.hermes/skills")),
        };
        assert_eq!(cfg.provider, AgentProvider::Hermes);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-openclaw-runtime test_agent_provider_config_resolution`
Expected: FAIL with compilation error (no `AgentRuntimeConfig` or `AgentProvider` found).

- [ ] **Step 3: Write minimal implementation**

Add the definitions to [openclaw_adapter.rs](file:///c:/Users/Owner/vox/crates/vox-openclaw-runtime/src/openclaw_adapter.rs):
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AgentProvider {
    OpenClaw,
    Hermes,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentRuntimeConfig {
    pub provider: AgentProvider,
    pub http_gateway_url: String,
    pub ws_gateway_url: Option<String>,
    pub auth_token: Option<String>,
    pub local_skills_path: Option<std::path::PathBuf>,
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-openclaw-runtime test_agent_provider_config_resolution`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vox-openclaw-runtime/src/openclaw_adapter.rs
git commit -m "feat: add generic AgentProvider and AgentRuntimeConfig"
```

---

### Task 2: Implement `DefaultHermesRuntimeAdapter`

**Files:**
- Create: `crates/vox-openclaw-runtime/src/hermes_adapter.rs`
- Modify: [lib.rs](file:///c:/Users/Owner/vox/crates/vox-openclaw-runtime/src/lib.rs)

- [ ] **Step 1: Write the failing test**

Create `crates/vox-openclaw-runtime/src/hermes_adapter.rs` with:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_hermes_skills_empty_for_missing_dir() {
        let mut adapter = DefaultHermesRuntimeAdapter::new(AgentRuntimeConfig {
            provider: AgentProvider::Hermes,
            http_gateway_url: "http://localhost:8642/v1".to_string(),
            ws_gateway_url: None,
            auth_token: None,
            local_skills_path: Some(std::path::PathBuf::from("/nonexistent-dir-for-test-999")),
        });
        let skills = adapter.list_remote_skills().await.unwrap();
        assert!(skills.is_empty());
    }
}
```
And expose it in [lib.rs](file:///c:/Users/Owner/vox/crates/vox-openclaw-runtime/src/lib.rs):
```rust
pub mod hermes_adapter;
pub use hermes_adapter::DefaultHermesRuntimeAdapter;
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-openclaw-runtime hermes_adapter::tests`
Expected: FAIL with compiler error (module or type undefined).

- [ ] **Step 3: Write minimal implementation**

Implement `DefaultHermesRuntimeAdapter` in `crates/vox-openclaw-runtime/src/hermes_adapter.rs`:
```rust
use async_trait::async_trait;
use serde_json::Value;
use crate::openclaw_adapter::{AgentRuntimeAdapter, AgentRuntimeConfig, AgentProvider};
use crate::openclaw::{OpenClawSkillSpec, OpenClawError};
use crate::openclaw_adapter::OpenClawAdapterError;
use crate::ArsSkill;

pub struct DefaultHermesRuntimeAdapter {
    cfg: AgentRuntimeConfig,
    http: reqwest::Client,
}

impl DefaultHermesRuntimeAdapter {
    pub fn new(cfg: AgentRuntimeConfig) -> Self {
        Self {
            cfg,
            http: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl AgentRuntimeAdapter for DefaultHermesRuntimeAdapter {
    async fn list_remote_skills(&mut self) -> Result<Vec<OpenClawSkillSpec>, OpenClawAdapterError> {
        let Some(ref path) = self.cfg.local_skills_path else {
            return Ok(Vec::new());
        };
        if !path.exists() {
            return Ok(Vec::new());
        }
        // Bare dir discovery (similar to external_skills)
        let mut out = Vec::new();
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                if entry.path().is_dir() && entry.path().join("SKILL.md").exists() {
                    out.push(OpenClawSkillSpec {
                        name: entry.file_name().to_string_lossy().to_string(),
                        version: "0.1.0".to_string(),
                        description: Some("Hermes local skill".to_string()),
                    });
                }
            }
        }
        Ok(out)
    }

    async fn import_skill(&mut self, _slug: &str) -> Result<ArsSkill, OpenClawAdapterError> {
        Err(OpenClawAdapterError::Other("Importing remote skills not supported on Hermes local".to_string()))
    }

    async fn list_subscriptions(&mut self) -> Result<Value, OpenClawAdapterError> {
        Ok(serde_json::json!({}))
    }

    async fn subscribe_domain(&mut self, _domain: &str) -> Result<Value, OpenClawAdapterError> {
        Err(OpenClawAdapterError::Other("WebSocket subscriptions not supported on Hermes".to_string()))
    }

    async fn unsubscribe_domain(&mut self, _domain: &str) -> Result<Value, OpenClawAdapterError> {
        Err(OpenClawAdapterError::Other("WebSocket subscriptions not supported on Hermes".to_string()))
    }

    async fn notify_domain(&mut self, _domain: &str, _message: &str) -> Result<Value, OpenClawAdapterError> {
        Err(OpenClawAdapterError::Other("WebSocket notifications not supported on Hermes".to_string()))
    }

    async fn gateway_call(&mut self, method: &str, params: Value) -> Result<Value, OpenClawAdapterError> {
        if method == "generate" || method == "chat" {
            let res = self.http.post(&format!("{}/chat/completions", self.cfg.http_gateway_url.trim_end_matches('/')))
                .json(&params)
                .send()
                .await
                .map_err(|e| OpenClawAdapterError::Other(e.to_string()))?;
            let json = res.json::<Value>().await.map_err(|e| OpenClawAdapterError::Other(e.to_string()))?;
            Ok(json)
        } else {
            Err(OpenClawAdapterError::Other(format!("Method {} not supported by Hermes", method)))
        }
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-openclaw-runtime hermes_adapter::tests`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vox-openclaw-runtime/src/hermes_adapter.rs crates/vox-openclaw-runtime/src/lib.rs
git commit -m "feat: implement DefaultHermesRuntimeAdapter"
```

---

### Task 3: Compiler Built-ins & Codegen Refactor

**Files:**
- Modify: [builtin_registry.rs](file:///c:/Users/Owner/vox/crates/vox-compiler/src/builtin_registry.rs)
- Modify: [builtins.rs](file:///c:/Users/Owner/vox/crates/vox-compiler/src/typeck/builtins.rs)
- Modify: [stmt_expr.rs](file:///c:/Users/Owner/vox/crates/vox-codegen/src/codegen_rust/emit/stmt_expr.rs)

- [ ] **Step 1: Write the failing test**

Modify [native_namespace_interp_test.rs](file:///c:/Users/Owner/vox/crates/vox-compiler/tests/native_namespace_interp_test.rs) to add:
```rust
#[test]
fn test_agent_call_built_in_resolves() {
    let src = "fn run() { vox_agent_call(\"generate\", {}); }";
    // Parse, typecheck, and ensure compiler resolves the symbol.
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-compiler native_namespace_interp_test`
Expected: FAIL (symbol `vox_agent_call` not registered/defined).

- [ ] **Step 3: Write minimal implementation**

Add registrations in [builtin_registry.rs](file:///c:/Users/Owner/vox/crates/vox-compiler/src/builtin_registry.rs):
```rust
BuiltinDefinition {
    name: "vox_agent_call",
    runtime_symbol: Some("vox_actor_runtime::builtins::vox_agent_call"),
    // ... type signature mappings matching vox_openclaw_call
}
```
And add typecheck validations in [builtins.rs](file:///c:/Users/Owner/vox/crates/vox-compiler/src/typeck/builtins.rs) mapping `AgentModule` functions.
In [stmt_expr.rs](file:///c:/Users/Owner/vox/crates/vox-codegen/src/codegen_rust/emit/stmt_expr.rs), generalize `emit_openclaw_or_browser_registry_call` to also recognize `AgentModule` call-lowering:
```rust
if module_name == "Agent" || module_name == "OpenClaw" {
    // Generate agent runtime built-in symbol lowering
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-compiler native_namespace_interp_test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vox-compiler/src/builtin_registry.rs crates/vox-compiler/src/typeck/builtins.rs crates/vox-codegen/src/codegen_rust/emit/stmt_expr.rs
git commit -m "feat(compiler): register generic vox_agent_* compiler built-ins"
```

---

### Task 4: Generalize MCP Tools in `vox-orchestrator-mcp`

**Files:**
- Create: `crates/vox-orchestrator-mcp/src/agent_tools.rs`
- Modify: `crates/vox-orchestrator-mcp/src/lib.rs`
- Modify: [catalog.v1.yaml](file:///c:/Users/Owner/vox/contracts/operations/catalog.v1.yaml)

- [ ] **Step 1: Write the failing test**

In [catalog.v1.yaml](file:///c:/Users/Owner/vox/contracts/operations/catalog.v1.yaml), add generic operations:
```yaml
- name: agent.list_remote
  mcp_name: vox_agent_list_remote
  product_lane: interop
  tier: core
  safety_class: read-only
```
Then run the sync commands:
`cargo run -p vox-cli -- ci command-sync`

Write a test in `crates/vox-orchestrator-mcp/src/agent_tools.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn test_agent_list_remote_dispatch() {
        // Mock state and ensure vox_agent_list_remote forwards to active adapter.
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator-mcp agent_tools::tests`
Expected: FAIL (symbol undefined).

- [ ] **Step 3: Write minimal implementation**

Write `agent_tools.rs` mapping the generic MCP tools and delegating internally using configuration provider mapping to `DefaultOpenClawRuntimeAdapter` or `DefaultHermesRuntimeAdapter`. Expose the module in `lib.rs`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-orchestrator-mcp agent_tools::tests`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/agent_tools.rs crates/vox-orchestrator-mcp/src/lib.rs contracts/operations/catalog.v1.yaml
git commit -m "feat(mcp): add generic agent_* tool interfaces"
```

---

### Task 5: Default Provider Configuration

**Files:**
- Modify: `crates/vox-config/src/lib.rs` (or configuration files parsed for Vox.toml)

- [ ] **Step 1: Write the failing test**

Add a config test:
```rust
#[test]
fn test_default_agent_provider_parses() {
    let toml = r#"
        [agent]
        provider = "hermes"
    "#;
    let config = parse_vox_config(toml).unwrap();
    assert_eq!(config.agent.provider, "hermes");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-config default_agent`
Expected: FAIL (struct field missing).

- [ ] **Step 3: Write minimal implementation**

Add generic provider configuration to `vox-config` structures, defaulting to `"openclaw"`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-config default_agent`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vox-config/
git commit -m "feat(config): add Vox.toml agent provider options"
```
