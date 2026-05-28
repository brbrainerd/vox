# 18 — hardcoded provider names

**Severity**: info  
**Itemized**: 91

### hv-1295 — `crates/vox-actor-runtime/src/llm/chat.rs:39`

**Substring**

```text
"openrouter"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openrouter\"" "crates/vox-actor-runtime/src/llm/chat.rs"`

**Confidence**: medium

---

### hv-1296 — `crates/vox-actor-runtime/src/llm/chat.rs:40`

**Substring**

```text
"openai"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openai\"" "crates/vox-actor-runtime/src/llm/chat.rs"`

**Confidence**: medium

---

### hv-1297 — `crates/vox-actor-runtime/src/llm/embed.rs:60`

**Substring**

```text
"openrouter"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openrouter\"" "crates/vox-actor-runtime/src/llm/embed.rs"`

**Confidence**: medium

---

### hv-1298 — `crates/vox-actor-runtime/src/llm/embed.rs:61`

**Substring**

```text
"openai"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openai\"" "crates/vox-actor-runtime/src/llm/embed.rs"`

**Confidence**: medium

---

### hv-1299 — `crates/vox-actor-runtime/src/llm/stream.rs:30`

**Substring**

```text
"openrouter"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openrouter\"" "crates/vox-actor-runtime/src/llm/stream.rs"`

**Confidence**: medium

---

### hv-1300 — `crates/vox-actor-runtime/src/llm/stream.rs:31`

**Substring**

```text
"openai"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openai\"" "crates/vox-actor-runtime/src/llm/stream.rs"`

**Confidence**: medium

---

### hv-1301 — `crates/vox-actor-runtime/src/llm/types.rs:62`

**Substring**

```text
"openrouter"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openrouter\"" "crates/vox-actor-runtime/src/llm/types.rs"`

**Confidence**: medium

---

### hv-1302 — `crates/vox-actor-runtime/src/llm/types.rs:86`

**Substring**

```text
"openai"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openai\"" "crates/vox-actor-runtime/src/llm/types.rs"`

**Confidence**: medium

---

### hv-1303 — `crates/vox-actor-runtime/src/llm/types.rs:142`

**Substring**

```text
"openrouter"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openrouter\"" "crates/vox-actor-runtime/src/llm/types.rs"`

**Confidence**: medium

---

### hv-1304 — `crates/vox-actor-runtime/src/llm/types.rs:145`

**Substring**

```text
"openai"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openai\"" "crates/vox-actor-runtime/src/llm/types.rs"`

**Confidence**: medium

---

### hv-1305 — `crates/vox-actor-runtime/src/llm/types.rs:148`

**Substring**

```text
"anthropic"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"anthropic\"" "crates/vox-actor-runtime/src/llm/types.rs"`

**Confidence**: medium

---

### hv-1306 — `crates/vox-actor-runtime/src/llm/types.rs:167`

**Substring**

```text
"openrouter"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openrouter\"" "crates/vox-actor-runtime/src/llm/types.rs"`

**Confidence**: medium

---

### hv-1307 — `crates/vox-actor-runtime/src/llm/types.rs:168`

**Substring**

```text
"openai"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openai\"" "crates/vox-actor-runtime/src/llm/types.rs"`

**Confidence**: medium

---

### hv-1308 — `crates/vox-actor-runtime/src/llm/types.rs:289`

**Substring**

```text
"openrouter"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openrouter\"" "crates/vox-actor-runtime/src/llm/types.rs"`

**Confidence**: medium

---

### hv-1309 — `crates/vox-actor-runtime/src/llm/wire.rs:28`

**Substring**

```text
"openrouter"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openrouter\"" "crates/vox-actor-runtime/src/llm/wire.rs"`

**Confidence**: medium

---

### hv-1310 — `crates/vox-actor-runtime/src/llm/wire.rs:32`

**Substring**

```text
"openai"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openai\"" "crates/vox-actor-runtime/src/llm/wire.rs"`

**Confidence**: medium

---

### hv-1311 — `crates/vox-actor-runtime/src/llm/wire.rs:36`

**Substring**

```text
"anthropic"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"anthropic\"" "crates/vox-actor-runtime/src/llm/wire.rs"`

**Confidence**: medium

---

### hv-1312 — `crates/vox-actor-runtime/src/llm/wire.rs:48`

**Substring**

```text
"openai"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openai\"" "crates/vox-actor-runtime/src/llm/wire.rs"`

