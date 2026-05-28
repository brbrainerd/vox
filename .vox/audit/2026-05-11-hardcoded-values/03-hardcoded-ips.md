# 03 — hardcoded ips

**Severity**: warning  
**Itemized**: 88

### hv-0174 — `apps/editor/vox-vscode/src/extension.ts:44`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "apps/editor/vox-vscode/src/extension.ts"`

**Confidence**: medium

---

### hv-0175 — `contracts/codex-api.openapi.yaml:10`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "contracts/codex-api.openapi.yaml"`

**Confidence**: medium

---

### hv-0176 — `contracts/config/env-vars.v1.yaml:3933`

**Substring**

```text
0.0.0.0
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "0.0.0.0" "contracts/config/env-vars.v1.yaml"`

**Confidence**: medium

---

### hv-0177 — `contracts/config/env-vars.v1.yaml:4107`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "contracts/config/env-vars.v1.yaml"`

**Confidence**: medium

---

### hv-0178 — `contracts/eval/mens-scorecard.baseline.json:13`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "contracts/eval/mens-scorecard.baseline.json"`

**Confidence**: medium

---

### hv-0179 — `contracts/mcp/http-gateway.openapi.yaml:16`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "contracts/mcp/http-gateway.openapi.yaml"`

**Confidence**: medium

---

### hv-0180 — `contracts/openclaw/discovery/well-known.minimal.json:3`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "contracts/openclaw/discovery/well-known.minimal.json"`

**Confidence**: medium

---

### hv-0181 — `contracts/openclaw/discovery/well-known.minimal.json:4`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "contracts/openclaw/discovery/well-known.minimal.json"`

**Confidence**: medium

---

### hv-0182 — `contracts/orchestration/providers.v1.yaml:130`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "contracts/orchestration/providers.v1.yaml"`

**Confidence**: medium

---

### hv-0183 — `contracts/reports/scaling-audit/findings-latest.json:4432`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0184 — `contracts/reports/scaling-audit/findings-latest.json:4615`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0185 — `contracts/reports/scaling-audit/findings-latest.json:8489`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0186 — `contracts/reports/scaling-audit/findings-latest.json:8501`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0187 — `contracts/reports/scaling-audit/findings-latest.json:13250`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0188 — `contracts/reports/scaling-audit/findings-latest.json:13414`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0189 — `contracts/reports/scaling-audit/findings-latest.json:14882`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0190 — `contracts/reports/scaling-audit/findings-latest.json:15145`

**Substring**

```text
0.0.0.0
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "0.0.0.0" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0191 — `crates/vox-actor-runtime/src/builtins/tests.rs:224`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-actor-runtime/src/builtins/tests.rs"`

**Confidence**: medium

---

### hv-0192 — `crates/vox-actor-runtime/src/inference_env.rs:254`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-actor-runtime/src/inference_env.rs"`

**Confidence**: medium

---

### hv-0193 — `crates/vox-actor-runtime/src/model_resolution.rs:377`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-actor-runtime/src/model_resolution.rs"`

**Confidence**: medium

---

### hv-0194 — `crates/vox-actor-runtime/src/model_resolution.rs:383`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-actor-runtime/src/model_resolution.rs"`

**Confidence**: medium

---

### hv-0195 — `crates/vox-actor-runtime/src/model_resolution.rs:500`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-actor-runtime/src/model_resolution.rs"`

**Confidence**: medium

---

### hv-0196 — `crates/vox-cli/src/commands/run.rs:173`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-cli/src/commands/run.rs"`

**Confidence**: medium

---

### hv-0197 — `crates/vox-cli/src/commands/run.rs:175`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-cli/src/commands/run.rs"`

**Confidence**: medium

---

### hv-0198 — `crates/vox-cli/src/commands/share.rs:132`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-cli/src/commands/share.rs"`

**Confidence**: medium

---

### hv-0199 — `crates/vox-cli/src/commands/share.rs:260`

**Substring**

```text
"127.0.0.1"
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"127.0.0.1\"" "crates/vox-cli/src/commands/share.rs"`

**Confidence**: high

---

