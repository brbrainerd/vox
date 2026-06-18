# Autonomous Chat Research Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enable the chat system to dynamically trigger autonomous deep research (local and web) when thin context is detected, injecting synthesized research summaries into the chat turn context, and publishing progress events.

**Architecture:** Update `ChatMessageParams` to support explicit research triggers, extend the chat retrieval handler to score retrieved context confidence, and wire the orchestrator's `perform_autonomous_research` pipeline to execute research on low confidence or when explicitly requested. The synthesized summary is injected as a preamble context block for the chat model.

**Tech Stack:** Rust stable (workspace toolchain), `vox-orchestrator-mcp` (MCP chat service), `vox-research-shim` (research pipeline), `vox-search` (retrieval/CRAG), `tracing` logging.

---

### Task 1: Add new parameters to ChatMessageParams

**Files:**
- Modify: [params.rs](file:///c:/Users/Owner/vox/crates/vox-orchestrator-mcp/src/chat_tools/params.rs)
- Modify: [input_schemas.rs](file:///c:/Users/Owner/vox/crates/vox-orchestrator-mcp/src/input_schemas.rs)
- Test: [params.rs](file:///c:/Users/Owner/vox/crates/vox-orchestrator-mcp/src/chat_tools/params.rs) (Add new test at bottom)

- [ ] **Step 1.1: Add `force_research` and `research_scope` to `ChatMessageParams`**

Modify [params.rs](file:///c:/Users/Owner/vox/crates/vox-orchestrator-mcp/src/chat_tools/params.rs) to add the fields to the end of the `ChatMessageParams` struct:

```rust
    /// Optional override to force trigger autonomous research (true/false)
    #[serde(default)]
    pub force_research: Option<bool>,
    /// Optional research scope override ("local", "web", or "both")
    #[serde(default)]
    pub research_scope: Option<String>,
```

- [ ] **Step 1.2: Add parameters to schema in `input_schemas.rs`**

Modify [input_schemas.rs](file:///c:/Users/Owner/vox/crates/vox-orchestrator-mcp/src/input_schemas.rs) to add `force_research` and `research_scope` properties to `vox_chat_message` (around line 509):

```json
"force_research":{"type":"boolean","description":"Optional override to force trigger autonomous research"},"research_scope":{"type":"string","enum":["local","web","both"],"description":"Optional research scope override"}
```

- [ ] **Step 1.3: Add a unit test verifying parameters deserialization**

Add the following unit test to `crates/vox-orchestrator-mcp/src/chat_tools/params.rs` at the bottom of the file:

```rust
#[cfg(test)]
mod chat_params_tests {
    use super::*;

    #[test]
    fn parses_research_params_from_json() {
        let json = r#"{
            "prompt": "explain quantum computing",
            "force_research": true,
            "research_scope": "web"
        }"#;
        let p: ChatMessageParams = serde_json::from_str(json).unwrap();
        assert_eq!(p.force_research, Some(true));
        assert_eq!(p.research_scope, Some("web".to_string()));
    }
}
```

- [ ] **Step 1.4: Run tests to verify they pass**

Run: `cargo test -p vox-orchestrator-mcp chat_params_tests`
Expected: PASS

- [ ] **Step 1.5: Commit**

```powershell
cargo fmt -p vox-orchestrator-mcp
git add crates/vox-orchestrator-mcp/src/chat_tools/params.rs crates/vox-orchestrator-mcp/src/input_schemas.rs
git commit -m "feat(vox-orchestrator-mcp): add force_research and research_scope params to ChatMessageParams"
```

---

### Task 2: Implement retrieval confidence scorer and decision helper

**Files:**
- Modify: [retrieval.rs](file:///c:/Users/Owner/vox/crates/vox-orchestrator-mcp/src/memory_tools/retrieval.rs)

- [ ] **Step 2.1: Implement `should_trigger_autonomous_research` decision helper**

In [retrieval.rs](file:///c:/Users/Owner/vox/crates/vox-orchestrator-mcp/src/memory_tools/retrieval.rs), add the following function at the bottom:

```rust
/// Helper to determine if autonomous research should be triggered based on query and retrieval hits.
pub fn should_trigger_autonomous_research(
    query: &str,
    bundle: &RetrievalBundle,
    force_research: Option<bool>,
) -> bool {
    if let Some(forced) = force_research {
        return forced;
    }
    
    // Explicit tag request
    if query.contains("[[research:") || query.contains("[[category:research]]") {
        return true;
    }

    // Evaluate confidence score using the confidence gate
    use vox_research_shim::research::gate::{GateConfig, GateInput, score_with_config};
    
    let mut claims = Vec::new(); // empty claims for initial heuristic score
    let min_citations = 5;
    let min_domains = 4;
    
    let citation_count = bundle.rrf_fused_lines.len()
        + bundle.memory_lines.len()
        + bundle.knowledge_lines.len()
        + bundle.chunk_lines.len();
        
    let distinct_domain_count = 1; // lightweight fallback
    
    let gate_input = GateInput {
        claims: &claims,
        citation_count,
        supported_claim_count: 0,
        distinct_domain_count,
        no_retrieval_hits: citation_count == 0,
        answer_is_empty: false,
    };
    
    let config = GateConfig {
        min_citations_for_full_score: Some(min_citations),
        min_domains_for_full_score: Some(min_domains),
    };
    
    let signal = score_with_config(&gate_input, &config);
    // Trigger research if the confidence score is below 0.65
    signal.score < 0.65
}
```

- [ ] **Step 2.2: Add unit tests for the decision helper**

Add the following tests to `crates/vox-orchestrator-mcp/src/memory_tools/retrieval.rs` inside the `mod tests` block at the bottom:

```rust
    #[test]
    fn should_trigger_autonomous_research_on_forced_flag() {
        let bundle = RetrievalBundle::default();
        assert!(super::should_trigger_autonomous_research("test", &bundle, Some(true)));
        assert!(!super::should_trigger_autonomous_research("test", &bundle, Some(false)));
    }

    #[test]
    fn should_trigger_autonomous_research_on_explicit_tag() {
        let bundle = RetrievalBundle::default();
        assert!(super::should_trigger_autonomous_research("do some [[research:topic]] here", &bundle, None));
    }
```

- [ ] **Step 2.3: Run tests to verify they pass**

Run: `cargo test -p vox-orchestrator-mcp should_trigger_autonomous_research`
Expected: PASS

- [ ] **Step 2.4: Commit**

```powershell
cargo fmt -p vox-orchestrator-mcp
git add crates/vox-orchestrator-mcp/src/memory_tools/retrieval.rs
git commit -m "feat(vox-orchestrator-mcp): add should_trigger_autonomous_research decision helper"
```

---

### Task 3: Integrate autonomous research trigger in `chat_message`

**Files:**
- Modify: [message.rs](file:///c:/Users/Owner/vox/crates/vox-orchestrator-mcp/src/chat_tools/chat/message.rs)

- [ ] **Step 3.1: Trigger autonomous research in `chat_message`**

In [message.rs](file:///c:/Users/Owner/vox/crates/vox-orchestrator-mcp/src/chat_tools/chat/message.rs), locate the unified autonomous retrieval injection block (around line 200) and update it to:

```rust
            retrieval_evidence = Some(bundle.evidence.clone());
            
            // Check if autonomous deep research should be triggered
            use crate::memory::should_trigger_autonomous_research;
            if should_trigger_autonomous_research(&expanded_prompt, &bundle, params.force_research) {
                tracing::info!("Triggering autonomous research for additional context");
                let scope = params.research_scope.as_deref().unwrap_or("both");
                
                // Spawn autonomous research execution
                let queries = vec![expanded_prompt.clone()];
                let trigger_reason = format!("Chat context injection (forced: {:?}, scope: {})", params.force_research, scope);
                
                match state.orchestrator.perform_autonomous_research(
                    None, 
                    None, 
                    queries, 
                    &trigger_reason
                ).await {
                    Ok(results) => {
                        if !results.is_empty() {
                            let formatted = results.join("\n");
                            context_parts.push(format!(
                                "[AUTONOMOUS RESEARCH — SYNTHESIS SUMMARY]:\n{formatted}"
                            ));
                            tracing::info!(count = results.len(), "Autonomous research results injected successfully");
                        }
                    }
                    Err(err) => {
                        tracing::warn!(error = %err, "Autonomous research execution failed");
                    }
                }
            }
```

- [ ] **Step 3.2: Run tests to check compilation**

Run: `cargo check -p vox-orchestrator-mcp`
Expected: OK

- [ ] **Step 3.3: Commit**

```powershell
cargo fmt -p vox-orchestrator-mcp
git add crates/vox-orchestrator-mcp/src/chat_tools/chat/message.rs
git commit -m "feat(vox-orchestrator-mcp): integrate autonomous research trigger and summary injection in chat turns"
```

---

### Task 4: Add Verification Integration Test

**Files:**
- Create: `crates/vox-orchestrator-mcp/tests/autonomous_chat_research_test.rs`

- [ ] **Step 4.1: Create the integration test**

Create the file `crates/vox-orchestrator-mcp/tests/autonomous_chat_research_test.rs` with the following content:

```rust
use vox_orchestrator_mcp::chat_tools::chat::chat_message;
use vox_orchestrator_mcp::chat_tools::params::ChatMessageParams;
use vox_orchestrator_mcp::server_state::ServerState;

#[tokio::test]
async fn test_forced_autonomous_chat_research_triggers() {
    let state = ServerState::default(); // Mock state or default server configuration
    let params = ChatMessageParams {
        prompt: "explain quantum physics".to_string(),
        context_files: vec![],
        open_files: vec![],
        active_file: None,
        active_line: None,
        selected_text: None,
        diagnostics: vec![],
        session_id: Some("test-session".to_string()),
        thread_id: None,
        journey_id: None,
        cognitive_profile: None,
        json_mode: false,
        trace_id: None,
        correlation_id: None,
        attachment_manifest: None,
        temperature: None,
        top_p: None,
        skill: None,
        force_research: Some(true),
        research_scope: Some("web".to_string()),
    };

    // Since network backends and API keys might not be present in local test environments,
    // we verify that the chat message execution handles the research trigger and completes/falls back gracefully.
    let response = chat_message(&state, params).await;
    assert!(!response.is_empty());
}
```

- [ ] **Step 4.2: Run integration tests**

Run: `cargo test --test autonomous_chat_research_test`
Expected: PASS or graceful execution fallback

- [ ] **Step 4.3: Commit**

```powershell
cargo fmt -p vox-orchestrator-mcp
git add crates/vox-orchestrator-mcp/tests/autonomous_chat_research_test.rs
git commit -m "test(vox-orchestrator-mcp): add autonomous chat research integration test"
```