**Confidence**: medium

---

### hv-1313 — `crates/vox-actor-runtime/src/model_resolution.rs:180`

**Substring**

```text
"openrouter"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openrouter\"" "crates/vox-actor-runtime/src/model_resolution.rs"`

**Confidence**: medium

---

### hv-1314 — `crates/vox-actor-runtime/src/model_resolution.rs:461`

**Substring**

```text
"openrouter"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openrouter\"" "crates/vox-actor-runtime/src/model_resolution.rs"`

**Confidence**: medium

---

### hv-1315 — `crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/llm_routing.rs:21`

**Substring**

```text
"OpenRouter"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"OpenRouter\"" "crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/llm_routing.rs"`

**Confidence**: medium

---

### hv-1316 — `crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/llm_routing.rs:28`

**Substring**

```text
"OpenAI"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"OpenAI\"" "crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/llm_routing.rs"`

**Confidence**: medium

---

### hv-1317 — `crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/llm_routing.rs:42`

**Substring**

```text
"Anthropic"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"Anthropic\"" "crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/llm_routing.rs"`

**Confidence**: medium

---

### hv-1318 — `crates/vox-cli/src/commands/diagnostics/doctor/common.rs:104`

**Substring**

```text
"openrouter"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openrouter\"" "crates/vox-cli/src/commands/diagnostics/doctor/common.rs"`

**Confidence**: medium

---

### hv-1319 — `crates/vox-cli/src/commands/diagnostics/doctor/provider_policy.rs:58`

**Substring**

```text
"openrouter"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openrouter\"" "crates/vox-cli/src/commands/diagnostics/doctor/provider_policy.rs"`

**Confidence**: medium

---

### hv-1320 — `crates/vox-cli/src/commands/diagnostics/doctor/provider_policy.rs:63`

**Substring**

```text
"groq"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"groq\"" "crates/vox-cli/src/commands/diagnostics/doctor/provider_policy.rs"`

**Confidence**: medium

---

### hv-1321 — `crates/vox-cli/src/commands/diagnostics/doctor/provider_policy.rs:68`

**Substring**

```text
"openai"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openai\"" "crates/vox-cli/src/commands/diagnostics/doctor/provider_policy.rs"`

**Confidence**: medium

---

### hv-1322 — `crates/vox-cli/src/commands/diagnostics/doctor/provider_policy.rs:73`

**Substring**

```text
"mistral"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"mistral\"" "crates/vox-cli/src/commands/diagnostics/doctor/provider_policy.rs"`

**Confidence**: medium

---

### hv-1323 — `crates/vox-cli/src/commands/model/pricing.rs:249`

**Substring**

```text
"openrouter"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openrouter\"" "crates/vox-cli/src/commands/model/pricing.rs"`

**Confidence**: medium

---

### hv-1324 — `crates/vox-cli/src/commands/status.rs:11`

**Substring**

```text
"openrouter"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openrouter\"" "crates/vox-cli/src/commands/status.rs"`

**Confidence**: medium

---

### hv-1325 — `crates/vox-cli/src/commands/status.rs:44`

**Substring**

```text
"openrouter"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openrouter\"" "crates/vox-cli/src/commands/status.rs"`

**Confidence**: medium

---

### hv-1326 — `crates/vox-cli/src/commands/status.rs:100`

**Substring**

```text
"openrouter"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openrouter\"" "crates/vox-cli/src/commands/status.rs"`

**Confidence**: medium

---

### hv-1327 — `crates/vox-code-audit/src/ai_analyze.rs:71`

**Substring**

```text
"openai"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openai\"" "crates/vox-code-audit/src/ai_analyze.rs"`

**Confidence**: medium

---

### hv-1328 — `crates/vox-code-audit/src/detectors/llm_provider_call.rs:216`

**Substring**

```text
"openai"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openai\"" "crates/vox-code-audit/src/detectors/llm_provider_call.rs"`

**Confidence**: medium

---

### hv-1329 — `crates/vox-code-audit/src/review/providers.rs:93`

**Substring**

```text
"openai"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openai\"" "crates/vox-code-audit/src/review/providers.rs"`

**Confidence**: medium

---

### hv-1330 — `crates/vox-gamify/src/ai/provider.rs:72`

**Substring**

```text
"openrouter"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openrouter\"" "crates/vox-gamify/src/ai/provider.rs"`