### hv-0200 — `crates/vox-cli/src/compilerd.rs:327`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-cli/src/compilerd.rs"`

**Confidence**: medium

---

### hv-0201 — `crates/vox-cli/src/compilerd.rs:414`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-cli/src/compilerd.rs"`

**Confidence**: medium

---

### hv-0202 — `crates/vox-cli/src/frontend.rs:57`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-cli/src/frontend.rs"`

**Confidence**: medium

---

### hv-0203 — `crates/vox-cli/src/templates/spa.rs:350`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-cli/src/templates/spa.rs"`

**Confidence**: medium

---

### hv-0204 — `crates/vox-cli/src/templates/spa.rs:380`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-cli/src/templates/spa.rs"`

**Confidence**: medium

---

### hv-0205 — `crates/vox-code-audit/src/detectors/magic_value.rs:142`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-code-audit/src/detectors/magic_value.rs"`

**Confidence**: medium

---

### hv-0206 — `crates/vox-code-audit/src/detectors/magic_value.rs:143`

**Substring**

```text
0.0.0.0
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "0.0.0.0" "crates/vox-code-audit/src/detectors/magic_value.rs"`

**Confidence**: medium

---

### hv-0207 — `crates/vox-code-audit/src/detectors/magic_value.rs:213`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-code-audit/src/detectors/magic_value.rs"`

**Confidence**: medium

---

### hv-0208 — `crates/vox-code-audit/src/detectors/magic_value.rs:241`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-code-audit/src/detectors/magic_value.rs"`

**Confidence**: medium

---

### hv-0209 — `crates/vox-codegen/src/codegen_ts/scaffold.rs:108`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-codegen/src/codegen_ts/scaffold.rs"`

**Confidence**: medium

---

### hv-0210 — `crates/vox-ml-cli/src/commands/ai/serve/mod.rs:250`

**Substring**

```text
"127.0.0.1"
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"127.0.0.1\"" "crates/vox-ml-cli/src/commands/ai/serve/mod.rs"`

**Confidence**: medium

---

### hv-0211 — `crates/vox-ml-cli/src/commands/populi_cli.rs:975`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-ml-cli/src/commands/populi_cli.rs"`

**Confidence**: medium

---

### hv-0212 — `crates/vox-ml-cli/src/commands/populi_lifecycle.rs:412`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-ml-cli/src/commands/populi_lifecycle.rs"`

**Confidence**: medium

---

### hv-0213 — `crates/vox-ml-cli/src/commands/populi_lifecycle.rs:414`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-ml-cli/src/commands/populi_lifecycle.rs"`

**Confidence**: medium

---

### hv-0214 — `crates/vox-openclaw-runtime/src/openclaw_adapter.rs:36`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-openclaw-runtime/src/openclaw_adapter.rs"`

**Confidence**: medium

---

### hv-0215 — `crates/vox-openclaw-runtime/src/openclaw_adapter.rs:42`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-openclaw-runtime/src/openclaw_adapter.rs"`

**Confidence**: medium

---

### hv-0216 — `crates/vox-openclaw-runtime/src/openclaw_discovery.rs:274`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-openclaw-runtime/src/openclaw_discovery.rs"`

**Confidence**: medium

---

### hv-0217 — `crates/vox-openclaw-runtime/src/openclaw_discovery.rs:278`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-openclaw-runtime/src/openclaw_discovery.rs"`

**Confidence**: medium

---

### hv-0218 — `crates/vox-openclaw-runtime/src/openclaw_gateway_ws.rs:38`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-openclaw-runtime/src/openclaw_gateway_ws.rs"`

**Confidence**: medium

---

### hv-0219 — `crates/vox-orchestrator-d/src/bin/vox_orchestrator_d.rs:56`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-orchestrator-d/src/bin/vox_orchestrator_d.rs"`

**Confidence**: medium

---

### hv-0220 — `crates/vox-orchestrator-mcp/src/http_gateway/origin_guard.rs:69`

**Substring**

```text
"127.0.0.1"
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"127.0.0.1\"" "crates/vox-orchestrator-mcp/src/http_gateway/origin_guard.rs"`

**Confidence**: medium

---

### hv-0221 — `crates/vox-orchestrator-mcp/src/http_gateway/origin_guard.rs:109`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-orchestrator-mcp/src/http_gateway/origin_guard.rs"`

**Confidence**: medium

---

### hv-0222 — `crates/vox-orchestrator-mcp/src/http_gateway/origin_guard.rs:170`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-orchestrator-mcp/src/http_gateway/origin_guard.rs"`

**Confidence**: medium

---

### hv-0223 — `crates/vox-orchestrator-mcp/src/llm_bridge/mod.rs:64`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-orchestrator-mcp/src/llm_bridge/mod.rs"`

**Confidence**: medium

---

### hv-0224 — `crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs:186`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs"`

**Confidence**: medium

---

### hv-0225 — `crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs:325`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs"`

**Confidence**: medium

---

### hv-0226 — `crates/vox-orchestrator-mcp/src/llm_bridge/provider_endpoints.rs:81`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-orchestrator-mcp/src/llm_bridge/provider_endpoints.rs"`

**Confidence**: medium

---

### hv-0227 — `crates/vox-orchestrator-mcp/src/llm_bridge/providers/probe.rs:18`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-orchestrator-mcp/src/llm_bridge/providers/probe.rs"`

**Confidence**: medium

---

### hv-0228 — `crates/vox-orchestrator-mcp/src/oratio_tools.rs:497`

**Substring**

```text
"127.0.0.1"
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"127.0.0.1\"" "crates/vox-orchestrator-mcp/src/oratio_tools.rs"`

**Confidence**: medium

---

### hv-0229 — `crates/vox-orchestrator/src/config/tests.rs:108`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-orchestrator/src/config/tests.rs"`

**Confidence**: medium

---

### hv-0230 — `crates/vox-orchestrator/src/config/tests.rs:122`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-orchestrator/src/config/tests.rs"`

**Confidence**: medium

---

### hv-0231 — `crates/vox-orchestrator/src/pii_filter.rs:50`

**Substring**

```text
192.168.1.42
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "192.168.1.42" "crates/vox-orchestrator/src/pii_filter.rs"`

**Confidence**: medium

---

### hv-0232 — `crates/vox-plugin-populi-mesh/src/mesh.rs:27`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-plugin-populi-mesh/src/mesh.rs"`

**Confidence**: medium

---

### hv-0233 — `crates/vox-plugin-populi-mesh/src/transport/mod.rs:533`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-plugin-populi-mesh/src/transport/mod.rs"`

**Confidence**: medium

---

### hv-0234 — `crates/vox-plugin-populi-mesh/src/transport/router.rs:229`

**Substring**

```text
"127.0.0.1"
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"127.0.0.1\"" "crates/vox-plugin-populi-mesh/src/transport/router.rs"`

**Confidence**: high

---

### hv-0235 — `crates/vox-plugin-populi-mesh/src/transport/router.rs:274`

**Substring**

```text
"127.0.0.1"
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"127.0.0.1\"" "crates/vox-plugin-populi-mesh/src/transport/router.rs"`

**Confidence**: high

---

### hv-0236 — `crates/vox-plugin-populi-mesh/src/transport/router.rs:322`

**Substring**

```text
"127.0.0.1"
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"127.0.0.1\"" "crates/vox-plugin-populi-mesh/src/transport/router.rs"`

**Confidence**: high

---

### hv-0237 — `crates/vox-plugin-populi-mesh/src/transport/router.rs:353`

**Substring**

```text
"127.0.0.1"
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"127.0.0.1\"" "crates/vox-plugin-populi-mesh/src/transport/router.rs"`

**Confidence**: high

---

### hv-0238 — `crates/vox-plugin-webhook/src/lib.rs:63`

**Substring**

```text
0.0.0.0
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "0.0.0.0" "crates/vox-plugin-webhook/src/lib.rs"`

**Confidence**: medium

---

### hv-0239 — `crates/vox-populi/src/lib.rs:160`

**Substring**

```text
"0.0.0.0"
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"0.0.0.0\"" "crates/vox-populi/src/lib.rs"`

**Confidence**: medium

---

### hv-0240 — `crates/vox-populi/src/lib.rs:404`