**Confidence**: medium

---

### hv-1331 — `crates/vox-gamify/src/cost.rs:191`

**Substring**

```text
"openrouter"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openrouter\"" "crates/vox-gamify/src/cost.rs"`

**Confidence**: medium

---

### hv-1332 — `crates/vox-gamify/src/cost.rs:199`

**Substring**

```text
"openrouter"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openrouter\"" "crates/vox-gamify/src/cost.rs"`

**Confidence**: medium

---

### hv-1333 — `crates/vox-gamify/src/cost.rs:231`

**Substring**

```text
"openrouter"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openrouter\"" "crates/vox-gamify/src/cost.rs"`

**Confidence**: medium

---

### hv-1334 — `crates/vox-gamify/src/cost.rs:241`

**Substring**

```text
"openrouter"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openrouter\"" "crates/vox-gamify/src/cost.rs"`

**Confidence**: medium

---

### hv-1335 — `crates/vox-gamify/src/cost.rs:265`

**Substring**

```text
"openrouter"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openrouter\"" "crates/vox-gamify/src/cost.rs"`

**Confidence**: medium

---

### hv-1336 — `crates/vox-hf-layout/src/lib.rs:262`

**Substring**

```text
"mistral"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"mistral\"" "crates/vox-hf-layout/src/lib.rs"`

**Confidence**: medium

---

### hv-1337 — `crates/vox-orchestrator-mcp/src/chat_tools/chat/message.rs:673`

**Substring**

```text
"openrouter"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openrouter\"" "crates/vox-orchestrator-mcp/src/chat_tools/chat/message.rs"`

**Confidence**: medium

---

### hv-1338 — `crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs:281`

**Substring**

```text
"openrouter"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openrouter\"" "crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs"`

**Confidence**: medium

---

### hv-1339 — `crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs:337`

**Substring**

```text
"groq"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"groq\"" "crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs"`

**Confidence**: medium

---

### hv-1340 — `crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs:420`

**Substring**

```text
"openrouter"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openrouter\"" "crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs"`

**Confidence**: medium

---

### hv-1341 — `crates/vox-orchestrator-mcp/src/llm_bridge/provider_auth.rs:29`

**Substring**

```text
"OpenRouter"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"OpenRouter\"" "crates/vox-orchestrator-mcp/src/llm_bridge/provider_auth.rs"`

**Confidence**: medium

---

### hv-1342 — `crates/vox-orchestrator-mcp/src/llm_bridge/provider_auth.rs:31`

**Substring**

```text
"Groq"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"Groq\"" "crates/vox-orchestrator-mcp/src/llm_bridge/provider_auth.rs"`

**Confidence**: medium

---

### hv-1343 — `crates/vox-orchestrator-mcp/src/llm_bridge/provider_auth.rs:35`

**Substring**

```text
"Mistral"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"Mistral\"" "crates/vox-orchestrator-mcp/src/llm_bridge/provider_auth.rs"`

**Confidence**: medium

---

### hv-1344 — `crates/vox-orchestrator-mcp/src/llm_bridge/provider_auth.rs:43`

**Substring**

```text
"Anthropic"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"Anthropic\"" "crates/vox-orchestrator-mcp/src/llm_bridge/provider_auth.rs"`

**Confidence**: medium

---

### hv-1345 — `crates/vox-orchestrator-mcp/src/llm_bridge/provider_auth.rs:175`

**Substring**

```text
"openai"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openai\"" "crates/vox-orchestrator-mcp/src/llm_bridge/provider_auth.rs"`

**Confidence**: medium

---

### hv-1346 — `crates/vox-orchestrator-types/build.rs:64`

**Substring**

```text
"OpenRouter"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"OpenRouter\"" "crates/vox-orchestrator-types/build.rs"`

**Confidence**: medium

---

### hv-1347 — `crates/vox-orchestrator-types/src/lib.rs:106`

**Substring**

```text
"openrouter"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openrouter\"" "crates/vox-orchestrator-types/src/lib.rs"`

**Confidence**: medium

---

### hv-1348 — `crates/vox-orchestrator/src/catalog_classifier.rs:39`

**Substring**

```text
"openai"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openai\"" "crates/vox-orchestrator/src/catalog_classifier.rs"`

**Confidence**: medium

---

### hv-1349 — `crates/vox-orchestrator/src/catalog_classifier.rs:41`

**Substring**