**Substring**

```text
0.0.0.0
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "0.0.0.0" "crates/vox-populi/src/lib.rs"`

**Confidence**: medium

---

### hv-0241 — `crates/vox-populi/src/lib.rs:415`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-populi/src/lib.rs"`

**Confidence**: medium

---

### hv-0242 — `crates/vox-populi/src/lib.rs:416`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-populi/src/lib.rs"`

**Confidence**: medium

---

### hv-0243 — `crates/vox-populi/src/mens/cloud/local_provider.rs:60`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-populi/src/mens/cloud/local_provider.rs"`

**Confidence**: high

---

### hv-0244 — `crates/vox-populi/src/mens/cloud/local_provider.rs:122`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-populi/src/mens/cloud/local_provider.rs"`

**Confidence**: medium

---

### hv-0245 — `crates/vox-populi/src/transport/mod.rs:688`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-populi/src/transport/mod.rs"`

**Confidence**: medium

---

### hv-0246 — `crates/vox-populi/src/transport/router.rs:258`

**Substring**

```text
"127.0.0.1"
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"127.0.0.1\"" "crates/vox-populi/src/transport/router.rs"`

**Confidence**: high

---

### hv-0247 — `crates/vox-populi/src/transport/router.rs:303`

**Substring**

```text
"127.0.0.1"
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"127.0.0.1\"" "crates/vox-populi/src/transport/router.rs"`

**Confidence**: high

---

### hv-0248 — `crates/vox-populi/src/transport/router.rs:351`

**Substring**

```text
"127.0.0.1"
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"127.0.0.1\"" "crates/vox-populi/src/transport/router.rs"`

**Confidence**: high

---

### hv-0249 — `crates/vox-populi/src/transport/router.rs:382`

**Substring**

```text
"127.0.0.1"
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"127.0.0.1\"" "crates/vox-populi/src/transport/router.rs"`

**Confidence**: high

---

### hv-0250 — `crates/vox-repository/src/populi_toml.rs:140`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-repository/src/populi_toml.rs"`

**Confidence**: medium

---

### hv-0251 — `crates/vox-repository/src/populi_toml.rs:148`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-repository/src/populi_toml.rs"`

**Confidence**: medium

---

### hv-0252 — `crates/vox-repository/src/populi_toml.rs:162`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-repository/src/populi_toml.rs"`

**Confidence**: medium

---

### hv-0253 — `crates/vox-repository/src/populi_toml.rs:164`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-repository/src/populi_toml.rs"`

**Confidence**: medium

---

### hv-0254 — `crates/vox-repository/src/populi_toml.rs:169`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-repository/src/populi_toml.rs"`

**Confidence**: medium

---

### hv-0255 — `crates/vox-repository/src/populi_toml.rs:173`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-repository/src/populi_toml.rs"`

**Confidence**: medium

---

### hv-0256 — `crates/vox-repository/src/populi_toml.rs:209`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-repository/src/populi_toml.rs"`

**Confidence**: medium

---

### hv-0257 — `crates/vox-share/src/backends/cloudflare.rs:48`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-share/src/backends/cloudflare.rs"`

**Confidence**: medium

---

### hv-0258 — `crates/vox-share/src/backends/lan.rs:36`

**Substring**

```text
"0.0.0.0"
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"0.0.0.0\"" "crates/vox-share/src/backends/lan.rs"`

**Confidence**: medium

---

### hv-0259 — `crates/vox-share/src/backends/lan.rs:55`

**Substring**

```text
0.0.0.0
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "0.0.0.0" "crates/vox-share/src/backends/lan.rs"`

**Confidence**: high

---

### hv-0260 — `crates/vox-share/src/sse_detect.rs:12`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-share/src/sse_detect.rs"`

**Confidence**: medium

---

### hv-0261 — `crates/vox-test-harness/src/portpicker.rs:8`

**Substring**

```text
127.0.0.1
```

**Why it matters**: Bare IPs are environment-specific and often wrong on IPv6-only or container networks.

**Fix** (extract-named-constant): Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "127.0.0.1" "crates/vox-test-harness/src/portpicker.rs"`

**Confidence**: high

---