```text
"openrouter"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openrouter\"" "crates/vox-orchestrator/src/catalog_classifier.rs"`

**Confidence**: medium

---

### hv-1350 — `crates/vox-orchestrator/src/catalog_classifier.rs:42`

**Substring**

```text
"groq"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"groq\"" "crates/vox-orchestrator/src/catalog_classifier.rs"`

**Confidence**: medium

---

### hv-1351 — `crates/vox-orchestrator/src/catalog.rs:614`

**Substring**

```text
"anthropic"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"anthropic\"" "crates/vox-orchestrator/src/catalog.rs"`

**Confidence**: medium

---

### hv-1352 — `crates/vox-orchestrator/src/dei_shim/selection/virtual_models.rs:13`

**Substring**

```text
"openrouter"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openrouter\"" "crates/vox-orchestrator/src/dei_shim/selection/virtual_models.rs"`

**Confidence**: medium

---

### hv-1353 — `crates/vox-orchestrator/src/dei_shim/selection/virtual_models.rs:61`

**Substring**

```text
"openrouter"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openrouter\"" "crates/vox-orchestrator/src/dei_shim/selection/virtual_models.rs"`

**Confidence**: medium

---

### hv-1354 — `crates/vox-orchestrator/src/events.rs:833`

**Substring**

```text
"openrouter"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openrouter\"" "crates/vox-orchestrator/src/events.rs"`

**Confidence**: medium

---

### hv-1355 — `crates/vox-orchestrator/src/models/registry.rs:994`

**Substring**

```text
"openrouter"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openrouter\"" "crates/vox-orchestrator/src/models/registry.rs"`

**Confidence**: medium

---

### hv-1356 — `crates/vox-orchestrator/src/models/scoring.rs:133`

**Substring**

```text
"groq"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"groq\"" "crates/vox-orchestrator/src/models/scoring.rs"`

**Confidence**: medium

---

### hv-1357 — `crates/vox-orchestrator/src/models/scoring.rs:387`

**Substring**

```text
"openrouter"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openrouter\"" "crates/vox-orchestrator/src/models/scoring.rs"`

**Confidence**: medium

---

### hv-1358 — `crates/vox-orchestrator/src/models/spec.rs:198`

**Substring**

```text
"openrouter"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openrouter\"" "crates/vox-orchestrator/src/models/spec.rs"`

**Confidence**: medium

---

### hv-1359 — `crates/vox-orchestrator/src/models/spec.rs:215`

**Substring**

```text
"groq"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"groq\"" "crates/vox-orchestrator/src/models/spec.rs"`

**Confidence**: medium

---

### hv-1360 — `crates/vox-orchestrator/src/models/spec.rs:223`

**Substring**

```text
"mistral"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"mistral\"" "crates/vox-orchestrator/src/models/spec.rs"`

**Confidence**: medium

---

### hv-1361 — `crates/vox-orchestrator/src/models/spec.rs:235`

**Substring**

```text
"anthropic"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"anthropic\"" "crates/vox-orchestrator/src/models/spec.rs"`

**Confidence**: medium

---

### hv-1362 — `crates/vox-orchestrator/src/models/tests.rs:30`

**Substring**

```text
"openrouter"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openrouter\"" "crates/vox-orchestrator/src/models/tests.rs"`

**Confidence**: medium

---

### hv-1363 — `crates/vox-orchestrator/src/models/tests.rs:41`

**Substring**

```text
"anthropic"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"anthropic\"" "crates/vox-orchestrator/src/models/tests.rs"`

**Confidence**: medium

---

### hv-1364 — `crates/vox-orchestrator/src/models/tests.rs:60`

**Substring**

```text
"openrouter"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openrouter\"" "crates/vox-orchestrator/src/models/tests.rs"`

**Confidence**: medium

---

### hv-1365 — `crates/vox-orchestrator/src/privacy_router.rs:174`

**Substring**

```text
openrouter
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "openrouter" "crates/vox-orchestrator/src/privacy_router.rs"`

**Confidence**: medium

---

### hv-1366 — `crates/vox-orchestrator/src/routing/policy.rs:171`

**Substring**

```text
"OpenRouter"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"OpenRouter\"" "crates/vox-orchestrator/src/routing/policy.rs"`

**Confidence**: medium

---

### hv-1367 — `crates/vox-orchestrator/src/runtime.rs:186`

**Substring**

```text
"openrouter"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openrouter\"" "crates/vox-orchestrator/src/runtime.rs"`

**Confidence**: medium

---

### hv-1368 — `crates/vox-orchestrator/src/runtime.rs:189`

**Substring**

```text
"groq"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"groq\"" "crates/vox-orchestrator/src/runtime.rs"`

**Confidence**: medium

---

### hv-1369 — `crates/vox-orchestrator/src/runtime.rs:191`

**Substring**

```text
"mistral"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"mistral\"" "crates/vox-orchestrator/src/runtime.rs"`

**Confidence**: medium

---

### hv-1370 — `crates/vox-orchestrator/src/runtime.rs:194`

**Substring**

```text
"anthropic"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"anthropic\"" "crates/vox-orchestrator/src/runtime.rs"`

**Confidence**: medium

---

### hv-1371 — `crates/vox-orchestrator/src/usage_policy.rs:56`

**Substring**

```text
"openrouter"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openrouter\"" "crates/vox-orchestrator/src/usage_policy.rs"`

**Confidence**: medium

---

### hv-1372 — `crates/vox-orchestrator/src/usage_policy.rs:58`

**Substring**

```text
"groq"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"groq\"" "crates/vox-orchestrator/src/usage_policy.rs"`

**Confidence**: medium

---

### hv-1373 — `crates/vox-orchestrator/src/usage_policy.rs:60`

**Substring**

```text
"mistral"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"mistral\"" "crates/vox-orchestrator/src/usage_policy.rs"`

**Confidence**: medium

---

### hv-1374 — `crates/vox-orchestrator/src/usage_policy.rs:69`

**Substring**

```text
"openrouter"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openrouter\"" "crates/vox-orchestrator/src/usage_policy.rs"`

**Confidence**: medium

---

### hv-1375 — `crates/vox-orchestrator/src/usage.rs:452`

**Substring**

```text
"openrouter"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openrouter\"" "crates/vox-orchestrator/src/usage.rs"`

**Confidence**: medium

---

### hv-1376 — `crates/vox-orchestrator/src/usage.rs:454`

**Substring**

```text
"groq"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"groq\"" "crates/vox-orchestrator/src/usage.rs"`

**Confidence**: medium

---

### hv-1377 — `crates/vox-orchestrator/src/usage.rs:460`

**Substring**

```text
"mistral"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"mistral\"" "crates/vox-orchestrator/src/usage.rs"`

**Confidence**: medium

---

### hv-1378 — `crates/vox-orchestrator/src/usage.rs:489`

**Substring**

```text
"openrouter"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openrouter\"" "crates/vox-orchestrator/src/usage.rs"`

**Confidence**: medium

---

### hv-1379 — `crates/vox-orchestrator/src/usage.rs:722`

**Substring**

```text
"openrouter"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openrouter\"" "crates/vox-orchestrator/src/usage.rs"`

**Confidence**: medium

---

### hv-1380 — `crates/vox-research-events/src/observation.rs:112`

**Substring**

```text
"openai"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openai\"" "crates/vox-research-events/src/observation.rs"`

**Confidence**: medium

---

### hv-1381 — `crates/vox-research-events/src/observation.rs:121`

**Substring**

```text
"openai"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openai\"" "crates/vox-research-events/src/observation.rs"`

**Confidence**: medium

---

### hv-1382 — `crates/vox-search/src/embedding_env.rs:46`

**Substring**

```text
"openai"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openai\"" "crates/vox-search/src/embedding_env.rs"`

**Confidence**: medium

---

### hv-1383 — `crates/vox-search/src/embedding_env.rs:71`

**Substring**

```text
"openrouter"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openrouter\"" "crates/vox-search/src/embedding_env.rs"`

**Confidence**: medium

---

### hv-1384 — `crates/vox-secrets/src/spec/registry/llm.rs:45`

**Substring**

```text
"openrouter"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"openrouter\"" "crates/vox-secrets/src/spec/registry/llm.rs"`

**Confidence**: medium

---

### hv-1385 — `crates/vox-telemetry/src/types.rs:515`

**Substring**

```text
"anthropic"
```

**Why it matters**: Stringly provider routing fights enum-based dispatch and telemetry tagging.

**Fix** (use-provider-enum): Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.

**SSOT**: `crates/vox-orchestrator-mcp`

**Verify**: `rg -nF "\"anthropic\"" "crates/vox-telemetry/src/types.rs"`

**Confidence**: medium

---

